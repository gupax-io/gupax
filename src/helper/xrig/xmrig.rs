use crate::constants::*;
use crate::disk::state::{P2pool, StartOptionsMode, XmrigProxy};
use crate::helper::p2pool::ImgP2pool;
use crate::helper::xrig::HashrateProvider;
use crate::helper::{Helper, ProcessName, ProcessSignal, ProcessState, check_died};
use crate::helper::{Pool, PubXvbApi, output, signal_end};
use crate::helper::{Process, check_user_input, sleep, sleep_end_loop};
use crate::human::HumanTime;
use crate::miscs::{client, output_console};
use crate::regex::XMRIG_REGEX;
use crate::utils::human::HumanNumber;
use cfg_if::cfg_if;
use enclose::{enc, enclose};
use log::*;
use readable::num::Unsigned;
use reqwest::header::AUTHORIZATION;
use reqwest_middleware::ClientWithMiddleware as Client;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::Path;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::*,
};

use super::Hashrate;
use super::xmrig_proxy::ImgProxy;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::process::{Child, ExitStatus};

// Gupax's console shows plain text, and the Windows pseudo console adds lines made
// of escape sequences only.
pub(super) fn printable(line: String) -> Option<String> {
    let line = strip_ansi_escapes::strip_str(line);
    (!line.trim().is_empty()).then_some(line)
}

// What the XMRig watchdog works on, from its start to its end.
struct XmrigWatch {
    process: Arc<Mutex<Process>>,
    gui_api: Arc<Mutex<PubXmrigApi>>,
    pub_api: Arc<Mutex<PubXmrigApi>>,
    api_ip_port: String,
    token: String,
    process_xvb: Arc<Mutex<Process>>,
    process_xp: Arc<Mutex<Process>>,
    process_p2pool: Arc<Mutex<Process>>,
    pub_api_xvb: Arc<Mutex<PubXvbApi>>,
    p2pool_state: P2pool,
    p2pool_img: Arc<Mutex<ImgP2pool>>,
    proxy_state: XmrigProxy,
    proxy_img: Arc<Mutex<ImgProxy>>,
    xmrig_img: Arc<Mutex<ImgXmrig>>,
}

// XMRig runs with rights that Gupax lacks to kill it.
struct XmrigChild {
    child: Child,
    stdin: Box<dyn IoWrite + Send>,
}

impl crate::helper::Kill for XmrigChild {
    // A Ctrl+C in its input asks XMRig to exit.
    fn kill(&mut self) -> std::io::Result<bool> {
        self.stdin.write_all(&[0x03])?;
        Ok(false)
    }
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        let status = self.child.wait()?;
        // XMRig ends with STATUS_CONTROL_C_EXIT on a Ctrl+C that comes before its handler.
        #[cfg(target_os = "windows")]
        if status.code() == Some(0xC000013A_u32 as i32) {
            return Ok(ExitStatus::default());
        }
        Ok(status)
    }
}

impl Helper {
    #[cold]
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    pub fn read_pty_xmrig(
        output_parse: Arc<Mutex<String>>,
        output_pub: Arc<Mutex<String>>,
        mut reader: Box<dyn std::io::Read + Send>,
        process_xvb: Arc<Mutex<Process>>,
        process_xp: Arc<Mutex<Process>>,
        process_p2pool: Arc<Mutex<Process>>,
        pub_api_xvb: &Arc<Mutex<PubXvbApi>>,
        p2pool_state: &P2pool,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        proxy_img: &Arc<Mutex<ImgProxy>>,
        proxy_state: &XmrigProxy,
    ) {
        let mut lines = crate::helper::text_lines(&mut reader);
        // XMRig writes why it cannot start before its banner.
        while let Some(Ok(line)) = lines.next() {
            let Some(line) = printable(line) else {
                continue;
            };
            if line.contains("ABOUT") {
                info!("Xmrig is started");
            }
            // need to verify if pool still working
            // for that need to catch "connect error"
            // only check if xvb process is used and xmrig-proxy is not.
            if process_xvb.lock().unwrap().is_alive() && !process_xp.lock().unwrap().is_alive() {
                let proxy_port = proxy_state
                    .current_ports(
                        process_xp.lock().unwrap().is_alive(),
                        &proxy_img.lock().unwrap(),
                    )
                    .0;
                let p2pool_port = p2pool_state.current_port(
                    process_p2pool.lock().unwrap().is_alive(),
                    &p2pool_img.lock().unwrap(),
                );
                Pool::update_current_pool(
                    &line,
                    proxy_port,
                    p2pool_port,
                    &process_xvb,
                    pub_api_xvb,
                    ProcessName::Xmrig,
                    p2pool_state.address.clone(),
                );
            }
            //			println!("{}", line); // For debugging.
            if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                error!("XMRig PTY Parse | Output error: {e}");
            }
            if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                error!("XMRig PTY Pub | Output error: {e}");
            }
        }
    }
    //---------------------------------------------------------------------------------------------------- XMRig specific, most functions are very similar to P2Pool's
    #[cold]
    #[inline(never)]
    // Just sets some signals for the watchdog thread to pick up on.
    pub fn stop_xmrig(helper: &Arc<Mutex<Self>>) {
        info!("XMRig | Attempting to stop...");
        Process::raise(&helper.lock().unwrap().xmrig, ProcessSignal::Stop);
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_xmrig);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_xmrig);
        *pub_api.lock().unwrap() = PubXmrigApi::new();
        *gui_api.lock().unwrap() = PubXmrigApi::new();
    }

    #[cold]
    #[inline(never)]
    // The "restart frontend" to a "frontend" function.
    // Basically calls to kill the current xmrig, waits a little, then starts the below function in a a new thread, then exit.
    pub fn restart_xmrig(
        helper: &Arc<Mutex<Self>>,
        state: &crate::disk::state::Xmrig,
        state_p2pool: &P2pool,
        state_proxy: &XmrigProxy,
        path: &Path,
    ) {
        info!("XMRig | Attempting to restart...");
        Process::raise(&helper.lock().unwrap().xmrig, ProcessSignal::Restart);

        let path = path.to_path_buf();
        // This thread lives to wait, start xmrig then die.
        thread::spawn(enc!((helper, state, state_p2pool, state_proxy)move || {
            while helper.lock().unwrap().xmrig.lock().unwrap().state != ProcessState::Waiting {
                warn!("XMRig | Want to restart but process is still alive, waiting...");
                sleep!(1000);
            }
            // Ok, process is not alive, start the new one!
            info!("XMRig | Old process seems dead, starting new one!");
            Self::start_xmrig(&helper, &state, &state_p2pool, &state_proxy, &path);
        }));
        info!("XMRig | Restart ... OK");
    }

    #[cold]
    #[inline(never)]
    pub fn start_xmrig(
        helper: &Arc<Mutex<Self>>,
        state: &crate::disk::state::Xmrig,
        p2pool_state: &P2pool,
        proxy_state: &XmrigProxy,
        path: &Path,
    ) {
        // get the stratum port of p2pool
        //
        let process_p2pool = Arc::clone(&helper.lock().unwrap().p2pool);
        let p2pool_img = Arc::clone(&helper.lock().unwrap().img_p2pool);

        let p2pool_stratum_port = p2pool_state.current_port(
            process_p2pool.lock().unwrap().is_alive(),
            &p2pool_img.lock().unwrap(),
        );
        helper.lock().unwrap().xmrig.lock().unwrap().set_starting();
        let api_ip_port = Self::mutate_img_xmrig(helper, state, p2pool_stratum_port);
        let mode = if state.simple {
            StartOptionsMode::Simple
        } else if !state.arguments.is_empty() {
            StartOptionsMode::Custom
        } else {
            StartOptionsMode::Advanced
        };
        let args = Self::build_xmrig_args(state, mode, p2pool_stratum_port);
        // Print arguments & user settings to console
        crate::disk::print_dash(&format!("XMRig | Launch arguments: {args:#?}"));
        info!("XMRig | Using path: [{}]", path.display());

        // Spawn watchdog thread
        let process = Arc::clone(&helper.lock().unwrap().xmrig);
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_xmrig);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_xmrig);
        let process_xvb = Arc::clone(&helper.lock().unwrap().xvb);
        let process_xp = Arc::clone(&helper.lock().unwrap().xmrig_proxy);
        let process_p2pool = Arc::clone(&helper.lock().unwrap().p2pool);
        let path = path.to_path_buf();
        let token = state.token.clone();
        let p2pool_state = p2pool_state.clone();
        let p2pool_img = Arc::clone(&helper.lock().unwrap().img_p2pool);
        let proxy_state = proxy_state.clone();
        let proxy_img = Arc::clone(&helper.lock().unwrap().img_proxy);
        let xmrig_img = Arc::clone(&helper.lock().unwrap().img_xmrig);
        let pub_api_xvb = Arc::clone(&helper.lock().unwrap().pub_api_xvb);
        let watch = XmrigWatch {
            process,
            gui_api,
            pub_api,
            api_ip_port,
            token,
            process_xvb,
            process_xp,
            process_p2pool,
            pub_api_xvb,
            p2pool_state,
            p2pool_img,
            proxy_state,
            proxy_img,
            xmrig_img,
        };
        thread::spawn(move || Self::spawn_xmrig_watchdog(watch, args, path));
    }
    pub fn mutate_img_xmrig(
        helper: &Arc<Mutex<Self>>,
        state: &crate::disk::state::Xmrig,
        stratum_port: u16,
    ) -> String {
        let mut api_ip = String::with_capacity(15);
        let mut api_port = String::with_capacity(5);
        if state.simple {
            api_ip = "127.0.0.1".to_string();
            api_port = "18088".to_string();

            *helper.lock().unwrap().img_xmrig.lock().unwrap() = ImgXmrig {
                threads: state.current_threads.to_string(),
                url: format!("127.0.0.1:{stratum_port} (Local P2Pool)"),
                api_port: XMRIG_API_PORT_DEFAULT,
                token: state.token.clone(),
            };
        } else if !state.arguments.is_empty() {
            // This parses the input and attempts to fill out
            // the [ImgXmrig]... This is pretty bad code...
            let mut last = "";
            let lock = helper.lock().unwrap();
            let mut xmrig_image = lock.img_xmrig.lock().unwrap();
            for arg in state.arguments.split_whitespace() {
                if let Some(token) = arg.strip_prefix("--http-access-token=") {
                    xmrig_image.token = token.to_string();
                }
                match last {
                    "--threads" => xmrig_image.threads = arg.to_string(),
                    "--url" => xmrig_image.url = arg.to_string(),
                    "--http-host" => {
                        api_ip = if arg == "localhost" {
                            "127.0.0.1".to_string()
                        } else {
                            arg.to_string()
                        }
                    }
                    "--http-port" => {
                        api_port = arg.to_string();
                        xmrig_image.api_port = arg.parse().unwrap_or(XMRIG_API_PORT_DEFAULT)
                    }
                    _ => (),
                }
                last = arg;
            }
        } else {
            let ip = if state.ip == "localhost" || state.ip.is_empty() {
                "127.0.0.1"
            } else {
                &state.ip
            };
            api_ip = if state.api_ip == "localhost" || state.api_ip.is_empty() {
                "127.0.0.1".to_string()
            } else {
                state.api_ip.to_string()
            };
            api_port = if state.api_port.is_empty() {
                XMRIG_API_PORT_DEFAULT.to_string()
            } else {
                state.api_port.to_string()
            };
            let url = format!("{}:{}", ip, state.port); // Combine IP:Port into one string
            *helper.lock().unwrap().img_xmrig.lock().unwrap() = ImgXmrig {
                url: url.clone(),
                threads: state.current_threads.to_string(),
                api_port: state.api_port.parse().unwrap_or(XMRIG_API_PORT_DEFAULT),
                token: state.token.clone(),
            };
        }

        format!("{api_ip}:{api_port}")
    }
    #[cold]
    #[inline(never)]
    // Takes in some [State/Xmrig] and parses it to build the actual command arguments.
    // Returns the [Vec] of actual arguments, and mutates the [ImgXmrig] for the main GUI thread
    // It returns a value... and mutates a deeply nested passed argument... this is some pretty bad code...
    pub fn build_xmrig_args(
        state: &crate::disk::state::Xmrig,
        // Allows to provide a different mode without mutating the state
        mode: StartOptionsMode,
        p2pool_stratum_port: u16,
    ) -> Vec<String> {
        let mut args = Vec::with_capacity(500);
        // some args needs to be added to both simple/advanced
        match mode {
            StartOptionsMode::Simple | StartOptionsMode::Advanced => {
                args.push("--no-color".to_string()); // No color escape codes
                args.push(format!("--http-access-token={}", state.token)); // HTTP API Port
                args.push("--http-no-restricted".to_string());
                args.push("--threads".to_string());
                args.push(state.current_threads.to_string()); // Threads
                if state.pause != 0 {
                    args.push("--pause-on-active".to_string());
                    args.push(state.pause.to_string());
                } // Pause on active
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
                args.push("--url".to_string());
                args.push(format!("127.0.0.1:{p2pool_stratum_port}")); // Local P2Pool (the default)
                args.push("--user".to_string());
                args.push(rig); // Rig name
                args.push("--http-host".to_string());
                args.push("127.0.0.1".to_string()); // HTTP API IP
                args.push("--http-port".to_string());
                args.push("18088".to_string()); // HTTP API Port
            }
            StartOptionsMode::Advanced => {
                // XMRig doesn't understand [localhost]
                let ip = if state.ip == "localhost" || state.ip.is_empty() {
                    "127.0.0.1"
                } else {
                    &state.ip
                };
                let api_ip = if state.api_ip == "localhost" || state.api_ip.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    state.api_ip.to_string()
                };
                let api_port = if state.api_port.is_empty() {
                    "18088".to_string()
                } else {
                    state.api_port.to_string()
                };
                let url = format!("{}:{}", ip, state.port); // Combine IP:Port into one string
                args.push("--user".to_string());
                args.push(state.address.clone()); // Wallet
                args.push(format!("--rig-id={}", state.rig)); // Rig ID
                args.push("--url".to_string());
                args.push(url.clone()); // IP/Port
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
                // This parses the input and attempts to fill out
                // the [ImgXmrig]... This is pretty bad code...
                // custom args from user input
                // This parses the input
                for arg in state.arguments.split_whitespace() {
                    let arg = if arg == "localhost" { "127.0.0.1" } else { arg };
                    args.push(arg.to_string());
                }
            }
        }
        args
    }

    // We spawn [pkexec] on Unix, with XMRig being the argument.
    #[cfg(target_os = "linux")]
    fn create_xmrig_cmd_unix(args: Vec<String>, path_binary: PathBuf) -> Command {
        let mut cmd = Command::new("pkexec");
        cmd.arg(path_binary.clone());
        cmd.args(args);
        crate::helper::run_in_folder_of(&mut cmd, &path_binary);
        cmd
    }

    // pkexec runs XMRig once its admin prompt is answered, and XMRig then prints its banner. Until
    // then, the kill of pkexec cancels the prompt.
    #[cfg(target_os = "linux")]
    fn prompt_answered(
        pkexec: &mut Child,
        output: std::io::PipeReader,
        stopped: impl Fn() -> bool,
    ) -> std::io::Result<Option<Box<dyn std::io::Read + Send>>> {
        use std::io::Read;
        let (sender, first_read) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let mut output = output;
            let mut first = vec![0; 4096];
            let read = output.read(&mut first).map(|n| {
                first.truncate(n);
                first
            });
            let _ = sender.send((read, output));
        });
        let ready = || {
            first_read
                .try_recv()
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))
        };
        let prompted = super::wait_ready(pkexec, ready, stopped, "pkexec", "XMRig wrote a line")?;
        let Some((first, output)) = prompted else {
            return Ok(None);
        };
        // The output starts with what was read while waiting.
        Ok(Some(Box::new(std::io::Cursor::new(first?).chain(output))))
    }

    // osascript opens the input of XMRig once the user accepted its admin prompt.
    #[cfg(any(target_os = "macos", all(test, unix)))]
    fn open_input(
        input: &Path,
        log: &Path,
        prompt: &mut Child,
        program: &str,
        stopped: impl Fn() -> bool,
    ) -> std::io::Result<Option<std::fs::File>> {
        use std::os::unix::fs::OpenOptionsExt;
        // Fails at once while nothing reads the input, and its writes return at once on a full
        // input.
        let open = || {
            std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(input)
                .map_err(|e| match e.raw_os_error() {
                    Some(libc::ENXIO) => std::io::ErrorKind::WouldBlock.into(),
                    _ => e,
                })
        };
        super::wait_ready(prompt, open, stopped, program, "XMRig read its input").map_err(|e| {
            let reason = std::fs::read_to_string(log).unwrap_or_default();
            std::io::Error::new(e.kind(), format!("{e}\n{}", reason.trim()).trim_end())
        })
    }

    // The root shell of a cancelled start can come late, and would read the input of the next
    // start under the same name.
    #[cfg(any(target_os = "macos", all(test, unix)))]
    fn input_name() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static STARTS: AtomicU32 = AtomicU32::new(0);
        let start = STARTS.fetch_add(1, Ordering::Relaxed);
        format!("xmrig_input_{}_{start}", std::process::id())
    }

    // The input of XMRig is a FIFO, and its output the log that its -l option writes.
    #[cfg(target_os = "macos")]
    fn start_macos(
        args: Vec<String>,
        path: PathBuf,
        stopped: impl Fn() -> bool,
    ) -> std::io::Result<Option<(Child, std::fs::File, std::fs::File)>> {
        use nix::sys::stat::Mode;
        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join(Self::input_name());
        let output_path = temp_dir.join("xmrig_output");
        let _ = std::fs::remove_file(&output_path);
        std::fs::create_dir_all(&temp_dir)?;
        nix::unistd::mkfifo(&input_path, Mode::S_IRWXU)?;
        let started = Self::start_with_fifo(args, path, &input_path, &output_path, stopped);
        // Its name serves until both ends are open.
        let _ = std::fs::remove_file(&input_path);
        started
    }

    #[cfg(target_os = "macos")]
    fn start_with_fifo(
        args: Vec<String>,
        path: PathBuf,
        input_path: &Path,
        output_path: &Path,
        stopped: impl Fn() -> bool,
    ) -> std::io::Result<Option<(Child, std::fs::File, std::fs::File)>> {
        // Opened before the admin prompt, after which a failure would leave XMRig out of reach.
        let output = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(output_path)?;
        let mut cmd = Self::create_xmrig_cmd_macos(args, path, input_path, output_path)?;
        // The console then shows why osascript failed.
        #[cfg(target_arch = "x86_64")]
        cmd.stderr(std::fs::OpenOptions::new().append(true).open(output_path)?);
        let mut prompt = cmd.spawn()?;
        let program = cmd.get_program().to_string_lossy();
        let input = Self::open_input(input_path, output_path, &mut prompt, &program, stopped)
            // The admin prompt would stay open.
            .inspect_err(|_| {
                let _ = prompt.kill();
            })?;
        let Some(input) = input else {
            return Ok(None);
        };
        #[cfg(target_arch = "x86_64")]
        (&input).write_all(b"go\n")?;
        Ok(Some((prompt, input, output)))
    }

    #[cfg(target_os = "macos")]
    fn create_xmrig_cmd_macos(
        mut args: Vec<String>,
        path: PathBuf,
        input_path: &Path,
        output_path: &Path,
    ) -> std::io::Result<Command> {
        args.push("-l".to_string());
        args.push(output_path.display().to_string());

        cfg_if!(
            // For Apple Silicon macs, sudo doesn't improve performance and osascript blocks use of performance cores
            if #[cfg(target_arch = "aarch64")] {
                use std::fs::{File, OpenOptions};

                let out_file = File::create(output_path)?;
                let in_file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(input_path)?;
                let mut cmd = Command::new(&path);
                cmd.args(args);
                // XMRig writes why its arguments are wrong there.
                cmd.stderr(out_file.try_clone()?);
                cmd.stdout(out_file);
                cmd.stdin(in_file);
            } else {
                let mut cmd = Command::new("osascript");
                cmd.arg("-e");
                cmd.arg(Self::admin_prompt(&Self::xmrig_script(&path, &args, input_path)));
            }
        );
        crate::helper::run_in_folder_of(&mut cmd, &path);
        Ok(cmd)
    }

    // The command of the root shell that runs XMRig after the macOS admin prompt.
    #[cfg(any(all(target_os = "macos", target_arch = "x86_64"), all(test, unix)))]
    fn xmrig_script(path: &Path, args: &[String], input: &Path) -> String {
        let quote = |arg: &str| shell_escape::unix::escape(arg.into()).into_owned();
        let xmrig = std::iter::once(path.display().to_string())
            .chain(args.iter().cloned())
            .map(|arg| quote(&arg))
            .collect::<Vec<_>>()
            .join(" ");
        // Gupax writes "go" once it opened the input, which a cancelled start removes.
        format!(
            "[ -p {0} ] && exec <> {0} && read go && [ \"$go\" = go ] && exec {xmrig}",
            quote(&input.display().to_string())
        )
    }

    // `do shell script` takes the command as an AppleScript string.
    #[cfg(any(all(target_os = "macos", target_arch = "x86_64"), all(test, unix)))]
    fn admin_prompt(script: &str) -> String {
        let script = script.replace('\\', "\\\\").replace('"', "\\\"");
        format!("do shell script \"{script}\" with administrator privileges")
    }

    #[cold]
    #[inline(never)]
    // The XMRig watchdog. Spawns 1 OS thread for reading a PTY (STDOUT+STDERR), and combines the [Child] with a PTY so STDIN actually works.
    // This isn't actually async, a tokio runtime is unfortunately needed because [Hyper] is an async library (HTTP API calls)
    #[tokio::main]
    async fn spawn_xmrig_watchdog(watch: XmrigWatch, args: Vec<String>, path: PathBuf) {
        let stopped = || watch.process.lock().unwrap().signal == ProcessSignal::Stop;
        cfg_if! {
            if #[cfg(windows)] {
                let started = super::elevated::start(
                    &path,
                    &args,
                    super::elevated::powershell,
                    stopped,
                    crate::components::update::GUPAX_REPLACED
                        .load(std::sync::atomic::Ordering::Relaxed),
                );
            } else if #[cfg(target_os = "linux")] {
                let started = crate::helper::spawn_piped(Self::create_xmrig_cmd_unix(args, path))
                    .and_then(|(mut pkexec, stdin, stdout)| {
                        let stdout = Self::prompt_answered(&mut pkexec, stdout, stopped)?;
                        Ok(stdout.map(|stdout| (pkexec, stdin, stdout)))
                    });
            } else if #[cfg(target_os = "macos")] {
                let started = Self::start_macos(args, path, stopped);
            }
        }
        let Some(started) = started.transpose() else {
            info!("XMRig Watchdog | Stopped before it started");
            watch.process.lock().unwrap().cancel_start();
            return;
        };
        let Some((child_pty, stdin, stdout)) = crate::helper::started(
            started,
            &mut watch.process.lock().unwrap(),
            &mut watch.gui_api.lock().unwrap().output,
        ) else {
            return;
        };
        Self::watch_xmrig(child_pty, stdin, stdout, watch).await;
    }

    #[allow(clippy::await_holding_lock)]
    async fn watch_xmrig(
        child_pty: Child,
        stdin: impl IoWrite + Send + 'static,
        stdout: impl std::io::Read + Send + 'static,
        watch: XmrigWatch,
    ) {
        let XmrigWatch {
            process,
            gui_api,
            pub_api,
            mut api_ip_port,
            ref token,
            process_xvb,
            process_xp,
            process_p2pool,
            ref pub_api_xvb,
            ref p2pool_state,
            ref p2pool_img,
            ref proxy_state,
            ref proxy_img,
            xmrig_img,
        } = watch;
        let mut xmrig = XmrigChild {
            child: child_pty,
            stdin: Box::new(stdin),
        };
        let (stdout, output_end) = output(Box::new(stdout));

        debug!("XMRig | Clearing GUI output...");
        gui_api.lock().unwrap().output.clear();

        // 3. Set process state
        debug!("XMRig | Setting process state...");
        let mut lock = process.lock().unwrap();
        lock.state = ProcessState::NotMining;
        lock.reset_signal_on_start();
        lock.start = Instant::now();
        lock.clear_output();
        drop(lock);

        let output_parse = Arc::clone(&process.lock().unwrap().output_parse);
        let output_pub = Arc::clone(&process.lock().unwrap().output_pub);
        // The reader blocks until the output of XMRig ends, which a forked XMRig keeps open.
        thread::spawn(
            enclose!((pub_api_xvb, process_xp, p2pool_state, p2pool_img, process_p2pool, proxy_img, proxy_state) move || {
                Self::read_pty_xmrig(output_parse, output_pub, stdout, process_xvb, process_xp, process_p2pool, &pub_api_xvb, &p2pool_state, &p2pool_img, &proxy_img, &proxy_state);
            }),
        );

        let client = client();
        let start = process.lock().unwrap().start;
        let api_uri_config = {
            if !api_ip_port.ends_with('/') {
                api_ip_port.push('/');
            }
            "http://".to_owned() + &api_ip_port + XMRIG_API_CONFIG_ENDPOINT
        };
        let api_uri_summary = {
            if !api_ip_port.ends_with('/') {
                api_ip_port.push('/');
            }
            "http://".to_owned() + &api_ip_port + XMRIG_API_SUMMARY_ENDPOINT
        };
        info!("XMRig | Final API URI: {api_uri_config}");

        // Reset stats before loop
        *pub_api.lock().unwrap() = PubXmrigApi::new();
        *gui_api.lock().unwrap() = PubXmrigApi::new();
        // pool used for process Status tab
        pub_api.lock().unwrap().pool = None;
        // 5. Loop as watchdog
        info!("XMRig | Entering watchdog mode... woof!");
        let hashrate_provider =
            HashrateProvider::Xmrig(gui_api.clone(), xmrig_img.clone(), client.clone());
        loop {
            // Set timer
            let now = Instant::now();
            debug!("XMRig Watchdog | ----------- Start of loop -----------");

            // Check if the process secretly died without us knowing :)
            if check_died(
                &mut xmrig,
                &output_end,
                &process,
                &start,
                &gui_api,
                &pub_api,
            ) {
                break;
            }
            // Stop on [Stop/Restart] SIGNAL
            if signal_end(
                &process,
                Some(&mut xmrig),
                &output_end,
                &start,
                &gui_api,
                &pub_api,
            ) {
                break;
            }
            // Check vector of user input
            check_user_input(&process, &mut xmrig.stdin);
            // Check if logs need resetting
            debug!("XMRig Watchdog | Attempting GUI log reset check");
            {
                let mut lock = gui_api.lock().unwrap();
                Self::check_reset_gui_output(&mut lock.output, ProcessName::Xmrig);
            }
            // Always update from output
            debug!("XMRig Watchdog | Starting [update_from_output()]");
            {
                let process_p2pool_lock = &process_p2pool.lock().unwrap();
                let mut process_lock = process.lock().unwrap();
                let process_xp_lock = &process_xp.lock().unwrap();
                let mut pub_api_lock = pub_api.lock().unwrap();
                PubXmrigApi::update_from_output(
                    &mut pub_api_lock,
                    start.elapsed(),
                    &mut process_lock,
                    process_p2pool_lock,
                    process_xp_lock,
                    proxy_img,
                    p2pool_img,
                    proxy_state,
                    p2pool_state,
                );
            }
            // Send an HTTP API request
            debug!("XMRig Watchdog | Attempting HTTP API request...");
            match PrivXmrigApi::request_xmrig_api(&client, &api_uri_summary, token).await {
                Ok(priv_api) => {
                    debug!("XMRig Watchdog | HTTP API request OK, attempting [update_from_priv()]");
                    PubXmrigApi::update_from_priv(&pub_api, priv_api);
                }
                Err(err) => {
                    warn!(
                        "XMRig Watchdog | Could not send HTTP API request to: {api_uri_summary}\n{err}"
                    );
                }
            }
            // if mining on proxy and proxy is not alive, switch back to p2pool node
            debug!("update from priv ok");
            // unlock first process_xp and then pub_api
            let p2pool_alive = process_p2pool.lock().unwrap().is_alive().to_owned();
            let xp_alive = process_xp.lock().unwrap().is_alive().to_owned();
            let xmrig_pool = pub_api.lock().unwrap().pool.to_owned();
            if (xmrig_pool
                == Some(Pool::XmrigProxy(
                    proxy_state
                        .current_ports(xp_alive, &proxy_img.lock().unwrap())
                        .0,
                ))
                || xmrig_pool.is_none())
                && !xp_alive
                && p2pool_alive
            {
                info!(
                    "XMRig Process |  redirect xmrig to p2pool since XMRig-Proxy is not alive and p2pool is alive"
                );
                let pool = Pool::P2pool(
                    p2pool_state.current_port(p2pool_alive, &p2pool_img.lock().unwrap()),
                );
                if let Err(err) = hashrate_provider.update_config(&pool).await {
                    // show to console error about updating xmrig config
                    warn!("XMRig Process | Failed request HTTP API Xmrig");
                    output_console(
                        &mut gui_api.lock().unwrap().output,
                        &format!("Failure to update xmrig config with HTTP API.\nError: {err}"),
                        ProcessName::Xmrig,
                    );
                } else {
                    debug!("XMRig Process | mining on P2Pool pool");
                }
            }
            // Sleep (only if 900ms hasn't passed)
            sleep_end_loop(now, ProcessName::Xmrig).await;
        }

        // 5. If loop broke, we must be done here.
        info!("XMRig Watchdog | Watchdog thread exiting... Goodbye!");
    }
}

//---------------------------------------------------------------------------------------------------- [ImgXmrig]
#[derive(Debug, Clone)]
pub struct ImgXmrig {
    pub threads: String,
    pub url: String,
    pub api_port: u16,
    pub token: String,
}

impl Default for ImgXmrig {
    fn default() -> Self {
        Self::new()
    }
}

impl ImgXmrig {
    pub fn new() -> Self {
        Self {
            threads: "???".to_string(),
            url: "???".to_string(),
            api_port: XMRIG_API_PORT_DEFAULT,
            token: String::new(),
        }
    }
}

//---------------------------------------------------------------------------------------------------- Public XMRig API
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PubXmrigApi {
    pub output: String,
    pub uptime: HumanTime,
    pub worker_id: String,
    pub resources: String,
    pub hashrate: String,
    pub diff: String,
    pub accepted: String,
    pub rejected: String,
    pub hashrate_raw: f32,
    pub hashrate_raw_1m: f32,
    pub hashrate_raw_15m: f32,
    pub pool: Option<Pool>,
}

impl Default for PubXmrigApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PubXmrigApi {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            uptime: HumanTime::new(),
            worker_id: UNKNOWN_DATA.to_string(),
            resources: UNKNOWN_DATA.to_string(),
            hashrate: UNKNOWN_DATA.to_string(),
            diff: UNKNOWN_DATA.to_string(),
            accepted: UNKNOWN_DATA.to_string(),
            rejected: UNKNOWN_DATA.to_string(),
            hashrate_raw: 0.0,
            hashrate_raw_1m: 0.0,
            hashrate_raw_15m: 0.0,
            pool: None,
        }
    }

    #[inline]
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

    // This combines the buffer from the PTY thread [output_pub]
    // with the actual [PubApiXmrig] output field.
    #[allow(clippy::too_many_arguments)]
    pub fn update_from_output(
        public: &mut Self,
        elapsed: std::time::Duration,
        process: &mut Process,
        process_p2pool: &Process,
        process_proxy: &Process,
        proxy_img: &Arc<Mutex<ImgProxy>>,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        proxy_state: &XmrigProxy,
        p2pool_state: &P2pool,
    ) {
        // 1. Take the process's current output buffer and combine it with Pub (if not empty)
        let mut output_pub = process.output_pub.lock().unwrap();

        {
            if !output_pub.is_empty() {
                public.output.push_str(&std::mem::take(&mut *output_pub));
            }
            // Update uptime
            public.uptime = HumanTime::into_human(elapsed);
        }
        drop(output_pub);

        let mut output_parse = process.output_parse.lock().unwrap();
        // 2. Check for "new job"/"no active...".
        if XMRIG_REGEX.new_job.is_match(&output_parse) {
            process.state.set_running(ProcessState::Alive);
            // get the pool we mine on to put it on stats
            if let Some(name_pool) = crate::regex::detect_pool_xmrig(
                &output_parse,
                proxy_state
                    .current_ports(process_proxy.is_alive(), &proxy_img.lock().unwrap())
                    .0,
                p2pool_state.current_port(process_p2pool.is_alive(), &p2pool_img.lock().unwrap()),
            ) {
                public.pool = Some(name_pool);
            }
        } else if XMRIG_REGEX.not_mining.is_match(&output_parse) {
            process.state.set_running(ProcessState::NotMining);
            public.pool = None;
        }

        // 3. Throw away [output_parse]
        output_parse.clear();
        drop(output_parse);
    }

    // Formats raw private data into ready-to-print human readable version.
    fn update_from_priv(public: &Arc<Mutex<Self>>, private: PrivXmrigApi) {
        let mut public = public.lock().unwrap();
        let hashrate_raw = match private.hashrate.total.first() {
            Some(Some(h)) => *h,
            _ => 0.0,
        };
        let hashrate_raw_1m = match private.hashrate.total.get(1) {
            Some(Some(h)) => *h,
            _ => 0.0,
        };
        let hashrate_raw_15m = match private.hashrate.total.last() {
            Some(Some(h)) => *h,
            _ => 0.0,
        };
        let total_hasrate = private
            .hashrate
            .total
            .iter()
            .map(|x| x.as_ref().map(|y| *y as u64))
            .collect::<Vec<Option<u64>>>();
        *public = Self {
            worker_id: private.worker_id,
            resources: HumanNumber::from_load(private.resources.load_average).to_string(),
            hashrate: HumanNumber::from_hashrate(&total_hasrate).to_string(),
            diff: Unsigned::from(private.connection.diff as usize).to_string(),
            accepted: Unsigned::from(private.connection.accepted as usize).to_string(),
            rejected: Unsigned::from(private.connection.rejected as usize).to_string(),
            hashrate_raw,
            hashrate_raw_1m,
            hashrate_raw_15m,
            ..std::mem::take(&mut *public)
        }
    }
}

//---------------------------------------------------------------------------------------------------- Private XMRig API
// This matches to some JSON stats in the HTTP call [summary],
// e.g: [wget -qO- localhost:18085/1/summary].
// XMRig doesn't initialize stats at 0 (or 0.0) and instead opts for [null]
// which means some elements need to be wrapped in an [Option] or else serde will [panic!].
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivXmrigApi {
    worker_id: String,
    resources: Resources,
    connection: Connection,
    hashrate: Hashrate,
}

impl PrivXmrigApi {
    #[inline]
    // Send an HTTP request to XMRig's API, serialize it into [Self] and return it
    async fn request_xmrig_api(
        client: &Client,
        api_uri: &str,
        token: &str,
    ) -> std::result::Result<Self, anyhow::Error> {
        let request = client
            .get(api_uri)
            .header(AUTHORIZATION, ["Bearer ", token].concat());
        Ok(request
            .timeout(std::time::Duration::from_millis(5000))
            .send()
            .await?
            .json()
            .await?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Resources {
    load_average: [Option<f32>; 3],
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Connection {
    diff: u128,
    accepted: u128,
    rejected: u128,
}

//  get the API port that would be used if xmrig was started with the current settings
// pub fn get_xmrig_api_port(xmrig_state: &Xmrig) -> u16 {
//     if xmrig_state.simple {
//         XMRIG_API_PORT_DEFAULT
//     } else if !xmrig_state.arguments.is_empty() {
//         let mut last = "";
//         for arg in xmrig_state.arguments.split_whitespace() {
//             if last == "--http-host" {
//                 return last.parse().unwrap_or(XMRIG_API_PORT_DEFAULT);
//             }
//             last = arg;
//         }
//         return XMRIG_API_PORT_DEFAULT;
//     } else {
//         return xmrig_state
//             .api_port
//             .parse()
//             .unwrap_or(XMRIG_API_PORT_DEFAULT);
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::{not_alive, shell_command, within};
    use crate::utils::macros::arc_mut;
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;

    fn xmrig() -> Arc<Mutex<Process>> {
        let process = not_alive(ProcessName::Xmrig);
        process.lock().unwrap().state = ProcessState::Alive;
        process
    }

    type StandIn = (Child, Box<dyn IoWrite + Send>, Box<dyn Read + Send>);

    // Plays XMRig, with its input and output piped.
    fn stand_in(mut cmd: Command) -> StandIn {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        (child, Box::new(stdin), Box::new(stdout))
    }

    // A stand-in that exited once it ran `script`.
    fn exited(script: &str) -> StandIn {
        let mut xmrig_run = stand_in(shell_command(script, script));
        xmrig_run.0.wait().unwrap();
        xmrig_run
    }

    // Returns the console output once the watchdog loop of `process` ended.
    fn watch((child, stdin, stdout): StandIn, process: &Arc<Mutex<Process>>) -> String {
        let gui_api = arc_mut!(PubXmrigApi::new());
        let watch = XmrigWatch {
            process: Arc::clone(process),
            gui_api: Arc::clone(&gui_api),
            pub_api: arc_mut!(PubXmrigApi::new()),
            api_ip_port: String::new(),
            token: String::new(),
            process_xvb: not_alive(ProcessName::Xvb),
            process_xp: not_alive(ProcessName::XmrigProxy),
            process_p2pool: not_alive(ProcessName::P2pool),
            pub_api_xvb: arc_mut!(PubXvbApi::new()),
            p2pool_state: P2pool::default(),
            p2pool_img: arc_mut!(ImgP2pool::new()),
            proxy_state: XmrigProxy::default(),
            proxy_img: arc_mut!(ImgProxy::new()),
            xmrig_img: arc_mut!(ImgXmrig::new()),
        };
        within(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(Helper::watch_xmrig(child, stdin, stdout, watch))
        });
        gui_api.lock().unwrap().output.clone()
    }

    // Tells whether the XMRig reader reads from a thread of the watchdog's runtime.
    struct RuntimeProbe(mpsc::Sender<bool>);

    impl Read for RuntimeProbe {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            let _ = self.0.send(tokio::runtime::Handle::try_current().is_ok());
            Ok(0)
        }
    }

    #[test]
    fn xmrig_reader_runs_apart_from_the_watchdog_runtime() {
        let (sender, on_runtime) = mpsc::channel();
        let (xmrig_run, stdin, _) = exited("exit 3");
        watch((xmrig_run, stdin, Box::new(RuntimeProbe(sender))), &xmrig());
        assert!(!on_runtime.recv_timeout(Duration::from_secs(5)).unwrap());
    }

    // Exits once it reads the Ctrl+C of the watchdog.
    #[cfg(windows)]
    fn until_ctrl_c() -> Command {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", "[void][Console]::In.Read()"]);
        cmd
    }

    #[cfg(unix)]
    fn until_ctrl_c() -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "dd bs=1 count=1 >/dev/null 2>&1"]);
        cmd
    }

    #[test]
    fn watchdog_stops_xmrig_on_its_stop_signal() {
        let process = xmrig();
        process.lock().unwrap().signal = ProcessSignal::Stop;
        let output = watch(stand_in(until_ctrl_c()), &process);
        assert!(output.contains("XMRig stopped"), "{output}");
        assert_eq!(process.lock().unwrap().state, ProcessState::Dead);
    }

    #[test]
    fn watchdog_shows_the_last_lines_of_xmrig_before_its_exit_status() {
        let process = xmrig();
        let output = watch(exited("echo ABOUT XMRig && exit 3"), &process);
        let exit = output.find("Exit status: [Failed]").expect(&output);
        assert!(output[..exit].contains("ABOUT XMRig"), "{output}");
        assert_eq!(process.lock().unwrap().state, ProcessState::Failed);
    }

    #[test]
    fn xmrig_output_keeps_a_stop_in_progress() {
        let process = xmrig();
        process.lock().unwrap().state = ProcessState::Middle;
        *process.lock().unwrap().output_parse.lock().unwrap() =
            "[2022-02-12 12:49:30.311]  net      new job from 192.168.2.1:3333 diff 402K algo rx/0 height 2241142 (11 tx)\n".to_string();
        PubXmrigApi::update_from_output(
            &mut PubXmrigApi::new(),
            Duration::ZERO,
            &mut process.lock().unwrap(),
            &not_alive(ProcessName::P2pool).lock().unwrap(),
            &not_alive(ProcessName::XmrigProxy).lock().unwrap(),
            &arc_mut!(ImgProxy::new()),
            &arc_mut!(ImgP2pool::new()),
            &XmrigProxy::default(),
            &P2pool::default(),
        );
        assert_eq!(process.lock().unwrap().state, ProcessState::Middle);
    }

    #[test]
    fn xmrig_lines_before_its_banner_are_shown() {
        let output_pub = arc_mut!(String::new());
        Helper::read_pty_xmrig(
            arc_mut!(String::new()),
            Arc::clone(&output_pub),
            Box::new(std::io::Cursor::new(
                "Error: C:\\XMRig\\xmrig.exe: not found\r\n * ABOUT XMRig/6.22\r\n",
            )),
            not_alive(ProcessName::Xvb),
            not_alive(ProcessName::XmrigProxy),
            not_alive(ProcessName::P2pool),
            &arc_mut!(PubXvbApi::new()),
            &P2pool::default(),
            &arc_mut!(ImgP2pool::new()),
            &arc_mut!(ImgProxy::new()),
            &XmrigProxy::default(),
        );
        assert_eq!(
            *output_pub.lock().unwrap(),
            "Error: C:\\XMRig\\xmrig.exe: not found\n * ABOUT XMRig/6.22\n"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn xmrig_ended_by_the_ctrl_c_of_a_stop_stopped_successfully() {
        // STATUS_CONTROL_C_EXIT
        let (child, stdin, _) = exited("exit -1073741510");
        let mut xmrig_run = XmrigChild { child, stdin };
        assert!(crate::helper::Kill::wait(&mut xmrig_run).unwrap().success());
    }

    // A FIFO, as the macOS start makes for the input of XMRig.
    #[cfg(unix)]
    fn fifo(dir: &tempfile::TempDir) -> PathBuf {
        let fifo = dir.path().join("xmrig_input");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        fifo
    }

    // Opens the input of XMRig while `prompt` runs, as the macOS start does.
    #[cfg(unix)]
    fn open_input(
        dir: &tempfile::TempDir,
        mut prompt: Command,
        stopped: bool,
    ) -> (std::io::Result<Option<std::fs::File>>, Child) {
        let (input, log) = (dir.path().join("xmrig_input"), dir.path().join("log"));
        prompt.stderr(std::fs::File::create(&log).unwrap());
        let mut prompt = prompt.spawn().unwrap();
        within(move || {
            let opened = Helper::open_input(&input, &log, &mut prompt, "osascript", || stopped);
            (opened, prompt)
        })
    }

    #[cfg(unix)]
    #[test]
    fn xmrig_input_opens_once_xmrig_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Command::new("cat");
        cat.arg(fifo(&dir)).stdout(Stdio::null());
        let (opened, mut xmrig_run) = open_input(&dir, cat, false);
        assert!(opened.unwrap().is_some());
        xmrig_run.wait().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn xmrig_input_never_blocks_the_watchdog() {
        let dir = tempfile::tempdir().unwrap();
        // Opens the input for reading, then sleeps.
        let mut reader = Command::new("sh");
        reader
            .args(["-c", "exec 0<\"$0\"; exec sleep 30"])
            .arg(fifo(&dir));
        let (opened, mut reader) = open_input(&dir, reader, false);
        let mut input = opened.unwrap().unwrap();
        let full = within(move || input.write_all(&[0; 1 << 20]));
        assert_eq!(full.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        reader.kill().unwrap();
        reader.wait().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn xmrig_input_shows_why_the_admin_prompt_failed() {
        let dir = tempfile::tempdir().unwrap();
        fifo(&dir);
        let mut prompt = Command::new("sh");
        prompt.args([
            "-c",
            "echo 'execution error: User canceled. (-128)' >&2; exit 1",
        ]);
        let error = open_input(&dir, prompt, false).0.unwrap_err().to_string();
        assert!(error.starts_with("osascript exited"), "{error}");
        assert!(error.ends_with("User canceled. (-128)"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn xmrig_input_waits_no_more_once_stopped() {
        let dir = tempfile::tempdir().unwrap();
        fifo(&dir);
        let mut sleep = Command::new("sleep");
        sleep.arg("30");
        let (opened, _) = open_input(&dir, sleep, true);
        assert!(opened.unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn each_start_names_its_own_input() {
        assert_ne!(Helper::input_name(), Helper::input_name());
    }

    #[cfg(unix)]
    #[test]
    fn a_cancelled_start_never_runs_xmrig() {
        let dir = tempfile::tempdir().unwrap();
        let ran = dir.path().join("ran");
        let xmrig_args = ["-c".to_string(), format!("touch '{}'", ran.display())];
        // Removed by the cancel.
        let input = dir.path().join("xmrig_input");
        let script = Helper::xmrig_script(Path::new("/bin/sh"), &xmrig_args, &input);
        let root_shell = Command::new("sh").arg("-c").arg(script).status().unwrap();
        assert!(!root_shell.success());
        assert!(!ran.exists());
        assert!(!input.exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_macos_root_shell_gives_xmrig_its_arguments_whole() {
        let dir = tempfile::tempdir().unwrap();
        // A shell, which writes the arguments after its script one per line.
        let xmrig = dir.path().join(r#"Jo's "xmrig" [x] $HOME"#);
        std::os::unix::fs::symlink("/bin/sh", &xmrig).unwrap();
        let written = dir.path().join("arguments");
        let args = [
            "--rig-id=Jo's rig",
            "a b",
            r#"x"y\z"#,
            "$HOME `id`",
            "--cuda-devices=0,1",
        ];
        let script = format!("printf '%s\\n' \"$@\" > '{}'", written.display());
        let mut xmrig_args = vec!["-c".to_string(), script, "xmrig".to_string()];
        xmrig_args.extend(args.map(String::from));
        let input = fifo(&dir);
        let mut root_shell = Command::new("sh")
            .arg("-c")
            .arg(Helper::xmrig_script(&xmrig, &xmrig_args, &input))
            .spawn()
            .unwrap();
        // Kept open until the root shell read "go", which a closed input would lose.
        let mut gupax_input = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&input)
            .unwrap();
        gupax_input.write_all(b"go\n").unwrap();
        assert!(root_shell.wait().unwrap().success());
        assert_eq!(
            std::fs::read_to_string(written).unwrap(),
            args.map(|arg| format!("{arg}\n")).concat()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_macos_admin_prompt_keeps_its_command_whole() {
        assert_eq!(
            Helper::admin_prompt(r#"'a "b"' \c"#),
            r#"do shell script "'a \"b\"' \\c" with administrator privileges"#
        );
    }

    // PowerShell starts the test binary again, which then runs as the elevated helper.
    #[cfg(target_os = "windows")]
    fn run_as_the_helper_when_started_with(variable: &str) {
        use clap::Parser;
        if std::env::var_os(variable).is_some() {
            crate::run_elevated_helper(elevated_helper::cli::Args::parse_from(
                std::env::args_os().skip_while(|arg| arg != "--").skip(1),
            ));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn xmrig_starts_through_powershell_and_the_helper() {
        use super::super::elevated::{start, unelevated};
        const HELPER: &str = "GUPAX_TEST_HELPER";
        const TEST: &str =
            "helper::xrig::xmrig::tests::xmrig_starts_through_powershell_and_the_helper";
        run_as_the_helper_when_started_with(HELPER);
        let test = std::env::current_exe().unwrap();
        // A hard link needs the volume of the test binary.
        let dir = tempfile::Builder::new()
            .prefix("Jo O'Neil [x] ")
            .tempdir_in(test.parent().unwrap())
            .unwrap();
        let helper = dir.path().join("Gupax.exe");
        std::fs::hard_link(&test, &helper).unwrap();
        // Echoes the arguments as XMRig receives them.
        let xmrig_exe = dir.path().join("xmrig.exe");
        std::fs::copy(r"C:\windows\system32\cmd.exe", &xmrig_exe).unwrap();
        let args = [
            "/C",
            "echo",
            "--rig-id=Jo's rig",
            "--cuda-devices=0,1",
            "&",
            "exit",
            "3",
        ];
        let (powershell_run, stdin, output, status) = within(move || {
            let (mut powershell_run, stdin, mut stdout) = start(
                &xmrig_exe,
                &args.map(String::from),
                |_, command_line| {
                    let mut cmd = unelevated(&helper, &format!("--exact {TEST} -- {command_line}"));
                    cmd.env(HELPER, "1");
                    cmd
                },
                || false,
                false,
            )
            .unwrap()
            .unwrap();
            // The helper exits once its output was read, so the watchdog then sees
            // PowerShell exited on its first pass.
            let mut output = Vec::new();
            let _ = stdout.read_to_end(&mut output);
            let status = powershell_run.wait().unwrap();
            (powershell_run, stdin, output, status)
        });
        let console = watch(
            (
                powershell_run,
                Box::new(stdin),
                Box::new(std::io::Cursor::new(output)),
            ),
            &xmrig(),
        );
        assert!(
            console.contains(r#""--rig-id=Jo's rig" --cuda-devices=0,1"#),
            "{console}"
        );
        assert!(console.contains("Exit status: [Failed]"), "{console}");
        assert_eq!(status.code(), Some(3));
    }

    // Whether pkexec, played by `script`, started XMRig, and the output of XMRig then.
    #[cfg(target_os = "linux")]
    fn prompt_answered(script: &'static str, stopped: bool) -> std::io::Result<Option<String>> {
        within(move || {
            let (mut pkexec, _stdin, stdout) =
                crate::helper::spawn_piped(shell_command(script, script)).unwrap();
            let stdout = Helper::prompt_answered(&mut pkexec, stdout, || stopped)?;
            Ok(stdout.map(|stdout| std::io::read_to_string(stdout).unwrap()))
        })
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_stop_cancels_the_admin_prompt_of_xmrig() {
        // Prints nothing, as pkexec while its prompt waits for an answer.
        assert_eq!(prompt_answered("exec sleep 30", true).unwrap(), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn xmrig_output_starts_with_its_banner_once_the_prompt_is_answered() {
        let output = prompt_answered("echo ABOUT; echo next", false).unwrap();
        assert_eq!(output.unwrap(), "ABOUT\nnext\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_xmrig_that_left_pkexec_without_a_line_fails_its_start() {
        // As XMRig with --background: its fork keeps the output open and writes nothing.
        let error = prompt_answered("sleep 2 & exit 0", false).unwrap_err();
        assert!(error.to_string().starts_with("pkexec exited"), "{error}");
    }
}
