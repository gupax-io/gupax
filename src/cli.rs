use clap::Parser;
use clap::Subcommand;
use clap::crate_authors;
use clap::crate_description;
use clap::crate_name;
use clap::crate_version;
use log::debug;
use log::info;
use log::warn;
use std::process::exit;

use crate::app::App;
use crate::miscs::print_disk_file;
use crate::miscs::print_gupax_p2pool_api;
use crate::resets::reset;
use crate::resets::reset_gupax_p2pool_api;
use crate::resets::reset_nodes;
use crate::resets::reset_pools;
use crate::resets::reset_state;

#[derive(Parser)]
#[command(name = crate_name!())]
#[command(author = crate_authors!())]
#[command(version = crate_version!())]
#[command(about = crate_description!(), long_about = None)]
#[command(next_line_help = true)]
#[group(required = false)]
pub struct Cli {
    #[command(subcommand)]
    pub info: Option<GupaxData>,
    #[clap(long, short, action)]
    pub logfile: bool,
    #[clap(long, action)]
    pub daemon: bool,
    /// Start Gupax hidden in the system tray
    #[clap(long, action, conflicts_with = "daemon")]
    pub tray: bool,
    // Elevated helper arguments sent by Gupax v2.0.2 and older: a Gupax still
    // running after a self-update starts the updated exe with them.
    #[cfg(target_os = "windows")]
    #[arg(
        long,
        hide = true,
        requires("name_stdin_pipe"),
        requires("name_stdout_pipe"),
        requires("binary_path"),
        requires("arguments"),
        conflicts_with = "logfile",
        conflicts_with = "daemon"
    )]
    pub elevated_helper: bool,
    #[cfg(target_os = "windows")]
    #[arg(long, hide = true)]
    pub name_stdin_pipe: Option<String>,
    #[cfg(target_os = "windows")]
    #[arg(long, hide = true)]
    pub name_stdout_pipe: Option<String>,
    #[cfg(target_os = "windows")]
    #[arg(long, hide = true)]
    pub binary_path: Option<std::path::PathBuf>,
    #[cfg(target_os = "windows")]
    #[arg(long, hide = true, required(false), value_delimiter = ',')]
    pub arguments: Vec<String>,
}

impl Cli {
    /// Whether this invocation runs the GUI, the only mode that takes the
    /// single-instance guard. The other subcommands print or reset something
    /// and exit.
    pub fn runs_gui(&self) -> bool {
        !self.daemon && self.info.as_ref().is_none_or(GupaxData::runs_gupax)
    }
}

#[derive(Subcommand)]
pub enum GupaxData {
    #[command(about = "Print Gupax state")]
    State,
    #[command(about = "Print the manual node list")]
    Nodes,
    #[command(about = "Print the P2Pool payout log, payout count, and total XMR mined")]
    Payouts,
    #[command(about = "Reset all Gupaxstate (your settings)")]
    ResetState,
    #[command(about = "Reset the manual node list in the [P2Pool] tab")]
    ResetNodes,
    #[command(about = "Reset the manual pool list in the [XMRig] tab")]
    ResetPools,
    #[command(about = "Reset the permanent P2Pool stats that appear in the [Status] tab")]
    ResetPayouts,
    #[command(about = "Reset all Gupax state (your settings)")]
    ResetAll,
    #[command(
        about = "Disable all auto-startup settings for this instance (auto-update, auto-ping, etc)",
        name = "no-startup"
    )]
    Nostartup,
}

impl GupaxData {
    /// Whether Gupax keeps running after this subcommand.
    fn runs_gupax(&self) -> bool {
        match self {
            GupaxData::Nostartup => true,
            GupaxData::State
            | GupaxData::Nodes
            | GupaxData::Payouts
            | GupaxData::ResetState
            | GupaxData::ResetNodes
            | GupaxData::ResetPools
            | GupaxData::ResetPayouts
            | GupaxData::ResetAll => false,
        }
    }
}
// #[cold]
// #[inline(never)]
pub fn parse_args<S: Into<String>>(mut app: App, args: &Cli, panic: S) -> App {
    info!("Parsing CLI arguments...");

    // Abort on panic
    let panic = panic.into();
    if !panic.is_empty() {
        warn!("[Gupax error] {panic}");
        exit(1);
    }
    app.daemon = args.daemon;
    if args.tray {
        app.start_in_tray_flag = true;
        app.window_state = crate::app::WindowState::StartingInTray;
    }
    if let Some(arg) = &args.info {
        match arg {
            GupaxData::State => {
                debug!("Printing state...");
                print_disk_file(&app.state_path);
                exit(0);
            }
            GupaxData::Nodes => {
                debug!("Printing node list...");
                print_disk_file(&app.node_path);
                exit(0);
            }
            GupaxData::Payouts => {
                debug!("Printing payouts...\n");
                print_gupax_p2pool_api(&app.gupax_p2pool_api);
                exit(0);
            }
            GupaxData::ResetState => {
                if let Ok(()) = reset_state(&app.state_path) {
                    println!("\nState reset ... OK");
                    exit(0);
                } else {
                    eprintln!("\nState reset ... FAIL");
                    exit(1)
                }
            }
            GupaxData::ResetNodes => {
                if let Ok(()) = reset_nodes(&app.node_path) {
                    println!("\nNode reset ... OK");
                    exit(0)
                } else {
                    eprintln!("\nNode reset ... FAIL");
                    exit(1)
                }
            }
            GupaxData::ResetPools => {
                if let Ok(()) = reset_pools(&app.pool_path) {
                    println!("\nPool reset ... OK");
                    exit(0)
                } else {
                    eprintln!("\nPool reset ... FAIL");
                    exit(1)
                }
            }
            GupaxData::ResetPayouts => {
                if let Ok(()) = reset_gupax_p2pool_api(&app.gupax_p2pool_api_path) {
                    println!("\nGupaxP2poolApi reset ... OK");
                    exit(0)
                } else {
                    eprintln!("\nGupaxP2poolApi reset ... FAIL");
                    exit(1)
                }
            }
            GupaxData::ResetAll => reset(
                &app.os_data_path,
                &app.state_path,
                &app.node_path,
                &app.pool_path,
                &app.gupax_p2pool_api_path,
            ),
            GupaxData::Nostartup => app.no_startup = true,
        }
    }
    app
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn helper_flags_of_older_gupax_still_parse() {
        let cli = Cli::try_parse_from([
            "Gupax.exe",
            "--elevated-helper",
            "--name-stdin-pipe",
            "stdin",
            "--name-stdout-pipe",
            "stdout",
            "--binary-path",
            r"C:\XMRig\xmrig.exe",
            "--arguments=--threads,6",
        ])
        .unwrap();
        assert!(cli.elevated_helper);
        assert_eq!(cli.arguments, ["--threads", "6"]);
    }
}
