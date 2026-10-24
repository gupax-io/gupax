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

// Use macOS shaped icon for macOS
#[cfg(target_os = "macos")]
pub const BYTES_ICON: &[u8] = include_bytes!("../../assets/images/icons/icon@2x.png");
#[cfg(not(target_os = "macos"))]
pub const BYTES_ICON: &[u8] = include_bytes!("../../assets/images/icons/icon.png");
#[cfg(target_os = "macos")]
pub const BYTES_TRAY_ICON_TEMPLATE: &[u8] =
    include_bytes!("../../assets/images/icons/tray-template.png");
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
pub fn p2pool_alive() -> &'static str {
    crate::utils::i18n::tr("P2Pool is online and fully synchronized")
}
pub fn p2pool_dead() -> &'static str {
    crate::utils::i18n::tr("P2Pool is offline")
}
pub fn p2pool_failed() -> &'static str {
    crate::utils::i18n::tr("P2Pool is offline and failed when exiting")
}
pub fn p2pool_middle() -> &'static str {
    crate::utils::i18n::tr("P2Pool is in the middle of (re)starting/stopping")
}
pub fn p2pool_syncing() -> &'static str {
    crate::utils::i18n::tr(
        "P2Pool is still syncing. This indicator will turn GREEN when P2Pool is ready",
    )
}

pub fn node_alive() -> &'static str {
    crate::utils::i18n::tr("Node is online and fully synchronized")
}
pub fn node_dead() -> &'static str {
    crate::utils::i18n::tr("Node is offline")
}
pub fn node_failed() -> &'static str {
    crate::utils::i18n::tr("Node is offline and failed when exiting")
}
pub fn node_middle() -> &'static str {
    crate::utils::i18n::tr("Node is in the middle of (re)starting/stopping")
}
pub fn node_syncing() -> &'static str {
    crate::utils::i18n::tr(
        "Node is still syncing. This indicator will turn GREEN when Node is ready",
    )
}
pub fn xmrig_alive() -> &'static str {
    crate::utils::i18n::tr("XMRig is online and mining")
}
pub fn xmrig_dead() -> &'static str {
    crate::utils::i18n::tr("XMRig is offline")
}
pub fn xmrig_failed() -> &'static str {
    crate::utils::i18n::tr("XMRig is offline and failed when exiting")
}
pub fn xmrig_middle() -> &'static str {
    crate::utils::i18n::tr("XMRig is in the middle of (re)starting/stopping")
}
pub fn xmrig_not_mining() -> &'static str {
    crate::utils::i18n::tr("XMRig is online, but not mining to any pool")
}

pub fn xmrig_proxy_alive() -> &'static str {
    crate::utils::i18n::tr("XMRig-Proxy is online and mining")
}
pub fn xmrig_proxy_dead() -> &'static str {
    crate::utils::i18n::tr("XMRig-Proxy is offline")
}
pub fn xmrig_proxy_failed() -> &'static str {
    crate::utils::i18n::tr("XMRig-Proxy is offline and failed when exiting")
}
pub fn xmrig_proxy_middle() -> &'static str {
    crate::utils::i18n::tr("XMRig-Proxy is in the middle of (re)starting/stopping")
}
pub fn xmrig_proxy_not_mining() -> &'static str {
    crate::utils::i18n::tr("XMRig-Proxy is online, but not mining to any pool")
}
pub fn xmrig_proxy_redirect() -> &'static str {
    crate::utils::i18n::tr(
        "point local xmrig instance on this proxy instead of the p2pool instance (recommended if using XvB)",
    )
}
pub fn xmrig_proxy_input() -> &'static str {
    crate::utils::i18n::tr("Send a command to XMRig-Proxy")
}
pub fn xmrig_proxy_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use simple XMRig-Proxy settings:
  - Mine to local P2Pool (localhost:3333)
  - redirect Xmrig local instance to the proxy
  - HTTP API @ localhost:18089"#,
    )
}
pub fn xmrig_proxy_advanced() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use advanced XMRig-Proxy settings:
  - Terminal input
  - disable/enable local xmrig instance redirection
  - Overriding command arguments
  - Custom HTTP API IP/Port
  - TLS setting
  - Keepalive setting"#,
    )
}
pub fn xmrig_proxy_path_not_file() -> &'static str {
    crate::utils::i18n::tr(
        "XMRig-Proxy binary not found at the given PATH in the Gupax tab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig-Proxy is located.",
    )
}
pub fn xmrig_proxy_path_not_valid() -> &'static str {
    crate::utils::i18n::tr(
        "XMRig-Proxy binary at the given PATH in the Gupax tab doesn't look like XMRig-Proxy! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig-Proxy is located.",
    )
}
pub fn xmrig_proxy_path_ok() -> &'static str {
    crate::utils::i18n::tr("XMRig-Proxy was found at the given PATH")
}
pub fn xmrig_proxy_path_empty() -> &'static str {
    crate::utils::i18n::tr(
        "XMRig-Proxy PATH is empty! To fix: goto the [GupaxAdvanced] tab, select [Open] and specify where XMRig is located.",
    )
}
pub fn status_xmrig_proxy_uptime() -> &'static str {
    crate::utils::i18n::tr("How long XMRig-Proxy has been online")
}
pub fn status_xmrig_proxy_pool() -> &'static str {
    crate::utils::i18n::tr("The pool XMRig-Proxy is currently mining to")
}
pub fn status_xmrig_proxy_hashrate() -> &'static str {
    crate::utils::i18n::tr("The average hashrate of XMRig-Proxy")
}

pub fn xvb_alive() -> &'static str {
    crate::utils::i18n::tr(
        "XvB process is configured and distributing hashrate, XvB pool is online",
    )
}
pub fn xvb_dead() -> &'static str {
    crate::utils::i18n::tr("XvB process is offline")
}
pub fn xvb_failed() -> &'static str {
    crate::utils::i18n::tr("XvB process is misconfigured or the XvB pool is offline")
}
pub fn xvb_middle() -> &'static str {
    crate::utils::i18n::tr("XvB is in the middle of (re)starting/stopping")
}
pub fn xvb_not_configured() -> &'static str {
    crate::utils::i18n::tr("You need to insert an existent token before starting XvB")
}
pub fn xvb_public_only() -> &'static str {
    crate::utils::i18n::tr("XvB process is started only to get public stats.")
}
pub fn xvb_sidechain() -> &'static str {
    crate::utils::i18n::tr("
If checked:\n
The algorithm will watch the estimated HR given for your address on the P2Pool network. This way, it will take into account external miners that are mining on P2Pool for your address without using the P2Pool node of Gupax. This estimation can be imprecised.\n
If unchecked (default):\n
The algorithm will watch the HR estimated by the stratum data of the p2pool node, which is more accurate but will only take into account the miners that are using your P2Pool node.
")
}
pub fn xvb_manual_pool() -> &'static str {
    crate::utils::i18n::tr(
        "Enable this to force the algorithm to connect to a specific XvB pool, without using the considered fastest",
    )
}
pub fn xvb_p2pool_buffer() -> &'static str {
    crate::utils::i18n::tr(
        "Set the % amount of additional HR to send to p2pool. Will reduce (if positive) or augment (if negative) the chances to miss the p2pool window.\n\n- In Auto or Hero mode, the algorithm will keep enough HR on the p2pool side to conform to the buffer\n\n- In Manual modes, the algorithm will ignore the p2pool buffer",
    )
}
pub fn xvb_timeframe() -> &'static str {
    crate::utils::i18n::tr(
        "Set the duration between each decision of the algorithm.\n\nIncreasing it will allow you to drop the minimum hashrate you will be able to send in an hour but the algorithm will be less reactive to hashrate fluctuation and vice versa.\n\nThe minimum time being sent to the target will always be at least 5 seconds.\n\nExample: If the hasrate you produce is 100KH/s, setting the duration to 5 minutes will allow you to send at minimum 833H/s since 100_000 / 600 * 5 = 833.33.\n\nAdvise: If you use your computer often with Gupax, prefer a short duration. Otherwise, or if you have a lot of hashrate produced on this machine, choose a longer duration.\n\nDefault is 5 minutes.",
    )
}

pub fn start_options_hover() -> &'static str {
    crate::utils::i18n::tr(
        "Start the process with theses options.\nThe \"Reset to simple/advanced options\" are arguments constructed from the settings.\nYou can replace them with your own",
    )
}
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
pub const BONE: egui::Color32 = egui::Color32::from_rgb(190, 190, 190); // In between LIGHT_GRAY <-> GRAY
pub const GRAY: egui::Color32 = egui::Color32::GRAY;
pub const LIGHT_GRAY: egui::Color32 = egui::Color32::LIGHT_GRAY;
pub const DARK_GRAY: egui::Color32 = egui::Color32::from_gray(13);

// IP fetching
pub fn ip_not_found() -> &'static str {
    crate::utils::i18n::tr("No ip found, try refreshing by clicking on the button above")
}

// [Duration] constants
pub const SECOND: std::time::Duration = std::time::Duration::from_secs(1);

// The explanation given to the user on why XMRig needs sudo.
pub fn xmrig_admin_reason() -> &'static str {
    crate::utils::i18n::tr(
        r#"The large hashrate difference between XMRig and other miners like Monero and P2Pool's built-in miners is mostly due to XMRig configuring CPU MSRs and setting up hugepages. Other miners like Monero or P2Pool's built-in miner do not do this. It can be done manually but it isn't recommended since XMRig does this for you automatically, but only if it has the proper admin privileges."#,
    )
}

// OS specific
#[cfg(target_os = "windows")]
pub const OS: &str = " Windows";
#[cfg(target_os = "windows")]
pub const OS_NAME: &str = "Windows";

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
pub fn status_gupax_uptime() -> &'static str {
    crate::utils::i18n::tr("How long Gupax has been online")
}
pub fn status_gupax_cpu_usage() -> &'static str {
    crate::utils::i18n::tr(
        "How much CPU Gupax is currently using. This accounts for all your threads (it is out of 100%)",
    )
}
pub fn status_gupax_memory_usage() -> &'static str {
    crate::utils::i18n::tr("How much memory Gupax is currently using in Megabytes")
}
pub fn status_gupax_system_cpu_usage() -> &'static str {
    crate::utils::i18n::tr(
        "How much CPU your entire system is currently using. This accounts for all your threads (it is out of 100%)",
    )
}
pub fn status_gupax_system_memory() -> &'static str {
    crate::utils::i18n::tr(
        "How much memory your entire system has (including swap) and is currently using in Gigabytes",
    )
}
pub fn status_gupax_system_cpu_model() -> &'static str {
    crate::utils::i18n::tr("The detected model of your system's CPU and its current frequency")
}
//--
pub fn status_p2pool_uptime() -> &'static str {
    crate::utils::i18n::tr("How long P2Pool has been online")
}
pub fn status_p2pool_payouts() -> &'static str {
    crate::utils::i18n::tr(
        "The total amount of payouts received in this instance of P2Pool and an extrapolated estimate of how many you will receive. Warning: these stats will be quite inaccurate if your P2Pool hasn't been running for a long time!",
    )
}
pub fn status_p2pool_xmr() -> &'static str {
    crate::utils::i18n::tr(
        "The total amount of XMR mined in this instance of P2Pool and an extrapolated estimate of how many you will mine in the future. Warning: these stats will be quite inaccurate if your P2Pool hasn't been running for a long time!",
    )
}
pub fn status_p2pool_hashrate() -> &'static str {
    crate::utils::i18n::tr(
        "The total amount of hashrate your P2Pool has pointed at it in 15 minute, 1 hour, and 24 hour averages",
    )
}
pub fn status_p2pool_shares() -> &'static str {
    crate::utils::i18n::tr("The total amount of shares found on P2Pool")
}
pub fn status_p2pool_current_shares() -> &'static str {
    crate::utils::i18n::tr("Current shares valid in the PPLNS Window for your address")
}
pub fn status_p2pool_effort() -> &'static str {
    crate::utils::i18n::tr(
        "The average amount of effort needed to find a share, and the current effort",
    )
}
pub fn status_p2pool_connections() -> &'static str {
    crate::utils::i18n::tr("The total amount of miner connections on this P2Pool")
}
pub fn status_p2pool_monero_node() -> &'static str {
    crate::utils::i18n::tr("The Monero node being used by P2Pool")
}
pub fn status_p2pool_pool() -> &'static str {
    crate::utils::i18n::tr("The P2Pool sidechain you're currently connected to")
}
pub fn status_p2pool_address() -> &'static str {
    crate::utils::i18n::tr("The Monero address P2Pool will send payouts to")
}
//--
pub fn status_xmrig_uptime() -> &'static str {
    crate::utils::i18n::tr("How long XMRig has been online")
}
pub fn status_xmrig_hashrate() -> &'static str {
    crate::utils::i18n::tr("The average hashrate of XMRig")
}
pub fn status_xmrig_difficulty() -> &'static str {
    crate::utils::i18n::tr("The current difficulty of the job XMRig is working on")
}
pub fn status_xmrig_shares() -> &'static str {
    crate::utils::i18n::tr("The amount of accepted and rejected shares")
}
pub fn status_xmrig_pool() -> &'static str {
    crate::utils::i18n::tr("The pool XMRig is currently mining to")
}
pub fn status_xmrig_threads() -> &'static str {
    crate::utils::i18n::tr("The amount of threads XMRig is currently using")
}
pub fn status_proxy_connections() -> &'static str {
    crate::utils::i18n::tr("The total amount of miner connections on this Proxy")
}
//--
pub fn status_xvb_time_remain() -> &'static str {
    crate::utils::i18n::tr("Minutes left before end of round")
}
pub fn status_xvb_round_type() -> &'static str {
    crate::utils::i18n::tr("The current round type")
}
pub fn status_xvb_players() -> &'static str {
    crate::utils::i18n::tr("Numbers of registered players and currently playing in the round")
}
pub fn status_xvb_donated_hr() -> &'static str {
    crate::utils::i18n::tr("Hashrate donated to the raffle")
}
pub fn status_xvb_winner() -> &'static str {
    crate::utils::i18n::tr("Current Raffle Winner")
}
pub fn status_xvb_share() -> &'static str {
    crate::utils::i18n::tr("Share effort")
}
pub fn status_xvb_block_reward() -> &'static str {
    crate::utils::i18n::tr("Block reward")
}
pub fn status_xvb_yearly() -> &'static str {
    crate::utils::i18n::tr("Estimated Reward (Yearly)")
}
// Status Node
pub fn status_node_uptime() -> &'static str {
    crate::utils::i18n::tr("How long the Node has been online")
}
pub fn status_node_block_height() -> &'static str {
    crate::utils::i18n::tr("The height of where the node is synchronized")
}
pub fn status_node_difficulty() -> &'static str {
    crate::utils::i18n::tr("current difficulty of the network")
}
pub fn status_node_db_size() -> &'static str {
    crate::utils::i18n::tr("Size of the database")
}
pub fn status_node_freespace() -> &'static str {
    crate::utils::i18n::tr("Free space left on the partition storing the database")
}
pub fn status_node_nettype() -> &'static str {
    crate::utils::i18n::tr("Type of network (mainnet, stagenet, testnet)")
}
pub fn status_node_out() -> &'static str {
    crate::utils::i18n::tr("Current number of active outbound connections")
}
pub fn status_node_in() -> &'static str {
    crate::utils::i18n::tr("Current number of active incoming connections")
}
pub fn status_node_sync() -> &'static str {
    crate::utils::i18n::tr("Does the node is synchronized with the network ?")
}
pub fn status_node_status() -> &'static str {
    crate::utils::i18n::tr("General status of the node")
}
// Status Submenus
pub fn status_submenu_processes() -> &'static str {
    crate::utils::i18n::tr("View the status of process related data for [Gupax|P2Pool|XMRig]")
}
pub fn status_submenu_p2pool() -> &'static str {
    crate::utils::i18n::tr("View P2Pool specific data")
}
pub fn status_submenu_hashrate() -> &'static str {
    crate::utils::i18n::tr("Compare your CPU hashrate with others")
}
//-- P2Pool
pub fn status_submenu_payout() -> &'static str {
    crate::utils::i18n::tr(
        "The total amount of payouts received via P2Pool across all time. This includes all payouts you have ever received using Gupax and P2Pool.",
    )
}
pub fn status_submenu_xmr() -> &'static str {
    crate::utils::i18n::tr(
        "The total of XMR mined via P2Pool across all time. This includes all the XMR you have ever mined using Gupax and P2Pool.",
    )
}
pub fn status_submenu_latest() -> &'static str {
    crate::utils::i18n::tr("Sort the payouts from latest to oldest")
}
pub fn status_submenu_oldest() -> &'static str {
    crate::utils::i18n::tr("Sort the payouts from oldest to latest")
}
pub fn status_submenu_biggest() -> &'static str {
    crate::utils::i18n::tr("Sort the payouts from biggest to smallest")
}
pub fn status_submenu_smallest() -> &'static str {
    crate::utils::i18n::tr("Sort the payouts from smallest to biggest")
}
pub fn status_submenu_automatic() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically calculate share/block time with your current P2Pool 1 hour average hashrate",
    )
}
pub fn status_submenu_manual() -> &'static str {
    crate::utils::i18n::tr(
        "Manually input a hashrate to calculate share/block time with current P2Pool/Monero network stats",
    )
}
pub fn status_submenu_hash() -> &'static str {
    crate::utils::i18n::tr("Use [Hash] as the hashrate metric")
}
pub fn status_submenu_kilo() -> &'static str {
    crate::utils::i18n::tr("Use [Kilo] as the hashrate metric (1,000x hash)")
}
pub fn status_submenu_mega() -> &'static str {
    crate::utils::i18n::tr("Use [Mega] as the hashrate metric (1,000,000x hash)")
}
pub fn status_submenu_giga() -> &'static str {
    crate::utils::i18n::tr("Use [Giga] as the hashrate metric (1,000,000,000x hash)")
}
pub fn status_submenu_p2pool_block_mean() -> &'static str {
    crate::utils::i18n::tr("The average time it takes for P2Pool to find a block")
}
pub fn status_submenu_your_p2pool_hashrate() -> &'static str {
    crate::utils::i18n::tr("Your 1 hour average hashrate on P2Pool")
}
pub fn status_submenu_p2pool_share_mean() -> &'static str {
    crate::utils::i18n::tr("The average time it takes for your hashrate to find a share on P2Pool")
}
pub fn status_submenu_solo_block_mean() -> &'static str {
    crate::utils::i18n::tr(
        "The average time it would take for your hashrate to find a block solo mining Monero",
    )
}
pub fn status_submenu_monero_difficulty() -> &'static str {
    crate::utils::i18n::tr(
        "The current Monero network's difficulty (how many hashes it will take on average to find a block)",
    )
}
pub fn status_submenu_monero_hashrate() -> &'static str {
    crate::utils::i18n::tr("The current Monero network's hashrate")
}
pub fn status_submenu_p2pool_difficulty() -> &'static str {
    crate::utils::i18n::tr(
        "The current P2Pool network's difficulty (how many hashes it will take on average to find a share)",
    )
}
pub fn status_submenu_p2pool_hashrate() -> &'static str {
    crate::utils::i18n::tr("The current P2Pool network's hashrate")
}
pub fn status_submenu_p2pool_miners() -> &'static str {
    crate::utils::i18n::tr("The current amount of miners on P2Pool")
}
pub fn status_submenu_p2pool_dominance() -> &'static str {
    crate::utils::i18n::tr(
        "The percent of hashrate P2Pool accounts for in the entire Monero network",
    )
}
pub fn status_submenu_your_p2pool_dominance() -> &'static str {
    crate::utils::i18n::tr("The percent of hashrate you account for in P2Pool")
}
pub fn status_submenu_your_monero_dominance() -> &'static str {
    crate::utils::i18n::tr("The percent of hashrate you account for in the entire Monero network")
}
//-- Benchmarks
pub fn status_submenu_your_cpu() -> &'static str {
    crate::utils::i18n::tr("The CPU detected by Gupax")
}
pub fn status_submenu_your_benchmarks() -> &'static str {
    crate::utils::i18n::tr(
        "How many benchmarks your CPU has had uploaded to [https://xmrig.com/benchmark] ",
    )
}
pub fn status_submenu_your_rank() -> &'static str {
    crate::utils::i18n::tr(
        "Your CPU's rank out of all CPUs listed on [https://xmrig.com/benchmark] (higher is better)",
    )
}
pub fn status_submenu_your_high() -> &'static str {
    crate::utils::i18n::tr(
        "The highest hashrate recorded for your CPU on [https://xmrig.com/benchmark]",
    )
}
pub fn status_submenu_your_average() -> &'static str {
    crate::utils::i18n::tr(
        "The average hashrate of your CPU based off the data at [https://xmrig.com/benchmark]",
    )
}
pub fn status_submenu_your_low() -> &'static str {
    crate::utils::i18n::tr(
        "The lowest hashrate recorded for your CPU on [https://xmrig.com/benchmark]",
    )
}
pub fn status_submenu_other_cpus() -> &'static str {
    crate::utils::i18n::tr(
        "A list of ALL the recorded CPU benchmarks. The CPUs most similar to yours are listed first. All this data is taken from [https://xmrig.com/benchmark].",
    )
}
pub fn status_submenu_other_cpu() -> &'static str {
    crate::utils::i18n::tr("The CPU name")
}
pub fn status_submenu_other_relative() -> &'static str {
    crate::utils::i18n::tr(
        "The relative hashrate power compared to the fastest recorded CPU, which is current: [AMD EPYC 7T83 64-Core Processor]",
    )
}
pub fn status_submenu_other_high() -> &'static str {
    crate::utils::i18n::tr("Highest hashrate record")
}
pub fn status_submenu_other_average() -> &'static str {
    crate::utils::i18n::tr("Average hashrate")
}
pub fn status_submenu_other_low() -> &'static str {
    crate::utils::i18n::tr("Lowest hashrate record")
}
pub fn status_submenu_other_rank() -> &'static str {
    crate::utils::i18n::tr("The rank of this CPU out of [1567] (lower is better)")
}
pub fn status_submenu_other_benchmarks() -> &'static str {
    crate::utils::i18n::tr(
        "How many benchmarks this CPU has had posted to [https://xmrig.com/benchmark]",
    )
}

// Gupax
pub fn language_toggle() -> &'static str {
    crate::utils::i18n::tr("Change the interface language")
}
pub fn gupax_auto_update() -> &'static str {
    crate::utils::i18n::tr("Automatically check for updates at startup")
}
pub fn gupax_auto_crawl() -> &'static str {
    crate::utils::i18n::tr(
        "Start the P2Pool compatible Nodes Finder at startup.\nIt will crawl the monero network to find nodes if the ones already found are not online",
    )
}
pub fn gupax_should_restart() -> &'static str {
    crate::utils::i18n::tr("A restart is required to apply changes")
}
// #[cfg(not(target_os = "macos"))]
// pub fn gupax_update_via_tor() -> &'static str { crate::utils::i18n::tr("Update through the Tor network. Tor is embedded within Gupax; a Tor system proxy is not required") }
// #[cfg(target_os = "macos")] // Arti library has issues on macOS
// pub fn gupax_update_via_tor() -> &'static str { crate::utils::i18n::tr("WARNING: This option is unstable on macOS. Update through the Tor network. Tor is embedded within Gupax; a Tor system proxy is not required") }
pub fn gupax_ask_before_quit() -> &'static str {
    crate::utils::i18n::tr("Ask before quitting Gupax")
}
pub fn gupax_save_before_quit() -> &'static str {
    crate::utils::i18n::tr("Automatically save any changed settings before quitting")
}
pub fn gupax_hide_to_tray() -> &'static str {
    crate::utils::i18n::tr(
        "Closing the window hides Gupax to the system tray instead of quitting. Use the tray menu (or left-click the tray icon) to show it again, and the tray's Quit to exit.\nOn Linux with GNOME, an AppIndicator extension is required for the tray icon to be visible.",
    )
}
pub fn gupax_start_with_tray() -> &'static str {
    crate::utils::i18n::tr(
        "Show the Gupax icon in the system tray at startup. Start Gupax with the --tray argument to start it hidden in the tray.\nOn Linux with GNOME, an AppIndicator extension is required for the tray icon to be visible.",
    )
}
pub fn gupax_auto_p2pool() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically start P2Pool on Gupax startup. If you are using [P2Pool Simple], this will NOT wait for your [Auto-Ping] to finish, it will start P2Pool on the pool you already have selected. This option will fail if your P2Pool settings aren't valid!",
    )
}
pub fn gupax_auto_node() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically start Node on Gupax startup. This option will fail if your P2Pool settings aren't valid!",
    )
}
pub fn gupax_auto_xmrig() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically start XMRig on Gupax startup. This option will fail if your XMRig settings aren't valid!",
    )
}
pub fn gupax_auto_xmrig_proxy() -> &'static str {
    crate::utils::i18n::tr("Automatically start XMRig-Proxy on Gupax startup.")
}
pub fn gupax_auto_xvb() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically start XvB on Gupax startup. This option will fail if your XvB settings aren't valid!",
    )
}
pub fn gupax_adjust() -> &'static str {
    crate::utils::i18n::tr("Adjust and set the width/height of the Gupax window")
}
pub fn gupax_renderer() -> &'static str {
    crate::utils::i18n::tr(
        "If you experience freezes, try the other renderer.If Gupax crashes, it will automatically restart the GUI with the other renderer.",
    )
}
pub fn gupax_width() -> &'static str {
    crate::utils::i18n::tr("Set the width of the Gupax window")
}
pub fn gupax_height() -> &'static str {
    crate::utils::i18n::tr("Set the height of the Gupax window")
}
pub fn gupax_scale() -> &'static str {
    crate::utils::i18n::tr(
        "Set the resolution scaling of the Gupax window (resize window to re-apply scaling)",
    )
}
pub fn gupax_lock_width() -> &'static str {
    crate::utils::i18n::tr("Automatically match the HEIGHT against the WIDTH in a 4:3 ratio")
}
pub fn gupax_lock_height() -> &'static str {
    crate::utils::i18n::tr("Automatically match the WIDTH against the HEIGHT in a 4:3 ratio")
}
pub fn gupax_no_lock() -> &'static str {
    crate::utils::i18n::tr("Allow individual selection of width and height")
}
pub fn gupax_set() -> &'static str {
    crate::utils::i18n::tr("Set the width/height of the Gupax window to the current values")
}
pub fn gupax_tab() -> &'static str {
    crate::utils::i18n::tr("Set the default tab Gupax starts on")
}
pub fn gupax_tab_about() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: About")
}
pub fn gupax_tab_status() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: Status")
}
pub fn gupax_tab_gupax() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: Gupax")
}
pub fn gupax_tab_p2pool() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: P2Pool")
}
pub fn gupax_tab_xmrig() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: XMRig")
}
pub fn gupax_tab_xmrig_proxy() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: Proxy")
}
pub fn gupax_tab_xvb() -> &'static str {
    crate::utils::i18n::tr("Set the tab Gupax starts on to: XvB")
}
pub fn gupax_tab_node() -> &'static str {
    crate::utils::i18n::tr("Set the default tab Gupax starts on to: Node")
}

pub fn gupax_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use simple Gupax settings:
  - Basic toggles"#,
    )
}
pub fn gupax_advanced() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use advanced Gupax settings:
  - Basic toggles
  - P2Pool/XMRig binary path selector
  - Gupax resolution sliders
  - Gupax start-up tab selector"#,
    )
}
pub fn gupax_updates() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use Updates Gupax settings:
  - Choose binaries source and version
  - update checker frequency
  - automatic updates
  - automatic restart"#,
    )
}
pub fn gupax_select() -> &'static str {
    crate::utils::i18n::tr("Open a file explorer to select a file")
}
pub fn gupax_path_p2pool() -> &'static str {
    crate::utils::i18n::tr(
        "The location of the P2Pool binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no file found at the given path",
    )
}
pub fn gupax_path_xmrig() -> &'static str {
    crate::utils::i18n::tr(
        "The location of the XMRig binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no file found at the given path",
    )
}
pub fn gupax_path_xmrig_proxy() -> &'static str {
    crate::utils::i18n::tr(
        "The location of the XMRig-Proxy binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no file found at the given path",
    )
}

// P2Pool
pub fn p2pool_use_local_node_button() -> &'static str {
    crate::utils::i18n::tr("Start with a local node")
}
pub const P2POOL_PORT_DEFAULT: u16 = 3333;
pub fn p2pool_main() -> &'static str {
    crate::utils::i18n::tr(
        "Use the P2Pool main-chain. This P2Pool finds blocks faster, but has a higher difficulty. Suitable for miners with more than 100kH/s",
    )
}
pub fn p2pool_mini() -> &'static str {
    crate::utils::i18n::tr(
        "Use the P2Pool mini-chain. This P2Pool finds blocks slower, but has a lower difficulty. Suitable for miners with less than 100kH/s",
    )
}
pub fn p2pool_nano() -> &'static str {
    crate::utils::i18n::tr(
        "Use the P2Pool nano-chain. This P2Pool finds blocks slower, but has a lower difficulty. Suitable for miners with less than 30kH/s",
    )
}
pub fn p2pool_out() -> &'static str {
    crate::utils::i18n::tr("How many out-bound peers to connect to? (you connecting to others)")
}
pub fn p2pool_in() -> &'static str {
    crate::utils::i18n::tr("How many in-bound peers to allow? (others connecting to you)")
}
pub fn p2pool_log() -> &'static str {
    crate::utils::i18n::tr(
        "Verbosity of the console log.\nA verbosity level more than 0 is recommended to let the P2Pool process detect more rapidly errors with the Monero Node.\nIf the level is at 0, it can take up to 2 minutes to detect an error.",
    )
}
pub fn p2pool_auto_node() -> &'static str {
    crate::utils::i18n::tr("Automatically ping the remote Monero nodes at Gupax startup")
}
// pub const P2POOL_AUTO_SELECT: &str =
// "Automatically select the fastest remote Monero node after pinging";
pub fn p2pool_backup_host_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Automatically switch to the other nodes listed if the current one is down.

Note: you must ping the remote nodes or this feature will default to only using the currently selected node."#,
    )
}
pub fn p2pool_backup_host_advanced() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically switch to the other nodes in your list if the current one is down.",
    )
}
pub fn p2pool_autoswitch_local_node() -> &'static str {
    crate::utils::i18n::tr(
        "Automatically switch to the local node when it will be ready to be used.",
    )
}
pub fn p2pool_select_fastest() -> &'static str {
    crate::utils::i18n::tr("Select the fastest remote Monero node")
}
pub fn p2pool_select_random() -> &'static str {
    crate::utils::i18n::tr("Select a random remote Monero node")
}
pub fn p2pool_select_last() -> &'static str {
    crate::utils::i18n::tr("Select the previous remote Monero node")
}
pub fn p2pool_select_next() -> &'static str {
    crate::utils::i18n::tr("Select the next remote Monero node")
}
pub fn p2pool_ping() -> &'static str {
    crate::utils::i18n::tr("Ping the built-in remote Monero nodes")
}
pub fn p2pool_address() -> &'static str {
    crate::utils::i18n::tr(
        "You must use a primary Monero address to mine on P2Pool (starts with a 4). It is highly recommended to create a new wallet since addresses are public on P2Pool!",
    )
}
pub fn p2pool_community_node_warning() -> &'static str {
    crate::utils::i18n::tr(
        r#"TL;DR: Run & use your own Monero Node.

Using a Remote Monero Node is convenient but comes at the cost of privacy and reliability.

You may encounter connection issues with remote nodes which may cause mining performance loss! Late info from laggy nodes will cause your mining jobs to start later than they should.

Running and using your own local Monero node improves privacy and ensures your connection is as stable as your own internet connection. This comes at the cost of downloading and syncing Monero's blockchain yourself (currently about 100GB for pruned nodes). If you have the disk space, consider using the [Node] tab and start the process."#,
    )
}

pub fn p2pool_input() -> &'static str {
    crate::utils::i18n::tr("Send a command to P2Pool")
}

pub fn p2pool_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use simple P2Pool settings:
  - Default P2Pool settings + Nano
  - Use Remote/local Monero node
  - Find a remote node"#,
    )
}
pub fn p2pool_advanced() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use advanced P2Pool settings:
  - Terminal input
  - Overriding command arguments
  - Manual node list
  - P2Pool Main/Mini/Nano selection
  - Out/In peer setting
  - Log level setting
  - Backup host setting"#,
    )
}
pub fn p2pool_crawler() -> &'static str {
    crate::utils::i18n::tr(
        r#"Set crawler P2Pool settings:
  - selection of found nodes
  - adjust crawler parameters"#,
    )
}
pub fn p2pool_name() -> &'static str {
    crate::utils::i18n::tr(
        "Add a unique name to identify this node; Only [A-Za-z0-9-_.] and spaces allowed; Max length = 30 characters",
    )
}
pub fn p2pool_node_ip() -> &'static str {
    crate::utils::i18n::tr(
        "Specify the Monero Node IP to connect to with P2Pool; It must be a valid IPv4 address or a valid domain name; Max length = 255 characters",
    )
}
pub fn p2pool_rpc_port() -> &'static str {
    crate::utils::i18n::tr("Specify the RPC port of the Monero node; [1-65535]")
}
pub fn p2pool_zmq_port() -> &'static str {
    crate::utils::i18n::tr("Specify the ZMQ port of the Monero node; [1-65535]")
}
pub fn p2pool_path_not_file() -> &'static str {
    crate::utils::i18n::tr(
        "P2Pool binary not found at the given PATH in the Gupax tab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where P2Pool is located.",
    )
}
pub fn p2pool_path_not_valid() -> &'static str {
    crate::utils::i18n::tr(
        "P2Pool binary at the given PATH in the Gupax tab doesn't look like P2Pool! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where P2Pool is located.",
    )
}
pub fn p2pool_path_ok() -> &'static str {
    crate::utils::i18n::tr("P2Pool was found at the given PATH")
}
pub fn p2pool_path_empty() -> &'static str {
    crate::utils::i18n::tr(
        "P2Pool PATH is empty! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where P2Pool is located.",
    )
}
pub const P2POOL_URL: &str = "https://github.com/SChernykh/p2pool";

pub fn crawler_parameters_help() -> &'static str {
    crate::utils::i18n::tr(
        "You can define parameters for the crawling. Depending on the value, it will make the crawling faster or slower to reach the requirements",
    )
}
// Node/Pool list
pub fn list_add() -> &'static str {
    crate::utils::i18n::tr("Add the current values to the list")
}
pub fn list_save() -> &'static str {
    crate::utils::i18n::tr("Save the current values to the already existing entry")
}
pub fn list_delete() -> &'static str {
    crate::utils::i18n::tr("Delete the currently selected entry")
}
pub fn list_clear() -> &'static str {
    crate::utils::i18n::tr("Clear all current values")
}
// Node
pub const NODE_RPC_PORT_DEFAULT: u16 = 18081;
pub const NODE_ZMQ_PORT_DEFAULT: u16 = 18083;
pub fn node_input() -> &'static str {
    crate::utils::i18n::tr("Send a command to Node")
}
pub fn node_prunning() -> &'static str {
    crate::utils::i18n::tr(
        "Reduce the database size to a third. Does not have any security/privacy impact.If you have enough storage, a full node is preferable to make the network even more decentralized.",
    )
}
pub fn node_start_detect_valid() -> &'static str {
    crate::utils::i18n::tr(
        "A monero Node has been detected running on your system.\n\nGupax can not start a Node if there is already one running on the same system.\nThis is a Node that can be used by Gupax for P2Pool.\n\nDo you want to use the already running Node ? You will have a limited control from Gupax.",
    )
}
pub fn node_start_detect_non_valid() -> &'static str {
    crate::utils::i18n::tr(
        "A monero Node has been detected running on your system.\n\nGupax can not start a Node if there is already one running on the same system.\nThis is a Node that can not be used by Gupax for P2Pool.\n\nYou will not be able to use P2Pool or start a Node from Gupax while this node is running.",
    )
}
#[cfg(not(windows))]
pub fn node_db_path_empty() -> &'static str {
    crate::utils::i18n::tr("If the PATH of the DB is empty, the default ~/.bitmonero will be used.")
}
#[cfg(windows)]
pub fn node_db_path_empty() -> &'static str {
    crate::utils::i18n::tr(
        r#"If the PATH of the DB is empty, the default C:\ProgramData\bitmonero will be used."#,
    )
}
pub fn node_db_dir() -> &'static str {
    crate::utils::i18n::tr("The DB path needs to be a correct path to a directory if not empty")
}
pub fn node_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use simple Node settings:
  - Default Node settings"#,
    )
}
pub fn node_advanced() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use advanced Node settings:
  - Prunning
  - Custom path for database
  - Terminal input
  - Overriding command arguments
  - Manual zmq port
  - Out/In peer setting
  - Log level setting
  - Disable DNS checkpoint
  - DNS blocking"#,
    )
}
pub fn gupax_path_node() -> &'static str {
    crate::utils::i18n::tr(
        "The location of the Node binary: Both absolute and relative paths are accepted; A red [X] will appear if there is no directory found at the given path",
    )
}
pub fn node_path_ok() -> &'static str {
    crate::utils::i18n::tr("PATH for DB is valid.")
}
pub fn node_path_not_file() -> &'static str {
    crate::utils::i18n::tr(
        "Node binary not found at the given PATH in the Gupax tab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where NODE is located.",
    )
}
pub fn node_path_not_valid() -> &'static str {
    crate::utils::i18n::tr(
        "Node binary at the given PATH in the Gupax tab doesn't look like Node! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where Node is located.",
    )
}
pub fn node_path_empty() -> &'static str {
    crate::utils::i18n::tr(
        "Node PATH is empty! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where Node is located.",
    )
}
pub const NODE_URL: &str = "https://github.com/monero-project/monero";
pub fn node_dns_blocklist() -> &'static str {
    crate::utils::i18n::tr(
        "Apply realtime blocklist from DNS to ban known malicious nodes. (recommended)",
    )
}
pub fn node_dns_checkpoint() -> &'static str {
    crate::utils::i18n::tr(
        "Do not retrieve checkpoints from DNS to prevent periodic lags (recommended)",
    )
}
pub fn node_api_bind() -> &'static str {
    crate::utils::i18n::tr("bind address of RPC API")
}
pub fn node_api_port() -> &'static str {
    crate::utils::i18n::tr("RPC API listen port")
}
pub fn node_zmq_bind() -> &'static str {
    crate::utils::i18n::tr("bind address of ZMQ API")
}
pub fn node_zmq_port() -> &'static str {
    crate::utils::i18n::tr("ZMQ API listen port")
}
pub fn node_full_mem() -> &'static str {
    crate::utils::i18n::tr("Use 2GB of RAM instead of 256MB for faster block verification")
}
// XMRig
pub const XMRIG_API_PORT_DEFAULT: u16 = 18088;
pub fn xmrig_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use simple XMRig settings:
  - Mine to local P2Pool (localhost:3333)
  - CPU thread slider
  - HTTP API @ localhost:18088"#,
    )
}
pub fn xmrig_advanced() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use advanced XMRig settings:
  - Terminal input
  - Overriding command arguments
  - Custom payout address
  - CPU thread slider
  - Manual pool list
  - Custom HTTP API IP/Port
  - TLS setting
  - Keepalive setting"#,
    )
}
pub fn xmrig_input() -> &'static str {
    crate::utils::i18n::tr("Send a command to XMRig")
}

pub fn xmrig_address() -> &'static str {
    crate::utils::i18n::tr(
        "Specify which Monero address to payout to. This does nothing if mining to P2Pool since the address being paid out to will be the one P2Pool started with. This doubles as a rig identifier for P2Pool and some pools.",
    )
}
pub fn xmrig_name() -> &'static str {
    crate::utils::i18n::tr(
        "Add a unique name to identify this pool; Only [A-Za-z0-9-_.] and spaces allowed; Max length = 30 characters",
    )
}
pub fn xmrig_ip() -> &'static str {
    crate::utils::i18n::tr(
        "Specify the pool IP to connect to with XMRig; It must be a valid IPv4 address or a valid domain name; Max length = 255 characters",
    )
}
pub fn xmrig_port() -> &'static str {
    crate::utils::i18n::tr("Specify the port of the pool; [1-65535]")
}
pub fn xmrig_rig() -> &'static str {
    crate::utils::i18n::tr(
        "Add an optional rig ID. This will be the name shown on the pool; Only [A-Za-z0-9-_] and spaces allowed; Max length = 30 characters",
    )
}
pub const XMRIG_URL: &str = "https://github.com/xmrig/xmrig";
#[cfg(not(target_os = "linux"))]
pub fn xmrig_pause() -> &'static str {
    crate::utils::i18n::tr(
        "THIS SETTING IS DISABLED IF SET TO [0]. Pause mining if user is active, resume after",
    )
}
pub fn xmrig_api_ip() -> &'static str {
    crate::utils::i18n::tr(
        "Specify which IP to bind to for XMRig's HTTP API; If empty: [localhost/127.0.0.1]",
    )
}
pub fn xmrig_api_port() -> &'static str {
    crate::utils::i18n::tr("Specify which port to bind to for XMRig's HTTP API; If empty: [18088]")
}
pub fn xmrig_api_token() -> &'static str {
    crate::utils::i18n::tr("Specify the token to authenticate on the HTTP API")
}
pub fn xmrig_tls() -> &'static str {
    crate::utils::i18n::tr("Enable SSL/TLS connections (needs pool support)")
}
pub fn xmrig_keepalive() -> &'static str {
    crate::utils::i18n::tr("Send keepalive packets to prevent timeout (needs pool support)")
}
pub fn xmrig_threads() -> &'static str {
    crate::utils::i18n::tr("Number of CPU threads to use for mining")
}
pub fn xmrig_path_not_file() -> &'static str {
    crate::utils::i18n::tr(
        "XMRig binary not found at the given PATH in the Gupaxtab! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig is located.",
    )
}
pub fn xmrig_path_not_valid() -> &'static str {
    crate::utils::i18n::tr(
        "XMRig binary at the given PATH in the Gupaxtab doesn't look like XMRig! To fix: goto the [Gupax Advanced] tab, select [Open] and specify where XMRig is located.",
    )
}
pub fn xmrig_path_ok() -> &'static str {
    crate::utils::i18n::tr("XMRig was found at the given PATH")
}
pub fn xmrig_path_empty() -> &'static str {
    crate::utils::i18n::tr(
        "XMRig PATH is empty! To fix: goto the [GupaxAdvanced] tab, select [Open] and specify where XMRig is located.",
    )
}
pub const XMRIG_PROXY_URL: &str = "https://github.com/xmrig/xmrig-proxy";
pub const PROXY_API_PORT_DEFAULT: u16 = 18089;
pub const PROXY_PORT_DEFAULT: u16 = 3355;

// XvB
pub fn xvb_manual_slider_manual_xvb_help() -> &'static str {
    crate::utils::i18n::tr(
        "Set the hashrate amount to donate to XvB manually, The remaining hashrate will be sent to p2pool. If the selected hashrate is more than your xmrig hashrate it will be overwritten",
    )
}
pub fn xvb_manual_slider_manual_p2pool_help() -> &'static str {
    crate::utils::i18n::tr(
        "Set the hashrate amount to keep on p2pool manually, The remaining hasrate will be donated to xvb. If the selected hashrate is more than your xmrig hashrate it will be overwritten ",
    )
}
pub const XVB_URL: &str = "https://xmrvsbeast.com";
// Simple/Advanced Submenu hover text
pub fn xvb_simple() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use simple XvB settings:
  - Auto mode by default
  - Hero mode available"#,
    )
}
pub fn xvb_advanced() -> &'static str {
    crate::utils::i18n::tr(
        r#"Use advanced XvB settings:
  - Selection of mode: 
      Auto,
      Hero,
      Manual XvB,
      Manual P2pool,
      Round
  - P2Pool Buffer"#,
    )
}
pub const XVB_URL_PUBLIC_API: &str = "https://xmrvsbeast.com/p2pool/stats";
pub const XVB_NODE_PORT: &str = "4247";
pub const XVB_NODE_EU: &str = "eu.xmrvsbeast.com";
pub const XVB_NODE_NA: &str = "na.xmrvsbeast.com";
pub const XVB_URL_RULES: &str = "https://xmrvsbeast.com/p2pool/rules.html";
// buffer in percentage of HR to have plus the requirement.
pub const XVB_SIDE_MARGIN_1H: f32 = 0.2;
// time is in ms
pub const XVB_TIME_ALGO: u64 = 300_000;
// minimum time to send to XvB if any
pub const XVB_MIN_TIME_SEND: u64 = 5_000;
pub fn xvb_hero_select() -> &'static str {
    crate::utils::i18n::tr(
        "Donate as much as possible while keeping a share on p2pool, increases the odds of your round winning\nWhen modified, the algorithm will use the new choice at the next decision.",
    )
}
pub fn xvb_failure_field() -> &'static str {
    crate::utils::i18n::tr("Failures")
}
pub fn xvb_donated_1h_field() -> &'static str {
    crate::utils::i18n::tr("Donated last hour")
}
pub fn xvb_donated_24h_field() -> &'static str {
    crate::utils::i18n::tr("Donated last 24 hours")
}
pub fn xvb_round_type_field() -> &'static str {
    crate::utils::i18n::tr("Round")
}
pub fn xvb_winner_field() -> &'static str {
    crate::utils::i18n::tr("Win")
}
pub fn xvb_mining_on_field() -> &'static str {
    crate::utils::i18n::tr("Currently Mining on")
}

pub const SOCKET_MONERO_LOCAL_OUTSIDE: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 18080));
// Common help
pub fn help_stratum_port() -> &'static str {
    crate::utils::i18n::tr("Specify the stratum port to bind to")
}
pub fn help_stratum_ip() -> &'static str {
    crate::utils::i18n::tr("Specify the stratum ip to bind to")
}
// Manual Mode
pub fn xvb_mode_manual_xvb_help() -> &'static str {
    crate::utils::i18n::tr(
        "Manually set the amount to donate to XmrVsBeast, If value is more than xmrig hashrate it might be changed",
    )
}
pub fn xvb_mode_manual_p2pool_help() -> &'static str {
    crate::utils::i18n::tr(
        "Manually set the amount to keep on P2pool, If value is more than xmrig hashrate it might be changed",
    )
}
pub fn xvb_mode_manual_donation_level_help() -> &'static str {
    crate::utils::i18n::tr("Manually set the XvB donation level")
}

// Manual Donation Levels
pub fn xvb_donation_level_donor_help() -> &'static str {
    crate::utils::i18n::tr(
        "To qualify at least 1 kH/s will be actively donated (1hr and 24hr avg.)",
    )
}
pub fn xvb_donation_level_vip_donor_help() -> &'static str {
    crate::utils::i18n::tr(
        "To qualify at least 10 kH/s will be actively donated (1hr and 24hr avg.)",
    )
}
pub fn xvb_donation_level_whale_donor_help() -> &'static str {
    crate::utils::i18n::tr(
        "To qualify at least 100 kH/s will be actively donated (1hr and 24hr avg.)",
    )
}
pub fn xvb_donation_level_mega_donor_help() -> &'static str {
    crate::utils::i18n::tr(
        "To qualify at least 1000 kH/s will be actively donated (1hr and 24hr avg.)",
    )
}

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
pub fn process_outside() -> &'static str {
    crate::utils::i18n::tr(
        "This process is running outside of Gupax.\nYou need to stop it before starting it in Gupax.",
    )
}
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

pub fn button_disabled_by_empty_list_nodes() -> &'static str {
    crate::utils::i18n::tr("disabled while no P2Pool compatible Nodes were found")
}
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
