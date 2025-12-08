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

use super::Helper;
use super::Process;
use crate::app::BackupNodes;
use crate::app::panels::middle::common::list_poolnode::PoolNode;
use crate::app::submenu_enum::SubmenuP2pool;
use crate::components::node::RemoteNode;
use crate::disk::node::Node as NodeString;
use crate::disk::state::Node;
use crate::disk::state::P2pool;
use crate::disk::state::P2poolChain;
use crate::disk::state::StartOptionsMode;
use crate::helper::ProcessName;
use crate::helper::ProcessSignal;
use crate::helper::ProcessState;
use crate::helper::check_died;
use crate::helper::check_user_input;
use crate::helper::crawler::Crawler;
use crate::helper::signal_end;
use crate::helper::sleep_end_loop;
use crate::regex::P2POOL_REGEX;
use crate::regex::contains_end_status;
use crate::regex::contains_statuscommand;
use crate::regex::contains_yourhashrate;
use crate::regex::contains_yourshare;
use crate::regex::contains_zmq_failure;
use crate::regex::estimated_hr;
use crate::regex::nb_current_shares;
use crate::utils::regex::contains_node;
use crate::utils::regex::contains_window_nb_blocks;
use crate::utils::regex::p2pool_monero_node;
use crate::utils::regex::pplns_window_nb_blocks;
use crate::{
    constants::*, disk::gupax_p2pool_api::GupaxP2poolApi, helper::MONERO_BLOCK_TIME_IN_SECONDS,
    human::*, macros::*, xmr::*,
};
use enclose::enc;
use log::*;
use serde::{Deserialize, Serialize};
use std::mem;
use std::path::Path;
use std::{
    fmt::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::*,
};
use tokio::time::sleep;
impl Helper {
    #[cold]
    #[inline(never)]
    fn read_pty_p2pool(
        output_parse: Arc<Mutex<String>>,
        output_pub: Arc<Mutex<String>>,
        reader: Box<dyn std::io::Read + Send>,
        gupax_p2pool_api: Arc<Mutex<GupaxP2poolApi>>,
        gui_api: Arc<Mutex<PubP2poolApi>>,
    ) {
        use std::io::BufRead;
        let mut stdout = std::io::BufReader::new(reader).lines();

        // Run a ANSI escape sequence filter for the first few lines.
        let mut i = 0;
        let mut status_output = false;
        while let Some(Ok(line)) = stdout.next() {
            let line = strip_ansi_escapes::strip_str(line);

            // status could be present before 20 lines with a low verbosity value
            if contains_statuscommand(&line) {
                status_output = true;
                continue;
            }
            if status_output {
                if contains_end_status(&line) {
                    // end of status
                    status_output = false;
                    continue;
                }
            } else {
                if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                    error!("P2Pool PTY Parse | Output error: {e}");
                }
                if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                    error!("P2Pool PTY Pub | Output error: {e}");
                }
                if i > 20 {
                    break;
                } else {
                    i += 1;
                }
            }
        }
        while let Some(Ok(line)) = stdout.next() {
            if contains_node(&line) {
                if let Some(node) = p2pool_monero_node(&line) {
                    if gui_api.lock().unwrap().current_node.as_ref() != Some(&node) {
                        gui_api.lock().unwrap().current_node = Some(node);
                    }
                } else {
                    error!(
                        "P2pool | PTY Getting data from status: Lines contains a Monero node but no value found: {line}"
                    );
                }
            }
            // if command status is sent by gupax process and not the user, forward it only to update_from_status method.
            // 25 lines after the command are the result of status, with last line finishing by update.
            if contains_statuscommand(&line) {
                status_output = true;
                continue;
            }
            if status_output {
                if contains_yourhashrate(&line) {
                    if let Some(ehr) = estimated_hr(&line) {
                        debug!(
                            "P2pool | PTY getting current estimated HR data from status: {ehr} KH/s"
                        );
                        // multiply by a thousand because value is given as kH/s instead H/s
                        gui_api.lock().unwrap().sidechain_ehr = ehr;
                        debug!(
                            "P2pool | PTY getting current estimated HR data from status: {} H/s",
                            gui_api.lock().unwrap().sidechain_ehr
                        );
                    } else {
                        error!(
                            "P2pool | PTY Getting data from status: Lines contains Your shares but no value found: {line}"
                        );
                    }
                }
                if contains_yourshare(&line) {
                    // update sidechain shares
                    if let Some(shares) = nb_current_shares(&line) {
                        debug!(
                            "P2pool | PTY getting current shares data from status: {shares} share"
                        );
                        gui_api.lock().unwrap().sidechain_shares = shares;
                    } else {
                        error!(
                            "P2pool | PTY Getting data from status: Lines contains Your shares but no value found: {line}"
                        );
                    }
                }
                if contains_window_nb_blocks(&line)
                    && let Some(nb_blocks) = pplns_window_nb_blocks(&line)
                {
                    gui_api.lock().unwrap().window_length_blocks = Some(nb_blocks);
                }
                if contains_end_status(&line) {
                    // end of status
                    status_output = false;
                }
                continue;
            }
            //			println!("{}", line); // For debugging.
            if P2POOL_REGEX.payout.is_match(&line) {
                debug!("P2Pool PTY | Found payout, attempting write: {line}");
                let (date, atomic_unit, block) = PayoutOrd::parse_raw_payout_line(&line);
                let formatted_log_line = GupaxP2poolApi::format_payout(&date, &atomic_unit, &block);
                GupaxP2poolApi::add_payout(
                    &mut gupax_p2pool_api.lock().unwrap(),
                    &formatted_log_line,
                    date,
                    atomic_unit,
                    block,
                );
                if let Err(e) = GupaxP2poolApi::write_to_all_files(
                    &gupax_p2pool_api.lock().unwrap(),
                    &formatted_log_line,
                ) {
                    error!("P2Pool PTY GupaxP2poolApi | Write error: {e}");
                }
            }
            if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                error!("P2Pool PTY Parse | Output error: {e}");
            }
            if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                error!("P2Pool PTY Pub | Output error: {e}");
            }
        }
    }
    //---------------------------------------------------------------------------------------------------- P2Pool specific
    #[cold]
    #[inline(never)]
    // Just sets some signals for the watchdog thread to pick up on.
    pub fn stop_p2pool(helper: &Arc<Mutex<Self>>) {
        info!("P2Pool | Attempting to stop...");
        helper.lock().unwrap().p2pool.lock().unwrap().signal = ProcessSignal::Stop;
        helper.lock().unwrap().p2pool.lock().unwrap().state = ProcessState::Middle;
    }

    #[cold]
    #[inline(never)]
    // The "restart frontend" to a "frontend" function.
    // Basically calls to kill the current p2pool, waits a little, then starts the below function in a a new thread, then exit.
    pub fn restart_p2pool(
        helper: &Arc<Mutex<Self>>,
        state: &P2pool,
        state_node: &Node,
        path: &Path,
        backup_hosts: BackupNodes,
        override_to_local_node: bool,
        crawler: &Arc<Mutex<Crawler>>,
    ) {
        info!("P2Pool | Attempting to restart...");
        helper.lock().unwrap().p2pool.lock().unwrap().signal = ProcessSignal::Restart;
        helper.lock().unwrap().p2pool.lock().unwrap().state = ProcessState::Middle;

        let helper = Arc::clone(helper);
        let state = state.clone();
        let state_node = state_node.clone();
        let path = path.to_path_buf();
        let crawler = crawler.clone();
        // This thread lives to wait, start p2pool then die.
        thread::spawn(move || {
            while helper.lock().unwrap().p2pool.lock().unwrap().state != ProcessState::Waiting {
                warn!("P2Pool | Want to restart but process is still alive, waiting...");
                sleep!(1000);
            }
            // Ok, process is not alive, start the new one!
            info!("P2Pool | Old process seems dead, starting new one!");
            Self::start_p2pool(
                &helper,
                &state,
                &state_node,
                &path,
                &backup_hosts,
                override_to_local_node,
                &crawler,
            );
        });
        info!("P2Pool | Restart ... OK");
    }

    #[cold]
    #[inline(never)]
    // The "frontend" function that parses the arguments, and spawns either the [Simple] or [Advanced] P2Pool watchdog thread.
    pub fn start_p2pool(
        helper: &Arc<Mutex<Self>>,
        state: &P2pool,
        state_node: &Node,
        path: &Path,
        backup_hosts: &BackupNodes,
        override_to_local_node: bool,
        crawler: &Arc<Mutex<Crawler>>,
    ) {
        let path = path.to_path_buf();
        // start a spawn here directly
        thread::spawn(
            enc!((helper,  state, state_node, path, backup_hosts, crawler)  move || {
                Self::prestart_p2pool(
                    &helper,
                    &state,
                    &state_node,
                    &path,
                    &backup_hosts,
                    override_to_local_node,
                    &crawler,
                )
            }),
        );
    }
    #[tokio::main]
    pub async fn prestart_p2pool(
        helper: &Arc<Mutex<Self>>,
        state: &P2pool,
        state_node: &Node,
        path: &Path,
        backup_hosts: &BackupNodes,
        override_to_local_node: bool,
        crawler: &Arc<Mutex<Crawler>>,
    ) {
        helper.lock().unwrap().p2pool.lock().unwrap().state = ProcessState::Middle;

        let simple = state.submenu != SubmenuP2pool::Advanced;
        let mode = if simple {
            StartOptionsMode::Simple
        } else if !state.arguments.is_empty() {
            StartOptionsMode::Custom
        } else {
            StartOptionsMode::Advanced
        };
        // get the rpc and zmq port used when starting the node if it is alive, else use current settings of the Node.
        // If the Node is started with different ports that the one used in settings when P2Pool was started,
        // the user will need to restart p2pool
        let node_process = Arc::clone(&helper.lock().unwrap().node);
        let img_node = Arc::clone(&helper.lock().unwrap().img_node);
        // img_node must have been updated when the node process started.
        // If the node was started from gupax, it was from the state values.
        // If the node process detected a local node started without gupax, it will be from the detected nodes.
        let (local_node_rpc, local_node_zmq) = state_node.current_ports(
            node_process.lock().unwrap().is_alive(),
            &img_node.lock().unwrap(),
        );
        if state.backup_host {
            // we want to add backup host but the crawler is still running and did not add at least the minimum of number of fast node (including medium nodes);
            // So we wait for the crawling to either add a minimum amount of backup host or to finish.
            while crawler.lock().unwrap().crawling
                && backup_hosts.lock().unwrap().len() < state.crawl_settings.nb_nodes_fast.into()
            {
                // sleep 100ms
                sleep!(100);
            }
        }
        let mut backup_nodes = vec![];
        if state.backup_host && backup_hosts.lock().unwrap().len() > 1 {
            backup_nodes = backup_hosts.lock().unwrap().clone();
        }

        // Once the crawling is completed or at least a minimum remote nodes are added, we use them.
        let args = Self::build_p2pool_args(
            state,
            path,
            &backup_nodes,
            override_to_local_node,
            local_node_zmq,
            local_node_rpc,
            mode,
        );
        let (api_path_local, api_path_network, api_path_pool, api_path_p2p) =
            Self::mutate_img_p2pool(state, helper, path);

        // Print arguments & user settings to console
        crate::disk::print_dash(&format!(
            "P2Pool | Launch arguments: {args:#?} | Local API Path: {api_path_local:#?} | Network API Path: {api_path_network:#?} | Pool API Path: {api_path_pool:#?} | P2P API Path {api_path_p2p:#?}"
        ));

        // Spawn watchdog thread
        let process = Arc::clone(&helper.lock().unwrap().p2pool);
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_p2pool);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_p2pool);
        let gupax_p2pool_api = Arc::clone(&helper.lock().unwrap().gupax_p2pool_api);
        let path = path.to_path_buf();
        let node_to_start_with = state
            .selected_remote_node
            .as_ref()
            .expect("P2Pool should always be started with a node set")
            .clone();
        // thread to check if the button for switching to local node if it is synced to restart p2pool.
        // starting the thread even if the option is disabled allows to apply the change immediately in case it is enabled again without asking the user to restart p2pool.
        // Start this thread only if we don't already override to local node
        if !override_to_local_node {
            thread::spawn(
                enc!((helper, state, state_node, path, backup_hosts, crawler) move || {
                    Self::watch_switch_p2pool_to_local_node(
                        &helper,
                        &state,
                        &state_node,
                        &path,
                        backup_hosts,
                        &crawler
                    );
                }),
            );
        }

        thread::spawn(move || {
            Self::spawn_p2pool_watchdog(
                process,
                gui_api,
                pub_api,
                args,
                path,
                api_path_local,
                api_path_network,
                api_path_pool,
                api_path_p2p,
                gupax_p2pool_api,
                node_to_start_with,
            );
        });
    }
    // Takes in a 95-char Monero address, returns the first and last
    // 8 characters separated with dots like so: [4abcdefg...abcdefgh]
    pub fn head_tail_of_monero_address(address: &str) -> String {
        if address.len() < 95 {
            return "???".to_string();
        }
        let head = &address[0..8];
        let tail = &address[87..95];
        head.to_owned() + "..." + tail
    }
    pub fn mutate_img_p2pool(
        state: &P2pool,
        helper: &Arc<Mutex<Self>>,
        path: &Path,
    ) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let path = path.to_path_buf();
        let mut api_path = path;
        api_path.pop();
        let simple = state.submenu != SubmenuP2pool::Advanced;
        if simple {
            *helper.lock().unwrap().img_p2pool.lock().unwrap() = ImgP2pool {
                chain: P2poolChain::Nano.to_string(),
                address: Self::head_tail_of_monero_address(&state.address),
                out_peers: "10".to_string(),
                in_peers: "10".to_string(),
                stratum_port: P2POOL_PORT_DEFAULT,
            };
        } else if !state.arguments.is_empty() {
            // This parses the input and attempts to fill out
            // the [ImgP2pool]... This is pretty bad code...
            let mut last = "";
            let lock = helper.lock().unwrap();
            let mut p2pool_image = lock.img_p2pool.lock().unwrap();
            let mut chain = P2poolChain::Main;
            for arg in state.arguments.split_whitespace() {
                match last {
                    "--mini" => {
                        chain = P2poolChain::Mini;
                        p2pool_image.chain = chain.to_string();
                    }
                    "--nano" => {
                        chain = P2poolChain::Nano;
                        p2pool_image.chain = chain.to_string();
                    }
                    // used for nano chain, Gupax will not recognize another custom chain
                    "--sidechain-config" => {
                        chain = P2poolChain::Nano;
                        p2pool_image.chain = chain.to_string();
                    }
                    "--wallet" => p2pool_image.address = Self::head_tail_of_monero_address(arg),
                    "--out-peers" => p2pool_image.out_peers = arg.to_string(),
                    "--in-peers" => p2pool_image.in_peers = arg.to_string(),
                    "--data-api" => api_path = PathBuf::from(arg),
                    "--stratum" => {
                        p2pool_image.stratum_port = last
                            .split(":")
                            .last()
                            .unwrap_or(&P2POOL_PORT_DEFAULT.to_string())
                            .parse()
                            .unwrap_or(P2POOL_PORT_DEFAULT)
                    }
                    _ => (),
                }
                p2pool_image.chain = chain.to_string();
                let arg = if arg == "localhost" { "127.0.0.1" } else { arg };
                last = arg;
            }
        } else {
            *helper.lock().unwrap().img_p2pool.lock().unwrap() = ImgP2pool {
                chain: state.chain.to_string(),
                address: Self::head_tail_of_monero_address(&state.address),
                stratum_port: state.stratum_port,
                out_peers: state.out_peers.to_string(),
                in_peers: state.in_peers.to_string(),
            };
        }
        let mut api_path_local = api_path.clone();
        let mut api_path_network = api_path.clone();
        let mut api_path_pool = api_path.clone();
        let mut api_path_p2p = api_path.clone();
        api_path_local.push(P2POOL_API_PATH_LOCAL);
        api_path_network.push(P2POOL_API_PATH_NETWORK);
        api_path_pool.push(P2POOL_API_PATH_POOL);
        api_path_p2p.push(P2POOL_API_PATH_P2P);
        (
            api_path_local,
            api_path_network,
            api_path_pool,
            api_path_p2p,
        )
    }
    #[cold]
    #[inline(never)]
    // Takes in some [State/P2pool] and parses it to build the actual command arguments.
    // Returns the [Vec] of actual arguments, and mutates the [ImgP2pool] for the main GUI thread
    // It returns a value... and mutates a deeply nested passed argument... this is some pretty bad code...
    pub fn build_p2pool_args(
        state: &P2pool,
        path: &Path,
        backup_hosts: &[PoolNode],
        override_to_local_node: bool,
        local_node_zmq_port: u16,
        local_node_rpc_port: u16,
        // Allows to provide a different mode without mutating the state
        mode: StartOptionsMode,
    ) -> Vec<String> {
        let mut args = Vec::with_capacity(500);
        let path = path.to_path_buf();
        let mut api_path = path;
        api_path.pop();

        // common Simple and Advanced args
        match mode {
            StartOptionsMode::Simple | StartOptionsMode::Advanced => {
                args.push("--wallet".to_string());
                args.push(state.address.clone()); // Wallet address
                args.push("--data-api".to_string());
                args.push(api_path.display().to_string()); // API Path
                args.push("--local-api".to_string()); // Enable API
                args.push("--no-color".to_string()); // Remove color escape sequences
                args.push("--light-mode".to_string()); // Assume user is not using P2Pool to mine.
            }
            StartOptionsMode::Custom => {}
        }
        // Specific args
        match mode {
            StartOptionsMode::Simple => {
                args.push("--nano".to_string());
                if state.local_node || override_to_local_node {
                    // use the local node
                    // Build the p2pool argument
                    args.push("--host".to_string());
                    args.push("127.0.0.1".to_string());
                    args.push("--rpc-port".to_string());
                    args.push(local_node_rpc_port.to_string());
                    args.push("--zmq-port".to_string());
                    args.push(local_node_zmq_port.to_string());
                } else if let Some(remote_node) = &state.selected_remote_node {
                    // Do we want to show the args if there's no selected remote ?
                    args.push("--host".to_string());
                    args.push(remote_node.ip.to_string());
                    args.push("--rpc-port".to_string());
                    args.push(remote_node.rpc.to_string());
                    args.push("--zmq-port".to_string());
                    args.push(remote_node.zmq.to_string());
                }

                if state.backup_host {
                    for node in backup_hosts.iter() {
                        // Add the backup node only if it's not the selected remote node, because it would had already been added
                        if state.selected_remote_node.is_none()
                            || state
                                .selected_remote_node
                                .as_ref()
                                .is_some_and(|s| s != node)
                        {
                            let ip = if node.ip() == "localhost" {
                                "127.0.0.1"
                            } else {
                                node.ip()
                            };
                            args.push("--host".to_string());
                            args.push(ip.to_string());
                            args.push("--rpc-port".to_string());
                            args.push(node.port().to_string());
                            args.push("--zmq-port".to_string());
                            args.push(node.custom().to_string()); // ZMQ PORT
                        }
                    }
                }
            }
            StartOptionsMode::Advanced => {
                match state.chain {
                    P2poolChain::Main => {}
                    P2poolChain::Mini => args.push("--mini".to_string()),
                    P2poolChain::Nano => args.push("--nano".to_string()),
                }
                // build the argument
                let ip = if state.ip == "localhost" {
                    "127.0.0.1"
                } else {
                    &state.ip
                };
                args.push("--loglevel".to_string());
                args.push(state.log_level.to_string()); // Log Level
                args.push("--out-peers".to_string());
                args.push(state.out_peers.to_string()); // Out Peers
                args.push("--in-peers".to_string());
                args.push(state.in_peers.to_string()); // In Peers            }
                args.push("--host".to_string());
                args.push(ip.to_string()); // IP
                args.push("--rpc-port".to_string());
                args.push(state.rpc.to_string()); // RPC
                args.push("--zmq-port".to_string());
                args.push(state.zmq.to_string()); // ZMQ

                // Add backup hosts
                if state.backup_host {
                    for node in backup_hosts.iter() {
                        let host = (node.ip(), node.port(), node.custom());
                        // Add the backup node only if it's not saved in state because it would had already been added
                        if host != (&state.ip, &state.rpc, &state.zmq) {
                            let ip = if node.ip() == "localhost" {
                                "127.0.0.1"
                            } else {
                                node.ip()
                            };
                            args.push("--host".to_string());
                            args.push(ip.to_string());
                            args.push("--rpc-port".to_string());
                            args.push(node.port().to_string());
                            args.push("--zmq-port".to_string());
                            args.push(node.custom().to_string());
                        }
                    }
                }
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

    #[cold]
    #[inline(never)]
    // The P2Pool watchdog. Spawns 1 OS thread for reading a PTY (STDOUT+STDERR), and combines the [Child] with a PTY so STDIN actually works.
    // or if P2Pool simple is false and extern is true, only prints data from stratum api.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::await_holding_lock)]
    #[tokio::main]
    async fn spawn_p2pool_watchdog(
        process: Arc<Mutex<Process>>,
        gui_api: Arc<Mutex<PubP2poolApi>>,
        pub_api: Arc<Mutex<PubP2poolApi>>,
        args: Vec<String>,
        path: std::path::PathBuf,
        api_path_local: std::path::PathBuf,
        api_path_network: std::path::PathBuf,
        api_path_pool: std::path::PathBuf,
        api_path_p2p: std::path::PathBuf,
        gupax_p2pool_api: Arc<Mutex<GupaxP2poolApi>>,
        node: RemoteNode,
    ) {
        // 1a. Create PTY
        debug!("P2Pool | Creating PTY...");
        let pty = portable_pty::native_pty_system();
        let pair = pty
            .openpty(portable_pty::PtySize {
                rows: 100,
                cols: 1000,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        // 1b. Create command
        debug!("P2Pool | Creating command...");
        let mut cmd = portable_pty::CommandBuilder::new(path.as_path());
        cmd.args(args);
        cmd.env("NO_COLOR", "true");
        cmd.cwd(path.as_path().parent().unwrap());
        // 1c. Create child
        debug!("P2Pool | Creating child...");
        let child_pty = Arc::new(Mutex::new(pair.slave.spawn_command(cmd).unwrap()));
        drop(pair.slave);

        // 2. Set process state
        debug!("P2Pool | Setting process state...");
        let mut lock = process.lock().unwrap();
        lock.state = ProcessState::Syncing;
        lock.signal = ProcessSignal::None;
        lock.start = Instant::now();
        let reader = pair.master.try_clone_reader().unwrap(); // Get STDOUT/STDERR before moving the PTY
        let mut stdin = pair.master.take_writer().unwrap();
        drop(lock);

        // 3. Spawn PTY read thread
        debug!("P2Pool | Spawning PTY read thread...");
        let output_parse = Arc::clone(&process.lock().unwrap().output_parse);
        let output_pub = Arc::clone(&process.lock().unwrap().output_pub);
        let gupax_p2pool_api = Arc::clone(&gupax_p2pool_api);
        let p2pool_api_c = Arc::clone(&gui_api);
        tokio::spawn(async move {
            Self::read_pty_p2pool(
                output_parse,
                output_pub,
                reader,
                gupax_p2pool_api,
                p2pool_api_c,
            );
        });
        let output_parse = Arc::clone(&process.lock().unwrap().output_parse);
        let output_pub = Arc::clone(&process.lock().unwrap().output_pub);

        debug!("P2Pool | Cleaning old [local] API files...");
        // Attempt to remove stale API file
        match std::fs::remove_file(&api_path_local) {
            Ok(_) => info!("P2Pool | Attempting to remove stale API file ... OK"),
            Err(e) => warn!("P2Pool | Attempting to remove stale API file ... FAIL ... {e}"),
        }
        // Attempt to create a default empty one.
        use std::io::Write;
        if std::fs::File::create(&api_path_local).is_ok() {
            let text = r#"{"hashrate_15m":0,"hashrate_1h":0,"hashrate_24h":0,"shares_found":0,"average_effort":0.0,"current_effort":0.0,"connections":0}"#;
            match std::fs::write(&api_path_local, text) {
                Ok(_) => info!("P2Pool | Creating default empty API file ... OK"),
                Err(e) => warn!("P2Pool | Creating default empty API file ... FAIL ... {e}"),
            }
        }
        debug!("P2Pool | Cleaning old [p2p] API files...");
        // Attempt to remove stale API file
        match std::fs::remove_file(&api_path_p2p) {
            Ok(_) => info!("P2Pool | Attempting to remove stale API file ... OK"),
            Err(e) => warn!("P2Pool | Attempting to remove stale API file ... FAIL ... {e}"),
        }
        // Attempt to create a default empty one.
        if std::fs::File::create(&api_path_p2p).is_ok() {
            let text = r#"{"connections":0,"incoming_connections":0,"peer_list_size":0,"peers":[],"uptime":0}"#;
            match std::fs::write(&api_path_p2p, text) {
                Ok(_) => info!("P2Pool | Creating default empty API file ... OK"),
                Err(e) => warn!("P2Pool | Creating default empty API file ... FAIL ... {e}"),
            }
        }
        let start = process.lock().unwrap().start;

        // Reset stats before loop, except action parameters without a need for saving to state.
        reset_data_p2pool(&pub_api, &gui_api);

        // Set the node used so that Stats can fetch it.
        // It will be updated with the output console of P2Pool while the process is still running
        gui_api.lock().unwrap().current_node = Some(NodeString {
            ip: node.ip.to_string(),
            rpc: node.rpc.to_string(),
            zmq: node.zmq.to_string(),
        });

        // 4. Loop as watchdog
        let mut first_loop = true;
        let mut last_p2pool_request = tokio::time::Instant::now();
        let mut last_status_request = tokio::time::Instant::now();

        info!("P2Pool | Entering watchdog mode... woof!");
        loop {
            // Set timer
            let now = Instant::now();
            debug!("P2Pool Watchdog | ----------- Start of loop -----------");
            {
                gui_api.lock().unwrap().tick = (last_p2pool_request.elapsed().as_secs() % 60) as u8;
                // Check if the process is secretly died without us knowing :)
                if check_died(
                    &child_pty,
                    &mut process.lock().unwrap(),
                    &start,
                    &mut gui_api.lock().unwrap().output,
                ) {
                    break;
                }

                // Check SIGNAL
                if signal_end(
                    &mut process.lock().unwrap(),
                    Some(&child_pty),
                    &start,
                    &mut gui_api.lock().unwrap().output,
                ) {
                    break;
                }
                // check that if prefer local node is true and local node is alived and p2pool was not started with local node

                // Check vector of user input
                check_user_input(&process, &mut stdin);
                // Check if logs need resetting
                debug!("P2Pool Watchdog | Attempting GUI log reset check");
                let mut lock = gui_api.lock().unwrap();
                Self::check_reset_gui_output(&mut lock.output, ProcessName::P2pool);
                drop(lock);

                // Always update from output
                debug!("P2Pool Watchdog | Starting [update_from_output()]");
                let mut process_lock = process.lock().unwrap();
                let mut pub_api_lock = pub_api.lock().unwrap();

                // if zmq fails were detected, we should increment the timer
                if let Some(timer) = &mut pub_api_lock.fails_zmq_since {
                    *timer += 1;
                }
                // after 5 seconds without being reset to 0, set to none.
                if pub_api_lock.fails_zmq_since.is_some_and(|t| t == 5) {
                    info!("P2Pool Watchdog | 5 seconds since a ZMQ failure was seen");
                    pub_api_lock.fails_zmq_since = None;
                }
                PubP2poolApi::update_from_output(
                    &mut pub_api_lock,
                    &output_parse,
                    &output_pub,
                    start.elapsed(),
                );

                // Read [local] API
                debug!("P2Pool Watchdog | Attempting [local] API file read");
                if let Ok(string) = Self::path_to_string(&api_path_local, ProcessName::P2pool) {
                    // Deserialize
                    if let Ok(local_api) = PrivP2poolLocalApi::from_str(&string) {
                        // Update the structs.
                        PubP2poolApi::update_from_local(&mut pub_api_lock, local_api);
                    }
                }
                // Read [p2p] API
                // allows to know if p2p is synced and connected to a Node.
                debug!("P2Pool Watchdog | Attempting [p2p] API file read");
                if let Ok(string) = Self::path_to_string(&api_path_p2p, ProcessName::P2pool) {
                    // Deserialize
                    if let Ok(p2p_api) = PrivP2PoolP2PApi::from_str(&string) {
                        // Update the structs.
                        PubP2poolApi::update_from_p2p(&mut pub_api_lock, p2p_api);
                    }
                }

                // check if state must be changed based on local and p2p API
                pub_api_lock.update_state(&mut process_lock);

                debug!("P2Pool Watchdog | Attempting [network] & [pool] API file read");
                if let (Ok(network_api), Ok(pool_api)) = (
                    Self::path_to_string(&api_path_network, ProcessName::P2pool),
                    Self::path_to_string(&api_path_pool, ProcessName::P2pool),
                ) && let (Ok(network_api), Ok(pool_api)) = (
                    PrivP2poolNetworkApi::from_str(&network_api),
                    PrivP2poolPoolApi::from_str(&pool_api),
                ) {
                    PubP2poolApi::update_from_network_pool(
                        &mut pub_api_lock,
                        network_api,
                        pool_api,
                    );
                    last_p2pool_request = tokio::time::Instant::now();
                }

                let last_status_request_expired =
                    last_status_request.elapsed() >= Duration::from_secs(60);
                if (last_status_request_expired || first_loop)
                    && process_lock.state == ProcessState::Alive
                {
                    debug!("P2Pool Watchdog | Reading status output of p2pool node");
                    #[cfg(target_os = "windows")]
                    if let Err(e) = write!(stdin, "statusfromgupax\r\n") {
                        error!("P2Pool Watchdog | STDIN error: {e}");
                    }
                    #[cfg(target_family = "unix")]
                    if let Err(e) = writeln!(stdin, "statusfromgupax") {
                        error!("P2Pool Watchdog | STDIN error: {e}");
                    }
                    // Flush.
                    if let Err(e) = stdin.flush() {
                        error!("P2Pool Watchdog | STDIN flush error: {e}");
                    }
                    last_status_request = tokio::time::Instant::now();
                }

                // Sleep (only if 900ms hasn't passed)
                if first_loop {
                    first_loop = false;
                }
            } // end of scope to drop lock
            sleep_end_loop(now, ProcessName::P2pool).await;
        }

        // 5. If loop broke, we must be done here.
        info!("P2Pool Watchdog | Watchdog thread exiting... Goodbye!");
    }
    #[tokio::main]
    #[allow(clippy::await_holding_lock)]
    async fn watch_switch_p2pool_to_local_node(
        helper: &Arc<Mutex<Helper>>,
        state: &P2pool,
        state_node: &Node,
        path_p2pool: &Path,
        backup_hosts: BackupNodes,
        crawler: &Arc<Mutex<Crawler>>,
    ) {
        // do not try to restart immediately after a first start, or else the two start will be in conflict.
        sleep(Duration::from_secs(10)).await;

        // check every seconds
        loop {
            let helper_lock = helper.lock().unwrap();
            let node_process = helper_lock.node.lock().unwrap();
            let process = helper_lock.p2pool.lock().unwrap();
            let gui_api = helper_lock.gui_api_p2pool.lock().unwrap();
            if gui_api.prefer_local_node
                && state.submenu != SubmenuP2pool::Advanced
                && !state.local_node
                && node_process.state == ProcessState::Alive
                && process.is_alive()
            {
                drop(gui_api);
                drop(process);
                drop(node_process);
                drop(helper_lock);
                Helper::restart_p2pool(
                    helper,
                    state,
                    state_node,
                    path_p2pool,
                    backup_hosts,
                    true,
                    crawler,
                );
                break;
            }
            drop(gui_api);
            drop(process);
            drop(node_process);
            drop(helper_lock);
            sleep(Duration::from_secs(1)).await;
        }
    }
}
//---------------------------------------------------------------------------------------------------- [ImgP2pool]
// A static "image" of data that P2Pool started with.
// This is just a snapshot of the user data when they initially started P2Pool.
// Created by [start_p2pool()] and return to the main GUI thread where it will store it.
// No need for an [Arc<Mutex>] since the Helper thread doesn't need this information.
#[derive(Debug, Clone)]
pub struct ImgP2pool {
    pub chain: String,     // Did the user start on the mini-chain?
    pub address: String, // What address is the current p2pool paying out to? (This gets shortened to [4xxxxx...xxxxxx])
    pub out_peers: String, // How many out-peers?
    pub in_peers: String, // How many in-peers?
    pub stratum_port: u16, // on which port p2pool is listening for stratum connections
}

impl Default for ImgP2pool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImgP2pool {
    pub fn new() -> Self {
        Self {
            chain: String::from("???"),
            address: String::from("???"),
            out_peers: String::from("???"),
            in_peers: String::from("???"),
            stratum_port: P2POOL_PORT_DEFAULT,
        }
    }
}

//---------------------------------------------------------------------------------------------------- Public P2Pool API
// Helper/GUI threads both have a copy of this, Helper updates
// the GUI's version on a 1-second interval from the private data.
#[derive(Debug, Clone, PartialEq)]
pub struct PubP2poolApi {
    // Output
    pub output: String,
    // Uptime
    pub uptime: HumanTime,
    // These are manually parsed from the STDOUT.
    pub payouts: u128,
    pub payouts_hour: f64,
    pub payouts_day: f64,
    pub payouts_month: f64,
    pub xmr: f64,
    pub xmr_hour: f64,
    pub xmr_day: f64,
    pub xmr_month: f64,
    // Local API
    pub hashrate: String,
    pub hashrate_15m: u64,
    pub hashrate_1h: u64,
    pub hashrate_24h: u64,
    pub shares_found: Option<u64>,
    pub average_effort: HumanNumber,
    pub current_effort: HumanNumber,
    pub connections: HumanNumber,
    // The API needs a raw ints to go off of and
    // there's not a good way to access it without doing weird
    // [Arc<Mutex>] shenanigans, so some raw ints are stored here.
    pub user_p2pool_hashrate_u64: u64,
    pub p2pool_difficulty_u64: u64,
    pub monero_difficulty_u64: u64,
    pub p2pool_hashrate_u64: u64,
    pub monero_hashrate_u64: u64,
    // Tick. Every loop this gets incremented.
    // At 60, it indicated we should read the below API files.
    pub tick: u8,
    // Network API
    pub monero_difficulty: HumanNumber, // e.g: [15,000,000]
    pub monero_hashrate: HumanNumber,   // e.g: [1.000 GH/s]
    pub hash: String,                   // Current block hash
    pub height: u32,
    pub reward: AtomicUnit,
    // Pool API
    pub p2pool_difficulty: HumanNumber,
    pub p2pool_hashrate: HumanNumber,
    pub miners: HumanNumber, // Current amount of miners on P2Pool sidechain
    // Mean (calculated in functions, not serialized)
    pub solo_block_mean: HumanTime, // Time it would take the user to find a solo block
    pub p2pool_block_mean: HumanTime, // Time it takes the P2Pool sidechain to find a block
    pub p2pool_share_mean: HumanTime, // Time it would take the user to find a P2Pool share
    // Percent
    pub p2pool_percent: HumanNumber, // Percentage of P2Pool hashrate capture of overall Monero hashrate.
    pub user_p2pool_percent: HumanNumber, // How much percent the user's hashrate accounts for in P2Pool.
    pub user_monero_percent: HumanNumber, // How much percent the user's hashrate accounts for in all of Monero hashrate.
    // from status
    pub sidechain_shares: u32,
    pub sidechain_ehr: f32,
    pub sidechain_height: u32,
    pub fails_zmq_since: Option<u32>,
    // from local/p2p
    pub p2p_connected: u32,
    pub node_connected: bool,
    pub prefer_local_node: bool,
    pub current_node: Option<NodeString>,
    pub window_length_blocks: Option<u64>,
}

impl Default for PubP2poolApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PubP2poolApi {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            uptime: HumanTime::new(),
            payouts: 0,
            payouts_hour: 0.0,
            payouts_day: 0.0,
            payouts_month: 0.0,
            xmr: 0.0,
            xmr_hour: 0.0,
            xmr_day: 0.0,
            xmr_month: 0.0,
            hashrate: HumanNumber::from_hashrate(&[None, None, None]).to_string(),
            hashrate_15m: 0,
            hashrate_1h: 0,
            hashrate_24h: 0,
            shares_found: None,
            average_effort: HumanNumber::unknown(),
            current_effort: HumanNumber::unknown(),
            connections: HumanNumber::unknown(),
            tick: 0,
            user_p2pool_hashrate_u64: 0,
            p2pool_difficulty_u64: 0,
            monero_difficulty_u64: 0,
            p2pool_hashrate_u64: 0,
            monero_hashrate_u64: 0,
            monero_difficulty: HumanNumber::unknown(),
            monero_hashrate: HumanNumber::unknown(),
            hash: String::from("???"),
            height: 0,
            reward: AtomicUnit::new(),
            p2pool_difficulty: HumanNumber::unknown(),
            p2pool_hashrate: HumanNumber::unknown(),
            miners: HumanNumber::unknown(),
            solo_block_mean: HumanTime::new(),
            p2pool_block_mean: HumanTime::new(),
            p2pool_share_mean: HumanTime::new(),
            p2pool_percent: HumanNumber::unknown(),
            user_p2pool_percent: HumanNumber::unknown(),
            user_monero_percent: HumanNumber::unknown(),
            sidechain_shares: 0,
            sidechain_ehr: 0.0,
            sidechain_height: 0,
            p2p_connected: 0,
            node_connected: false,
            prefer_local_node: true,
            fails_zmq_since: None,
            current_node: None,
            window_length_blocks: None,
        }
    }

    #[inline]
    // The issue with just doing [gui_api = pub_api] is that values get overwritten.
    // This doesn't matter for any of the values EXCEPT for the output, so we must
    // manually append it instead of overwriting.
    // This is used in the "helper" thread.
    pub(super) fn combine_gui_pub_api(gui_api: &mut Self, pub_api: &mut Self) {
        let mut output = std::mem::take(&mut gui_api.output);
        let buf = std::mem::take(&mut pub_api.output);
        if !buf.is_empty() {
            output.push_str(&buf);
        }
        *gui_api = Self {
            output,
            tick: std::mem::take(&mut gui_api.tick),
            sidechain_shares: std::mem::take(&mut gui_api.sidechain_shares),
            sidechain_ehr: std::mem::take(&mut gui_api.sidechain_ehr),
            prefer_local_node: std::mem::take(&mut gui_api.prefer_local_node),
            current_node: std::mem::take(&mut gui_api.current_node),
            window_length_blocks: std::mem::take(&mut gui_api.window_length_blocks),
            ..pub_api.clone()
        };
    }

    #[inline]
    // Essentially greps the output for [x.xxxxxxxxxxxx XMR] where x = a number.
    // It sums each match and counts along the way, handling an error by not adding and printing to console.
    fn calc_payouts_and_xmr(output: &str) -> (u128 /* payout count */, f64 /* total xmr */) {
        let iter = P2POOL_REGEX.payout.find_iter(output);
        let mut sum: f64 = 0.0;
        let mut count: u128 = 0;
        for i in iter {
            if let Some(word) = P2POOL_REGEX.payout_float.find(i.as_str()) {
                match word.as_str().parse::<f64>() {
                    Ok(num) => {
                        sum += num;
                        count += 1;
                    }
                    Err(e) => error!("P2Pool | Total XMR sum calculation error: [{e}]"),
                }
            }
        }
        (count, sum)
    }

    // Mutate "watchdog"'s [PubP2poolApi] with data the process output.
    pub(super) fn update_from_output(
        public: &mut Self,
        output_parse: &Arc<Mutex<String>>,
        output_pub: &Arc<Mutex<String>>,
        elapsed: std::time::Duration,
    ) {
        // 1. Take the process's current output buffer and combine it with Pub (if not empty)
        let mut output_pub = output_pub.lock().unwrap();
        if !output_pub.is_empty() {
            public.output.push_str(&std::mem::take(&mut *output_pub));
        }

        drop(output_pub);
        // 2. Parse the full STDOUT
        let mut output_parse = output_parse.lock().unwrap();
        let (payouts_new, xmr_new) = Self::calc_payouts_and_xmr(&output_parse);
        // if the node is offline, p2pool can not function properly. Requires at least p2pool log level 1
        // if log level 0, it will take 2 minutes to detect that the node is offline.
        if contains_zmq_failure(&output_parse) {
            warn!("P2Pool Watchdog | a ZMQ failure was seen, check connection to Node");
            public.fails_zmq_since = Some(0);
        }

        // 3. Throw away [output_parse]
        output_parse.clear();
        drop(output_parse);
        // 4. Add to current values
        let (payouts, xmr) = (public.payouts + payouts_new, public.xmr + xmr_new);

        // 5. Calculate hour/day/month given elapsed time
        let elapsed_as_secs_f64 = elapsed.as_secs_f64();
        // Payouts
        let per_sec = (payouts as f64) / elapsed_as_secs_f64;
        let payouts_hour = (per_sec * 60.0) * 60.0;
        let payouts_day = payouts_hour * 24.0;
        let payouts_month = payouts_day * 30.0;
        // Total XMR
        let per_sec = xmr / elapsed_as_secs_f64;
        let xmr_hour = (per_sec * 60.0) * 60.0;
        let xmr_day = xmr_hour * 24.0;
        let xmr_month = xmr_day * 30.0;

        if payouts_new != 0 {
            debug!("P2Pool Watchdog | New [Payout] found in output ... {payouts_new}");
            debug!("P2Pool Watchdog | Total [Payout] should be ... {payouts}");
            debug!(
                "P2Pool Watchdog | Correct [Payout per] should be ... [{payouts_hour}/hour, {payouts_day}/day, {payouts_month}/month]"
            );
        }
        if xmr_new != 0.0 {
            debug!("P2Pool Watchdog | New [XMR mined] found in output ... {xmr_new}");
            debug!("P2Pool Watchdog | Total [XMR mined] should be ... {xmr}");
            debug!(
                "P2Pool Watchdog | Correct [XMR mined per] should be ... [{xmr_hour}/hour, {xmr_day}/day, {xmr_month}/month]"
            );
        }

        // 6. Mutate the struct with the new info
        *public = Self {
            uptime: HumanTime::into_human(elapsed),
            payouts,
            xmr,
            payouts_hour,
            payouts_day,
            payouts_month,
            xmr_hour,
            xmr_day,
            xmr_month,
            ..std::mem::take(public)
        };
    }

    // Mutate [PubP2poolApi] with data from a [PrivP2poolLocalApi] and the process output.
    pub(super) fn update_from_local(public: &mut Self, local: PrivP2poolLocalApi) {
        *public = Self {
            hashrate: HumanNumber::from_hashrate(&[
                Some(local.hashrate_15m),
                Some(local.hashrate_1h),
                Some(local.hashrate_24h),
            ])
            .to_string(),
            hashrate_15m: local.hashrate_15m,
            hashrate_1h: local.hashrate_1h,
            hashrate_24h: local.hashrate_24h,
            shares_found: Some(local.shares_found),
            average_effort: HumanNumber::to_percent(local.average_effort),
            current_effort: HumanNumber::to_percent(local.current_effort),
            connections: HumanNumber::from_u32(local.connections),
            user_p2pool_hashrate_u64: local.hashrate_1h,
            ..std::mem::take(&mut *public)
        };
    }
    // Mutate [PubP2poolApi] with data from a [PrivP2PoolP2PApi] and the process output.
    pub(super) fn update_from_p2p(public: &mut Self, p2p: PrivP2PoolP2PApi) {
        *public = Self {
            p2p_connected: p2p.connections,
            // above 120s, the node is disconnected.
            // It will take two minutes to detect that the node is dead.
            // If the timeframe is reduced, it can have false positive.
            node_connected: p2p.zmq_last_active.is_some_and(|x| x < 120),
            ..std::mem::take(&mut *public)
        };
    }

    // Mutate [PubP2poolApi] with data from a [PrivP2pool(Network|Pool)Api].
    pub(super) fn update_from_network_pool(
        public: &mut Self,
        net: PrivP2poolNetworkApi,
        pool: PrivP2poolPoolApi,
    ) {
        let user_hashrate = public.user_p2pool_hashrate_u64; // The user's total P2Pool hashrate
        let monero_difficulty = net.difficulty;
        let monero_hashrate = monero_difficulty / MONERO_BLOCK_TIME_IN_SECONDS;
        let p2pool_hashrate = pool.pool_statistics.hashRate;
        let p2pool_difficulty = pool.pool_statistics.sidechainDifficulty;
        // These [0] checks prevent dividing by 0 (it [panic!()]s)
        let p2pool_block_mean;
        let user_p2pool_percent;
        if p2pool_hashrate == 0 {
            p2pool_block_mean = HumanTime::new();
            user_p2pool_percent = HumanNumber::unknown();
        } else {
            p2pool_block_mean = HumanTime::into_human(std::time::Duration::from_secs(
                monero_difficulty / p2pool_hashrate,
            ));
            let f = (user_hashrate as f64 / p2pool_hashrate as f64) * 100.0;
            user_p2pool_percent = HumanNumber::from_f64_to_percent_6_point(f);
        };
        let p2pool_percent;
        let user_monero_percent;
        if monero_hashrate == 0 {
            p2pool_percent = HumanNumber::unknown();
            user_monero_percent = HumanNumber::unknown();
        } else {
            let f = (p2pool_hashrate as f64 / monero_hashrate as f64) * 100.0;
            p2pool_percent = HumanNumber::from_f64_to_percent_6_point(f);
            let f = (user_hashrate as f64 / monero_hashrate as f64) * 100.0;
            user_monero_percent = HumanNumber::from_f64_to_percent_6_point(f);
        };
        let solo_block_mean;
        let p2pool_share_mean;
        if user_hashrate == 0 {
            solo_block_mean = HumanTime::new();
            p2pool_share_mean = HumanTime::new();
        } else {
            solo_block_mean = HumanTime::into_human(std::time::Duration::from_secs(
                monero_difficulty / user_hashrate,
            ));
            p2pool_share_mean = HumanTime::into_human(std::time::Duration::from_secs(
                p2pool_difficulty / user_hashrate,
            ));
        }
        *public = Self {
            p2pool_difficulty_u64: p2pool_difficulty,
            monero_difficulty_u64: monero_difficulty,
            p2pool_hashrate_u64: p2pool_hashrate,
            monero_hashrate_u64: monero_hashrate,
            monero_difficulty: HumanNumber::from_u64(monero_difficulty),
            monero_hashrate: HumanNumber::from_u64_to_gigahash_3_point(monero_hashrate),
            hash: net.hash,
            height: net.height,
            reward: AtomicUnit::from_u64(net.reward),
            p2pool_difficulty: HumanNumber::from_u64(p2pool_difficulty),
            p2pool_hashrate: HumanNumber::from_u64_to_megahash_3_point(p2pool_hashrate),
            miners: HumanNumber::from_u32(pool.pool_statistics.miners),
            sidechain_height: pool.pool_statistics.sidechainHeight,
            solo_block_mean,
            p2pool_block_mean,
            p2pool_share_mean,
            p2pool_percent,
            user_p2pool_percent,
            user_monero_percent,
            ..std::mem::take(&mut *public)
        };
    }
    fn update_state(&self, process: &mut Process) {
        if process.state == ProcessState::Syncing
            && self.node_connected
            && self.p2p_connected > 1
            && self.sidechain_height > 1000
            && self.fails_zmq_since.is_none()
        {
            process.state = ProcessState::Alive;
        }
        if process.state == ProcessState::Alive
            && (self.sidechain_height < 1000
                || !self.node_connected
                || self.p2p_connected == 0
                || self.fails_zmq_since.is_some())
        {
            process.state = ProcessState::Syncing;
        }
    }

    #[inline]
    pub fn calculate_share_or_block_time(hashrate: u64, difficulty: u64) -> HumanTime {
        if hashrate == 0 {
            HumanTime::new()
        } else {
            HumanTime::from_u64(difficulty / hashrate)
        }
    }

    #[inline]
    pub fn calculate_dominance(my_hashrate: u64, global_hashrate: u64) -> HumanNumber {
        if global_hashrate == 0 {
            HumanNumber::unknown()
        } else {
            let f = (my_hashrate as f64 / global_hashrate as f64) * 100.0;
            HumanNumber::from_f64_to_percent_6_point(f)
        }
    }
}

//---------------------------------------------------------------------------------------------------- Private P2Pool "Local" Api
// This matches directly to P2Pool's [local/stratum] JSON API file (excluding a few stats).
// P2Pool seems to initialize all stats at 0 (or 0.0), so no [Option] wrapper seems needed.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub(super) struct PrivP2poolLocalApi {
    pub hashrate_15m: u64,
    pub hashrate_1h: u64,
    pub hashrate_24h: u64,
    pub shares_found: u64,
    pub average_effort: f32,
    pub current_effort: f32,
    pub connections: u32, // This is a `uint32_t` in `p2pool`
}

impl Default for PrivP2poolLocalApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivP2poolLocalApi {
    fn new() -> Self {
        Self {
            hashrate_15m: 0,
            hashrate_1h: 0,
            hashrate_24h: 0,
            shares_found: 0,
            average_effort: 0.0,
            current_effort: 0.0,
            connections: 0,
        }
    }

    // Deserialize the above [String] into a [PrivP2poolApi]
    pub(super) fn from_str(string: &str) -> std::result::Result<Self, serde_json::Error> {
        match serde_json::from_str::<Self>(string) {
            Ok(a) => Ok(a),
            Err(e) => {
                warn!("P2Pool Local API | Could not deserialize API data: {e}");
                Err(e)
            }
        }
    }
}

//---------------------------------------------------------------------------------------------------- Private P2Pool "Network" API
// This matches P2Pool's [network/stats] JSON API file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct PrivP2poolNetworkApi {
    pub difficulty: u64,
    pub hash: String,
    pub height: u32,
    pub reward: u64,
    pub timestamp: u32,
}

impl Default for PrivP2poolNetworkApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivP2poolNetworkApi {
    fn new() -> Self {
        Self {
            difficulty: 0,
            hash: String::from("???"),
            height: 0,
            reward: 0,
            timestamp: 0,
        }
    }

    pub(super) fn from_str(string: &str) -> std::result::Result<Self, serde_json::Error> {
        match serde_json::from_str::<Self>(string) {
            Ok(a) => Ok(a),
            Err(e) => {
                warn!("P2Pool Network API | Could not deserialize API data: {e}");
                Err(e)
            }
        }
    }
}

//---------------------------------------------------------------------------------------------------- Private P2Pool "Pool" API
// This matches P2Pool's [pool/stats] JSON API file.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub(super) struct PrivP2poolPoolApi {
    pub pool_statistics: PoolStatistics,
}

impl Default for PrivP2poolPoolApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivP2poolPoolApi {
    fn new() -> Self {
        Self {
            pool_statistics: PoolStatistics::new(),
        }
    }

    pub(super) fn from_str(string: &str) -> std::result::Result<Self, serde_json::Error> {
        match serde_json::from_str::<Self>(string) {
            Ok(a) => Ok(a),
            Err(e) => {
                warn!("P2Pool Pool API | Could not deserialize API data: {e}");
                Err(e)
            }
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub(super) struct PoolStatistics {
    pub hashRate: u64,
    pub miners: u32,
    pub sidechainHeight: u32,
    pub sidechainDifficulty: u64,
}
impl Default for PoolStatistics {
    fn default() -> Self {
        Self::new()
    }
}
impl PoolStatistics {
    fn new() -> Self {
        Self {
            hashRate: 0,
            miners: 0,
            sidechainHeight: 0,
            sidechainDifficulty: 0,
        }
    }
}
//---------------------------------------------------------------------------------------------------- Private P2Pool "Network" API
// This matches P2Pool's [local/p2p] JSON API file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct PrivP2PoolP2PApi {
    pub connections: u32,
    pub zmq_last_active: Option<u32>,
}

impl Default for PrivP2PoolP2PApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivP2PoolP2PApi {
    fn new() -> Self {
        Self {
            connections: 0,
            zmq_last_active: None,
        }
    }

    pub(super) fn from_str(string: &str) -> std::result::Result<Self, serde_json::Error> {
        match serde_json::from_str::<Self>(string) {
            Ok(a) => Ok(a),
            Err(e) => {
                warn!("P2Pool Network API | Could not deserialize API data: {e}");
                Err(e)
            }
        }
    }
}
fn reset_data_p2pool(pub_api: &Arc<Mutex<PubP2poolApi>>, gui_api: &Arc<Mutex<PubP2poolApi>>) {
    let current_pref = mem::take(&mut pub_api.lock().unwrap().prefer_local_node);
    // even if it is a restart, we want to keep set values by the user without the need from him to click on save button.

    *pub_api.lock().unwrap() = PubP2poolApi::new();
    *gui_api.lock().unwrap() = PubP2poolApi::new();
    // to keep the value modified by xmrig even if xvb is dead.
    pub_api.lock().unwrap().prefer_local_node = current_pref;
}
