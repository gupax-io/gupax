use crate::APP_DEFAULT_HEIGHT;
use crate::APP_DEFAULT_WIDTH;
use crate::GUPAX_TAB_ABOUT;
use crate::GUPAX_TAB_GUPAX;
use crate::GUPAX_TAB_NODE;
use crate::GUPAX_TAB_P2POOL;
use crate::GUPAX_TAB_STATUS;
use crate::GUPAX_TAB_XMRIG;
use crate::GUPAX_TAB_XMRIG_PROXY;
use crate::GUPAX_TAB_XVB;
use crate::GUPAX_VERSION;
use crate::OS;
use crate::cli::Cli;
use crate::cli::parse_args;
use crate::components::gupax::FileWindow;
use crate::components::node::Ping;
use crate::components::node::RemoteNodes;
use crate::components::update::Update;
use crate::disk::consts::GUPAX_P2POOL_API_DIRECTORY;
use crate::disk::consts::NODE_TOML;
use crate::disk::consts::POOL_TOML;
use crate::disk::consts::STATE_TOML;
use crate::disk::create_gupax_dir;
use crate::disk::create_gupax_p2pool_dir;
use crate::disk::get_gupax_data_path;
use crate::disk::gupax_p2pool_api::GupaxP2poolApi;
use crate::disk::node::Node;
use crate::disk::pool::Pool;
use crate::disk::state::GupaxTheme;
use crate::disk::state::State;
use crate::errors::ErrorButtons;
use crate::errors::ErrorFerris;
use crate::errors::ErrorState;
use crate::helper::Helper;
use crate::helper::Process;
use crate::helper::ProcessName;
use crate::helper::crawler::Crawler;
use crate::helper::node::ImgNode;
use crate::helper::node::PubNodeApi;
use crate::helper::notification::NotificationApi;
use crate::helper::p2pool::ImgP2pool;
use crate::helper::p2pool::PubP2poolApi;
use crate::helper::sys_info::Sys;
use crate::helper::xrig::xmrig::ImgXmrig;
use crate::helper::xrig::xmrig::PubXmrigApi;
use crate::helper::xrig::xmrig_proxy::ImgProxy;
use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
use crate::helper::xvb::PubXvbApi;
use crate::helper::xvb::priv_stats::RuntimeMode;
use crate::inits::init_text_styles;
use crate::miscs::cmp_f64;
use crate::miscs::get_exe;
use crate::miscs::get_exe_dir;
use crate::utils::constants::VISUALS_GUPAX_DARK;
use crate::utils::constants::VISUALS_GUPAX_LIGHT;
use crate::utils::macros::arc_mut;
use crate::utils::sudo::SudoState;
use copy_dir::copy_dir;
use derive_more::derive::Display;
use eframe::CreationContext;
use egui::Context;
use egui::Vec2;
use egui::vec2;
use log::debug;
use log::error;
use log::info;
use log::warn;
use panels::middle::common::list_poolnode::PoolNode;
use serde::Deserialize;
use serde::Serialize;
use std::fs::remove_dir_all;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use std::time::SystemTime;
use strum::EnumCount;
use strum::EnumIter;

pub mod eframe_impl;
pub mod keys;
pub mod panels;
pub mod quit;
pub mod submenu_enum;

pub type BackupNodes = Arc<Mutex<Vec<PoolNode>>>;
//---------------------------------------------------------------------------------------------------- Struct + Impl
// The state of the outer main [App].
// See the [State] struct in [state.rs] for the
// actual inner state of the tab settings.
#[allow(dead_code)]
pub struct App {
    // Misc state
    pub tab: Tab,   // What tab are we on?
    pub size: Vec2, // Top-level width and Top-level height
    // Alpha (transparency)
    // This value is used to incrementally increase/decrease
    // the transparency when resizing. Basically, it fades
    // in/out of black to hide jitter when resizing with [init_text_styles()]
    pub alpha: u8,
    // This is a one time trigger so [init_text_styles()] isn't
    // called 60x a second when resizing the window. Instead,
    // it only gets called if this bool is true and the user
    // is hovering over egui (ctx.is_pointer_over_area()).
    pub must_resize: bool, // Sets the flag so we know to [init_text_styles()]
    pub resizing: bool,    // Are we in the process of resizing? (For black fade in/out)
    // State
    pub og: Arc<Mutex<State>>,      // og = Old state to compare against
    pub state: State,               // state = Working state (current settings)
    pub update: Arc<Mutex<Update>>, // State for update data [update.rs]
    pub file_window: Arc<Mutex<FileWindow>>, // State for the path selector in [Gupax]
    pub ping: Arc<Mutex<Ping>>,     // Ping data found in [node.rs]
    pub og_node_vec: Vec<(String, PoolNode)>, // Manual Node database
    pub node_vec: Vec<(String, PoolNode)>, // Manual Node database
    pub og_pool_vec: Vec<(String, PoolNode)>, // Manual Pool database
    pub pool_vec: Vec<(String, PoolNode)>, // Manual Pool database
    pub diff: bool,                 // This bool indicates state changes
    // Restart state:
    // If Gupax updated itself, this represents that the
    // user should (but isn't required to) restart Gupax.
    pub restart: Arc<Mutex<Restart>>,
    // Error State:
    // These values are essentially global variables that
    // indicate if an error message needs to be displayed
    // (it takes up the whole screen with [error_msg] and buttons for ok/quit/etc)
    pub error_state: ErrorState,
    // Helper/API State:
    // This holds everything related to the data processed by the "helper thread".
    // This includes the "helper" threads public P2Pool/XMRig's API.
    pub helper: Arc<Mutex<Helper>>, // [Helper] state, mostly for Gupax uptime
    pub pub_sys: Arc<Mutex<Sys>>,   // [Sys] state, read by [Status], mutated by [Helper]
    pub node: Arc<Mutex<Process>>,  // [Node] process state
    pub p2pool: Arc<Mutex<Process>>, // [P2Pool] process state
    pub xmrig: Arc<Mutex<Process>>, // [XMRig] process state
    pub xmrig_proxy: Arc<Mutex<Process>>, // [XMRig-Proxy] process state
    pub xvb: Arc<Mutex<Process>>,   // [Xvb] process state
    pub node_api: Arc<Mutex<PubNodeApi>>, // Public ready-to-print node API made by the "helper" thread
    pub p2pool_api: Arc<Mutex<PubP2poolApi>>, // Public ready-to-print P2Pool API made by the "helper" thread
    pub xmrig_api: Arc<Mutex<PubXmrigApi>>, // Public ready-to-print XMRig API made by the "helper" thread
    pub xmrig_proxy_api: Arc<Mutex<PubXmrigProxyApi>>, // Public ready-to-print XMRigProxy API made by the "helper" thread
    pub xvb_api: Arc<Mutex<PubXvbApi>>,                // Public XvB API
    pub notifications_api: Arc<Mutex<NotificationApi>>, // Public XvB API
    pub p2pool_img: Arc<Mutex<ImgP2pool>>, // A one-time snapshot of what data P2Pool started with
    pub xmrig_img: Arc<Mutex<ImgXmrig>>,   // A one-time snapshot of what data XMRig started with
    pub ip_local: Arc<Mutex<Option<IpAddr>>>,
    pub ip_public: Arc<Mutex<Option<Ipv4Addr>>>,
    pub proxy_port_reachable: Arc<Mutex<bool>>, // is the proxy port reachable from public ip ?
    pub crawler: Arc<Mutex<Crawler>>,
    // STDIN Buffer
    pub node_stdin: String, // The buffer between the node console and the [Helper]
    pub p2pool_stdin: String, // The buffer between the p2pool console and the [Helper]
    pub xmrig_stdin: String, // The buffer between the xmrig console and the [Helper]
    pub xmrig_proxy_stdin: String, // The buffer between the xmrig-proxy console and the [Helper]
    // Sudo State
    pub sudo: Arc<Mutex<SudoState>>, // This is just a dummy struct on [Windows].
    // State from [--flags]
    pub no_startup: bool,
    // Gupax-P2Pool API
    // Gupax's P2Pool API (e.g: ~/.local/share/gupax/p2pool/)
    // This is a file-based API that contains data for permanent stats.
    // The below struct holds everything needed for it, the paths, the
    // actual stats, and all the functions needed to mutate them.
    pub gupax_p2pool_api: Arc<Mutex<GupaxP2poolApi>>,
    // Static stuff
    pub benchmarks: Vec<Benchmark>,     // XMRig CPU benchmarks
    pub pid: sysinfo::Pid,              // Gupax's PID
    pub max_threads: u16,               // Max amount of detected system threads
    pub now: Instant,                   // Internal timer
    pub exe: String,                    // Path for [Gupax] binary
    pub dir: String,                    // Directory [Gupax] binary is in
    pub resolution: Vec2,               // Frame resolution
    pub os: &'static str,               // OS
    pub admin: bool,                    // Are we admin? (for Windows)
    pub os_data_path: PathBuf,          // OS data path (e.g: ~/.local/share/gupax/)
    pub gupax_p2pool_api_path: PathBuf, // Gupax-P2Pool API path (e.g: ~/.local/share/gupax/p2pool/)
    pub state_path: PathBuf,            // State file path
    pub node_path: PathBuf,             // Node file path
    pub pool_path: PathBuf,             // Pool file path
    pub backup_hosts: BackupNodes,      // P2Pool backup nodes
    pub version: &'static str,          // Gupax version
    pub name_version: String,           // [Gupax vX.X.X]
    #[cfg(target_os = "windows")]
    pub xmrig_outside_warning_acknowledge: bool,
    pub wgpu_render_state: Option<eframe::egui_wgpu::RenderState>,
}

impl App {
    #[cold]
    #[inline(never)]
    pub fn cc(cc: &CreationContext<'_>, resolution: Vec2, mut app: Self) -> Self {
        init_text_styles(
            &cc.egui_ctx,
            crate::miscs::clamp_scale(app.state.gupax.selected_scale),
        );
        app.set_theme(&cc.egui_ctx);
        app.wgpu_render_state = Some(cc.wgpu_render_state.clone().unwrap());
        Self { resolution, ..app }
    }

    pub fn set_theme(&self, ctx: &Context) {
        match self.state.gupax.theme {
            GupaxTheme::Dark => ctx.set_visuals(VISUALS_GUPAX_DARK.clone()),
            GupaxTheme::Light => ctx.set_visuals(VISUALS_GUPAX_LIGHT.clone()),
            GupaxTheme::System => {
                if let Some(system_theme) = ctx.system_theme() {
                    ctx.set_theme(system_theme);
                } else {
                    ctx.set_visuals(VISUALS_GUPAX_DARK.clone());
                }
            }
        };
    }

    #[cold]
    #[inline(never)]
    pub fn save_before_quit(&mut self) {
        if let Err(e) = State::save(&mut self.state, &self.state_path) {
            error!("State file: {e}");
        }
        if let Err(e) = Node::save(&self.node_vec, &self.node_path) {
            error!("Node list: {e}");
        }
        if let Err(e) = Pool::save(&self.pool_vec, &self.pool_path) {
            error!("Pool list: {e}");
        }
    }

    #[cold]
    #[inline(never)]
    pub fn new(now: Instant, args: &Cli) -> Self {
        info!("Initializing App Struct...");
        info!("App Init | P2Pool & XMRig processes...");
        let p2pool = arc_mut!(Process::new(
            ProcessName::P2pool,
            String::new(),
            PathBuf::new()
        ));
        let xmrig = arc_mut!(Process::new(
            ProcessName::Xmrig,
            String::new(),
            PathBuf::new()
        ));
        let xmrig_proxy = arc_mut!(Process::new(
            ProcessName::XmrigProxy,
            String::new(),
            PathBuf::new()
        ));
        let xvb = arc_mut!(Process::new(
            ProcessName::Xvb,
            String::new(),
            PathBuf::new()
        ));
        let node = arc_mut!(Process::new(
            ProcessName::Node,
            String::new(),
            PathBuf::new()
        ));
        let p2pool_api = arc_mut!(PubP2poolApi::new());
        let xmrig_api = arc_mut!(PubXmrigApi::new());
        let xmrig_proxy_api = arc_mut!(PubXmrigProxyApi::new());
        let xvb_api = arc_mut!(PubXvbApi::new());
        let node_api = arc_mut!(PubNodeApi::new());
        let node_img = arc_mut!(ImgNode::default());
        let p2pool_img = arc_mut!(ImgP2pool::new());
        let xmrig_img = arc_mut!(ImgXmrig::new());
        let proxy_img = arc_mut!(ImgProxy::new());
        let ip_local = arc_mut!(None);
        let ip_public = arc_mut!(None);
        let proxy_port_reachable = arc_mut!(false);
        let ports_detected_local_node = arc_mut!(None);
        let notifications_api = Arc::new(Mutex::new(NotificationApi {
            notifications: vec![],
        }));

        info!("App Init | Sysinfo...");
        // We give this to the [Helper] thread.
        //

        let mut sysinfo = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::nothing()
                .with_cpu(sysinfo::CpuRefreshKind::everything())
                .with_processes(sysinfo::ProcessRefreshKind::nothing().with_cpu())
                .with_memory(sysinfo::MemoryRefreshKind::everything()),
        );
        sysinfo.refresh_all();
        let pid = match sysinfo::get_current_pid() {
            Ok(pid) => pid,
            Err(e) => {
                error!("App Init | Failed to get sysinfo PID: {e}");
                exit(1)
            }
        };
        let pub_sys = arc_mut!(Sys::new());

        // CPU Benchmark data initialization.
        info!("App Init | Initializing CPU benchmarks...");
        let benchmarks: Vec<Benchmark> = {
            let cpu = sysinfo.cpus()[0].brand();
            let mut json: Vec<Benchmark> =
                serde_json::from_slice(include_bytes!("../../assets/cpu.json")).unwrap();
            json.sort_by(|a, b| cmp_f64(strsim::jaro(&b.cpu, cpu), strsim::jaro(&a.cpu, cpu)));
            json
        };
        info!("App Init | Assuming user's CPU is: {}", benchmarks[0].cpu);

        info!("App Init | The rest of the [App]...");
        let sysinfo = arc_mut!(sysinfo);
        let mut app = Self {
            tab: Tab::default(),
            ping: arc_mut!(Ping::new(RemoteNodes::default())),
            size: vec2(APP_DEFAULT_WIDTH, APP_DEFAULT_HEIGHT),
            must_resize: true,
            og: arc_mut!(State::new()),
            state: State::new(),
            update: arc_mut!(Update::new(
                String::new(),
                PathBuf::new(),
                PathBuf::new(),
                PathBuf::new(),
                PathBuf::new()
            )),
            file_window: FileWindow::new(),
            og_node_vec: Node::new_vec(),
            node_vec: Node::new_vec(),
            og_pool_vec: Pool::new_vec(),
            pool_vec: Pool::new_vec(),
            restart: arc_mut!(Restart::No),
            diff: false,
            error_state: ErrorState::new(),
            helper: arc_mut!(Helper::new(
                now,
                pub_sys.clone(),
                p2pool.clone(),
                xmrig.clone(),
                xmrig_proxy.clone(),
                xvb.clone(),
                node.clone(),
                p2pool_api.clone(),
                xmrig_api.clone(),
                xvb_api.clone(),
                xmrig_proxy_api.clone(),
                node_api.clone(),
                node_img.clone(),
                p2pool_img.clone(),
                xmrig_img.clone(),
                proxy_img.clone(),
                arc_mut!(GupaxP2poolApi::new()),
                ip_local.clone(),
                ip_public.clone(),
                proxy_port_reachable.clone(),
                ports_detected_local_node.clone(),
                sysinfo.clone(),
                notifications_api.clone()
            )),
            node,
            p2pool,
            xmrig,
            xmrig_proxy,
            xvb,
            node_api,
            p2pool_api,
            xvb_api,
            xmrig_api,
            xmrig_proxy_api,
            crawler: Crawler::new(),
            p2pool_img,
            xmrig_img,
            node_stdin: String::with_capacity(10),
            p2pool_stdin: String::with_capacity(10),
            xmrig_stdin: String::with_capacity(10),
            xmrig_proxy_stdin: String::with_capacity(10),
            sudo: arc_mut!(SudoState::new()),
            resizing: false,
            alpha: 0,
            no_startup: false,
            gupax_p2pool_api: arc_mut!(GupaxP2poolApi::new()),
            pub_sys,
            benchmarks,
            pid,
            max_threads: benri::threads!() as u16,
            now,
            admin: false,
            exe: String::new(),
            dir: String::new(),
            resolution: Vec2::new(APP_DEFAULT_HEIGHT, APP_DEFAULT_WIDTH),
            os: OS,
            os_data_path: PathBuf::new(),
            gupax_p2pool_api_path: PathBuf::new(),
            state_path: PathBuf::new(),
            node_path: PathBuf::new(),
            pool_path: PathBuf::new(),
            backup_hosts: Arc::new(Mutex::new(vec![])),
            version: GUPAX_VERSION,
            name_version: format!("Gupax {GUPAX_VERSION}"),
            ip_local,
            ip_public,
            proxy_port_reachable,
            notifications_api,
            #[cfg(target_os = "windows")]
            xmrig_outside_warning_acknowledge: false,
            wgpu_render_state: None,
        };
        //---------------------------------------------------------------------------------------------------- App init data that *could* panic
        info!("App Init | Getting EXE path...");
        let mut panic = String::new();
        // Get exe path
        app.exe = match get_exe() {
            Ok(exe) => exe,
            Err(e) => {
                panic = format!("get_exe(): {e}");
                app.error_state
                    .set(panic.clone(), ErrorFerris::Panic, ErrorButtons::Quit);
                String::new()
            }
        };
        // Get exe directory path
        app.dir = match get_exe_dir() {
            Ok(dir) => dir,
            Err(e) => {
                panic = format!("get_exe_dir(): {e}");
                app.error_state
                    .set(panic.clone(), ErrorFerris::Panic, ErrorButtons::Quit);
                String::new()
            }
        };
        // Get OS data path
        app.os_data_path = match get_gupax_data_path() {
            Ok(dir) => dir,
            Err(e) => {
                panic = format!("get_os_data_path(): {e}");
                app.error_state
                    .set(panic.clone(), ErrorFerris::Panic, ErrorButtons::Quit);
                PathBuf::new()
            }
        };

        info!("App Init | Setting TOML path...");
        // Set [*.toml] path
        app.state_path.clone_from(&app.os_data_path);
        app.state_path.push(STATE_TOML);
        app.node_path.clone_from(&app.os_data_path);
        app.node_path.push(NODE_TOML);
        app.pool_path.clone_from(&app.os_data_path);
        app.pool_path.push(POOL_TOML);
        // Set GupaxP2poolApi path
        app.gupax_p2pool_api_path = crate::disk::get_gupax_p2pool_path(&app.os_data_path);
        app.gupax_p2pool_api
            .lock()
            .unwrap()
            .fill_paths(&app.gupax_p2pool_api_path);

        // Apply arg state
        // It's not safe to [--reset] if any of the previous variables
        // are unset (null path), so make sure we just abort if the [panic] String contains something.
        info!("App Init | Applying argument state...");
        let mut app = parse_args(app, args, panic);

        use crate::disk::errors::TomlError::*;

        // need to upgrade old Gupax state file that is still using a node by the name.
        app.migrate_gupaxx_gupax();
        // if none exist, create:
        if !app.os_data_path.exists() {
            if let Err(e) = create_gupax_dir(&app.os_data_path) {
                panic = format!("get_os_data_path(): {e}");
                app.error_state
                    .set(panic.clone(), ErrorFerris::Panic, ErrorButtons::Quit);
            }
            let mut gupax_p2pool_dir = app.os_data_path.to_path_buf();
            gupax_p2pool_dir.push(GUPAX_P2POOL_API_DIRECTORY);
            if let Err(e) = create_gupax_p2pool_dir(&gupax_p2pool_dir) {
                panic = format!("get_os_data_path(): {e}");
                app.error_state
                    .set(panic.clone(), ErrorFerris::Panic, ErrorButtons::Quit);
            }
        }

        // Read disk state
        info!("App Init | Reading disk state...");
        app.state = match State::get(&app.state_path) {
            Ok(toml) => toml,
            Err(err) => {
                error!("State ... {err}");
                let set = match err {
                    Io(e) => Some((e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit)),
                    Path(e) => Some((e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit)),
                    Serialize(e) => Some((e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit)),
                    Deserialize(e) => Some((e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit)),
                    Format(e) => Some((e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit)),
                    Merge(e) => Some((e.to_string(), ErrorFerris::Error, ErrorButtons::ResetState)),
                    _ => None,
                };
                if let Some((e, ferris, button)) = set {
                    app.error_state.set(format!("State file: {}\n\nTry deleting: {}\n\n(Warning: this will delete your Gupax settings)\n\n", e, app.state_path.display()), ferris, button);
                }

                State::new()
            }
        };

        // Clamp window resolution scaling values.
        app.state.gupax.selected_scale = crate::miscs::clamp_scale(app.state.gupax.selected_scale);

        app.og = arc_mut!(app.state.clone());
        // Read node list
        info!("App Init | Reading node list...");
        app.node_vec = match Node::get(&app.node_path) {
            Ok(toml) => toml,
            Err(err) => {
                error!("Node ... {err}");
                let (e, ferris, button) = match err {
                    Io(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Path(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Serialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Deserialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Format(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Merge(e) => (e.to_string(), ErrorFerris::Error, ErrorButtons::ResetState),
                    Parse(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                };
                app.error_state.set(format!("Node list: {}\n\nTry deleting: {}\n\n(Warning: this will delete your custom node list)\n\n", e, app.node_path.display()), ferris, button);
                Node::new_vec()
            }
        };
        app.og_node_vec.clone_from(&app.node_vec);
        debug!("Node Vec:");
        debug!("{:#?}", app.node_vec);
        // Read pool list
        info!("App Init | Reading pool list...");
        app.pool_vec = match Pool::get(&app.pool_path) {
            Ok(toml) => toml,
            Err(err) => {
                error!("Pool ... {err}");
                let (e, ferris, button) = match err {
                    Io(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Path(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Serialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Deserialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Format(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Merge(e) => (e.to_string(), ErrorFerris::Error, ErrorButtons::ResetState),
                    Parse(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                };
                app.error_state.set(format!("Pool list: {}\n\nTry deleting: {}\n\n(Warning: this will delete your custom pool list)\n\n", e, app.pool_path.display()), ferris, button);
                Pool::new_vec()
            }
        };
        app.og_pool_vec.clone_from(&app.pool_vec);
        debug!("Pool Vec:");
        debug!("{:#?}", app.pool_vec);

        //----------------------------------------------------------------------------------------------------
        // Read [GupaxP2poolApi] disk files
        let mut gupax_p2pool_api = app.gupax_p2pool_api.lock().unwrap();
        match GupaxP2poolApi::create_all_files(&app.gupax_p2pool_api_path) {
            Ok(_) => info!("App Init | Creating Gupax-P2Pool API files ... OK"),
            Err(err) => {
                error!("GupaxP2poolApi ... {err}");
                let (e, ferris, button) = match err {
                    Io(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Path(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Serialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Deserialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Format(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Merge(e) => (e.to_string(), ErrorFerris::Error, ErrorButtons::ResetState),
                    Parse(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                };
                app.error_state.set(format!("Gupax P2Pool Stats: {}\n\nTry deleting: {}\n\n(Warning: this will delete your P2Pool payout history...!)\n\n", e, app.gupax_p2pool_api_path.display()), ferris, button);
            }
        }
        info!("App Init | Reading Gupax-P2Pool API files...");
        match gupax_p2pool_api.read_all_files_and_update() {
            Ok(_) => {
                info!(
                    "GupaxP2poolApi ... Payouts: {} | XMR (atomic-units): {}",
                    gupax_p2pool_api.payout, gupax_p2pool_api.xmr,
                );
            }
            Err(err) => {
                error!("GupaxP2poolApi ... {err}");
                let (e, ferris, button) = match err {
                    Io(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Path(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Serialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Deserialize(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Format(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                    Merge(e) => (e.to_string(), ErrorFerris::Error, ErrorButtons::ResetState),
                    Parse(e) => (e.to_string(), ErrorFerris::Panic, ErrorButtons::Quit),
                };
                app.error_state.set(format!("Gupax P2Pool Stats: {}\n\nTry deleting: {}\n\n(Warning: this will delete your P2Pool payout history...!)\n\n", e, app.gupax_p2pool_api_path.display()), ferris, button);
            }
        };
        drop(gupax_p2pool_api);
        app.helper.lock().unwrap().gupax_p2pool_api = Arc::clone(&app.gupax_p2pool_api);

        //----------------------------------------------------------------------------------------------------
        let mut og = app.og.lock().unwrap(); // Lock [og]
        // Handle max threads
        info!("App Init | Handling max thread overflow...");
        og.xmrig.max_threads = app.max_threads;
        let current = og.xmrig.current_threads;
        let max = og.xmrig.max_threads;
        if current > max {
            og.xmrig.current_threads = max;
        }
        // Handle [node_vec] overflow
        info!("App Init | Handling [node_vec] overflow");
        if og.p2pool.selected_node.index > app.og_node_vec.len() {
            warn!(
                "App | Overflowing manual node index [{} > {}]",
                og.p2pool.selected_node.index,
                app.og_node_vec.len()
            );
            let (name, node) = match app.og_node_vec.first() {
                Some(zero) => zero.clone(),
                None => Node::new_tuple(),
            };
            og.p2pool.selected_node.index = 0;
            og.p2pool.selected_node.name.clone_from(&name);
            og.p2pool
                .selected_node
                .ip
                .clone_from(&node.ip().to_string());
            og.p2pool
                .selected_node
                .rpc
                .clone_from(&node.port().to_string());
            og.p2pool
                .selected_node
                .zmq_rig
                .clone_from(&node.custom().to_string());
            app.state.p2pool.selected_node.index = 0;
            app.state.p2pool.selected_node.name = name;
            app.state.p2pool.selected_node.ip = node.ip().to_string();
            app.state.p2pool.selected_node.rpc = node.port().to_string();
            app.state.p2pool.selected_node.zmq_rig = node.custom().to_string();
        }
        // Handle [pool_vec] overflow
        info!("App Init | Handling [pool_vec] overflow...");
        if og.xmrig.selected_pool.index > app.og_pool_vec.len() {
            warn!(
                "App | Overflowing manual pool index [{} > {}], resetting to 1",
                og.xmrig.selected_pool.index,
                app.og_pool_vec.len()
            );
            let (name, pool) = match app.og_pool_vec.first() {
                Some(zero) => zero.clone(),
                None => Pool::new_tuple(),
            };
            og.xmrig.selected_pool.index = 0;
            og.xmrig.selected_pool.name.clone_from(&name);
            og.xmrig.selected_pool.ip.clone_from(&pool.ip().to_string());
            og.xmrig
                .selected_pool
                .rpc
                .clone_from(&pool.port().to_string());
            app.state.xmrig.selected_pool.index = 0;
            app.state.xmrig.selected_pool.name = name;
            app.state.xmrig.selected_pool.ip = pool.ip().to_string();
            app.state.xmrig.selected_pool.rpc = pool.port().to_string();
            if og.xmrig_proxy.selected_pool.index > app.og_pool_vec.len() {
                warn!(
                    "App | Overflowing manual pool index [{} > {}], resetting to 1",
                    og.xmrig_proxy.selected_pool.index,
                    app.og_pool_vec.len()
                );
                let (name, pool) = match app.og_pool_vec.first() {
                    Some(zero) => zero.clone(),
                    None => Pool::new_tuple(),
                };
                og.xmrig_proxy.selected_pool.index = 0;
                og.xmrig_proxy.selected_pool.name.clone_from(&name);
                og.xmrig_proxy
                    .selected_pool
                    .ip
                    .clone_from(&pool.ip().to_string());
                og.xmrig_proxy
                    .selected_pool
                    .rpc
                    .clone_from(&pool.port().to_string());
                app.state.xmrig_proxy.selected_pool.index = 0;
                app.state.xmrig_proxy.selected_pool.name = name;
                app.state.xmrig_proxy.selected_pool.ip = pool.ip().to_string();
                app.state.xmrig_proxy.selected_pool.rpc = pool.port().to_string();
            }
        }

        // Apply TOML values to [Update]
        info!("App Init | Applying TOML values to [Update]...");
        let node_path = og.gupax.absolute_node_path.clone();
        let p2pool_path = og.gupax.absolute_p2pool_path.clone();
        let xmrig_path = og.gupax.absolute_xmrig_path.clone();
        let xmrig_proxy_path = og.gupax.absolute_xp_path.clone();
        app.update = arc_mut!(Update::new(
            app.exe.clone(),
            p2pool_path,
            xmrig_path,
            xmrig_proxy_path,
            node_path
        ));

        // Set state version as compiled in version
        info!("App Init | Setting state Gupax version...");
        og.version.lock().unwrap().gupax = GUPAX_VERSION.to_string();
        app.state.version.lock().unwrap().gupax = GUPAX_VERSION.to_string();

        // Set saved [Tab], only if it is not hidden
        info!("App Init | Setting saved [Tab]...");
        if Tab::from_show_processes(&app.state.gupax.show_processes).contains(&app.state.gupax.tab)
        {
            app.tab = app.state.gupax.tab
        }

        // Set saved prefer local node to runtime
        app.p2pool_api.lock().unwrap().prefer_local_node = app.state.p2pool.prefer_local_node;

        // Set saved choice of use of sidechain HR
        app.xvb_api.lock().unwrap().use_p2pool_sidechain_hr = app.state.xvb.use_p2pool_sidechain_hr;

        // Set saved choice for notifications
        app.notifications_api.lock().unwrap().notifications = app.state.gupax.notifications.clone();

        // Set saved Hero mode to runtime.
        debug!("Setting runtime_mode & runtime_manual_amount");
        // apply hero if simple mode saved with checkbox true, will let default to auto otherwise
        if app.state.xvb.simple {
            if app.state.xvb.simple_hero_mode {
                app.xvb_api.lock().unwrap().stats_priv.runtime_mode = RuntimeMode::Hero
            }
        } else {
            app.xvb_api.lock().unwrap().stats_priv.runtime_mode = app.state.xvb.mode.clone().into();
        }
        app.xvb_api.lock().unwrap().stats_priv.runtime_manual_amount =
            app.state.xvb.manual_amount_raw;

        drop(og); // Unlock [og]

        // Spawn the "Helper" thread.
        info!("Helper | Spawning helper thread...");
        Helper::spawn_helper(&app.helper, app.pid, app.max_threads);
        info!("Helper ... OK");

        // Check for privilege. Should be Admin on [Windows] and NOT root on Unix.
        info!("App Init | Checking for privilege level...");
        #[cfg(target_os = "windows")]
        if is_elevated::is_elevated() {
            app.admin = true;
        } else {
            error!("Windows | Admin user not detected!");
            app.error_state.set("Gupax was not launched as Administrator!\nBe warned, XMRig might have less hashrate!".to_string(), ErrorFerris::Sudo, ErrorButtons::WindowsAdmin);
        }
        #[cfg(target_family = "unix")]
        if sudo_check::check() != sudo_check::RunningAs::User {
            let id = sudo_check::check();
            error!("Unix | Regular user not detected: [{id:?}]");
            app.error_state.set(format!("Gupax was launched as: [{id:?}]\nPlease launch Gupax with regular user permissions."), ErrorFerris::Panic, ErrorButtons::Quit);
        }
        // macOS re-locates "dangerous" applications into some read-only "/private" directory.
        // It _seems_ to be fixed by moving [Gupax.app] into "/Applications".
        // So, detect if we are in in "/private" and warn the user.
        #[cfg(target_os = "macos")]
        if app.exe.starts_with("/private") {
            app.error_state.set(format!("macOS thinks Gupax is a virus!\n(macOS has relocated Gupax for security reasons)\n\nThe directory: [{}]\nSince this is a private read-only directory, it causes issues with updates and correctly locating P2Pool/XMRig. Please move Gupax into the [Applications] directory, this lets macOS relax a little.\n", app.exe), ErrorFerris::Panic, ErrorButtons::Quit);
        }

        info!("App ... OK");

        // Save the new version in the state file
        if let Err(e) = State::save(&mut app.state, &app.state_path) {
            error!("State file: {e}");
        }
        app
    }
    fn migrate_gupaxx_gupax(&mut self) {
        // check if gupax and gupaxx state path both exist
        let mut gupaxx_path = self.os_data_path.to_path_buf();
        gupaxx_path.pop();
        gupaxx_path.push("gupaxx");
        let mut gupaxx_state_path = gupaxx_path.to_path_buf();
        gupaxx_state_path.push(STATE_TOML);
        if gupaxx_state_path.exists() {
            if self.os_data_path.exists() {
                // check which one was used last
                // Some peoples may have installed gupaxx but for some reason keep to use gupax
                // We need to still use the settings from gupax in these case

                // We compare the last time gupax/gupaxx was used. If gupax is used more recently than gupaxx, we keep the settings of gupax.
                // accessed metadata is better but it's not a certain value. modified is used in backup.
                // Must be done before anything read the state file, else the result would be useless
                let mut time_modified_gupax = SystemTime::now();
                let mut time_accessed_gupax = None;
                if let Ok(metadata) = std::fs::metadata(&self.state_path) {
                    if let Ok(time) = metadata.accessed() {
                        time_accessed_gupax = Some(time)
                    }
                    if let Ok(time) = metadata.modified() {
                        time_modified_gupax = time
                    }
                }
                let mut time_modified_gupaxx = SystemTime::now();
                let mut time_accessed_gupaxx = None;
                if let Ok(metadata) = std::fs::metadata(&gupaxx_state_path) {
                    if let Ok(time) = metadata.accessed() {
                        time_accessed_gupaxx = Some(time)
                    }
                    if let Ok(time) = metadata.modified() {
                        time_modified_gupaxx = time
                    }
                }
                // do not migrate if gupax is already at v2
                let gupax_version = State::get_major_version(&self.state_path).unwrap_or_default();
                if gupax_version >= 2 {
                    return;
                }

                // if gupax state is not yet upgraded and gupaxx was accessed more recently, backup gupax and move gupaxx to gupax.
                let gupaxx_more_recent_access = if let Some(accessed_time_gupax) =
                    time_accessed_gupax
                    && let Some(accessed_time_gupaxx) = time_accessed_gupaxx
                    && let Ok(gupax_accessed) = accessed_time_gupax.elapsed()
                    && let Ok(gupaxx_accessed) = accessed_time_gupaxx.elapsed()
                    && gupax_accessed > gupaxx_accessed
                {
                    true
                } else {
                    false
                };
                let gupaxx_more_recent_modification = if let Ok(gupax_modified) =
                    time_modified_gupax.elapsed()
                    && let Ok(gupaxx_modified) = time_modified_gupaxx.elapsed()
                    && gupax_modified > gupaxx_modified
                {
                    true
                } else {
                    false
                };

                if gupaxx_more_recent_access || gupaxx_more_recent_modification {
                    info!(" App Init | Migration of gupaxx local data to replace gupax");
                    // backup gupax local data
                    let mut backup_path = self.os_data_path.to_path_buf();
                    backup_path.pop();
                    backup_path.push("gupax_v1");
                    if backup_path.exists() {
                        warn!(
                            "backup path of gupax exists, but this is a new migration. Replacing backup."
                        );
                        if let Err(e) = remove_dir_all(&backup_path) {
                            warn!("error while removing backup path: {e}")
                        }
                    }

                    if let Err(e) = copy_dir(&self.os_data_path, backup_path) {
                        warn!(
                            "Gupax local data was attempted to be backup to gupax_v1, but this error occurred: {e}"
                        );
                    }
                    // copy gupaxx local data to gupax
                    if let Err(e) = remove_dir_all(&self.os_data_path) {
                        warn!("error while removing gupax path: {e}")
                    }
                    if let Err(e) = copy_dir(gupaxx_path, &self.os_data_path) {
                        warn!(
                            "Gupaxx local data was attempted to be moved to gupax, but this error occurred: {e}"
                        );
                    }
                }
            } else {
                // gupax does not exist but gupaxx is
                // copy gupaxx local data to gupax
                info!(" App Init | Migration of gupaxx local data to gupax");
                if let Err(e) = copy_dir(gupaxx_path, &self.os_data_path) {
                    warn!(
                        "Gupaxx local data was attempted to be moved to gupax, but this error occurred: {e}"
                    );
                }
            }
        }
    }
}
//---------------------------------------------------------------------------------------------------- [Tab] Enum + Impl
// The tabs inside [App].
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Display, EnumIter, EnumCount, Default,
)]
pub enum Tab {
    #[default]
    About,
    Status,
    #[display("Gupax")]
    Gupax,
    Node,
    P2pool,
    Xmrig,
    #[display("Proxy")]
    XmrigProxy,
    Xvb,
}

impl Tab {
    pub fn linked_process(&self) -> Option<ProcessName> {
        match self {
            Tab::About => None,
            Tab::Status => None,
            Tab::Gupax => None,
            Tab::Node => Some(ProcessName::Node),
            Tab::P2pool => Some(ProcessName::P2pool),
            Tab::Xmrig => Some(ProcessName::Xmrig),
            Tab::XmrigProxy => Some(ProcessName::XmrigProxy),
            Tab::Xvb => Some(ProcessName::Xvb),
        }
    }
    pub fn msg_default_tab(&self) -> &str {
        match self {
            Tab::About => GUPAX_TAB_ABOUT,
            Tab::Status => GUPAX_TAB_STATUS,
            Tab::Gupax => GUPAX_TAB_GUPAX,
            Tab::Node => GUPAX_TAB_NODE,
            Tab::P2pool => GUPAX_TAB_P2POOL,
            Tab::Xmrig => GUPAX_TAB_XMRIG,
            Tab::XmrigProxy => GUPAX_TAB_XMRIG_PROXY,
            Tab::Xvb => GUPAX_TAB_XVB,
        }
    }
    pub fn from_process_name(process: &ProcessName) -> Self {
        match process {
            ProcessName::Node => Tab::Node,
            ProcessName::P2pool => Tab::P2pool,
            ProcessName::Xmrig => Tab::Xmrig,
            ProcessName::XmrigProxy => Tab::XmrigProxy,
            ProcessName::Xvb => Tab::Xvb,
        }
    }
    pub fn from_show_processes(processes: &[ProcessName]) -> Vec<Self> {
        // tabs that can not be hidden
        let mut tabs = vec![Self::About, Self::Status, Self::Gupax];
        processes
            .iter()
            .for_each(|p| tabs.push(Tab::from_process_name(p)));
        tabs
    }
}
//---------------------------------------------------------------------------------------------------- [Restart] Enum
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    No,  // We don't need to restart
    Yes, // We updated, user should probably (but isn't required to) restart
}
//---------------------------------------------------------------------------------------------------- CPU Benchmarks.
#[derive(Debug, Serialize, Deserialize)]
pub struct Benchmark {
    pub cpu: String,
    pub rank: u16,
    pub percent: f32,
    pub benchmarks: u16,
    pub average: f32,
    pub high: f32,
    pub low: f32,
}
#[cfg(test)]
mod test {
    use crate::miscs::cmp_f64;

    #[test]
    fn detect_benchmark_cpu() {
        use crate::app::Benchmark;
        let cpu = "AMD Ryzen 9 5950X 16-Core Processor";

        let benchmarks: Vec<Benchmark> = {
            let mut json: Vec<Benchmark> =
                serde_json::from_slice(include_bytes!("../../assets/cpu.json")).unwrap();
            json.sort_by(|a, b| cmp_f64(strsim::jaro(&b.cpu, cpu), strsim::jaro(&a.cpu, cpu)));
            json
        };

        assert!(benchmarks[0].cpu == "AMD Ryzen 9 5950X 16-Core Processor");
    }
}
