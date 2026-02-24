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
use monero_crawler_lib::capability_checkers::{is_rpc_capable, is_zmq_capable};
use readable::byte::Byte;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    io::PipeReader,
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use sysinfo::{MemoryRefreshKind, System};
use tokio::spawn;

use crate::{
    disk::state::{Node, StartOptionsMode},
    helper::{
        ProcessName, ProcessSignal, ProcessState, check_died, check_died_process, check_user_input,
        signal_end, sleep_end_loop,
    },
    macros::sleep,
    utils::constants::{NODE_RPC_PORT_DEFAULT, NODE_ZMQ_PORT_DEFAULT, SOCKET_MONERO_LOCAL_OUTSIDE},
};
use std::fmt::Write;
use std::io::Write as IoWrite;

use super::{Helper, HumanNumber, HumanTime, Process};

impl Helper {
    #[cold]
    #[inline(never)]
    fn read_pty_node(
        output_parse: Arc<Mutex<String>>,
        output_pub: Arc<Mutex<String>>,
        reader: PipeReader,
    ) {
        use std::io::BufRead;
        let mut stdout = std::io::BufReader::new(reader).lines();

        // Run a ANSI escape sequence filter.
        while let Some(Ok(line)) = stdout.next() {
            let line = strip_ansi_escapes::strip_str(line);
            if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                error!("Node PTY Parse | Output error: {e}");
            }
            if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                error!("Node PTY Pub | Output error: {e}");
            }
        }
        while let Some(Ok(line)) = stdout.next() {
            if let Err(e) = writeln!(output_parse.lock().unwrap(), "{line}") {
                error!("P2Pool PTY Parse | Output error: {e}");
            }
            if let Err(e) = writeln!(output_pub.lock().unwrap(), "{line}") {
                error!("P2Pool PTY Pub | Output error: {e}");
            }
        }
    }
    pub fn build_node_args(
        state: &crate::disk::state::Node,
        mode: StartOptionsMode,
    ) -> Vec<String> {
        let mut args = Vec::with_capacity(500);

        // [Simple]
        match mode {
            StartOptionsMode::Simple => {
                // Build the node argument to be compatible with p2pool, prune by default
                args.push("--zmq-pub".to_string());
                args.push("tcp://127.0.0.1:18083".to_string()); // Local P2Pool (the default)
                args.push("--out-peers".to_string());
                args.push("32".to_string());
                args.push("--in-peers".to_string());
                args.push("64".to_string()); // Rig name
                args.push("--add-priority-node".to_string());
                args.push("p2pmd.xmrvsbeast.com:18080".to_string());
                args.push("--add-priority-node".to_string());
                args.push("nodes.hashvault.pro:18080".to_string());
                args.push("--disable-dns-checkpoints".to_string());
                args.push("--enable-dns-blocklist".to_string());
                args.push("--sync-pruned-blocks".to_string());
                args.push("--prune-blockchain".to_string());
            }
            StartOptionsMode::Advanced => {
                let dir = if state.path_db.is_empty() {
                    String::from(".bitmonero")
                } else {
                    state.path_db.to_string()
                };
                args.push("--data-dir".to_string());
                args.push(dir);
                args.push("--zmq-pub".to_string());
                args.push(format!("tcp://{}:{}", state.zmq_ip, state.zmq_port));
                args.push("--rpc-bind-ip".to_string());
                args.push(state.api_ip.clone());
                args.push("--rpc-bind-port".to_string());
                args.push(state.api_port.to_string());
                args.push("--out-peers".to_string());
                args.push(state.out_peers.to_string());
                args.push("--in-peers".to_string());
                args.push(state.in_peers.to_string());
                args.push("--log-level".to_string());
                args.push(state.log_level.to_string());
                args.push("--sync-pruned-blocks".to_string());
                if state.dns_blocklist {
                    args.push("--enable-dns-blocklist".to_string());
                }
                if state.disable_dns_checkpoint {
                    args.push("--disable-dns-checkpoints".to_string());
                }
                if state.pruned {
                    args.push("--prune-blockchain".to_string());
                }
            }
            StartOptionsMode::Custom => {
                // This parses the input
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
    // Just sets some signals for the watchdog thread to pick up on.
    pub fn stop_node(helper: &Arc<Mutex<Self>>) {
        info!("Node | Attempting to stop...");
        helper.lock().unwrap().node.lock().unwrap().signal = ProcessSignal::Stop;
        helper.lock().unwrap().node.lock().unwrap().state = ProcessState::Middle;
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_node);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_node);
        *pub_api.lock().unwrap() = PubNodeApi::new();
        *gui_api.lock().unwrap() = PubNodeApi::new();
    }
    #[cold]
    #[inline(never)]
    // The "restart frontend" to a "frontend" function.
    // Basically calls to kill the current p2pool, waits a little, then starts the below function in a a new thread, then exit.
    pub fn restart_node(helper: &Arc<Mutex<Self>>, state: &Node, path: &Path) {
        info!("Node | Attempting to restart...");
        helper.lock().unwrap().node.lock().unwrap().signal = ProcessSignal::Restart;
        helper.lock().unwrap().node.lock().unwrap().state = ProcessState::Middle;

        let helper = Arc::clone(helper);
        let state = state.clone();
        let path = path.to_path_buf();
        // This thread lives to wait, start p2pool then die.
        thread::spawn(move || {
            while helper.lock().unwrap().node.lock().unwrap().state != ProcessState::Waiting {
                warn!("Node | Want to restart but process is still alive, waiting...");
                sleep!(1000);
            }
            // Ok, process is not alive, start the new one!
            info!("Node | Old process seems dead, starting new one!");
            Self::start_node(&helper, &state, &path);
        });
        info!("Node | Restart ... OK");
    }
    #[cold]
    #[inline(never)]
    // The "frontend" function that parses the arguments, and spawns either the [Simple] or [Advanced] Node watchdog thread.
    pub fn start_node(helper: &Arc<Mutex<Self>>, state: &Node, path: &Path) {
        let mut args = vec![];

        let ports_detected_local_node = *helper
            .lock()
            .unwrap()
            .ports_detected_local_node
            .lock()
            .unwrap();
        if let Some((rpc_port, zmq_port)) = ports_detected_local_node {
            // Set the ImgNode to ports of the detected Node
            *helper.lock().unwrap().img_node.lock().unwrap() = ImgNode { rpc_port, zmq_port };
            crate::disk::print_dash(&format!(
                "Node | Starting the Process to watch a detected local Node with ports:\nrpc: {rpc_port}\nzmq: {zmq_port}"
            ));
            // Need to reset the detected ports value so that the next time we are here to start from Gupax it will not take the old values.
            *helper
                .lock()
                .unwrap()
                .ports_detected_local_node
                .lock()
                .unwrap() = None;
        } else {
            helper.lock().unwrap().node.lock().unwrap().state = ProcessState::Middle;
            let mode = if state.simple {
                StartOptionsMode::Simple
            } else if !state.arguments.is_empty() {
                StartOptionsMode::Custom
            } else {
                StartOptionsMode::Advanced
            };
            // Set the ImgNode with ports that the Node started with
            *helper.lock().unwrap().img_node.lock().unwrap() = ImgNode::new(state, &mode);
            args = Self::build_node_args(state, mode);

            // Print arguments & user settings to console
            crate::disk::print_dash(&format!("Node | Launch arguments: {args:#?}"));
        }

        // Spawn watchdog thread
        let process = Arc::clone(&helper.lock().unwrap().node);
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_node);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_node);
        let path = path.to_path_buf();
        let state = state.clone();
        let sys = Arc::clone(&helper.lock().unwrap().sys_info);
        let img_node = helper.lock().unwrap().img_node.lock().unwrap().clone();
        thread::spawn(move || {
            Self::spawn_node_watchdog(
                &process,
                &gui_api,
                &pub_api,
                args,
                path,
                state,
                ports_detected_local_node,
                &sys,
                img_node,
            );
        });
    }
    #[tokio::main]
    #[allow(clippy::await_holding_lock)]
    #[allow(clippy::too_many_arguments)]
    async fn spawn_node_watchdog(
        process: &Arc<Mutex<Process>>,
        gui_api: &Arc<Mutex<PubNodeApi>>,
        pub_api: &Arc<Mutex<PubNodeApi>>,
        args: Vec<String>,
        path: std::path::PathBuf,
        state: Node,
        ports_detected_local_node: Option<(u16, u16)>,
        sys: &Arc<Mutex<System>>,
        img: ImgNode,
    ) {
        process.lock().unwrap().start = Instant::now();
        // spawn pty if we are starting it from gupax
        debug!("Node | Creating PTY...");
        let mut child_pty = None;
        let mut output_pub = None;
        let mut stdin: Option<Box<dyn IoWrite + Send>> = None;
        let (stdin_reader, stdin_writer) = std::io::pipe().unwrap();
        let (stdout_reader, stdout_writer) = std::io::pipe().unwrap();
        if ports_detected_local_node.is_none() {
            // 4. Spawn PTY read thread
            debug!("Node | Spawning PTY read thread...");
            let output_parse = Arc::clone(&process.lock().unwrap().output_parse);
            output_pub = Some(Arc::clone(&process.lock().unwrap().output_pub));
            // 1b. Create command
            debug!("Node | Creating command...");
            let mut cmd = Command::new(&path);
            cmd.stdin(Stdio::from(stdin_reader));
            cmd.stdout(Stdio::from(stdout_writer));
            cmd.args(args);
            // if in simple state and enough free memory, enable full memory env
            if state.simple {
                let mut sys = sysinfo::System::new();
                sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
                if sys.available_memory() > 4_000_000_000 {
                    cmd.env("MONERO_RANDOMX_FULL_MEM", "1");
                }
            } else if state.full_memory {
                cmd.env("MONERO_RANDOMX_FULL_MEM", "1");
            }
            cmd.current_dir(path.as_path().parent().unwrap());
            // 1c. Create child
            debug!("Node | Creating child...");
            child_pty = Some(Arc::new(Mutex::new(cmd.spawn().unwrap())));
            stdin = Some(Box::new(stdin_writer));
            spawn(enc!((output_parse, output_pub) async move {
                Self::read_pty_node(output_parse, output_pub.unwrap(), stdout_reader);
            }));
        }
        // set state
        let client = Client::new();
        process.lock().unwrap().state = ProcessState::Syncing;
        process.lock().unwrap().signal = ProcessSignal::None;
        // reset stats
        *pub_api.lock().unwrap() = PubNodeApi::new();
        *gui_api.lock().unwrap() = PubNodeApi::new();
        // loop
        let start = process.lock().unwrap().start;
        info!("Node | Entering watchdog mode... woof!");
        loop {
            let now = Instant::now();
            debug!("Node Watchdog | ----------- Start of loop -----------");
            {
                // scope to drop locked mutex before the sleep
                // check state
                if let Some(child) = &child_pty
                    && check_died(
                        child,
                        &mut process.lock().unwrap(),
                        &start,
                        &mut gui_api.lock().unwrap().output,
                    )
                {
                    break;
                } else if check_died_process(
                    &mut process.lock().unwrap(),
                    &start,
                    &mut gui_api.lock().unwrap().output,
                    &mut sys.lock().unwrap(),
                ) {
                    break;
                }
                // check signal
                // if needed, kill by the pid
                if signal_end(
                    &mut process.lock().unwrap(),
                    child_pty.as_ref(),
                    &start,
                    &mut gui_api.lock().unwrap().output,
                ) {
                    break;
                }
                // check user input
                if let Some(stdin) = &mut stdin {
                    check_user_input(process, stdin);
                }
                // get data output/api

                // Check if logs need resetting
                debug!("Node Watchdog | Attempting GUI log reset check");
                {
                    Self::check_reset_gui_output(
                        &mut gui_api.lock().unwrap().output,
                        ProcessName::Node,
                    );
                }
                // No need to check output since monerod has a sufficient API
                // Always update from output
                if let Some(output) = &output_pub {
                    debug!("Node Watchdog | Starting [update_from_output()]");
                    PubNodeApi::update_from_output(pub_api, output, start.elapsed());
                }
                // update data from api
                debug!("Node Watchdog | Attempting HTTP API request...");
                // need to construct the socket from String and possibly domain name
                let adr = if let Some((rpc, _)) = ports_detected_local_node {
                    format!("127.0.0.1:{rpc}")
                } else {
                    format!("{}:{}", state.api_ip, img.rpc_port)
                };
                if let Ok(sockets) = adr.to_socket_addrs()
                    && let Some(socket) = sockets.last()
                {
                    match PrivNodeApi::request_api(&client, &socket).await {
                        Ok(priv_api) => {
                            debug!(
                                "Node Watchdog | HTTP API request OK, attempting [update_from_priv()]"
                            );
                            if priv_api.result.synchronized && priv_api.result.status == "OK" {
                                process.lock().unwrap().state = ProcessState::Alive
                            }
                            PubNodeApi::update_from_priv(pub_api, priv_api);
                        }
                        Err(err) => {
                            // if node is just starting, do not throw an error
                            match start.elapsed() {
                                x if x < Duration::from_secs(30) => {}
                                x if x >= Duration::from_secs(30)
                                    && x <= Duration::from_secs(60) =>
                                {
                                    warn!(
                                        "Node Watchdog | Could not send HTTP API request to node\n{err}\nPossibly because it is just starting"
                                    );
                                }
                                _ => {
                                    warn!(
                                        "Node Watchdog | Could not send HTTP API request to node\n{err}"
                                    );
                                }
                            }
                        }
                    }
                } else {
                    warn!(
                        "Node Watchdog | Could not send HTTP API request to node\nConversion of ip/domain and port rpc failed"
                    );
                }
            }
            // do not use more than 1 second for the loop
            sleep_end_loop(now, ProcessName::Node).await;
        }

        // 5. If loop broke, we must be done here.
        info!("Node Watchdog | Watchdog thread exiting... Goodbye!");
        // sleep
    }
}
#[derive(Clone)]
pub struct PubNodeApi {
    pub output: String,
    pub uptime: HumanTime,
    pub blockheight: HumanNumber,
    pub difficulty: HumanNumber,
    pub database_size: String,
    pub free_space: String,
    pub nettype: String,
    pub outgoing_connections: u16,
    pub incoming_connections: u16,
    pub status: String,
    pub synchronized: bool,
}
impl Default for PubNodeApi {
    fn default() -> Self {
        Self::new()
    }
}

impl PubNodeApi {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            uptime: HumanTime::new(),
            blockheight: HumanNumber::unknown(),
            difficulty: HumanNumber::unknown(),
            database_size: HumanNumber::unknown().to_string(),
            free_space: HumanNumber::unknown().to_string(),
            nettype: String::from("???"),
            outgoing_connections: 0,
            incoming_connections: 0,
            status: String::from("Offline"),
            synchronized: false,
        }
    }
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
    fn update_from_priv(public: &Arc<Mutex<Self>>, private: PrivNodeApi) {
        let mut public = public.lock().unwrap();
        *public = Self {
            blockheight: HumanNumber::from_u64(private.result.height),
            difficulty: HumanNumber::from_u64(private.result.difficulty),
            database_size: Byte::from(private.result.database_size).to_string(),
            // free space can be unavailable if the node was started with a restricted rpc.
            // In this case 18 exa will be reported.
            // If that's the case, it means the user did start the node with restricted rpc,
            // so it should be aware of what this means.
            // A noob user that doesn't know what an rpc interface is should not see
            // this message.
            free_space: if private.result.free_space >= 18_000_000_000_000_000_000 {
                "Restricted RPC".to_string()
            } else {
                Byte::from(private.result.free_space).to_string()
            },
            nettype: private.result.nettype,
            outgoing_connections: private.result.outgoing_connections_count,
            incoming_connections: private.result.incoming_connections_count,
            status: private.result.status,
            synchronized: private.result.synchronized,
            ..std::mem::take(&mut *public)
        }
    }
    pub fn update_from_output(
        public: &Arc<Mutex<Self>>,
        output_pub: &Arc<Mutex<String>>,
        elapsed: std::time::Duration,
    ) {
        // 1. Take the process's current output buffer and combine it with Pub (if not empty)
        let mut output_pub = output_pub.lock().unwrap();

        {
            let mut public = public.lock().unwrap();
            if !output_pub.is_empty() {
                public.output.push_str(&std::mem::take(&mut *output_pub));
            }
            // Update uptime
            public.uptime = HumanTime::into_human(elapsed);
        }
    }
}
#[derive(Deserialize, Serialize)]
struct PrivNodeApi {
    result: ResultNodeJson,
}
#[derive(Deserialize, Serialize)]
struct ResultNodeJson {
    pub height: u64,
    pub difficulty: u64,
    pub database_size: u64,
    pub free_space: u64,
    pub nettype: String,
    pub outgoing_connections_count: u16,
    pub incoming_connections_count: u16,
    pub status: String,
    pub synchronized: bool,
}
impl PrivNodeApi {
    async fn request_api(
        client: &Client,
        socket: &SocketAddr,
    ) -> std::result::Result<Self, anyhow::Error> {
        let adr = format!("http://{}:{}/json_rpc", socket.ip(), socket.port());
        let private = client
            .post(adr)
            .body(r#"{"jsonrpc":"2.0","id":"0","method":"get_info"}"#)
            .send()
            .await?
            .json::<PrivNodeApi>()
            .await?;
        Ok(private)
    }
}
#[derive(Debug, Clone)]
pub struct ImgNode {
    pub rpc_port: u16,
    pub zmq_port: u16,
}

impl Default for ImgNode {
    fn default() -> Self {
        Self {
            rpc_port: NODE_RPC_PORT_DEFAULT,
            zmq_port: NODE_ZMQ_PORT_DEFAULT,
        }
    }
}

impl ImgNode {
    pub fn new(state: &crate::disk::state::Node, mode: &StartOptionsMode) -> ImgNode {
        match mode {
            StartOptionsMode::Simple => ImgNode::default(),
            StartOptionsMode::Advanced => {
                let (rpc_port, zmq_port) = state.ports();
                ImgNode { rpc_port, zmq_port }
            }
            StartOptionsMode::Custom => {
                // This parses the input and attempts to fill out
                // the [ImgXmrig]... This is pretty bad code...
                let mut last = "";
                let mut img = ImgNode::default();
                for arg in state.arguments.split_whitespace() {
                    match last {
                        "--zmq-rpc-bind-port" => {
                            if let Ok(port) = arg.parse::<u16>() {
                                img.zmq_port = port
                            }
                        }
                        "--zmq-pub" => {
                            if let Some(port_str) = last.split(":").last()
                                && let Ok(port) = port_str.parse::<u16>()
                            {
                                img.zmq_port = port;
                            }
                        }
                        "--rpc-bind-port" => {
                            if let Ok(port) = arg.parse::<u16>() {
                                img.rpc_port = port
                            }
                        }
                        _ => (),
                    }
                    last = arg;
                }
                img
            }
        }
    }
}

pub fn spawn_local_outside_checker(tx: Arc<OnceLock<CheckLocalOutsideNode>>) {
    thread::spawn(move || check_local_node_outside(tx));
}

#[tokio::main]
async fn check_local_node_outside(tx: Arc<OnceLock<CheckLocalOutsideNode>>) {
    let local_outside_node_ports = ProcessName::Node.ports_listen_sys();
    if let Some(set_ports) = local_outside_node_ports
        && !set_ports.is_empty()
    {
        let ports = set_ports.iter().cloned().collect::<Vec<u16>>();
        let timeout = Duration::from_millis(1500);
        if let Some(zmq) = is_zmq_capable(SOCKET_MONERO_LOCAL_OUTSIDE, &ports, timeout).await {
            if let Some(rpc) = is_rpc_capable(
                SOCKET_MONERO_LOCAL_OUTSIDE,
                18081,
                &ports,
                timeout,
                &Client::new(),
            )
            .await
            {
                tx.set(CheckLocalOutsideNode::Valid(rpc, zmq)).unwrap();
            } else {
                tx.set(CheckLocalOutsideNode::NonValid).unwrap();
                warn!("the detected node does not have a valid zmq port/response");
            }
        } else {
            tx.set(CheckLocalOutsideNode::NonValid).unwrap();
            warn!("the detected node does not have a valid rpc port/response");
        }
    } else {
        tx.set(CheckLocalOutsideNode::None).unwrap();
    }
}

#[derive(Clone, Debug)]
pub enum CheckLocalOutsideNode {
    // rpc port, zmq port
    Valid(u16, u16),
    NonValid,
    None,
}
