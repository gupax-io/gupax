// Gupax - GUI Uniting P2Pool And XMRig
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

pub const GUPAX_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION")); // e.g: v1.0.0
pub const P2POOL_VERSION: &str = "v4.12";
pub const XMRIG_VERSION: &str = "v6.24.0";
pub const XMRIG_PROXY_VERSION: &str = "v6.24.0";
pub const NODE_VERSION: &str = "v18.4.4";
pub const COMMIT: &str = env!("COMMIT"); // set in build.rs
// e.g: Gupax_v1_0_0
// Would have been [Gupax_v1.0.0] but P2Pool truncates everything after [.]
pub const GUPAX_VERSION_UNDERSCORE: &str = concat!(
    "Gupax_v",
    env!("CARGO_PKG_VERSION_MAJOR"),
    "_",
    env!("CARGO_PKG_VERSION_MINOR"),
    "_",
    env!("CARGO_PKG_VERSION_PATCH"),
);

// App frame resolution, [4:3] aspect ratio, [1.33:1]
pub const APP_MIN_WIDTH: f32 = 640.0;
pub const APP_MIN_HEIGHT: f32 = 480.0;
pub const APP_MAX_WIDTH: f32 = 3840.0;
pub const APP_MAX_HEIGHT: f32 = 2160.0;
// Default, 1280x960
pub const APP_DEFAULT_WIDTH: f32 = 1280.0;
pub const APP_DEFAULT_HEIGHT: f32 = 960.0;
pub const APP_DEFAULT_CONSOLE_HEIGHT: u32 = 360;
// App resolution scaling
pub const APP_MIN_SCALE: f32 = 0.1;
pub const APP_MAX_SCALE: f32 = 2.0;
pub const APP_DEFAULT_SCALE: f32 = 1.0;

// Constants specific for Linux distro packaging of Gupax
#[cfg(feature = "distro")]
pub const DISTRO_NO_UPDATE: &str = r#"This [Gupax] was compiled for use as a Linux distro package. Built-in updates are disabled. The below settings [Update-via-Tor] & [Auto-Update] will not do anything. Please use your package manager to update [Gupax/P2Pool/XMRig]."#;

// Use macOS shaped icon for macOS
#[cfg(target_os = "macos")]
pub const BYTES_ICON: &[u8] = include_bytes!("../../assets/images/icons/icon@2x.png");
#[cfg(not(target_os = "macos"))]
pub const BYTES_ICON: &[u8] = include_bytes!("../../assets/images/icons/icon.png");
pub const BYTES_XVB: &[u8] = include_bytes!("../../assets/images/logos/xvb.png");
pub const BYTES_XMRIG: &[u8] = include_bytes!("../../assets/images/logos/xmrig.png");
pub const BYTES_MONERO: &[u8] = include_bytes!("../../assets/images/logos/monero.png");
pub const BYTES_P2POOL: &[u8] = include_bytes!("../../assets/images/logos/p2pool.png");
pub const BYTES_BANNER: &[u8] = include_bytes!("../../assets/images/banner.png");
pub const HORIZONTAL: &str = "--------------------------------------------";
pub const HORI_CONSOLE: &str = "---------------------------------------------------------------------------------------------------------------------------";

// Keyboard shortcuts
pub const KEYBOARD_SHORTCUTS: &str = r#"*---------------------------------------*
|             Key shortcuts             |
|---------------------------------------|
|             F11 | Fullscreen          |
|          Escape | Quit screen         |
|              Up | Start/Restart       |
|            Down | Stop                |
|               Z | Left Tab            |
|               X | Right Tab           |
|               C | Left Submenu        |
|               V | Right Submenu       |
|               S | Save                |
|               R | Reset               |
*---------------------------------------*"#;
// P2Pool & XMRig default API stuff
#[cfg(target_os = "windows")]
pub const P2POOL_API_PATH_LOCAL: &str = r"local\stratum";
#[cfg(target_os = "windows")]
pub const P2POOL_API_PATH_NETWORK: &str = r"network\stats";
#[cfg(target_os = "windows")]
pub const P2POOL_API_PATH_POOL: &str = r"pool\stats";
#[cfg(target_family = "windows")]
pub const P2POOL_API_PATH_P2P: &str = r"local\p2p";
#[cfg(target_family = "unix")]
pub const P2POOL_API_PATH_LOCAL: &str = "local/stratum";
#[cfg(target_family = "unix")]
pub const P2POOL_API_PATH_NETWORK: &str = "network/stats";
#[cfg(target_family = "unix")]
pub const P2POOL_API_PATH_POOL: &str = "pool/stats";
#[cfg(target_family = "unix")]
pub const P2POOL_API_PATH_P2P: &str = "local/p2p";
pub const XMRIG_API_SUMMARY_ENDPOINT: &str = "1/summary"; // The default relative URI of XMRig's API summary
pub const XMRIG_API_CONFIG_ENDPOINT: &str = "1/config"; // The default relative URI of XMRig's API config

// Process state tooltips (online, offline, etc)
pub const P2POOL_ALIVE: &str = "P2Pool is online and fully synchronized";
pub const P2POOL_DEAD: &str = "P2Pool is offline";
pub const P2POOL_FAILED: &str = "P2Pool is offline and failed when exiting";
pub const P2POOL_MIDDLE: &str = "P2Pool is in the middle of (re)starting/stopping";
pub const P2POOL_SYNCING: &str =
    "P2Pool is still syncing. This indicator will turn GREEN when P2Pool is ready";

pub const NODE_ALIVE: &str = "Node is online and fully synchronized";
pub const NODE_DEAD: &str = "Node is offline";
pub const NODE_FAILED: &str = "Node is offline and failed when exiting";
pub const NODE_MIDDLE: &str = "Node is in the middle of (re)starting/stopping";
pub const NODE_SYNCING: &str =
    "Node is still syncing. This indicator will turn GREEN when Node is ready";
pub const XMRIG_ALIVE: &str = "XMRig is online and mining";
pub const XMRIG_DEAD: &str = "XMRig is offline";
pub const XMRIG_FAILED: &str = "XMRig is offline and failed when exiting";
pub const XMRIG_MIDDLE: &str = "XMRig is in the middle of (re)starting/stopping";
pub const XMRIG_NOT_MINING: &str = "XMRig is online, but not mining to any pool";

pub const XMRIG_PROXY_ALIVE: &str = "XMRig-Proxy is online and mining";
pub const XMRIG_PROXY_DEAD: &str = "XMRig-Proxy is offline";
pub const XMRIG_PROXY_FAILED: &str = "XMRig-Proxy is offline and failed when exiting";
pub const XMRIG_PROXY_MIDDLE: &str = "XMRig-Proxy is in the middle of (re)starting/stopping";
pub const XMRIG_PROXY_NOT_MINING: &str = "XMRig-Proxy is online, but not mining to any pool";
pub const XMRIG_PROXY_REDIRECT: &str = "point local xmrig instance on this proxy instead of the p2pool instance (recommended if using XvB)";
pub const XMRIG_PROXY_INPUT: &str = "Send a command to XMRig-Proxy";
pub const XMRIG_PROXY_SIMPLE: &str = r#"Use simple XMRig-Proxy settings:
  - Mine to local P2Pool (localhost:3333)
  - redirect Xmrig local instance to the proxy
  - HTTP API @ localhost:18089"#;
pub const XMRIG_PROXY_ADVANCED: &str = r#"Use advanced XMRig-Proxy settings:
  - Terminal input
  - disable/enable local xmrig instance redirection
  - Overriding command arguments
  - Custom HTTP API IP/Port
  - TLS setting
  - Keepalive setting"#;
pub const XMRIG_PROXY_PATH_NOT_FILE: &str = "XMRig-Proxy binary not found at the given PATH in the Gupax tab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig-Proxy is located.";
pub const XMRIG_PROXY_PATH_NOT_VALID: &str = "XMRig-Proxy binary at the given PATH in the Gupax tab doesn't look like XMRig-Proxy! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig-Proxy is located.";
pub const XMRIG_PROXY_PATH_OK: &str = "XMRig-Proxy was found at the given PATH";
pub const XMRIG_PROXY_PATH_EMPTY: &str = "XMRig-Proxy PATH is empty! To fix: goto the [GupaxAdvanced] tab, select [Open] and specify where XMRig is located.";
pub const STATUS_XMRIG_PROXY_UPTIME: &str = "How long XMRig-Proxy has been online";
pub const STATUS_XMRIG_PROXY_POOL: &str = "The pool XMRig-Proxy is currently mining to";
pub const STATUS_XMRIG_PROXY_HASHRATE: &str = "The average hashrate of XMRig-Proxy";

pub const XVB_ALIVE: &str =
    "XvB process is configured and distributing hashrate, XvB pool is online";
pub const XVB_DEAD: &str = "XvB process is offline";
pub const XVB_FAILED: &str = "XvB process is misconfigured or the XvB pool is offline";
pub const XVB_MIDDLE: &str = "XvB is in the middle of (re)starting/stopping";
pub const XVB_NOT_CONFIGURED: &str = "You need to insert an existent token before starting XvB";
pub const XVB_PUBLIC_ONLY: &str = "XvB process is started only to get public stats.";
pub const XVB_SIDECHAIN: &str = "
If checked:\n
The algorithm will watch the estimated HR given for your address on the P2Pool network. This way, it will take into account external miners that are mining on P2Pool for your address without using the P2Pool node of Gupax. This estimation can be imprecised.\n
If unchecked (default):\n
The algorithm will watch the HR estimated by the stratum data of the p2pool node, which is more accurate but will only take into account the miners that are using your P2Pool node.
";
pub const XVB_MANUAL_POOL: &str = "Enable this to force the algorithm to connect to a specific XvB pool, without using the considered fastest";
pub const XVB_P2POOL_BUFFER: &str = "Set the % amount of additional HR to send to p2pool. Will reduce (if positive) or augment (if negative) the chances to miss the p2pool window.\n\n- In Auto or Hero mode, the algorithm will keep enough HR on the p2pool side to conform to the buffer\n\n- In Manual modes, the algorithm will ignore the p2pool buffer";

pub const START_OPTIONS_HOVER: &str = "Start the process with theses options.\nThe \"Reset to simple/advanced options\" are arguments constructed from the settings.\nYou can replace them with your own";
pub const NODE_START_OPTIONS_HINT: &str = "--zmq-pub tcp://<ip>:port --out-peers 32 --in-peers 64 --add-priority-node <ip>:<port> --disable-dns-checkpoints --enable-dns-blocklist --sync-pruned-blocks --prune-blockchain";
pub const P2POOL_START_OPTIONS_HINT: &str = "--wallet <primary address> --host <IP> --rpc-port <PORT> --zmq-port <PORT> --data-api <PATH> --local-api --no-color --mini --light-mode";
// also valid for xmrig-proxy
pub const XMRIG_START_OPTIONS_HINT: &str =
    "-o <IP:PORT> -t <THREADS> --user <RIG> --no-color --http-host <BIND> --http-port <PORT>";

// This is the typical space added when using
// [ui.separator()] or [ui.group()]
// Used for subtracting the width/height so
// things actually line up.
pub const SPACE: f32 = 10.0;

// Some colors
pub const RED: egui::Color32 = egui::Color32::from_rgb(230, 50, 50);
pub const GREEN: egui::Color32 = egui::Color32::from_rgb(100, 230, 100);
// pub const BLUE: egui::Color32 = egui::Color32::from_rgb(100, 175, 255);
pub const ORANGE: egui::Color32 = egui::Color32::from_rgb(255, 120, 40);
pub const YELLOW: egui::Color32 = egui::Color32::from_rgb(230, 230, 100);
pub const BRIGHT_YELLOW: egui::Color32 = egui::Color32::from_rgb(250, 250, 100);
pub const BONE: egui::Color32 = egui::Color32::from_rgb(190, 190, 190); // In between LIGHT_GRAY <-> GRAY
pub const GRAY: egui::Color32 = egui::Color32::GRAY;
pub const LIGHT_GRAY: egui::Color32 = egui::Color32::LIGHT_GRAY;
pub const BLACK: egui::Color32 = egui::Color32::BLACK;
pub const DARK_GRAY: egui::Color32 = egui::Color32::from_gray(13);

// IP fetching
pub const IP_NOT_FOUND: &str = "No ip found, try refreshing by clicking on the button above";

// [Duration] constants
pub const SECOND: std::time::Duration = std::time::Duration::from_secs(1);

// The explanation given to the user on why XMRig needs sudo.
pub const XMRIG_ADMIN_REASON: &str = r#"The large hashrate difference between XMRig and other miners like Monero and P2Pool's built-in miners is mostly due to XMRig configuring CPU MSRs and setting up hugepages. Other miners like Monero or P2Pool's built-in miner do not do this. It can be done manually but it isn't recommended since XMRig does this for you automatically, but only if it has the proper admin privileges."#;
// Password buttons
pub const PASSWORD_TEXT: &str = "Enter sudo/admin password...";
pub const PASSWORD_LEAVE: &str = "Return to the previous screen";
pub const PASSWORD_ENTER: &str = "Attempt with the current password";
pub const PASSWORD_HIDE: &str = "Toggle hiding/showing the password";

// OS specific
#[cfg(target_os = "windows")]
pub const OS: &str = " Windows";
#[cfg(target_os = "windows")]
pub const OS_NAME: &str = "Windows";
#[cfg(target_os = "windows")]
pub const WINDOWS_NOT_ADMIN: &str = "XMRig will most likely mine slower than normal without Administrator permissions. Please consider restarting Gupax as an Administrator.";

#[cfg(target_os = "macos")]
pub const OS: &str = " macOS";
#[cfg(target_os = "macos")]
pub const OS_NAME: &str = "macOS";

#[cfg(target_os = "linux")]
pub const OS: &str = "🐧 Linux";
#[cfg(target_os = "linux")]
pub const OS_NAME: &str = "Linux";

// Tooltips
// Status
pub const STATUS_GUPAX_UPTIME: &str = "How long Gupax has been online";
pub const STATUS_GUPAX_CPU_USAGE: &str =
    "How much CPU Gupax is currently using. This accounts for all your threads (it is out of 100%)";
pub const STATUS_GUPAX_MEMORY_USAGE: &str = "How much memory Gupax is currently using in Megabytes";
pub const STATUS_GUPAX_SYSTEM_CPU_USAGE: &str = "How much CPU your entire system is currently using. This accounts for all your threads (it is out of 100%)";
pub const STATUS_GUPAX_SYSTEM_MEMORY: &str =
    "How much memory your entire system has (including swap) and is currently using in Gigabytes";
pub const STATUS_GUPAX_SYSTEM_CPU_MODEL: &str =
    "The detected model of your system's CPU and its current frequency";
//--
pub const STATUS_P2POOL_UPTIME: &str = "How long P2Pool has been online";
pub const STATUS_P2POOL_PAYOUTS: &str = "The total amount of payouts received in this instance of P2Pool and an extrapolated estimate of how many you will receive. Warning: these stats will be quite inaccurate if your P2Pool hasn't been running for a long time!";
pub const STATUS_P2POOL_XMR: &str = "The total amount of XMR mined in this instance of P2Pool and an extrapolated estimate of how many you will mine in the future. Warning: these stats will be quite inaccurate if your P2Pool hasn't been running for a long time!";
pub const STATUS_P2POOL_HASHRATE: &str = "The total amount of hashrate your P2Pool has pointed at it in 15 minute, 1 hour, and 24 hour averages";
pub const STATUS_P2POOL_SHARES: &str = "The total amount of shares found on P2Pool";
pub const STATUS_P2POOL_CURRENT_SHARES: &str =
    "Current shares valid in the PPLNS Window for your address";
pub const STATUS_P2POOL_EFFORT: &str =
    "The average amount of effort needed to find a share, and the current effort";
pub const STATUS_P2POOL_CONNECTIONS: &str = "The total amount of miner connections on this P2Pool";
pub const STATUS_P2POOL_MONERO_NODE: &str = "The Monero node being used by P2Pool";
pub const STATUS_P2POOL_POOL: &str = "The P2Pool sidechain you're currently connected to";
pub const STATUS_P2POOL_ADDRESS: &str = "The Monero address P2Pool will send payouts to";
//--
pub const STATUS_XMRIG_UPTIME: &str = "How long XMRig has been online";
pub const STATUS_XMRIG_HASHRATE: &str = "The average hashrate of XMRig";
pub const STATUS_XMRIG_DIFFICULTY: &str = "The current difficulty of the job XMRig is working on";
pub const STATUS_XMRIG_SHARES: &str = "The amount of accepted and rejected shares";
pub const STATUS_XMRIG_POOL: &str = "The pool XMRig is currently mining to";
pub const STATUS_XMRIG_THREADS: &str = "The amount of threads XMRig is currently using";
pub const STATUS_PROXY_CONNECTIONS: &str = "The total amount of miner connections on this Proxy";
//--
pub const STATUS_XVB_TIME_REMAIN: &str = "Minutes left before end of round";
pub const STATUS_XVB_ROUND_TYPE: &str = "The current round type";
pub const STATUS_XVB_PLAYERS: &str =
    "Numbers of registered players and currently playing in the round";
pub const STATUS_XVB_DONATED_HR: &str = "Hashrate donated to the raffle";
pub const STATUS_XVB_WINNER: &str = "Current Raffle Winner";
pub const STATUS_XVB_SHARE: &str = "Share effort";
pub const STATUS_XVB_BLOCK_REWARD: &str = "Block reward";
pub const STATUS_XVB_YEARLY: &str = "Estimated Reward (Yearly)";
// Status Node
pub const STATUS_NODE_UPTIME: &str = "How long the Node has been online";
pub const STATUS_NODE_BLOCK_HEIGHT: &str = "The height of where the node is synchronized";
pub const STATUS_NODE_DIFFICULTY: &str = "current difficulty of the network";
pub const STATUS_NODE_DB_SIZE: &str = "Size of the database";
pub const STATUS_NODE_FREESPACE: &str = "Free space left on the partition storing the database";
pub const STATUS_NODE_NETTYPE: &str = "Type of network (mainnet, stagenet, testnet)";
pub const STATUS_NODE_OUT: &str = "Current number of active outbound connections";
pub const STATUS_NODE_IN: &str = "Current number of active incoming connections";
pub const STATUS_NODE_SYNC: &str = "Does the node is synchronized with the network ?";
pub const STATUS_NODE_STATUS: &str = "General status of the node";
// Status Submenus
pub const STATUS_SUBMENU_PROCESSES: &str =
    "View the status of process related data for [Gupax|P2Pool|XMRig]";
pub const STATUS_SUBMENU_P2POOL: &str = "View P2Pool specific data";
pub const STATUS_SUBMENU_HASHRATE: &str = "Compare your CPU hashrate with others";
//-- P2Pool
pub const STATUS_SUBMENU_PAYOUT: &str = "The total amount of payouts received via P2Pool across all time. This includes all payouts you have ever received using Gupax and P2Pool.";
pub const STATUS_SUBMENU_XMR: &str = "The total of XMR mined via P2Pool across all time. This includes all the XMR you have ever mined using Gupax and P2Pool.";
pub const STATUS_SUBMENU_LATEST: &str = "Sort the payouts from latest to oldest";
pub const STATUS_SUBMENU_OLDEST: &str = "Sort the payouts from oldest to latest";
pub const STATUS_SUBMENU_BIGGEST: &str = "Sort the payouts from biggest to smallest";
pub const STATUS_SUBMENU_SMALLEST: &str = "Sort the payouts from smallest to biggest";
pub const STATUS_SUBMENU_AUTOMATIC: &str =
    "Automatically calculate share/block time with your current P2Pool 1 hour average hashrate";
pub const STATUS_SUBMENU_MANUAL: &str = "Manually input a hashrate to calculate share/block time with current P2Pool/Monero network stats";
pub const STATUS_SUBMENU_HASH: &str = "Use [Hash] as the hashrate metric";
pub const STATUS_SUBMENU_KILO: &str = "Use [Kilo] as the hashrate metric (1,000x hash)";
pub const STATUS_SUBMENU_MEGA: &str = "Use [Mega] as the hashrate metric (1,000,000x hash)";
pub const STATUS_SUBMENU_GIGA: &str = "Use [Giga] as the hashrate metric (1,000,000,000x hash)";
pub const STATUS_SUBMENU_P2POOL_BLOCK_MEAN: &str =
    "The average time it takes for P2Pool to find a block";
pub const STATUS_SUBMENU_YOUR_P2POOL_HASHRATE: &str = "Your 1 hour average hashrate on P2Pool";
pub const STATUS_SUBMENU_P2POOL_SHARE_MEAN: &str =
    "The average time it takes for your hashrate to find a share on P2Pool";
pub const STATUS_SUBMENU_SOLO_BLOCK_MEAN: &str =
    "The average time it would take for your hashrate to find a block solo mining Monero";
pub const STATUS_SUBMENU_MONERO_DIFFICULTY: &str = "The current Monero network's difficulty (how many hashes it will take on average to find a block)";
pub const STATUS_SUBMENU_MONERO_HASHRATE: &str = "The current Monero network's hashrate";
pub const STATUS_SUBMENU_P2POOL_DIFFICULTY: &str = "The current P2Pool network's difficulty (how many hashes it will take on average to find a share)";
pub const STATUS_SUBMENU_P2POOL_HASHRATE: &str = "The current P2Pool network's hashrate";
pub const STATUS_SUBMENU_P2POOL_MINERS: &str = "The current amount of miners on P2Pool";
pub const STATUS_SUBMENU_P2POOL_DOMINANCE: &str =
    "The percent of hashrate P2Pool accounts for in the entire Monero network";
pub const STATUS_SUBMENU_YOUR_P2POOL_DOMINANCE: &str =
    "The percent of hashrate you account for in P2Pool";
pub const STATUS_SUBMENU_YOUR_MONERO_DOMINANCE: &str =
    "The percent of hashrate you account for in the entire Monero network";
//-- Benchmarks
pub const STATUS_SUBMENU_YOUR_CPU: &str = "The CPU detected by Gupax";
pub const STATUS_SUBMENU_YOUR_BENCHMARKS: &str =
    "How many benchmarks your CPU has had uploaded to [https://xmrig.com/benchmark] ";
pub const STATUS_SUBMENU_YOUR_RANK: &str =
    "Your CPU's rank out of all CPUs listed on [https://xmrig.com/benchmark] (higher is better)";
pub const STATUS_SUBMENU_YOUR_HIGH: &str =
    "The highest hashrate recorded for your CPU on [https://xmrig.com/benchmark]";
pub const STATUS_SUBMENU_YOUR_AVERAGE: &str =
    "The average hashrate of your CPU based off the data at [https://xmrig.com/benchmark]";
pub const STATUS_SUBMENU_YOUR_LOW: &str =
    "The lowest hashrate recorded for your CPU on [https://xmrig.com/benchmark]";
pub const STATUS_SUBMENU_OTHER_CPUS: &str = "A list of ALL the recorded CPU benchmarks. The CPUs most similar to yours are listed first. All this data is taken from [https://xmrig.com/benchmark].";
pub const STATUS_SUBMENU_OTHER_CPU: &str = "The CPU name";
pub const STATUS_SUBMENU_OTHER_RELATIVE: &str = "The relative hashrate power compared to the fastest recorded CPU, which is current: [AMD EPYC 7T83 64-Core Processor]";
pub const STATUS_SUBMENU_OTHER_HIGH: &str = "Highest hashrate record";
pub const STATUS_SUBMENU_OTHER_AVERAGE: &str = "Average hashrate";
pub const STATUS_SUBMENU_OTHER_LOW: &str = "Lowest hashrate record";
pub const STATUS_SUBMENU_OTHER_RANK: &str = "The rank of this CPU out of [1567] (lower is better)";
pub const STATUS_SUBMENU_OTHER_BENCHMARKS: &str =
    "How many benchmarks this CPU has had posted to [https://xmrig.com/benchmark]";

// Gupax
pub const GUPAX_UPDATE: &str = "Check for updates on Gupax and bundled versions of P2Pool and XMRig via GitHub's API and upgrade automatically";
pub const GUPAX_AUTO_UPDATE: &str = "Automatically check for updates at startup";
pub const GUPAX_AUTO_CRAWL: &str = "Start the P2Pool compatible Nodes Finder at startup.\nIt will crawl the monero network to find nodes if the ones already found are not online";
pub const GUPAX_BUNDLED_UPDATE: &str = "Update XMRig and P2Pool with bundled versions of latest Gupax. It will replace any present xmrig and p2pool binary in their specified path.";
pub const GUPAX_SHOULD_RESTART: &str =
    "Gupax was updated. A restart is recommended but not required";
// #[cfg(not(target_os = "macos"))]
// pub const GUPAX_UPDATE_VIA_TOR:   &str = "Update through the Tor network. Tor is embedded within Gupax; a Tor system proxy is not required";
// #[cfg(target_os = "macos")] // Arti library has issues on macOS
// pub const GUPAX_UPDATE_VIA_TOR:   &str = "WARNING: This option is unstable on macOS. Update through the Tor network. Tor is embedded within Gupax; a Tor system proxy is not required";
pub const GUPAX_ASK_BEFORE_QUIT: &str = "Ask before quitting Gupax";
pub const GUPAX_SAVE_BEFORE_QUIT: &str = "Automatically save any changed settings before quitting";
pub const GUPAX_AUTO_P2POOL: &str = "Automatically start P2Pool on Gupax startup. If you are using [P2Pool Simple], this will NOT wait for your [Auto-Ping] to finish, it will start P2Pool on the pool you already have selected. This option will fail if your P2Pool settings aren't valid!";
pub const GUPAX_AUTO_NODE: &str = "Automatically start Node on Gupax startup. This option will fail if your P2Pool settings aren't valid!";
pub const GUPAX_AUTO_XMRIG: &str = "Automatically start XMRig on Gupax startup. This option will fail if your XMRig settings aren't valid!";
pub const GUPAX_AUTO_XMRIG_PROXY: &str = "Automatically start XMRig-Proxy on Gupax startup.";
pub const GUPAX_AUTO_XVB: &str = "Automatically start XvB on Gupax startup. This option will fail if your XvB settings aren't valid!";
pub const GUPAX_ADJUST: &str = "Adjust and set the width/height of the Gupax window";
pub const GUPAX_WIDTH: &str = "Set the width of the Gupax window";
pub const GUPAX_HEIGHT: &str = "Set the height of the Gupax window";
pub const GUPAX_SCALE: &str =
    "Set the resolution scaling of the Gupax window (resize window to re-apply scaling)";
pub const GUPAX_LOCK_WIDTH: &str =
    "Automatically match the HEIGHT against the WIDTH in a 4:3 ratio";
pub const GUPAX_LOCK_HEIGHT: &str =
    "Automatically match the WIDTH against the HEIGHT in a 4:3 ratio";
pub const GUPAX_NO_LOCK: &str = "Allow individual selection of width and height";
pub const GUPAX_SET: &str = "Set the width/height of the Gupax window to the current values";
pub const GUPAX_TAB: &str = "Set the default tab Gupax starts on";
pub const GUPAX_TAB_ABOUT: &str = "Set the tab Gupax starts on to: About";
pub const GUPAX_TAB_STATUS: &str = "Set the tab Gupax starts on to: Status";
pub const GUPAX_TAB_GUPAX: &str = "Set the tab Gupax starts on to: Gupax";
pub const GUPAX_TAB_P2POOL: &str = "Set the tab Gupax starts on to: P2Pool";
pub const GUPAX_TAB_XMRIG: &str = "Set the tab Gupax starts on to: XMRig";
pub const GUPAX_TAB_XMRIG_PROXY: &str = "Set the tab Gupax starts on to: Proxy";
pub const GUPAX_TAB_XVB: &str = "Set the tab Gupax starts on to: XvB";
pub const GUPAX_TAB_NODE: &str = "Set the default tab Gupax starts on to: Node";

pub const GUPAX_SIMPLE: &str = r#"Use simple Gupax settings:
  - Update button
  - Basic toggles"#;
pub const GUPAX_ADVANCED: &str = r#"Use advanced Gupax settings:
  - Update button
  - Basic toggles
  - P2Pool/XMRig binary path selector
  - Gupax resolution sliders
  - Gupax start-up tab selector"#;
pub const GUPAX_SELECT: &str = "Open a file explorer to select a file";
pub const GUPAX_PATH_P2POOL: &str = "The location of the P2Pool binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no file found at the given path";
pub const GUPAX_PATH_XMRIG: &str = "The location of the XMRig binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no file found at the given path";
pub const GUPAX_PATH_XMRIG_PROXY: &str = "The location of the XMRig-Proxy binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no file found at the given path";

// P2Pool
pub const P2POOL_USE_LOCAL_NODE_BUTTON: &str = "Start with a local node";
pub const P2POOL_PORT_DEFAULT: u16 = 3333;
pub const P2POOL_MAIN: &str = "Use the P2Pool main-chain. This P2Pool finds blocks faster, but has a higher difficulty. Suitable for miners with more than 100kH/s";
pub const P2POOL_MINI: &str = "Use the P2Pool mini-chain. This P2Pool finds blocks slower, but has a lower difficulty. Suitable for miners with less than 100kH/s";
pub const P2POOL_NANO: &str = "Use the P2Pool nano-chain. This P2Pool finds blocks slower, but has a lower difficulty. Suitable for miners with less than 30kH/s";
pub const P2POOL_OUT: &str = "How many out-bound peers to connect to? (you connecting to others)";
pub const P2POOL_IN: &str = "How many in-bound peers to allow? (others connecting to you)";
pub const P2POOL_LOG: &str = "Verbosity of the console log.\nA verbosity level more than 0 is recommended to let the P2Pool process detect more rapidly errors with the Monero Node.\nIf the level is at 0, it can take up to 2 minutes to detect an error.";
pub const P2POOL_AUTO_NODE: &str = "Automatically ping the remote Monero nodes at Gupax startup";
// pub const P2POOL_AUTO_SELECT: &str =
// "Automatically select the fastest remote Monero node after pinging";
pub const P2POOL_BACKUP_HOST_SIMPLE: &str = r#"Automatically switch to the other nodes listed if the current one is down.

Note: you must ping the remote nodes or this feature will default to only using the currently selected node."#;
pub const P2POOL_BACKUP_HOST_ADVANCED: &str =
    "Automatically switch to the other nodes in your list if the current one is down.";
pub const P2POOL_AUTOSWITCH_LOCAL_NODE: &str =
    "Automatically switch to the local node when it will be ready to be used.";
pub const P2POOL_SELECT_FASTEST: &str = "Select the fastest remote Monero node";
pub const P2POOL_SELECT_RANDOM: &str = "Select a random remote Monero node";
pub const P2POOL_SELECT_LAST: &str = "Select the previous remote Monero node";
pub const P2POOL_SELECT_NEXT: &str = "Select the next remote Monero node";
pub const P2POOL_PING: &str = "Ping the built-in remote Monero nodes";
pub const P2POOL_ADDRESS: &str = "You must use a primary Monero address to mine on P2Pool (starts with a 4). It is highly recommended to create a new wallet since addresses are public on P2Pool!";
pub const P2POOL_COMMUNITY_NODE_WARNING: &str = r#"TL;DR: Run & use your own Monero Node.

Using a Remote Monero Node is convenient but comes at the cost of privacy and reliability.

You may encounter connection issues with remote nodes which may cause mining performance loss! Late info from laggy nodes will cause your mining jobs to start later than they should.

Running and using your own local Monero node improves privacy and ensures your connection is as stable as your own internet connection. This comes at the cost of downloading and syncing Monero's blockchain yourself (currently about 100GB for pruned nodes). If you have the disk space, consider using the [Node] tab and start the process."#;

pub const P2POOL_INPUT: &str = "Send a command to P2Pool";

pub const P2POOL_SIMPLE: &str = r#"Use simple P2Pool settings:
  - Default P2Pool settings + Nano
  - Use Remote/local Monero node
  - Find a remote node"#;
pub const P2POOL_ADVANCED: &str = r#"Use advanced P2Pool settings:
  - Terminal input
  - Overriding command arguments
  - Manual node list
  - P2Pool Main/Mini/Nano selection
  - Out/In peer setting
  - Log level setting
  - Backup host setting"#;
pub const P2POOL_CRAWLER: &str = r#"Set crawler P2Pool settings:
  - selection of found nodes
  - adjust crawler parameters"#;
pub const P2POOL_NAME: &str = "Add a unique name to identify this node; Only [A-Za-z0-9-_.] and spaces allowed; Max length = 30 characters";
pub const P2POOL_NODE_IP: &str = "Specify the Monero Node IP to connect to with P2Pool; It must be a valid IPv4 address or a valid domain name; Max length = 255 characters";
pub const P2POOL_RPC_PORT: &str = "Specify the RPC port of the Monero node; [1-65535]";
pub const P2POOL_ZMQ_PORT: &str = "Specify the ZMQ port of the Monero node; [1-65535]";
pub const P2POOL_PATH_NOT_FILE: &str = "P2Pool binary not found at the given PATH in the Gupax tab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where P2Pool is located.";
pub const P2POOL_PATH_NOT_VALID: &str = "P2Pool binary at the given PATH in the Gupax tab doesn't look like P2Pool! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where P2Pool is located.";
pub const P2POOL_PATH_OK: &str = "P2Pool was found at the given PATH";
pub const P2POOL_PATH_EMPTY: &str = "P2Pool PATH is empty! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where P2Pool is located.";
pub const P2POOL_URL: &str = "https://github.com/SChernykh/p2pool";

pub const CRAWLER_PARAMETERS_HELP: &str = "You can define parameters for the crawling. Depending on the value, it will make the crawling faster or slower to reach the requirements";
// Node/Pool list
pub const LIST_ADD: &str = "Add the current values to the list";
pub const LIST_SAVE: &str = "Save the current values to the already existing entry";
pub const LIST_DELETE: &str = "Delete the currently selected entry";
pub const LIST_CLEAR: &str = "Clear all current values";
// Node
pub const NODE_RPC_PORT_DEFAULT: u16 = 18081;
pub const NODE_ZMQ_PORT_DEFAULT: u16 = 18083;
pub const NODE_INPUT: &str = "Send a command to Node";
pub const NODE_PRUNNING: &str = "Reduce the database size to a third. Does not have any security/privacy impact.If you have enough storage, a full node is preferable to make the network even more decentralized.";
pub const NODE_START_DETECT_VALID: &str = "A monero Node has been detected running on your system.\n\nGupax can not start a Node if there is already one running on the same system.\nThis is a Node that can be used by Gupax for P2Pool.\n\nDo you want to use the already running Node ? You will have a limited control from Gupax.";
pub const NODE_START_DETECT_NON_VALID: &str = "A monero Node has been detected running on your system.\n\nGupax can not start a Node if there is already one running on the same system.\nThis is a Node that can not be used by Gupax for P2Pool.\n\nYou will not be able to use P2Pool or start a Node from Gupax while this node is running.";
#[cfg(not(windows))]
pub const NODE_DB_PATH_EMPTY: &str =
    "If the PATH of the DB is empty, the default ~/.bitmonero will be used.";
#[cfg(windows)]
pub const NODE_DB_PATH_EMPTY: &str =
    r#"If the PATH of the DB is empty, the default C:\ProgramData\bitmonero will be used."#;
pub const NODE_DB_DIR: &str = "The DB path needs to be a correct path to a directory if not empty";
pub const NODE_SIMPLE: &str = r#"Use simple Node settings:
  - Default Node settings"#;
pub const NODE_ADVANCED: &str = r#"Use advanced Node settings:
  - Prunning
  - Custom path for database
  - Terminal input
  - Overriding command arguments
  - Manual zmq port
  - Out/In peer setting
  - Log level setting
  - Disable DNS checkpoint
  - DNS blocking"#;
pub const GUPAX_PATH_NODE: &str = "The location of the Node binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no directory found at the given path";
pub const NODE_PATH_OK: &str = "PATH for DB is valid.";
pub const NODE_PATH_NOT_FILE: &str = "Node binary not found at the given PATH in the Gupax tab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where NODE is located.";
pub const NODE_PATH_NOT_VALID: &str = "Node binary at the given PATH in the Gupax tab doesn't look like Node! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where Node is located.";
pub const NODE_PATH_EMPTY: &str = "Node PATH is empty! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where Node is located.";
pub const NODE_URL: &str = "https://github.com/monero-project/monero";
pub const NODE_DNS_BLOCKLIST: &str =
    "Apply realtime blocklist from DNS to ban known malicious nodes. (recommended)";
pub const NODE_DNS_CHECKPOINT: &str =
    "Do not retrieve checkpoints from DNS to prevent periodic lags (recommended)";
pub const NODE_API_BIND: &str = "bind address of RPC API";
pub const NODE_API_PORT: &str = "RPC API listen port";
pub const NODE_ZMQ_BIND: &str = "bind address of ZMQ API";
pub const NODE_ZMQ_PORT: &str = "ZMQ API listen port";
pub const NODE_FULL_MEM: &str = "Use 2GB of RAM instead of 256MB for faster block verification";
// XMRig
pub const XMRIG_API_PORT_DEFAULT: u16 = 18088;
pub const XMRIG_SIMPLE: &str = r#"Use simple XMRig settings:
  - Mine to local P2Pool (localhost:3333)
  - CPU thread slider
  - HTTP API @ localhost:18088"#;
pub const XMRIG_ADVANCED: &str = r#"Use advanced XMRig settings:
  - Terminal input
  - Overriding command arguments
  - Custom payout address
  - CPU thread slider
  - Manual pool list
  - Custom HTTP API IP/Port
  - TLS setting
  - Keepalive setting"#;
pub const XMRIG_INPUT: &str = "Send a command to XMRig";

pub const XMRIG_ADDRESS: &str = "Specify which Monero address to payout to. This does nothing if mining to P2Pool since the address being paid out to will be the one P2Pool started with. This doubles as a rig identifier for P2Pool and some pools.";
pub const XMRIG_NAME: &str = "Add a unique name to identify this pool; Only [A-Za-z0-9-_.] and spaces allowed; Max length = 30 characters";
pub const XMRIG_IP: &str = "Specify the pool IP to connect to with XMRig; It must be a valid IPv4 address or a valid domain name; Max length = 255 characters";
pub const XMRIG_PORT: &str = "Specify the port of the pool; [1-65535]";
pub const XMRIG_RIG: &str = "Add an optional rig ID. This will be the name shown on the pool; Only [A-Za-z0-9-_] and spaces allowed; Max length = 30 characters";
pub const XMRIG_URL: &str = "https://github.com/xmrig/xmrig";
#[cfg(not(target_os = "linux"))]
pub const XMRIG_PAUSE: &str =
    "THIS SETTING IS DISABLED IF SET TO [0]. Pause mining if user is active, resume after";
pub const XMRIG_API_IP: &str =
    "Specify which IP to bind to for XMRig's HTTP API; If empty: [localhost/127.0.0.1]";
pub const XMRIG_API_PORT: &str =
    "Specify which port to bind to for XMRig's HTTP API; If empty: [18088]";
pub const XMRIG_API_TOKEN: &str = "Specify the token to authenticate on the HTTP API";
pub const XMRIG_TLS: &str = "Enable SSL/TLS connections (needs pool support)";
pub const XMRIG_KEEPALIVE: &str = "Send keepalive packets to prevent timeout (needs pool support)";
pub const XMRIG_THREADS: &str = "Number of CPU threads to use for mining";
pub const XMRIG_PATH_NOT_FILE: &str = "XMRig binary not found at the given PATH in the Gupaxtab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig is located.";
pub const XMRIG_PATH_NOT_VALID: &str = "XMRig binary at the given PATH in the Gupaxtab doesn't look like XMRig! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig is located.";
pub const XMRIG_PATH_OK: &str = "XMRig was found at the given PATH";
pub const XMRIG_PATH_EMPTY: &str = "XMRig PATH is empty! To fix: goto the [GupaxAdvanced] tab, select [Open] and specify where XMRig is located.";
pub const XMRIG_PROXY_URL: &str = "https://github.com/xmrig/xmrig-proxy";
pub const PROXY_API_PORT_DEFAULT: u16 = 18089;
pub const PROXY_PORT_DEFAULT: u16 = 3355;

// XvB
pub const XVB_MANUAL_SLIDER_MANUAL_XVB_HELP: &str = "Set the hashrate amount to donate to XvB manually, The remaining hashrate will be sent to p2pool. If the selected hashrate is more than your xmrig hashrate it will be overwritten";
pub const XVB_MANUAL_SLIDER_MANUAL_P2POOL_HELP: &str = "Set the hashrate amount to keep on p2pool manually, The remaining hasrate will be donated to xvb. If the selected hashrate is more than your xmrig hashrate it will be overwritten ";
pub const XVB_URL: &str = "https://xmrvsbeast.com";
// Simple/Advanced Submenu hover text
pub const XVB_SIMPLE: &str = r#"Use simple XvB settings:
  - Auto mode by default
  - Hero mode available"#;
pub const XVB_ADVANCED: &str = r#"Use advanced XvB settings:
  - Selection of mode: 
      Auto,
      Hero,
      Manual XvB,
      Manual P2pool,
      Round
  - P2Pool Buffer"#;
pub const XVB_URL_PUBLIC_API: &str = "https://xmrvsbeast.com/p2pool/stats";
pub const XVB_NODE_PORT: &str = "4247";
pub const XVB_NODE_EU: &str = "eu.xmrvsbeast.com";
pub const XVB_NODE_NA: &str = "na.xmrvsbeast.com";
pub const XVB_URL_RULES: &str = "https://xmrvsbeast.com/p2pool/rules.html";
// buffer in percentage of HR to have plus the requirement.
pub const XVB_SIDE_MARGIN_1H: f32 = 0.2;
// time is in ms
pub const XVB_TIME_ALGO: u64 = 60_000;
// minimum time to send to XvB if any
pub const XVB_MIN_TIME_SEND: u64 = 50;
pub const XVB_HERO_SELECT: &str = "Donate as much as possible while keeping a share on p2pool, increases the odds of your round winning\nWhen modified, the algorithm will use the new choice at the next decision.";
pub const XVB_FAILURE_FIELD: &str = "Failures";
pub const XVB_DONATED_1H_FIELD: &str = "Donated last hour";
pub const XVB_DONATED_24H_FIELD: &str = "Donated last 24 hours";
pub const XVB_ROUND_TYPE_FIELD: &str = "Round";
pub const XVB_WINNER_FIELD: &str = "Win";
pub const XVB_MINING_ON_FIELD: &str = "Currently Mining on";

pub const XVB_ROUND_DONOR_MIN_HR: u32 = 1000;
pub const XVB_ROUND_DONOR_VIP_MIN_HR: u32 = 10000;
pub const XVB_ROUND_DONOR_WHALE_MIN_HR: u32 = 100000;
pub const XVB_ROUND_DONOR_MEGA_MIN_HR: u32 = 1000000;

pub const SOCKET_MONERO_LOCAL_OUTSIDE: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 18080));
// Common help
pub const HELP_STRATUM_PORT: &str = "Specify the stratum port to bind to";
pub const HELP_STRATUM_IP: &str = "Specify the stratum ip to bind to";
// Manual Mode
pub const XVB_MODE_MANUAL_XVB_HELP: &str = "Manually set the amount to donate to XmrVsBeast, If value is more than xmrig hashrate it might be changed";
pub const XVB_MODE_MANUAL_P2POOL_HELP: &str = "Manually set the amount to keep on P2pool, If value is more than xmrig hashrate it might be changed";
pub const XVB_MODE_MANUAL_DONATION_LEVEL_HELP: &str = "Manually set the XvB donation level";

// Manual Donation Levels
pub const XVB_DONATION_LEVEL_DONOR_HELP: &str =
    "To qualify at least 1 kH/s will be actively donated (1hr and 24hr avg.)";
pub const XVB_DONATION_LEVEL_VIP_DONOR_HELP: &str =
    "To qualify at least 10 kH/s will be actively donated (1hr and 24hr avg.)";
pub const XVB_DONATION_LEVEL_WHALE_DONOR_HELP: &str =
    "To qualify at least 100 kH/s will be actively donated (1hr and 24hr avg.)";
pub const XVB_DONATION_LEVEL_MEGA_DONOR_HELP: &str =
    "To qualify at least 1000 kH/s will be actively donated (1hr and 24hr avg.)";

// Unknown Data, replace HumanNumlber::unknown()
pub const UNKNOWN_DATA: &str = "???";
// MAIN chain PPLNS window has a dynamic number of block based on the frequency and size of payouts
pub const BLOCK_PPLNS_WINDOW_MAIN_MAX: u64 = 2160;
pub const BLOCK_PPLNS_WINDOW_MINI: u64 = 2160;
pub const BLOCK_PPLNS_WINDOW_NANO: u64 = 2160;
pub const SECOND_PER_BLOCK_P2POOL_MAIN: u64 = 10;
pub const SECOND_PER_BLOCK_P2POOL_MINI: u64 = 10;
pub const SECOND_PER_BLOCK_P2POOL_NANO: u64 = 30;
// Time PPLNS WINDOW in seconds
// it is an estimation based on number of block in a pplns window and block time (10s). The difficulty of the network should adapt to get close to this value.
// pub const TIME_PPLNS_WINDOW_MINI: Duration = Duration::from_secs(BLOCK_PPLNS_WINDOW_MINI * SECOND_PER_BLOCK_P2POOL);
// pub const TIME_PPLNS_WINDOW_MAIN: Duration = Duration::from_secs(BLOCK_PPLNS_WINDOW_MAIN * SECOND_PER_BLOCK_P2POOL);
pub const PROCESS_OUTSIDE: &str =
    "This process is running outside of Gupax.\nYou need to stop it before starting it in Gupax.";
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

//---------------------------------------------------------------------------------------------------- Visuals
use egui::epaint::{Shadow, Stroke};

use egui::{Color32, CornerRadius, Visuals};

use egui::style::{Selection, WidgetVisuals, Widgets};
use once_cell::sync::Lazy;

pub const ACCENT_COLOR: Color32 = Color32::from_rgb(200, 100, 100);
pub const BG: Color32 = Color32::from_gray(20);

// This is based off [`Visuals::dark()`].
pub static VISUALS_GUPAX_DARK: Lazy<Visuals> = Lazy::new(|| {
    let selection = Selection {
        bg_fill: ACCENT_COLOR,
        stroke: Stroke::new(1.0, Color32::from_gray(255)),
    };

    // Based off default dark() mode.
    // https://docs.rs/egui/0.24.1/src/egui/style.rs.html#1210
    let widgets = Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: BG,
            bg_stroke: Stroke::new(1.0, Color32::from_gray(60)), // separators, indentation lines
            fg_stroke: Stroke::new(1.0, Color32::from_gray(140)), // normal text color
            corner_radius: CornerRadius::same(10),
            expansion: 0.0,
            weak_bg_fill: BG,
        },
        inactive: WidgetVisuals {
            bg_fill: Color32::from_gray(50),
            bg_stroke: Default::default(),
            fg_stroke: Stroke::new(1.0, Color32::from_gray(180)), // button text
            corner_radius: CornerRadius::same(10),
            expansion: 0.0,
            weak_bg_fill: Color32::from_gray(50),
        },
        hovered: WidgetVisuals {
            bg_fill: Color32::from_gray(80),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(150)), // e.g. hover over window edge or button
            fg_stroke: Stroke::new(1.5, Color32::from_gray(240)),
            corner_radius: CornerRadius::same(10),
            expansion: 1.0,
            weak_bg_fill: Color32::from_gray(80),
        },
        active: WidgetVisuals {
            bg_fill: Color32::from_gray(55),
            bg_stroke: Stroke::new(1.0, Color32::WHITE),
            fg_stroke: Stroke::new(2.0, Color32::WHITE),
            corner_radius: CornerRadius::same(10),
            expansion: 1.0,
            weak_bg_fill: Color32::from_gray(120),
        },
        open: WidgetVisuals {
            bg_fill: Color32::from_gray(27),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(60)),
            fg_stroke: Stroke::new(1.0, Color32::from_gray(210)),
            corner_radius: CornerRadius::same(10),
            expansion: 0.0,
            weak_bg_fill: Color32::from_gray(120),
        },
    };

    // https://docs.rs/egui/0.24.1/src/egui/style.rs.html#1113
    Visuals {
        widgets,
        selection,
        hyperlink_color: Color32::from_rgb(90, 170, 255),
        faint_bg_color: Color32::from_additive_luminance(5), // visible, but barely so
        extreme_bg_color: Color32::from_gray(10),            // e.g. TextEdit background
        code_bg_color: Color32::from_gray(64),
        warn_fg_color: Color32::from_rgb(255, 143, 0), // orange
        error_fg_color: Color32::from_rgb(255, 0, 0),  // red
        window_corner_radius: CornerRadius::same(6),
        window_shadow: Shadow::NONE,
        popup_shadow: Shadow::NONE,
        override_text_color: Some(BONE),

        ..Visuals::dark()
    }
});

// Light mode version of [`Visuals::dark()`] → based on `Visuals::light()`
pub static VISUALS_GUPAX_LIGHT: Lazy<Visuals> = Lazy::new(|| {
    let selection = Selection {
        bg_fill: ACCENT_COLOR, // keep accent the same
        stroke: Stroke::new(1.0, Color32::from_gray(200)),
    };

    // Adapted from default light() in egui 0.24.1
    let widgets = Widgets {
        noninteractive: WidgetVisuals {
            bg_fill: Color32::WHITE,
            bg_stroke: Stroke::new(1.0, Color32::from_gray(200)), // light separators
            fg_stroke: Stroke::new(1.0, Color32::from_gray(80)),  // normal text
            corner_radius: CornerRadius::same(10),
            expansion: 0.0,
            weak_bg_fill: Color32::WHITE,
        },
        inactive: WidgetVisuals {
            bg_fill: Color32::from_gray(240),
            bg_stroke: Default::default(),
            fg_stroke: Stroke::new(1.0, Color32::from_gray(100)), // button text
            corner_radius: CornerRadius::same(10),
            expansion: 0.0,
            weak_bg_fill: Color32::from_gray(240),
        },
        hovered: WidgetVisuals {
            bg_fill: Color32::from_gray(225),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(150)),
            fg_stroke: Stroke::new(1.5, Color32::from_gray(0)),
            corner_radius: CornerRadius::same(10),
            expansion: 1.0,
            weak_bg_fill: Color32::from_gray(225),
        },
        active: WidgetVisuals {
            bg_fill: Color32::from_gray(210),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(0)),
            fg_stroke: Stroke::new(2.0, Color32::from_gray(0)),
            corner_radius: CornerRadius::same(10),
            expansion: 1.0,
            weak_bg_fill: Color32::from_gray(180),
        },
        open: WidgetVisuals {
            bg_fill: Color32::from_gray(245),
            bg_stroke: Stroke::new(1.0, Color32::from_gray(200)),
            fg_stroke: Stroke::new(1.0, Color32::from_gray(60)),
            corner_radius: CornerRadius::same(10),
            expansion: 0.0,
            weak_bg_fill: Color32::from_gray(230),
        },
    };

    Visuals {
        widgets,
        selection,
        hyperlink_color: Color32::from_rgb(0, 102, 204), // deeper blue for contrast
        faint_bg_color: Color32::from_additive_luminance(250), // barely off-white
        extreme_bg_color: Color32::from_gray(245),       // e.g. TextEdit bg
        code_bg_color: Color32::from_gray(230),
        warn_fg_color: Color32::from_rgb(200, 100, 0), // muted orange
        error_fg_color: Color32::from_rgb(200, 0, 0),  // muted red
        window_corner_radius: CornerRadius::same(6),
        window_shadow: Shadow::NONE,
        popup_shadow: Shadow::NONE,
        ..Visuals::light()
    }
}); // CRAWL consts

pub const BUTTON_DISABLED_BY_EMPTY_LIST_NODES: &str =
    "disabled while no P2Pool compatible Nodes were found";
pub const EXPECT_BUTTON_DISABLED: &str = "button should be disabled if there is no found nodes";

//---------------------------------------------------------------------------------------------------- CONSTANTS
#[cfg(test)]
mod test {

    #[test]
    fn default_app_ratio_is_4_by_3() {
        assert_eq!(
            format!("{:.3}", crate::APP_MIN_WIDTH / crate::APP_MIN_HEIGHT),
            "1.333"
        );
        assert_eq!(
            format!(
                "{:.3}",
                crate::APP_DEFAULT_WIDTH / crate::APP_DEFAULT_HEIGHT
            ),
            "1.333"
        );
    }

    #[test]
    fn git_commit_eq_or_gt_40_chars() {
        assert!(crate::COMMIT.len() >= 40);
    }
}
