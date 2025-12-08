// Gupax
//
// Copyright (c) 2024-2025 Cyrix126
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use enclose::enc;
use log::{debug, error, info, warn};
use reqwest::header::AUTHORIZATION;
use reqwest_middleware::ClientWithMiddleware as Client;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::time::Duration;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};
use tokio::spawn;

use crate::disk::state::{P2pool, StartOptionsMode, XmrigProxy};
use crate::helper::p2pool::ImgP2pool;
use crate::helper::xrig::current_api_url_xrig;
use crate::human::{HumanNumber, HumanTime};
use crate::miscs::client;
use crate::{
    GUPAX_VERSION_UNDERSCORE,
    helper::{
        Helper, Process, ProcessName, ProcessSignal, ProcessState, check_died, check_user_input,
        signal_end, sleep_end_loop,
        xrig::update_xmrig_config,
        xvb::{PubXvbApi, nodes::Pool},
    },
    macros::sleep,
    miscs::output_console,
    regex::{XMRIG_REGEX, detect_pool_xmrig},
};
use crate::{PROXY_API_PORT_DEFAULT, PROXY_PORT_DEFAULT, XMRIG_API_SUMMARY_ENDPOINT};

use super::xmrig::{ImgXmrig, PubXmrigApi};
impl Helper {
    // Takes in some [State/XmrigProxy] and parses it to build the actual command arguments.
    // Returns the [Vec] of actual arguments,
    #[cold]
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    pub async fn read_pty_xp(
        output_parse: Arc<Mutex<String>>,
        output_pub: Arc<Mutex<String>>,
        reader: Box<dyn std::io::Read + Send>,
        process_xvb: Arc<Mutex<Process>>,
        pub_api_xvb: &Arc<Mutex<PubXvbApi>>,
        process_p2pool: Arc<Mutex<Process>>,
        p2pool_state: &P2pool,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        proxy_state: &XmrigProxy,
    ) {
        use std::io::BufRead;
        let mut stdout = std::io::BufReader::new(reader).lines();

        // Run a ANSI escape sequence filter for the first few lines.
        let mut i = 0;
        while let Some(Ok(line)) = stdout.next() {
            let line = strip_ansi_escapes::strip_str(line);
            if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                error!("XMRig-Proxy PTY Parse | Output error: {e}");
            }
            if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                error!("XMRig-Proxy PTY Pub | Output error: {e}");
            }
            if i > 7 {
                break;
            } else {
                i += 1;
            }
        }

        while let Some(Ok(line)) = stdout.next() {
            // need to verify if node still working
            // for that need to catch "connect error"
            // only switch nodes of XvB if XvB process is used
            if process_xvb.lock().unwrap().is_alive() {
                let p2pool_port = p2pool_state.current_port(
                    process_p2pool.lock().unwrap().is_alive(),
                    &p2pool_img.lock().unwrap(),
                );

                Pool::update_current_pool(
                    &line,
                    proxy_state.bind_port(),
                    p2pool_port,
                    &process_xvb,
                    pub_api_xvb,
                    ProcessName::XmrigProxy,
                );
            }
            //			println!("{}", line); // For debugging.
            if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                error!("XMRig-Proxy PTY Parse | Output error: {e}");
            }
            if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                error!("XMRig-Proxy PTY Pub | Output error: {e}");
            }
        }
    }
    pub fn build_xp_args(
        state: &crate::disk::state::XmrigProxy,
        mode: StartOptionsMode,
        p2pool_stratum_port: u16,
    ) -> Vec<String> {
        let mut args = Vec::with_capacity(500);
        let api_ip;
        let api_port;
        let ip;
        let port;
        match mode {
            StartOptionsMode::Simple | StartOptionsMode::Advanced => {
                args.push(format!("--http-access-token={}", state.token)); // HTTP API Port
                args.push("--http-no-restricted".to_string());
                args.push("--no-color".to_string()); // No color
            }
            _ => (),
        }
        match mode {
            StartOptionsMode::Simple => {
                // Build the xmrig argument
                let rig = if state.simple_rig.is_empty() {
                    GUPAX_VERSION_UNDERSCORE.to_string()
                } else {
                    state.simple_rig.clone()
                }; // Rig name
                args.push("-o".to_string());
                args.push(format!("127.0.0.1:{p2pool_stratum_port}")); // Local P2Pool (the default)
                args.push("-b".to_string());
                args.push(format!("0.0.0.0:{PROXY_PORT_DEFAULT}"));
                args.push("--user".to_string());
                args.push(rig); // Rig name
                args.push("--http-host".to_string());
                args.push("127.0.0.1".to_string()); // HTTP API IP
                args.push("--http-port".to_string());
                args.push(PROXY_API_PORT_DEFAULT.to_string()); // HTTP API Port
            }
            StartOptionsMode::Advanced => {
                // XMRig doesn't understand [localhost]
                let p2pool_ip = if state.p2pool_ip == "localhost" || state.p2pool_ip.is_empty() {
                    "127.0.0.1"
                } else {
                    &state.p2pool_ip
                };
                api_ip = if state.api_ip == "localhost" || state.api_ip.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    state.api_ip.to_string()
                };
                api_port = if state.api_port.is_empty() {
                    PROXY_API_PORT_DEFAULT.to_string()
                } else {
                    state.api_port.to_string()
                };
                ip = if state.ip == "localhost" {
                    "127.0.0.1".to_string()
                } else if state.ip.is_empty() {
                    "0.0.0.0".to_string()
                } else {
                    state.ip.to_string()
                };

                port = if state.port.is_empty() {
                    PROXY_PORT_DEFAULT.to_string()
                } else {
                    state.port.to_string()
                };
                let p2pool_url = format!("{}:{}", p2pool_ip, state.p2pool_port); // Combine IP:Port into one string
                let bind_url = format!("{ip}:{port}"); // Combine IP:Port into one string
                args.push("--user".to_string());
                args.push(state.address.clone()); // Wallet
                args.push(format!("--rig-id={}", state.rig)); // Rig ID
                args.push("-o".to_string());
                args.push(p2pool_url.clone()); // IP/Port
                args.push("-b".to_string());
                args.push(bind_url.clone()); // IP/Port
                args.push("--http-host".to_string());
                args.push(api_ip.to_string()); // HTTP API IP
                args.push("--http-port".to_string());
                args.push(api_port.to_string()); // HTTP API Port
                if state.tls {
                    args.push("--tls".to_string());
                } // TLS
                if state.keepalive {
                    args.push("--keepalive".to_string());
                } // Keepalive
            }
            StartOptionsMode::Custom => {
                for arg in state.arguments.split_whitespace() {
                    let arg = if arg == "localhost" { "127.0.0.1" } else { arg };
                    args.push(arg.to_string());
                }
            }
        }
        args
    }

    pub fn mutate_img_proxy(helper: &Arc<Mutex<Self>>, state: &crate::disk::state::XmrigProxy) {
        if state.simple {
            *helper.lock().unwrap().img_proxy.lock().unwrap() = ImgProxy {
                api_port: PROXY_API_PORT_DEFAULT,
                port: PROXY_PORT_DEFAULT,
                token: state.token.clone(),
            }
        } else if !state.arguments.is_empty() {
            // This parses the input and attempts to fill out
            // the [ImgXmrig]... This is pretty bad code...
            let mut last = "";
            let lock = helper.lock().unwrap();
            let mut proxy_image = lock.img_proxy.lock().unwrap();
            for arg in state.arguments.split_whitespace() {
                match last {
                    "--bind" | "-b" => {
                        proxy_image.port = last
                            .split(":")
                            .last()
                            .unwrap_or_default()
                            .parse()
                            .unwrap_or(PROXY_PORT_DEFAULT);
                    }
                    "--http-host" => {
                        proxy_image.api_port = last.parse().unwrap_or(PROXY_API_PORT_DEFAULT)
                    }
                    l if l.contains("--http-access-token=") => {
                        proxy_image.token = l.split_once("=").unwrap().1.to_string();
                    }
                    _ => {}
                }
                last = arg;
            }
        } else {
            *helper.lock().unwrap().img_proxy.lock().unwrap() = ImgProxy {
                api_port: state.api_port.parse().unwrap_or(PROXY_API_PORT_DEFAULT),
                port: state.port.parse().unwrap_or(PROXY_PORT_DEFAULT),
                token: state.token.clone(),
            };
        }
    }
    pub fn stop_xp(helper: &Arc<Mutex<Self>>) {
        info!("XMRig-Proxy | Attempting to stop...");
        helper.lock().unwrap().xmrig_proxy.lock().unwrap().signal = ProcessSignal::Stop;
        info!("locked signal ok");
        helper.lock().unwrap().xmrig_proxy.lock().unwrap().state = ProcessState::Middle;
        info!("locked state ok");
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_xp);
        info!("clone gui ok");
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_xp);
        info!("clone pub ok");
        *pub_api.lock().unwrap() = PubXmrigProxyApi::new();
        info!("pub api reset ok");
        *gui_api.lock().unwrap() = PubXmrigProxyApi::new();
        info!("gui api reset ok");
    }
    // The "restart frontend" to a "frontend" function.
    // Basically calls to kill the current xmrig-proxy, waits a little, then starts the below function in a a new thread, then exit.
    pub fn restart_xp(
        helper: &Arc<Mutex<Self>>,
        state: &crate::disk::state::XmrigProxy,
        state_p2pool: &P2pool,
        path: &Path,
    ) {
        info!("XMRig-Proxy | Attempting to restart...");
        helper.lock().unwrap().xmrig_proxy.lock().unwrap().state = ProcessState::Middle;
        helper.lock().unwrap().xmrig_proxy.lock().unwrap().signal = ProcessSignal::Restart;

        let path = path.to_path_buf();
        // This thread lives to wait, start xmrig_proxy then die.
        thread::spawn(enc!((helper, state,  state_p2pool, path)move || {
            while helper.lock().unwrap().xmrig_proxy.lock().unwrap().state != ProcessState::Waiting
            {
                warn!("XMRig-proxy | Want to restart but process is still alive, waiting...");
                sleep!(1000);
            }
            // Ok, process is not alive, start the new one!
            info!("XMRig-Proxy | Old process seems dead, starting new one!");
            Self::start_xp(&helper, &state,  &state_p2pool, &path);
        }));
        info!("XMRig-Proxy | Restart ... OK");
    }
    pub fn start_xp(
        helper: &Arc<Mutex<Self>>,
        state_proxy: &crate::disk::state::XmrigProxy,
        state_p2pool: &P2pool,
        path: &Path,
    ) {
        helper.lock().unwrap().xmrig_proxy.lock().unwrap().state = ProcessState::Middle;

        let mode = if state_proxy.simple {
            StartOptionsMode::Simple
        } else if !state_proxy.arguments.is_empty() {
            StartOptionsMode::Custom
        } else {
            StartOptionsMode::Advanced
        };

        // get the stratum port of p2pool
        let process_p2pool = Arc::clone(&helper.lock().unwrap().p2pool);
        let p2pool_img = Arc::clone(&helper.lock().unwrap().img_p2pool);
        let p2pool_stratum_port = state_p2pool.current_port(
            process_p2pool.lock().unwrap().is_alive(),
            &p2pool_img.lock().unwrap(),
        );
        // store the data used for startup to make it available to the other processes.
        Helper::mutate_img_proxy(helper, state_proxy);
        let args = Self::build_xp_args(state_proxy, mode, p2pool_stratum_port);
        // Print arguments & user settings to console
        crate::disk::print_dash(&format!("XMRig-Proxy | Launch arguments: {args:#?}"));
        info!("XMRig-Proxy | Using path: [{}]", path.display());

        // Spawn watchdog thread
        let process = Arc::clone(&helper.lock().unwrap().xmrig_proxy);
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_xp);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_xp);
        let process_xvb = Arc::clone(&helper.lock().unwrap().xvb);
        let process_xmrig = Arc::clone(&helper.lock().unwrap().xmrig);
        let path = path.to_path_buf();
        let state = state_proxy.clone();
        let state_p2pool = state_p2pool.clone();
        let pub_api_xvb = Arc::clone(&helper.lock().unwrap().pub_api_xvb);
        let pub_api_xmrig = Arc::clone(&helper.lock().unwrap().pub_api_xmrig);
        let xmrig_img = Arc::clone(&helper.lock().unwrap().img_xmrig);
        thread::spawn(move || {
            Self::spawn_xp_watchdog(
                &process,
                &gui_api,
                &pub_api,
                args,
                path,
                &state,
                process_xvb,
                process_xmrig,
                &pub_api_xvb,
                &pub_api_xmrig,
                &xmrig_img,
                process_p2pool,
                &state_p2pool,
                &p2pool_img,
            );
        });
    }
    #[tokio::main]
    #[allow(clippy::await_holding_lock)]
    #[allow(clippy::too_many_arguments)]
    async fn spawn_xp_watchdog(
        process: &Arc<Mutex<Process>>,
        gui_api: &Arc<Mutex<PubXmrigProxyApi>>,
        pub_api: &Arc<Mutex<PubXmrigProxyApi>>,
        args: Vec<String>,
        path: std::path::PathBuf,
        state: &XmrigProxy,
        process_xvb: Arc<Mutex<Process>>,
        process_xmrig: Arc<Mutex<Process>>,
        pub_api_xvb: &Arc<Mutex<PubXvbApi>>,
        pub_api_xmrig: &Arc<Mutex<PubXmrigApi>>,
        xmrig_img: &Arc<Mutex<ImgXmrig>>,
        process_p2pool: Arc<Mutex<Process>>,
        p2pool_state: &P2pool,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
    ) {
        process.lock().unwrap().start = Instant::now();
        // spawn pty
        debug!("XMRig-Proxy | Creating PTY...");
        let pty = portable_pty::native_pty_system();
        let pair = pty
            .openpty(portable_pty::PtySize {
                rows: 100,
                cols: 1000,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        // 4. Spawn PTY read thread
        debug!("XMRig-Proxy | Spawning PTY read thread...");
        let reader = pair.master.try_clone_reader().unwrap(); // Get STDOUT/STDERR before moving the PTY
        let output_parse = Arc::clone(&process.lock().unwrap().output_parse);
        let output_pub = Arc::clone(&process.lock().unwrap().output_pub);
        spawn(
            enc!((pub_api_xvb, output_parse, output_pub, process_p2pool, p2pool_state, p2pool_img,  state) async move {
                Self::read_pty_xp(output_parse, output_pub, reader, process_xvb, &pub_api_xvb, process_p2pool, &p2pool_state, &p2pool_img, &state).await;
            }),
        );
        // 1b. Create command
        debug!("XMRig-Proxy | Creating command...");
        let mut cmd = portable_pty::cmdbuilder::CommandBuilder::new(path.clone());
        cmd.args(args);
        cmd.cwd(path.as_path().parent().unwrap());
        // 1c. Create child
        debug!("XMRig-Proxy | Creating child...");
        let child_pty = Arc::new(Mutex::new(pair.slave.spawn_command(cmd).unwrap()));
        drop(pair.slave);
        let mut stdin = pair.master.take_writer().unwrap();
        // to refactor to let user use his own ports
        let api_summary_xp = format!(
            "http://127.0.0.1:{}/{}",
            state.api_port(),
            XMRIG_API_SUMMARY_ENDPOINT
        );

        // set state
        let client = client();
        process.lock().unwrap().state = ProcessState::NotMining;
        process.lock().unwrap().signal = ProcessSignal::None;
        // reset stats
        *pub_api.lock().unwrap() = PubXmrigProxyApi::new();
        *gui_api.lock().unwrap() = PubXmrigProxyApi::new();
        // loop
        let start = process.lock().unwrap().start;
        debug!("Xmrig-Proxy Watchdog | enabling verbose mode");
        #[cfg(target_os = "windows")]
        if let Err(e) = write!(stdin, "v\r\n") {
            error!("P2Pool Watchdog | STDIN error: {e}");
        }
        #[cfg(target_family = "unix")]
        if let Err(e) = writeln!(stdin, "v") {
            error!("XMRig-Proxy Watchdog | STDIN error: {e}");
        }
        debug!("XMRig-Proxy Watchdog | checking connections");
        #[cfg(target_os = "windows")]
        if let Err(e) = write!(stdin, "c\r\n") {
            error!("XMRig-Proxy Watchdog | STDIN error: {e}");
        }
        #[cfg(target_family = "unix")]
        if let Err(e) = writeln!(stdin, "c") {
            error!("XMRig-Proxy Watchdog | STDIN error: {e}");
        }
        info!("XMRig-Proxy | Entering watchdog mode... woof!");
        let mut last_redirect_request = Instant::now();
        let mut first_loop = true;
        loop {
            let now = Instant::now();
            debug!("XMRig-Proxy Watchdog | ----------- Start of loop -----------");
            {
                if check_died(
                    &child_pty,
                    &mut process.lock().unwrap(),
                    &start,
                    &mut gui_api.lock().unwrap().output,
                ) {
                    break;
                }
                // check signal
                if signal_end(
                    &mut process.lock().unwrap(),
                    Some(&child_pty.clone()),
                    &start,
                    &mut gui_api.lock().unwrap().output,
                ) {
                    break;
                }
                // check user input
                check_user_input(process, &mut stdin);
                // get data output/api

                // Check if logs need resetting
                debug!("XMRig-Proxy Watchdog | Attempting GUI log reset check");
                Self::check_reset_gui_output(
                    &mut gui_api.lock().unwrap().output,
                    ProcessName::XmrigProxy,
                );
                // Always update from output
                // todo: check difference with xmrig
                debug!("XMRig-Proxy Watchdog | Starting [update_from_output()]");
                let process_p2pool_lock = process_p2pool.lock().unwrap();
                let mut process_lock = process.lock().unwrap();
                let mut pub_api_lock = pub_api.lock().unwrap();
                PubXmrigProxyApi::update_from_output(
                    &mut pub_api_lock,
                    &output_pub,
                    &output_parse,
                    start.elapsed(),
                    &mut process_lock,
                    &process_p2pool_lock,
                    p2pool_img,
                    p2pool_state,
                    state,
                );
                drop(pub_api_lock);
                drop(process_lock);
                // update data from api
                debug!("XMRig-Proxy Watchdog | Attempting HTTP API request...");
                match PrivXmrigProxyApi::request_xp_api(&client, &api_summary_xp, &state.token)
                    .await
                {
                    Ok(priv_api) => {
                        debug!(
                            "XMRig-Proxy Watchdog | HTTP API request OK, attempting [update_from_priv()]"
                        );
                        PubXmrigProxyApi::update_from_priv(pub_api, priv_api);
                    }
                    Err(err) => {
                        warn!(
                            "XMRig-Proxy Watchdog | Could not send HTTP API request to: {api_summary_xp}\n{err}"
                        );
                    }
                }
                // update xmrig to use xmrig-proxy if option enabled and local xmrig alive
                // if the request was just sent, do not repeat it, let xmrig time to apply the change.
                let pool = Pool::XmrigProxy(state.bind_port()); // get current port of xmrig-proxy
                if (state.redirect_local_xmrig
                    && pub_api_xmrig.lock().unwrap().pool.as_ref() != Some(&pool)
                    && (process_xmrig.lock().unwrap().state == ProcessState::Alive
                        || process_xmrig.lock().unwrap().state == ProcessState::NotMining))
                    && (first_loop || last_redirect_request.elapsed() > Duration::from_secs(5))
                {
                    last_redirect_request = Instant::now();
                    info!("redirect local xmrig instance to xmrig-proxy");
                    let api_uri =
                        current_api_url_xrig(true, Some(&xmrig_img.lock().unwrap()), None);
                    if let Err(err) = update_xmrig_config(
                        &client,
                        &api_uri,
                        &xmrig_img.lock().unwrap().token,
                        &pool,
                        "",
                        GUPAX_VERSION_UNDERSCORE,
                    )
                    .await
                    {
                        // show to console error about updating xmrig config
                        warn!("XMRig-Proxy Process | Failed request HTTP API Xmrig");
                        output_console(
                            &mut gui_api.lock().unwrap().output,
                            &format!("Failure to update xmrig config with HTTP API.\nError: {err}"),
                            ProcessName::XmrigProxy,
                        );
                    } else {
                        debug!("XMRig-Proxy Process | mining on Xmrig-Proxy pool");
                    }
                }
            } // locked are dropped here
            // do not use more than 1 second for the loop
            sleep_end_loop(now, ProcessName::XmrigProxy).await;
            if first_loop {
                first_loop = false;
            }
        }

        // 5. If loop broke, we must be done here.
        info!("XMRig-Proxy Watchdog | Watchdog thread exiting... Goodbye!");
        // sleep
    }
}
//---------------------------------------------------------------------------------------------------- [ImgProxy]
#[derive(Debug, Clone)]
pub struct ImgProxy {
    pub api_port: u16,
    pub port: u16,
    pub token: String,
}

impl Default for ImgProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl ImgProxy {
    pub fn new() -> Self {
        Self {
            api_port: PROXY_API_PORT_DEFAULT,
            port: PROXY_PORT_DEFAULT,
            token: String::new(),
        }
    }
}
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct PubXmrigProxyApi {
    pub output: String,
    pub uptime: HumanTime,
    pub accepted: u32,
    pub rejected: u32,
    pub hashrate: String,
    pub hashrate_1m: f32,
    pub hashrate_10m: f32,
    pub hashrate_1h: f32,
    pub hashrate_12h: f32,
    pub hashrate_24h: f32,
    pub miners: u16,
    pub pool: Option<Pool>,
}

impl Default for PubXmrigProxyApi {
    fn default() -> Self {
        Self::new()
    }
}
impl PubXmrigProxyApi {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            uptime: HumanTime::new(),
            accepted: 0,
            rejected: 0,
            hashrate: HumanNumber::from_hashrate(&[None, None, None, None, None, None]).to_string(),
            hashrate_1m: 0.0,
            hashrate_10m: 0.0,
            hashrate_1h: 0.0,
            hashrate_12h: 0.0,
            hashrate_24h: 0.0,
            miners: 0,
            pool: None,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update_from_output(
        public: &mut Self,
        output_parse: &Arc<Mutex<String>>,
        output_pub: &Arc<Mutex<String>>,
        elapsed: std::time::Duration,
        process: &mut Process,
        process_p2pool: &Process,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        p2pool_state: &P2pool,
        state: &XmrigProxy,
    ) {
        // 1. Take the process's current output buffer and combine it with Pub (if not empty)
        let mut output_pub = output_pub.lock().unwrap();

        {
            if !output_pub.is_empty() {
                public.output.push_str(&std::mem::take(&mut *output_pub));
            }
            // Update uptime
            public.uptime = HumanTime::into_human(elapsed);
        }

        drop(output_pub);
        let mut output_parse = output_parse.lock().unwrap();
        // 2. Check for "new job"/"no active...".
        if XMRIG_REGEX.new_job.is_match(&output_parse)
            || XMRIG_REGEX.valid_conn.is_match(&output_parse)
        {
            process.state = ProcessState::Alive;
            // get the pool we mine on to put it on stats
            if let Some(name_pool) = detect_pool_xmrig(
                &output_parse,
                state.bind_port(),
                p2pool_state.current_port(process_p2pool.is_alive(), &p2pool_img.lock().unwrap()),
            ) {
                public.pool = Some(name_pool);
            }
        } else if XMRIG_REGEX.timeout.is_match(&output_parse)
            || XMRIG_REGEX.invalid_conn.is_match(&output_parse)
            || XMRIG_REGEX.error.is_match(&output_parse)
        {
            process.state = ProcessState::NotMining;
            public.pool = None;
        }
        // 3. Throw away [output_parse]
        output_parse.clear();
        drop(output_parse);
    }
    // same method as PubXmrigApi, why not make a trait ?
    pub fn combine_gui_pub_api(gui_api: &mut Self, pub_api: &mut Self) {
        let output = std::mem::take(&mut gui_api.output);
        let buf = std::mem::take(&mut pub_api.output);
        *gui_api = Self {
            output,
            ..pub_api.clone()
        };
        if !buf.is_empty() {
            gui_api.output.push_str(&buf);
        }
    }
    fn update_from_priv(public: &Arc<Mutex<Self>>, private: PrivXmrigProxyApi) {
        let mut public = public.lock().unwrap();
        let mut total_hashrate = private
            .hashrate
            .total
            .iter()
            .map(|x| Some(*x as u64))
            .collect::<Vec<Option<u64>>>();
        total_hashrate.remove(5);
        *public = Self {
            accepted: private.results.accepted,
            rejected: private.results.rejected,
            hashrate: HumanNumber::from_hashrate(&total_hashrate).to_string(),
            hashrate_1m: private.hashrate.total[0],
            hashrate_10m: private.hashrate.total[1],
            hashrate_1h: private.hashrate.total[2],
            hashrate_12h: private.hashrate.total[3],
            hashrate_24h: private.hashrate.total[4],
            miners: private.miners.now,
            ..std::mem::take(&mut *public)
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PrivXmrigProxyApi {
    hashrate: HashrateProxy,
    miners: Miners,
    results: Results,
}

#[derive(Deserialize, Serialize)]
struct Results {
    accepted: u32,
    rejected: u32,
}

#[derive(Deserialize, Serialize)]
struct HashrateProxy {
    total: [f32; 6],
}

#[derive(Deserialize, Serialize)]
struct Miners {
    now: u16,
    max: u16,
}
impl PrivXmrigProxyApi {
    #[inline]
    // Send an HTTP request to XMRig's API, serialize it into [Self] and return it
    async fn request_xp_api(
        client: &Client,
        api_uri: &str,
        token: &str,
    ) -> std::result::Result<Self, anyhow::Error> {
        let request = client
            .get(api_uri)
            .header(AUTHORIZATION, ["Bearer ", token].concat());
        let mut private = request
            .timeout(std::time::Duration::from_millis(5000))
            .send()
            .await?
            .json::<PrivXmrigProxyApi>()
            .await?;
        // every hashrate value of xmrig-proxy is in kH/s, so we put convert it into H/s
        for h in &mut private.hashrate.total {
            *h *= 1000.0
        }
        Ok(private)
    }
}
