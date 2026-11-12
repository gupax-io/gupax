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

// This file represents the "helper" thread, which is the full separate thread
// that runs alongside the main [App] GUI thread. It exists for the entire duration
// of Gupax so that things can be handled without locking up the GUI thread.
//
// This thread is a continual 1 second loop, collecting available jobs on the
// way down and (if possible) asynchronously executing them at the very end.
//
// The main GUI thread will interface with this thread by mutating the Arc<Mutex>'s
// found here, e.g: User clicks [Stop P2Pool] -> Arc<Mutex<ProcessSignal> is set
// indicating to this thread during its loop: "I should stop P2Pool!", e.g:
//
//     if p2pool.lock().unwrap().signal == ProcessSignal::Stop {
//         stop_p2pool(),
//     }
//
// This also includes all things related to handling the child processes (P2Pool/XMRig):
// piping their stdout/stderr/stdin, accessing their APIs (HTTP + disk files), etc.

use crate::components::gupax::FileType;
use crate::components::update::{NODE_BINARY, P2POOL_BINARY, XMRIG_BINARY, XMRIG_PROXY_BINARY};
use crate::helper::notification::NotificationApi;
use crate::helper::sys_info::Sys;
//---------------------------------------------------------------------------------------------------- Import
use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
use crate::helper::{
    p2pool::{ImgP2pool, PubP2poolApi},
    xrig::{xmrig::ImgXmrig, xmrig::PubXmrigApi},
};
// use crate::utils::errors::process_running;
use crate::{constants::*, disk::gupax_p2pool_api::GupaxP2poolApi, human::*, macros::*};
use derive_more::derive::Display;
use enclose::enc;
use log::*;
use node::{ImgNode, PubNodeApi};
use port_check::is_port_reachable_with_timeout;
use readable::up::Uptime;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Write};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::*,
};
use strum::{EnumCount, EnumIter};
use sysinfo::{Pid, ProcessRefreshKind, System};
use xrig::xmrig_proxy::ImgProxy;

use self::xvb::{PubXvbApi, nodes::Pool};
pub mod crawler;
pub mod node;
pub mod notification;
pub mod p2pool;
pub mod payout_sync;
pub mod sys_info;
pub mod tests;
pub mod xrig;
pub mod xvb;

//---------------------------------------------------------------------------------------------------- Constants
// The max amount of bytes of process output we are willing to
// hold in memory before it's too much and we need to reset.
const MAX_GUI_OUTPUT_BYTES: usize = 500_000;
// Just a little leeway so a reset will go off before the [String] allocates more memory.
const GUI_OUTPUT_LEEWAY: usize = MAX_GUI_OUTPUT_BYTES - 1000;

// Some constants for generating hashrate/difficulty.
const MONERO_BLOCK_TIME_IN_SECONDS: u64 = 120;

//---------------------------------------------------------------------------------------------------- [Helper] Struct
// A meta struct holding all the data that gets processed in this thread
pub struct Helper {
    pub instant: Instant,                             // Gupax start as an [Instant]
    pub uptime: HumanTime,                            // Gupax uptime formatting for humans
    pub pub_sys: Arc<Mutex<Sys>>, // The public API for [sysinfo] that the [Status] tab reads from
    pub p2pool: Arc<Mutex<Process>>, // P2Pool process state
    pub node: Arc<Mutex<Process>>, // P2Pool process state
    pub xmrig: Arc<Mutex<Process>>, // XMRig process state
    pub xmrig_proxy: Arc<Mutex<Process>>, // XMRig process state
    pub xvb: Arc<Mutex<Process>>, // XvB process state
    pub gui_api_p2pool: Arc<Mutex<PubP2poolApi>>, // P2Pool API state (for GUI thread)
    pub gui_api_xmrig: Arc<Mutex<PubXmrigApi>>, // XMRig API state (for GUI thread)
    pub gui_api_xp: Arc<Mutex<PubXmrigProxyApi>>, // XMRig-Proxy API state (for GUI thread)
    pub gui_api_xvb: Arc<Mutex<PubXvbApi>>, // XMRig API state (for GUI thread)
    pub gui_api_node: Arc<Mutex<PubNodeApi>>, // Node API state (for GUI thread)
    pub img_node: Arc<Mutex<ImgNode>>, // A static "image" of the data XMRig started with
    pub img_p2pool: Arc<Mutex<ImgP2pool>>, // A static "image" of the data P2Pool started with
    pub img_xmrig: Arc<Mutex<ImgXmrig>>, // A static "image" of the data XMRig started with
    pub img_proxy: Arc<Mutex<ImgProxy>>, // A static "image" of the data XMRig started with
    pub_api_p2pool: Arc<Mutex<PubP2poolApi>>, // P2Pool API state (for Helper/P2Pool thread)
    pub_api_xmrig: Arc<Mutex<PubXmrigApi>>, // XMRig API state (for Helper/XMRig thread)
    pub_api_xp: Arc<Mutex<PubXmrigProxyApi>>, // XMRig-Proxy API state (for Helper/XMRig-Proxy thread)
    pub_api_node: Arc<Mutex<PubNodeApi>>,     // Node API state (for Helper/Node thread)
    pub_api_xvb: Arc<Mutex<PubXvbApi>>,       // XvB API state (for Helper/XvB thread)
    pub gupax_p2pool_api: Arc<Mutex<GupaxP2poolApi>>, //
    pub ip_public: Arc<Mutex<Option<Ipv4Addr>>>,
    pub ip_local: Arc<Mutex<Option<IpAddr>>>,
    pub proxy_port_reachable: Arc<Mutex<bool>>,
    // consider it true if it is Some
    pub ports_detected_local_node: Arc<Mutex<Option<(u16, u16)>>>,
    pub sys_info: Arc<Mutex<System>>,
    pub notifications_api: Arc<Mutex<NotificationApi>>,
}

// The communication between the data here and the GUI thread goes as follows:
// [GUI] <---> [Helper] <---> [Watchdog] <---> [Private Data only available here]
//
// Both [GUI] and [Helper] own their separate [Pub*Api] structs.
// Since P2Pool & XMRig will be updating their information out of sync,
// it's the helpers job to lock everything, and move the watchdog [Pub*Api]s
// on a 1-second interval into the [GUI]'s [Pub*Api] struct, atomically.

//---------------------------------------------------------------------------------------------------- [Process] Struct
// This holds all the state of a (child) process.
// The main GUI thread will use this to display console text, online state, etc.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Process {
    pub name: ProcessName,     // P2Pool or XMRig?
    pub state: ProcessState,   // The state of the process (alive, dead, etc)
    pub signal: ProcessSignal, // Did the user click [Start/Stop/Restart]?
    pub external: bool,        // A detected Node that Gupax did not start
    // STDIN Problem:
    //     - User can input many many commands in 1 second
    //     - The process loop only processes every 1 second
    //     - If there is only 1 [String] holding the user input,
    //       the user could overwrite their last input before
    //       the loop even has a chance to process their last command
    // STDIN Solution:
    //     - When the user inputs something, push it to a [Vec]
    //     - In the process loop, loop over every [Vec] element and
    //       send each one individually to the process stdin
    //
    pub input: Vec<String>,

    // The below are the handles to the actual child process.
    // [Simple] has no STDIN, but [Advanced] does. A PTY (pseudo-terminal) is
    // required for P2Pool/XMRig to open their STDIN pipe.
    //	child: Option<Arc<Mutex<Box<dyn portable_pty::Child + Send + std::marker::Sync>>>>, // STDOUT/STDERR is combined automatically thanks to this PTY, nice
    //	stdin: Option<Box<dyn portable_pty::MasterPty + Send>>, // A handle to the process's MasterPTY/STDIN

    // This is the process's private output [String], used by both [Simple] and [Advanced].
    // "parse" contains the output that will be parsed, then tossed out. "pub" will be written to
    // the same as parse, but it will be [swap()]'d by the "helper" thread into the GUIs [String].
    // The "helper" thread synchronizes this swap so that the data in here is moved there
    // roughly once a second. GUI thread never touches this.
    output_parse: Arc<Mutex<String>>,
    output_pub: Arc<Mutex<String>>,

    // Start time of process.
    start: std::time::Instant,

    // Pid of process if needed
    // Only used for Node for now to check if it still exist without an expensive operation, but can allow a lot more in the future by getting data about the process.
    pid: Option<Pid>,
}

//---------------------------------------------------------------------------------------------------- [Process] Impl
impl Process {
    pub fn new(name: ProcessName, _args: String, _path: PathBuf) -> Self {
        Self {
            name,
            state: ProcessState::Dead,
            signal: ProcessSignal::None,
            external: false,
            start: Instant::now(),
            //			stdin: Option::None,
            //			child: Option::None,
            output_parse: arc_mut!(String::with_capacity(500)),
            output_pub: arc_mut!(String::with_capacity(500)),
            input: vec![String::new()],
            pid: None,
        }
    }

    #[inline]
    // Convenience functions
    pub fn is_alive(&self) -> bool {
        self.state == ProcessState::Alive
            || self.state == ProcessState::Middle
            || self.state == ProcessState::Syncing
            || self.state == ProcessState::NotMining
            || self.state == ProcessState::OfflinePoolsAll
    }

    #[inline]
    pub fn is_waiting(&self) -> bool {
        self.state == ProcessState::Middle || self.state == ProcessState::Waiting
    }

    /// Clear the signal once the child is spawned: a Restart is done by
    /// then, and a Stop raised while starting still has to be carried out.
    pub fn reset_signal_on_start(&mut self) {
        if self.signal != ProcessSignal::Stop {
            self.signal = ProcessSignal::None;
        }
    }

    // Lines left by the previous run would show in the console and state of the next one.
    pub fn clear_output(&self) {
        self.output_parse.lock().unwrap().clear();
        self.output_pub.lock().unwrap().clear();
    }
    pub fn _initialize_process_pid(&mut self, sys: Arc<Mutex<System>>) -> bool {
        if let Some(process) = sys
            .lock()
            .unwrap()
            .processes_by_exact_name(self.name.binary_name().as_ref())
            .next()
        {
            self.pid = Some(process.pid());
            return true;
        }
        false
    }
}

//---------------------------------------------------------------------------------------------------- [Process*] Enum
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum ProcessState {
    Alive, // Process is online, GREEN!
    #[default]
    Dead, // Process is dead, BLACK!
    Failed, // Process is dead AND exited with a bad code, RED!
    Middle, // Process is in the middle of something ([re]starting/stopping), YELLOW!
    Waiting, // Process was successfully killed by a restart, and is ready to be started again, YELLOW!

    // Only for P2Pool and XvB, ORANGE.
    // XvB: Xmrig or P2pool are not alive
    Syncing,

    // Only for XMRig and XvB, ORANGE.
    // XvB: token or address are invalid even if syntax correct
    NotMining,
    // XvB: if pool of XvB become unusable (ex: offline).
    OfflinePoolsAll,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub enum ProcessSignal {
    #[default]
    None,
    Stop,
    Restart,
    UpdatePools(Pool),
}

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Debug,
    Display,
    EnumIter,
    EnumCount,
    Serialize,
    Deserialize,
    Default,
    PartialOrd,
    Ord,
)]
pub enum ProcessName {
    Node,
    P2pool,
    // Its own spelling.
    #[display("XMRig")]
    Xmrig,
    #[display("Proxy")]
    XmrigProxy,
    #[default]
    Xvb,
}

/// Raises a process's stop flag; its watchdog kills the process.
pub type StopFn = fn(&Arc<Mutex<Helper>>);

impl ProcessName {
    pub const fn stop_fn(&self) -> StopFn {
        match self {
            ProcessName::Node => Helper::stop_node,
            ProcessName::P2pool => Helper::stop_p2pool,
            ProcessName::Xmrig => Helper::stop_xmrig,
            ProcessName::XmrigProxy => Helper::stop_xp,
            ProcessName::Xvb => Helper::stop_xvb,
        }
    }
    pub const fn binary_name(&self) -> &str {
        match self {
            ProcessName::Node => NODE_BINARY,
            ProcessName::P2pool => P2POOL_BINARY,
            ProcessName::Xmrig => XMRIG_BINARY,
            ProcessName::XmrigProxy => XMRIG_PROXY_BINARY,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_binary_path_empty(&self) -> &str {
        match self {
            ProcessName::Node => NODE_PATH_EMPTY,
            ProcessName::P2pool => P2POOL_PATH_EMPTY,
            ProcessName::Xmrig => XMRIG_PATH_EMPTY,
            ProcessName::XmrigProxy => XMRIG_PROXY_PATH_EMPTY,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_binary_path_not_file(&self) -> &str {
        match self {
            ProcessName::Node => NODE_PATH_NOT_FILE,
            ProcessName::P2pool => P2POOL_PATH_NOT_FILE,
            ProcessName::Xmrig => XMRIG_PATH_NOT_FILE,
            ProcessName::XmrigProxy => XMRIG_PROXY_PATH_NOT_FILE,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_binary_path_invalid(&self) -> &str {
        match self {
            ProcessName::Node => NODE_PATH_NOT_VALID,
            ProcessName::P2pool => P2POOL_PATH_NOT_VALID,
            ProcessName::Xmrig => XMRIG_PATH_NOT_VALID,
            ProcessName::XmrigProxy => XMRIG_PROXY_PATH_NOT_VALID,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_binary_path_not_executable(&self) -> &str {
        match self {
            ProcessName::Node => NODE_PATH_NOT_EXECUTABLE,
            ProcessName::P2pool => P2POOL_PATH_NOT_EXECUTABLE,
            ProcessName::Xmrig => XMRIG_PATH_NOT_EXECUTABLE,
            ProcessName::XmrigProxy => XMRIG_PROXY_PATH_NOT_EXECUTABLE,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_binary_path_ok(&self) -> &str {
        match self {
            ProcessName::Node => NODE_PATH_OK,
            ProcessName::P2pool => P2POOL_PATH_OK,
            ProcessName::Xmrig => XMRIG_PATH_OK,
            ProcessName::XmrigProxy => XMRIG_PROXY_PATH_OK,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_path_edit(&self) -> &str {
        match self {
            ProcessName::Node => GUPAX_PATH_NODE,
            ProcessName::P2pool => GUPAX_PATH_P2POOL,
            ProcessName::Xmrig => GUPAX_PATH_XMRIG,
            ProcessName::XmrigProxy => GUPAX_PATH_XMRIG_PROXY,
            ProcessName::Xvb => "",
        }
    }
    pub const fn msg_auto_help(&self) -> &str {
        match self {
            ProcessName::Node => GUPAX_AUTO_NODE,
            ProcessName::P2pool => GUPAX_AUTO_P2POOL,
            ProcessName::Xmrig => GUPAX_AUTO_XMRIG,
            ProcessName::XmrigProxy => GUPAX_AUTO_XMRIG_PROXY,
            ProcessName::Xvb => GUPAX_AUTO_XVB,
        }
    }
    pub const fn file_type(&self) -> Option<FileType> {
        match self {
            ProcessName::Node => Some(FileType::Node),
            ProcessName::P2pool => Some(FileType::P2pool),
            ProcessName::Xmrig => Some(FileType::Xmrig),
            ProcessName::XmrigProxy => Some(FileType::XmrigProxy),
            ProcessName::Xvb => None,
        }
    }
    pub const fn start_options_hint(&self) -> &str {
        match self {
            ProcessName::Node => NODE_START_OPTIONS_HINT,
            ProcessName::P2pool => P2POOL_START_OPTIONS_HINT,
            ProcessName::Xmrig | ProcessName::XmrigProxy => XMRIG_START_OPTIONS_HINT,
            ProcessName::Xvb => "",
        }
    }
    pub fn ports_listen_sys(&self) -> Vec<u16> {
        let mut ports = vec![];
        if let Ok(set) = listeners::get_all() {
            for listener in set.iter() {
                if listener.process.name == self.binary_name() {
                    ports.push(listener.socket.port());
                }
            }
        }
        ports
    }
    pub fn is_process_running(&self, sys: &mut System) -> bool {
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        sys.processes_by_exact_name(self.binary_name().as_ref())
            .next()
            .is_some()
    }
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:#?}")
    }
}
impl std::fmt::Display for ProcessSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:#?}")
    }
}

//---------------------------------------------------------------------------------------------------- [Helper]
impl Helper {
    //---------------------------------------------------------------------------------------------------- General Functions
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instant: std::time::Instant,
        pub_sys: Arc<Mutex<Sys>>,
        p2pool: Arc<Mutex<Process>>,
        xmrig: Arc<Mutex<Process>>,
        xmrig_proxy: Arc<Mutex<Process>>,
        xvb: Arc<Mutex<Process>>,
        node: Arc<Mutex<Process>>,
        gui_api_p2pool: Arc<Mutex<PubP2poolApi>>,
        gui_api_xmrig: Arc<Mutex<PubXmrigApi>>,
        gui_api_xvb: Arc<Mutex<PubXvbApi>>,
        gui_api_xp: Arc<Mutex<PubXmrigProxyApi>>,
        gui_api_node: Arc<Mutex<PubNodeApi>>,
        img_node: Arc<Mutex<ImgNode>>,
        img_p2pool: Arc<Mutex<ImgP2pool>>,
        img_xmrig: Arc<Mutex<ImgXmrig>>,
        img_proxy: Arc<Mutex<ImgProxy>>,
        gupax_p2pool_api: Arc<Mutex<GupaxP2poolApi>>,
        ip_local: Arc<Mutex<Option<IpAddr>>>,
        ip_public: Arc<Mutex<Option<Ipv4Addr>>>,
        proxy_port_reachable: Arc<Mutex<bool>>,
        ports_detected_local_node: Arc<Mutex<Option<(u16, u16)>>>,
        sys_info: Arc<Mutex<System>>,
        notifications_api: Arc<Mutex<NotificationApi>>,
    ) -> Self {
        Self {
            instant,
            pub_sys,
            uptime: HumanTime::into_human(instant.elapsed()),
            pub_api_p2pool: arc_mut!(PubP2poolApi::new()),
            pub_api_xmrig: arc_mut!(PubXmrigApi::new()),
            pub_api_xp: arc_mut!(PubXmrigProxyApi::new()),
            pub_api_xvb: arc_mut!(PubXvbApi::new()),
            pub_api_node: arc_mut!(PubNodeApi::new()),
            // These are created when initializing [App], since it needs a handle to it as well
            p2pool,
            xmrig,
            xmrig_proxy,
            xvb,
            node,
            gui_api_p2pool,
            gui_api_xmrig,
            gui_api_xvb,
            gui_api_xp,
            gui_api_node,
            img_node,
            img_p2pool,
            img_xmrig,
            img_proxy,
            gupax_p2pool_api,
            ip_local,
            ip_public,
            proxy_port_reachable,
            ports_detected_local_node,
            sys_info,
            notifications_api,
        }
    }

    // Reset output if larger than max bytes.
    // This will also append a message showing it was reset.
    fn check_reset_gui_output(output: &mut String, name: ProcessName) {
        let len = output.len();
        if len > GUI_OUTPUT_LEEWAY {
            info!("{name} Watchdog | Output is nearing {MAX_GUI_OUTPUT_BYTES} bytes, resetting!");
            let text = format!(
                "{HORI_CONSOLE}\n{name} GUI log is exceeding the maximum: {MAX_GUI_OUTPUT_BYTES} bytes!\nI've reset the logs for you!\n{HORI_CONSOLE}\n\n\n\n"
            );
            output.clear();
            output.push_str(&text);
            debug!("{name} Watchdog | Resetting GUI output ... OK");
        } else {
            debug!("{name} Watchdog | GUI output reset not needed! Current byte length ... {len}");
        }
    }

    // Read P2Pool/XMRig's API file to a [String].
    fn path_to_string(
        path: &Path,
        name: ProcessName,
    ) -> std::result::Result<String, std::io::Error> {
        match std::fs::read_to_string(path) {
            Ok(s) => Ok(s),
            Err(e) => {
                warn!("{} API | [{}] read error: {}", name, path.display(), e);
                Err(e)
            }
        }
    }
    //---------------------------------------------------------------------------------------------------- The "helper"

    #[cold]
    #[inline(never)]
    // The "helper" thread. Syncs data between threads here and the GUI.
    #[allow(clippy::await_holding_lock)]
    pub fn spawn_helper(helper: &Arc<Mutex<Self>>, pid: sysinfo::Pid, max_threads: u16) {
        // The ordering of these locks is _very_ important. They MUST be in sync with how the main GUI thread locks stuff
        // or a deadlock will occur given enough time. They will eventually both want to lock the [Arc<Mutex>] the other
        // thread is already locking. Yes, I figured this out the hard way, hence the vast amount of debug!() messages.
        // Example of different order (BAD!):
        //
        // GUI Main       -> locks [p2pool] first
        // Helper         -> locks [gui_api_p2pool] first
        // GUI Status Tab -> tries to lock [gui_api_p2pool] -> CAN'T
        // Helper         -> tries to lock [p2pool] -> CAN'T
        //
        // These two threads are now in a deadlock because both
        // are trying to access locks the other one already has.
        //
        // The locking order here must be in the same chronological
        // order as the main GUI thread (top to bottom).

        let helper = Arc::clone(helper);
        let lock = helper.lock().unwrap();
        let node = Arc::clone(&lock.node);
        let p2pool = Arc::clone(&lock.p2pool);
        let xmrig = Arc::clone(&lock.xmrig);
        let xmrig_proxy = Arc::clone(&lock.xmrig_proxy);
        let xvb = Arc::clone(&lock.xvb);
        let pub_sys = Arc::clone(&lock.pub_sys);
        let gui_api_node = Arc::clone(&lock.gui_api_node);
        let gui_api_p2pool = Arc::clone(&lock.gui_api_p2pool);
        let gui_api_xmrig = Arc::clone(&lock.gui_api_xmrig);
        let gui_api_xp = Arc::clone(&lock.gui_api_xp);
        let gui_api_xvb = Arc::clone(&lock.gui_api_xvb);
        let pub_api_node = Arc::clone(&lock.pub_api_node);
        let pub_api_p2pool = Arc::clone(&lock.pub_api_p2pool);
        let pub_api_xmrig = Arc::clone(&lock.pub_api_xmrig);
        let pub_api_xp = Arc::clone(&lock.pub_api_xp);
        let pub_api_xvb = Arc::clone(&lock.pub_api_xvb);
        let sysinfo = Arc::clone(&lock.sys_info);
        drop(lock);

        let sysinfo_cpu = sysinfo::CpuRefreshKind::everything();
        let sysinfo_processes = sysinfo::ProcessRefreshKind::nothing().with_cpu();
        thread::spawn(move || {
            info!(
                "Helper | Hello from helper thread! Entering loop where I will spend the rest of my days..."
            );
            // Begin loop
            loop {
                // 1. Loop init timestamp
                let start = Instant::now();
                debug!("Helper | ----------- Start of loop -----------");

                // Ignore the invasive [debug!()] messages on the right side of the code.
                // The reason why they are there are so that it's extremely easy to track
                // down the culprit of an [Arc<Mutex>] deadlock. I know, they're ugly.

                // 2. Lock... EVERYTHING!
                let mut lock = helper.lock().unwrap();
                debug!("Helper | Locked (1/17) ... [helper]");
                let node = node.lock().unwrap();
                debug!("Helper | Locked (2/17) ... [node]");
                let p2pool = p2pool.lock().unwrap();
                debug!("Helper | Locked (3/17) ... [p2pool]");
                let xmrig = xmrig.lock().unwrap();
                debug!("Helper | Locked (4/17) ... [xmrig]");
                let xmrig_proxy = xmrig_proxy.lock().unwrap();
                debug!("Helper | Locked (5/17) ... [xmrig_proxy]");
                let xvb = xvb.lock().unwrap();
                debug!("Helper | Locked (6/17) ... [xvb]");
                let mut lock_pub_sys = pub_sys.lock().unwrap();
                debug!("Helper | Locked (8/17) ... [pub_sys]");
                let mut gui_api_node = gui_api_node.lock().unwrap();
                debug!("Helper | Locked (7/17) ... [gui_api_node]");
                let mut gui_api_p2pool = gui_api_p2pool.lock().unwrap();
                debug!("Helper | Locked (9/17) ... [gui_api_p2pool]");
                let mut gui_api_xmrig = gui_api_xmrig.lock().unwrap();
                debug!("Helper | Locked (10/17) ... [gui_api_xmrig]");
                let mut gui_api_xp = gui_api_xp.lock().unwrap();
                debug!("Helper | Locked (11/17) ... [gui_api_xp]");
                let mut gui_api_xvb = gui_api_xvb.lock().unwrap();
                debug!("Helper | Locked (12/17) ... [gui_api_xvb]");
                let mut pub_api_node = pub_api_node.lock().unwrap();
                debug!("Helper | Locked (13/17) ... [pub_api_node]");
                let mut pub_api_p2pool = pub_api_p2pool.lock().unwrap();
                debug!("Helper | Locked (14/17) ... [pub_api_p2pool]");
                let mut pub_api_xmrig = pub_api_xmrig.lock().unwrap();
                debug!("Helper | Locked (15/17) ... [pub_api_xmrig]");
                let mut pub_api_xp = pub_api_xp.lock().unwrap();
                debug!("Helper | Locked (16/17) ... [pub_api_xp]");
                let mut pub_api_xvb = pub_api_xvb.lock().unwrap();
                debug!("Helper | Locked (17/17) ... [pub_api_xvb]");
                // Calculate Gupax's uptime always.
                lock.uptime = HumanTime::into_human(lock.instant.elapsed());
                // If [Node] is alive...
                if node.is_alive() {
                    debug!("Helper | Node is alive! Running [combine_gui_pub_api()]");
                    PubNodeApi::combine_gui_pub_api(&mut gui_api_node, &mut pub_api_node);
                } else {
                    debug!("Helper | Node is dead! Skipping...");
                }
                // If [P2Pool] is alive...
                if p2pool.is_alive() {
                    debug!("Helper | P2Pool is alive! Running [combine_gui_pub_api()]");
                    PubP2poolApi::combine_gui_pub_api(&mut gui_api_p2pool, &mut pub_api_p2pool);
                } else {
                    debug!("Helper | P2Pool is dead! Skipping...");
                }
                // If [XMRig] is alive...
                if xmrig.is_alive() {
                    debug!("Helper | XMRig is alive! Running [combine_gui_pub_api()]");
                    PubXmrigApi::combine_gui_pub_api(&mut gui_api_xmrig, &mut pub_api_xmrig);
                } else {
                    debug!("Helper | XMRig is dead! Skipping...");
                }
                // If [XMRig-Proxy] is alive...
                if xmrig_proxy.is_alive() {
                    debug!("Helper | XMRig-Proxy is alive! Running [combine_gui_pub_api()]");
                    PubXmrigProxyApi::combine_gui_pub_api(&mut gui_api_xp, &mut pub_api_xp);
                } else {
                    debug!("Helper | XMRig-Proxy is dead! Skipping...");
                }
                // If [XvB] is alive...
                if xvb.is_alive() {
                    debug!("Helper | XvB is alive! Running [combine_gui_pub_api()]");
                    PubXvbApi::combine_gui_pub_api(&mut gui_api_xvb, &mut pub_api_xvb);
                } else {
                    debug!("Helper | XvB is dead! Skipping...");
                }

                // 2. Selectively refresh [sysinfo] for only what we need (better performance).
                let mut sysinfo_lock = sysinfo.lock().unwrap();
                sysinfo_lock.refresh_cpu_specifics(sysinfo_cpu);
                debug!("Helper | Sysinfo refresh (1/3) ... [cpu]");
                sysinfo_lock.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::All,
                    false,
                    sysinfo_processes,
                );
                debug!("Helper | Sysinfo refresh (2/3) ... [processes]");
                sysinfo_lock.refresh_memory();
                debug!("Helper | Sysinfo refresh (3/3) ... [memory]");
                debug!("Helper | Sysinfo OK, running [update_pub_sys_from_sysinfo()]");
                Self::update_pub_sys_from_sysinfo(
                    &sysinfo_lock,
                    &mut lock_pub_sys,
                    &pid,
                    &lock,
                    max_threads,
                );
                drop(sysinfo_lock);

                // check for notifications

                // 3. Drop... (almost) EVERYTHING... IN REVERSE!
                drop(lock_pub_sys);
                debug!("Helper | Unlocking (1/17) ... [pub_sys]");
                drop(xvb);
                debug!("Helper | Unlocking (2/17) ... [xvb]");
                drop(xmrig_proxy);
                debug!("Helper | Unlocking (3/17) ... [xmrig_proxy]");
                drop(xmrig);
                debug!("Helper | Unlocking (4/17) ... [xmrig]");
                drop(p2pool);
                debug!("Helper | Unlocking (5/17) ... [p2pool]");
                drop(node);
                debug!("Helper | Unlocking (6/17) ... [node]");
                drop(pub_api_xvb);
                debug!("Helper | Unlocking (7/17) ... [pub_api_xvb]");
                drop(pub_api_xp);
                debug!("Helper | Unlocking (8/17) ... [pub_api_xp]");
                drop(pub_api_xmrig);
                debug!("Helper | Unlocking (9/17) ... [pub_api_xmrig]");
                drop(pub_api_p2pool);
                debug!("Helper | Unlocking (10/17) ... [pub_api_p2pool]");
                drop(pub_api_node);
                debug!("Helper | Unlocking (11/17) ... [pub_api_node]");
                drop(gui_api_xvb);
                debug!("Helper | Unlocking (12/17) ... [gui_api_xvb]");
                drop(gui_api_xp);
                debug!("Helper | Unlocking (13/17) ... [gui_api_xp]");
                drop(gui_api_xmrig);
                debug!("Helper | Unlocking (14/17) ... [gui_api_xmrig]");
                drop(gui_api_p2pool);
                debug!("Helper | Unlocking (15/17) ... [gui_api_p2pool]");
                drop(gui_api_node);
                debug!("Helper | Unlocking (16/17) ... [gui_api_node]");
                drop(lock);
                debug!("Helper | Unlocking (17/17) ... [helper]");

                // 4. Calculate if we should sleep or not.
                // If we should sleep, how long?
                let elapsed = start.elapsed().as_millis();
                if elapsed < 1000 {
                    // Casting from u128 to u64 should be safe here, because [elapsed]
                    // is less than 1000, meaning it can fit into a u64 easy.
                    let sleep = (1000 - elapsed) as u64;
                    debug!("Helper | END OF LOOP - Sleeping for [{sleep}]ms...");
                    sleep!(sleep);
                } else {
                    debug!("Helper | END OF LOOP - Not sleeping!");
                }

                // 5. End loop
            }
        });
    }
    pub fn spawn_ip_fetch(helper: &Arc<Mutex<Self>>) {
        thread::spawn(enc!((helper) move || {
            Self::ip_fetch(&helper);
        }));
    }
    #[tokio::main]
    async fn ip_fetch(helper: &Arc<Mutex<Self>>) {
        *helper.lock().unwrap().ip_public.lock().unwrap() = public_ip::addr_v4().await;
        *helper.lock().unwrap().ip_local.lock().unwrap() = local_ip_address::local_ip().ok();
    }
    pub fn spawn_proxy_port_reachable(helper: &Arc<Mutex<Self>>, port: u16) {
        thread::spawn(enc!((helper) move || {
            Self::proxy_port_reachable(&helper, port);
        }));
    }
    #[tokio::main]
    async fn proxy_port_reachable(helper: &Arc<Mutex<Self>>, port: u16) {
        let ip = helper.lock().unwrap().ip_public.lock().unwrap().to_owned();
        if let Some(ip) = ip {
            *helper.lock().unwrap().proxy_port_reachable.lock().unwrap() =
                is_port_reachable_with_timeout((ip, port), Duration::from_millis(500));
            return;
        } else {
            *helper.lock().unwrap().ip_public.lock().unwrap() = public_ip::addr_v4().await;
        }
    }
}

trait ApiOutput {
    fn output(&mut self) -> &mut String;
}

impl ApiOutput for PubNodeApi {
    fn output(&mut self) -> &mut String {
        &mut self.output
    }
}

impl ApiOutput for PubP2poolApi {
    fn output(&mut self) -> &mut String {
        &mut self.output
    }
}

impl ApiOutput for PubXmrigApi {
    fn output(&mut self) -> &mut String {
        &mut self.output
    }
}

impl ApiOutput for PubXmrigProxyApi {
    fn output(&mut self) -> &mut String {
        &mut self.output
    }
}

// common functions inside watchdog thread
fn check_died<A: ApiOutput>(
    child_pty: &mut impl Kill,
    output_end: &OutputEnd,
    process: &Mutex<Process>,
    start: &Instant,
    gui_api: &Mutex<A>,
    pub_api: &Mutex<A>,
) -> bool {
    // Check if the process secretly died without us knowing :)
    let Ok(Some(status)) = child_pty.try_wait() else {
        return false;
    };
    // With no lock held: the reader can need one to store the last lines.
    output_end.wait(OUTPUT_END_LIMIT);
    let mut process = process.lock().unwrap();
    debug!(
        "{} Watchdog | Process secretly died on us! Getting exit status...",
        process.name
    );
    show_exit(
        &mut process,
        start,
        Ok(status.success()),
        gui_api.lock().unwrap().output(),
        pub_api.lock().unwrap().output(),
    );
    debug!(
        "{} Watchdog | Secret dead process reap OK, breaking",
        process.name
    );
    true
}

const OUTPUT_END_LIMIT: Duration = Duration::from_secs(1);

// Its end comes from a read made after the process exited, so it holds every line the
// process wrote.
struct Output {
    inner: Box<dyn Read + Send>,
    exit: mpsc::Receiver<()>,
    exited: bool,
    // Dropped with the reader, once it stored every line, even one cut before its newline.
    _dropped: mpsc::Sender<()>,
}

impl Read for Output {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.inner.read(buf) {
                Ok(0) | Err(_) if self.exited => return Ok(0),
                // The macOS log file grows while XMRig runs.
                Ok(0) | Err(_) => {
                    self.exited = !matches!(
                        self.exit.recv_timeout(Duration::from_millis(100)),
                        Err(mpsc::RecvTimeoutError::Timeout)
                    )
                }
                read => return read,
            }
        }
    }
}

struct OutputEnd {
    exit: mpsc::Sender<()>,
    dropped: mpsc::Receiver<()>,
}

impl OutputEnd {
    // Once the process exited: returns when the reader dropped the output, or after `limit`.
    fn wait(&self, limit: Duration) {
        let _ = self.exit.send(());
        let _ = self.dropped.recv_timeout(limit);
    }
}

fn output(inner: Box<dyn Read + Send>) -> (Box<dyn Read + Send>, OutputEnd) {
    let (exit, exit_receiver) = mpsc::channel();
    let (dropped_sender, dropped) = mpsc::channel();
    let output = Output {
        inner,
        exit: exit_receiver,
        exited: false,
        _dropped: dropped_sender,
    };
    (Box::new(output), OutputEnd { exit, dropped })
}

// For a process that has no output to wait for.
fn ended_output() -> OutputEnd {
    output(Box::new(std::io::empty())).1
}

// Ends the console of a run. `exit` tells whether the process ended as expected.
fn show_exit(
    process: &mut Process,
    start: &Instant,
    exit: std::io::Result<bool>,
    gui_api_output_raw: &mut String,
    pub_api_output: &mut String,
) {
    let exit_status = match exit {
        Ok(true) => {
            process.state = ProcessState::Dead;
            "Successful"
        }
        Ok(false) => {
            process.state = ProcessState::Failed;
            "Failed"
        }
        Err(_) => {
            process.state = ProcessState::Failed;
            "Unknown Error"
        }
    };
    let uptime = Uptime::from(start.elapsed());
    info!(
        "{} | Stopped ... Uptime was: [{}], Exit status: [{}]",
        process.name, uptime, exit_status
    );
    // The Helper loop moves the last lines to the GUI only while the process is alive.
    gui_api_output_raw.push_str(&std::mem::take(pub_api_output));
    gui_api_output_raw.push_str(&std::mem::take(&mut *process.output_pub.lock().unwrap()));
    show_stopped(
        gui_api_output_raw,
        process.name,
        start.elapsed(),
        Some(exit_status),
    );
    // The thread of a restart starts the process again once it waits.
    match process.signal {
        ProcessSignal::Restart => process.state = ProcessState::Waiting,
        _ => process.signal = ProcessSignal::None,
    }
}

// The banner that ends the console of a run, framed by horizontal rules. Its uptime reads as in the
// Status tab, however the run ended.
fn show_stopped(
    gui_api_output_raw: &mut String,
    name: impl Display,
    uptime: Duration,
    exit_status: Option<&str>,
) {
    let uptime = HumanTime::into_human(uptime).display(false);
    let exit_status = exit_status.map_or(String::new(), |e| format!(" | Exit status: [{e}]"));
    if let Err(e) = writeln!(
        gui_api_output_raw,
        "{HORI_CONSOLE}\n{name} stopped | Uptime: [{uptime}]{exit_status}\n{HORI_CONSOLE}\n\n\n\n"
    ) {
        error!("{name} Watchdog | GUI Uptime/Exit status write failed: {e}");
    }
}

// The start of a watchdog, whose console tells why the process did not start. Its callers lock
// the process before the GUI API, as the Helper loop does.
fn started<T>(
    start: std::io::Result<T>,
    process: &mut Process,
    gui_api_output_raw: &mut String,
) -> Option<T> {
    let e = match start {
        Ok(started) => return Some(started),
        Err(e) => e,
    };
    error!("{} Watchdog | Could not start: {e}", process.name);
    process.state = ProcessState::Failed;
    process.signal = ProcessSignal::None;
    crate::miscs::output_console(
        gui_api_output_raw,
        &format!("{} could not start.\nError: {e}", process.name),
        process.name,
    );
    None
}

// Allow to check if a process outside of Gupax is still alive, without having a pty to it
// Used when using a detected local node instead of one started by Gupax
pub fn check_died_process(
    process: &mut Process,
    start: &Instant,
    gui_api_output_raw: &mut String,
    sys_info: &mut System,
) -> bool {
    if !process.name.is_process_running(sys_info) {
        process.state = ProcessState::Failed;
        debug!(
            "{} Watchdog | Process secretly died on us! can not get exit status...",
            process.name
        );
        let uptime = Uptime::from(start.elapsed());
        info!("{} | Stopped ... Uptime was: [{}]", process.name, uptime);
        show_stopped(gui_api_output_raw, process.name, start.elapsed(), None);
        process.signal = ProcessSignal::None;
        debug!(
            "{} Watchdog | Secret dead process reap OK, breaking",
            process.name
        );
        return true;
    }
    false
}

fn check_user_input(process: &Arc<Mutex<Process>>, stdin: &mut Box<dyn std::io::Write + Send>) {
    let mut lock = process.lock().unwrap();
    if !lock.input.is_empty() {
        let input = std::mem::take(&mut lock.input);
        for line in input {
            if line.is_empty() {
                continue;
            }
            debug!(
                "{} Watchdog | User input not empty, writing to STDIN: [{}]",
                lock.name, line
            );
            #[cfg(target_os = "windows")]
            if let Err(e) = write!(stdin, "{line}\r\n") {
                error!("{} Watchdog | STDIN error: {}", lock.name, e);
            }
            #[cfg(target_family = "unix")]
            if let Err(e) = writeln!(stdin, "{line}") {
                error!("{} Watchdog | STDIN error: {}", lock.name, e);
            }
            // Flush.
            if let Err(e) = stdin.flush() {
                error!("{} Watchdog | STDIN flush error: {}", lock.name, e);
            }
        }
    }
}
// A child that Gupax started.
trait Kill {
    // Tells whether it forced the exit, which then says nothing of how the process ran.
    fn kill(&mut self) -> std::io::Result<bool>;
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>>;
    fn wait(&mut self) -> std::io::Result<ExitStatus>;
}

impl Kill for Child {
    fn kill(&mut self) -> std::io::Result<bool> {
        Child::kill(self).map(|()| true)
    }
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        Child::try_wait(self)
    }
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        Child::wait(self)
    }
}

/// If the process is not started by Gupax, we use a pid kill instead of the terminal.
/// Won't work with xmrig as admin unless we resask for sudo but we don't manage an external xmrig miner, only possibly a local node.
fn signal_end<A: ApiOutput>(
    process_mutex: &Mutex<Process>,
    child_pty: Option<&mut impl Kill>,
    output_end: &OutputEnd,
    start: &Instant,
    gui_api: &Mutex<A>,
    pub_api: &Mutex<A>,
) -> bool {
    let mut process = process_mutex.lock().unwrap();
    // Restart are only for process started by Gupax
    let restart = process.signal == ProcessSignal::Restart && child_pty.is_some();
    if process.signal == ProcessSignal::Stop || restart {
        debug!(
            "{} Watchdog | {:?} SIGNAL caught",
            process.name, process.signal
        );
        // This actually sends a SIGHUP to p2pool (closes the PTY, hangs up on p2pool)
        let exit = if let Some(child_pty) = child_pty {
            let name = process.name;
            // The GUI reads the state while the process ends, which a restart has to keep.
            drop(process);
            let forced = child_pty
                .kill()
                .inspect_err(|e| error!("{name} Watchdog | Kill error: {e}"))
                .unwrap_or(false);
            // Wait to get the exit status
            let exit = child_pty.wait();
            output_end.wait(OUTPUT_END_LIMIT);
            process = process_mutex.lock().unwrap();
            exit.map(|status| forced || status.success())
        } else {
            // send the pid kill
            // https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html#method.kill_and_wait
            // find the process pid
            // kill and wait
            // it cost us almost 100ms, does a refresh and lock would cost less instead ?
            let s = System::new_all();
            Ok(
                s.processes_by_exact_name(process.name.binary_name().as_ref())
                    .last()
                    .is_some_and(|pid| pid.kill_and_wait().is_ok()),
            )
        };
        // This is written directly into the GUI API, because sometimes the 900ms event loop can't catch it.
        show_exit(
            &mut process,
            start,
            exit,
            gui_api.lock().unwrap().output(),
            pub_api.lock().unwrap().output(),
        );
        debug!(
            "{} Watchdog | {} SIGNAL done, breaking",
            process.name,
            if restart { "Restart" } else { "Stop" }
        );
        return true;
    }
    false
}
async fn sleep_end_loop(now: Instant, name: impl Display) {
    // Sleep (only if 999ms hasn't passed)
    let elapsed = now.elapsed().as_millis();
    // Since logic goes off if less than 1000, casting should be safe
    if elapsed < 1000 {
        let sleep = (1000 - elapsed) as u64;
        debug!("{name} Watchdog | END OF LOOP - Sleeping for [{sleep}]ms...");
        tokio::time::sleep(Duration::from_millis(sleep)).await;
    } else {
        debug!("{name} Watchdog | END OF LOOP - Not sleeping!");
    }
}

// Starts `cmd` with its input and output piped. Its stderr goes to the output too, where
// processes write why their arguments are wrong.
// Takes `cmd`, whose copy of the output writer would keep the output open once the process exited.
fn spawn_piped(
    mut cmd: std::process::Command,
) -> std::io::Result<(Child, std::io::PipeWriter, std::io::PipeReader)> {
    let (stdin_reader, stdin_writer) = std::io::pipe()?;
    let (stdout_reader, stdout_writer) = std::io::pipe()?;
    cmd.stderr(stdout_writer.try_clone()?);
    cmd.stdin(stdin_reader).stdout(stdout_writer);
    let child = cmd.spawn().map_err(|e| {
        let program = cmd.get_program().display();
        std::io::Error::new(e.kind(), format!("{program}: {e}"))
    })?;
    Ok((child, stdin_writer, stdout_reader))
}

// A binary at the root has no folder, and then fails to start with the reason.
fn run_in_folder_of(cmd: &mut std::process::Command, binary: &Path) {
    if let Some(folder) = binary.parent() {
        cmd.current_dir(folder);
    }
}

// The shell of the platform, as a program and the arguments that run its script.
#[cfg(all(test, windows))]
fn shell(windows: &str, _unix: &str) -> (PathBuf, Vec<String>) {
    let cmd = r"C:\Windows\System32\cmd.exe";
    (cmd.into(), vec!["/C".into(), windows.into()])
}

#[cfg(all(test, unix))]
fn shell(_windows: &str, unix: &str) -> (PathBuf, Vec<String>) {
    ("/bin/sh".into(), vec!["-c".into(), unix.into()])
}

#[cfg(test)]
fn shell_command(windows: &str, unix: &str) -> std::process::Command {
    let (program, args) = shell(windows, unix);
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    cmd
}

// A child that exited with `code`.
#[cfg(test)]
fn exited(code: i32) -> Child {
    let exit = format!("exit {code}");
    let mut child = shell_command(&exit, &exit).spawn().unwrap();
    child.wait().unwrap();
    child
}

// A child that runs until it is killed.
#[cfg(all(test, windows))]
fn sleeping() -> Child {
    std::process::Command::new(r"C:\Windows\System32\PING.EXE")
        .args(["-n", "10", "127.0.0.1"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

#[cfg(all(test, unix))]
fn sleeping() -> Child {
    std::process::Command::new("sleep")
        .arg("10")
        .spawn()
        .unwrap()
}

#[cfg(test)]
fn not_alive(name: ProcessName) -> Arc<Mutex<Process>> {
    arc_mut!(Process::new(name, String::new(), PathBuf::new()))
}

// Fails a test that would wait forever on the call.
#[cfg(test)]
fn within<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, returned) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(call());
    });
    returned
        .recv_timeout(Duration::from_secs(20))
        .expect("the call still waited after 20 s")
}
