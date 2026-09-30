use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::app::submenu_enum::SubmenuP2pool;
use crate::app::{App, AppEgui, Tab, WindowState};
use crate::components::node::RemoteNodes;
#[cfg(any(target_os = "windows", not(feature = "distro")))]
use crate::errors::{ErrorButtons, ErrorFerris};
use crate::helper::{Helper, ProcessName, ProcessState};
use crate::inits::init_text_styles;
use crate::tray::QuitRequest;
#[cfg(not(feature = "distro"))]
use crate::utils::errors::WarnUpdateData;
use crate::utils::renderer::RendererAttempts;
use crate::{NODE_MIDDLE, P2POOL_MIDDLE, SECOND, XMRIG_MIDDLE, XMRIG_PROXY_MIDDLE, XVB_MIDDLE};
use derive_more::derive::{Deref, DerefMut};
use log::{debug, error, info, warn};

/// The eframe app around [`AppEgui`], with the state that must stay on the
/// main thread: the tray icon is not `Send` on Windows/macOS. Re-created
/// with each window on Linux.
pub struct GuiApp {
    pub app: AppEgui,
    /// Set on the first frame, emptied when the tray is disabled.
    tray_slot: crate::tray::TraySlot,
    tray_channel: Rc<crate::tray::TrayChannel>,
    /// Tray creation failed: it is not retried every frame, and the window
    /// is never hidden, as nothing could show it again.
    tray_failed: bool,
}

impl GuiApp {
    pub fn cc(
        cc: &eframe::CreationContext<'_>,
        resolution: egui::Vec2,
        app: AppEgui,
        tray_slot: crate::tray::TraySlot,
        tray_channel: Rc<crate::tray::TrayChannel>,
    ) -> Self {
        // The renderer works: eframe creates it before the app.
        RendererAttempts::new(&app.inner.lock().os_data_path).clear();
        let app = AppEgui::cc(cc, resolution, app);
        tray_channel.set_context(&cc.egui_ctx);
        crate::tray::show_on_reopen(tray_channel.sender());
        Self {
            app,
            tray_slot,
            tray_channel,
            tray_failed: false,
        }
    }

    /// Create or remove the tray icon to match the settings, and return
    /// whether one is displayed. Hiding the window depends on it, so this
    /// runs every frame (see [`crate::tray::TrayManager::icon_visible`]).
    fn tray_sync(&mut self) -> bool {
        let wants_tray = {
            let app = self.app.inner.lock();
            // The icon stays removed while the children are being stopped.
            app.quit_phase != crate::app::QuitPhase::Stopping
                && (app.state.gupax.auto.hide_to_tray
                    || app.state.gupax.auto.start_with_tray
                    || app.start_in_tray_flag)
        };
        let mut slot = self.tray_slot.lock();
        if wants_tray && slot.is_none() && !self.tray_failed {
            match crate::tray::TrayManager::new(self.tray_channel.sender()) {
                Ok(tray) => *slot = Some(tray),
                Err(e) => {
                    warn!("Tray | creation failed, tray features are disabled: {e}");
                    self.tray_failed = true;
                }
            }
        } else if !wants_tray && slot.is_some() {
            *slot = None;
        }
        drop(slot);
        crate::tray::icon_displayed(&self.tray_slot)
    }

    /// Update the tray's Show/Hide menu entry, and whether Gupax is a
    /// windowed app, to the window state. The callees skip unchanged values.
    fn tray_refresh(&self) {
        let visible = self.app.inner.lock().window_state == WindowState::Visible;
        let mut slot = self.tray_slot.lock();
        // Gupax is a background app while its window is hidden and a tray
        // icon exists to show it again.
        crate::tray::set_windowed_app(visible || slot.is_none());
        if let Some(tray) = slot.as_mut() {
            tray.set_window_visible(visible);
        }
    }

    fn show_window(&self, ctx: &egui::Context) {
        use egui::viewport::ViewportCommand;
        let mut app = self.app.inner.lock();
        app.window_state = WindowState::Visible;
        // [--tray] keeps a tray icon until the first show; from then on the
        // settings decide.
        app.start_in_tray_flag = false;
        let parked = std::mem::take(&mut app.window_parked);
        drop(app);
        if !crate::tray::HIDE_BY_CLOSING {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        }
        if parked {
            let monitor = ctx.input(|i| i.viewport().monitor_size);
            let size = ctx.viewport_rect().size();
            let corner = monitor
                .map(|m| ((m - size) / 2.0).max(egui::Vec2::ZERO))
                .unwrap_or(egui::Vec2::splat(64.0));
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(corner.to_pos2()));
        }
        // winit counts a minimized window as visible, so showing it also
        // un-minimizes it.
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    /// Stop the children and wait for them on a separate thread, so a
    /// visible window keeps painting the shutdown screen; a hidden or
    /// minimized one stays as it is. That thread sends
    /// [`crate::tray::TrayCmd::Exit`] once they exited.
    fn begin_quit(&self, ctx: &egui::Context) {
        let mut app = self.app.inner.lock();
        if app.quit_phase == crate::app::QuitPhase::Stopping {
            return;
        }
        info!("Tray | Quit selected, shutting down...");
        app.quit_phase = crate::app::QuitPhase::Stopping;
        let wait = app.stop_all();
        drop(app);
        // Only this thread may drop the tray icon.
        *self.tray_slot.lock() = None;
        std::thread::spawn(move || {
            wait.wait_children();
            crate::tray::request(crate::tray::TrayCmd::Exit(0));
        });
        ctx.request_repaint();
    }

    /// Store the window size in logical pixels, the unit a window is created
    /// with; egui points include Gupax's zoom.
    fn remember_window_size(&self, ctx: &egui::Context) {
        let size = ctx.viewport_rect().size() * ctx.zoom_factor();
        if size.x > 0.0 && size.y > 0.0 {
            self.app.inner.lock().last_window_size = Some(size);
        }
    }
}

/// [--tray] on Linux: create the tray before any window, so not even a
/// hidden one exists until asked for; [`gui_background_loop`] then waits.
/// Returns whether a window must be created right away, which is the case
/// everywhere else: Windows/macOS need a running event loop for their tray
/// and hide the first window in `GuiApp::logic`.
pub fn start_in_tray(
    app: &AppEgui,
    tray_slot: &crate::tray::TraySlot,
    tray_channel: &crate::tray::TrayChannel,
) -> bool {
    if !crate::tray::HIDE_BY_CLOSING || app.inner.lock().window_state != WindowState::StartingInTray
    {
        return true;
    }
    match crate::tray::TrayManager::new(tray_channel.sender()) {
        Ok(tray) => {
            // The icon registers on another thread; [`wait_for_show`] shows
            // the window if the icon is not displayed at its first check.
            *tray_slot.lock() = Some(tray);
            app.inner.lock().window_state = WindowState::HiddenToTray;
            false
        }
        Err(e) => {
            warn!("Tray | creation failed, starting with a window: {e}");
            app.inner.lock().window_state = WindowState::Visible;
            true
        }
    }
}

/// Wait until the tray (or a second Gupax launch) asks to show the
/// window. A Quit command shuts Gupax down here; a closed channel shows
/// the window, as nothing else can reach Gupax then.
///
/// The wait times out regularly: with no window there are no frames, so
/// the tray icon is checked here. When the icon stops being displayed,
/// the window is shown, as nothing else could show it.
fn wait_for_show(
    app: &AppEgui,
    tray_slot: &crate::tray::TraySlot,
    tray_channel: &crate::tray::TrayChannel,
) {
    loop {
        match tray_channel.rx.recv_timeout(SECOND) {
            Ok(cmd) => {
                // Drained with the commands queued after it, so a Quit takes
                // precedence over a Restart and several clicks give one show.
                let mut drained = crate::tray::drain(&tray_channel.rx);
                drained.push(cmd);
                match crate::tray::settle(&drained, false, app, tray_slot) {
                    Some(QuitRequest::Now) => crate::tray::quit_from_tray(app, tray_slot),
                    // Returning shows the window with the question.
                    Some(QuitRequest::Ask) => app.inner.lock().ask_quit_confirmation(),
                    None => {}
                }
                if drained.show || drained.toggle {
                    app.inner.lock().start_in_tray_flag = false;
                }
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if !crate::tray::icon_displayed(tray_slot) {
                    warn!("Tray | the icon is no longer displayed, showing the window");
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                warn!("Tray | command channel is gone, showing the window");
                return;
            }
        }
    }
}

/// On Linux hiding to the tray closes the window (`eframe::run_native`
/// returns while Gupax keeps running), and this loop re-creates it on the
/// next tray activation, until Gupax quits.
pub fn gui_background_loop(
    app: &AppEgui,
    tray_slot: &crate::tray::TraySlot,
    tray_channel: &Rc<crate::tray::TrayChannel>,
    initial_window_size: Option<egui::Vec2>,
    resolution: egui::Vec2,
    name_version: &str,
) {
    while app.inner.lock().window_state == WindowState::HiddenToTray {
        info!("Tray | running in the background without a window");
        if let Some(tray) = tray_slot.lock().as_mut() {
            tray.set_window_visible(false);
        }
        wait_for_show(app, tray_slot, tray_channel);
        info!("Tray | creating the window");
        let mut guard = app.inner.lock();
        guard.window_state = WindowState::Visible;
        guard.hide_close_pending = false;
        let size = guard.last_window_size.or(initial_window_size);
        drop(guard);
        run_gui(app, tray_slot, tray_channel, size, resolution, name_version);
    }
}

/// Switch to the other renderer when the configured one crashed the previous
/// start. Returns `false` when both renderers crashed.
pub fn pick_renderer(app: &AppEgui) -> bool {
    let mut guard = app.inner.lock();
    let attempts = RendererAttempts::new(&guard.os_data_path);
    if attempts.both_crashed() {
        error!("Both renderers crashed Gupax on its previous starts");
        error!("Delete [{}] to try them again", attempts.path().display());
        error!("Please open an issue on https://github.com/gupax-io/gupax/issues");
        return false;
    }
    let renderer = guard.current_renderer();
    if attempts.crashed(renderer) {
        let use_glow = !guard.state.gupax.renderer_use_glow;
        guard.persist_gupax_flag(|gupax| gupax.renderer_use_glow = use_glow);
        warn!(
            "The {renderer} renderer crashed Gupax on its previous start, switching to {}",
            guard.current_renderer()
        );
    }
    true
}

/// Run the eframe event loop until the window closes. If the configured
/// renderer crashes, switch to the other one and retry once (the new
/// choice is kept at the next state save).
pub fn run_gui(
    app: &AppEgui,
    tray_slot: &crate::tray::TraySlot,
    tray_channel: &Rc<crate::tray::TrayChannel>,
    initial_window_size: Option<egui::Vec2>,
    resolution: egui::Vec2,
    name_version: &str,
) {
    let starting_in_tray = app.inner.lock().window_state == WindowState::StartingInTray;
    // Built for each run: `NativeOptions::clone` drops the builder hooks.
    let options = |renderer| {
        let mut options = crate::inits::init_options(initial_window_size);
        options.renderer = renderer;
        if starting_in_tray {
            crate::tray::start_as_background_app(&mut options);
            app.inner.lock().window_parked = true;
        }
        options
    };
    let (renderer, attempts) = {
        let guard = app.inner.lock();
        (
            guard.current_renderer(),
            RendererAttempts::new(&guard.os_data_path),
        )
    };
    attempts.record(renderer);
    info!("starting Gupax with renderer: {renderer}");
    if let Err(e) = eframe::run_native(
        name_version,
        options(renderer),
        app_creator(app, tray_slot, tray_channel, resolution),
    ) {
        let mut guard = app.inner.lock();
        error!(
            "eframe crashed using the renderer: {}.Error: {e}",
            guard.current_renderer()
        );
        warn!(
            "Use the other renderer temporarily, the new renderer will be used at next startup if the settings are saved"
        );
        guard.state.gupax.renderer_use_glow = !guard.state.gupax.renderer_use_glow;
        let renderer = guard.current_renderer();
        warn!("Restarting with Gupax with renderer {renderer}");
        drop(guard);
        attempts.record(renderer);
        if let Err(e) = eframe::run_native(
            name_version,
            options(renderer),
            app_creator(app, tray_slot, tray_channel, resolution),
        ) {
            error!(
                "eframe crashed using the renderer: {}.Error: {e}",
                app.inner.lock().current_renderer()
            );
            error!(
                "crashed with both renderer: Please open an issue on https://github.com/gupax-io/gupax/issues"
            );
            // [init_auto] already started the node and the miners, and no
            // window is left to quit from. The renderer switch was for the
            // retry only: revert it before the shutdown saves the settings.
            let mut guard = app.inner.lock();
            guard.state.gupax.renderer_use_glow = !guard.state.gupax.renderer_use_glow;
            drop(guard);
            crate::tray::quit_from_tray(app, tray_slot);
        }
    }
    tray_channel.clear_context();
}

fn app_creator(
    app: &AppEgui,
    tray_slot: &crate::tray::TraySlot,
    tray_channel: &Rc<crate::tray::TrayChannel>,
    resolution: egui::Vec2,
) -> eframe::AppCreator<'static> {
    let app = app.clone();
    let tray_slot = tray_slot.clone();
    let tray_channel = tray_channel.clone();
    Box::new(move |cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Ok(Box::new(GuiApp::cc(
            cc,
            resolution,
            app,
            tray_slot,
            tray_channel,
        )))
    })
}

impl eframe::App for GuiApp {
    /// eframe can also exit by itself: macOS Cmd+Q and the Dock's Quit send
    /// no close request for [`App::quit`] to route. Gupax quits then, unless
    /// it is already quitting or the window only closed to the tray.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let app = self.app.inner.lock();
        let handled = app.quit_phase != crate::app::QuitPhase::None
            || app.window_state == WindowState::HiddenToTray;
        drop(app);
        if !handled {
            crate::tray::quit_from_tray(&self.app, &self.tray_slot);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.remember_window_size(ctx);
        self.app.inner.lock().refresh_remote_nodes();
        let tray_active = self.tray_sync();
        // With no tray icon, a [--tray] start shows its off-screen window.
        if !tray_active && self.app.inner.lock().window_state == WindowState::StartingInTray {
            self.show_window(ctx);
        }
        let drained = crate::tray::drain(&self.tray_channel.rx);
        let quitting = self.app.inner.lock().quit_phase != crate::app::QuitPhase::None;
        match crate::tray::settle(&drained, quitting, &self.app, &self.tray_slot) {
            Some(QuitRequest::Now) => self.begin_quit(ctx),
            Some(QuitRequest::Ask) => {
                self.show_window(ctx);
                self.app.inner.lock().ask_quit_confirmation();
            }
            None => {}
        }
        if drained.show || drained.toggle {
            let app = self.app.inner.lock();
            let hidden = app.window_state == WindowState::HiddenToTray;
            drop(app);
            // A toggle also shows a minimized window.
            let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
            // With no icon displayed, a toggle shows the window: nothing could
            // show a hidden one again.
            if drained.show || hidden || minimized || !tray_active {
                debug!("Tray | showing the window");
                self.show_window(ctx);
            } else {
                debug!("Tray | hiding the window to the tray");
                self.app.inner.lock().hide_to_tray(ctx, true);
            }
        }
        // Here: eframe skips [ui] while the window is minimized, or occluded
        // on macOS, and closes the window on a close request left uncancelled.
        // Before [tray_refresh], so a close that hides to the tray updates the
        // menu in the same frame.
        self.app.inner.lock().quit(ctx, tray_active);
        self.tray_refresh();
        // [--tray] on Windows/macOS: hide on the first frame (Linux creates no
        // window, see [start_in_tray]).
        let mut app = self.app.inner.lock();
        if app.window_state == WindowState::StartingInTray && tray_active {
            app.hide_to_tray(ctx, false);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut app = self.app.inner.lock();
        if mitigate_wgpu_mem_leak(ui.ctx()) {
            return;
        }
        if app.quit_phase != crate::app::QuitPhase::None {
            shutdown_screen(ui);
            return;
        }
        // Nothing is on screen in either tray state, and returning skips the
        // once-a-second `request_repaint_after` below, so a hidden window gets
        // no periodic frames. eframe calls [ui] for a hidden window:
        // `ViewportInfo::visible()` only covers minimized and occluded.
        if matches!(
            app.window_state,
            WindowState::HiddenToTray | WindowState::StartingInTray
        ) {
            // The frame hiding the window is still displayed: paint it with the
            // theme background, as the clear color is dark.
            egui::CentralPanel::default().show(ui, |_| {});
            return;
        }
        debug!("App | ----------- Start of [update()] -----------");
        // Handle Keys
        let (key, wants_input) = app.keys_handle(ui.ctx());

        // Refresh AT LEAST once a second
        debug!("App | Refreshing frame once per second");
        ui.ctx().request_repaint_after(SECOND);

        // Get P2Pool/XMRig process state.
        // These values are checked multiple times so
        // might as well check only once here to save
        // on a bunch of [.lock().unwrap()]s.
        let mut process_states = ProcessStatesGui::new(&app);
        // resize window and fonts if button "set" has been clicked in Gupax tab
        if app.must_resize {
            init_text_styles(ui.ctx(), app.state.gupax.selected_scale);
            app.must_resize = false;
        }
        // check for windows that a local instance of xmrig is not running outside of Gupax. Important because it could lead to crashes on this platform.
        // Warn only once per restart of Gupax.
        #[cfg(target_os = "windows")]
        if !app.xmrig_outside_warning_acknowledge
            && ProcessName::Xmrig
                .is_process_running(&mut app.helper.lock().unwrap().sys_info.lock().unwrap())
            && !process_states.find(ProcessName::Xmrig).alive
        {
            app.error_state.set("An instance of xmrig is running outside of Gupax.\nThis is not supported and could lead to crashes on this platform.\nPlease stop your local instance and start xmrig from Gupax Xmrig tab.", ErrorFerris::Error, ErrorButtons::Okay);
            app.xmrig_outside_warning_acknowledge = true;
        }

        #[cfg(not(feature = "distro"))]
        app.ask_download_binaries();
        // If there's an error, display [ErrorState] on the whole screen until user responds
        debug!("App | Checking if there is an error in [ErrorState]");
        if app.error_state.error {
            app.quit_error_panel(ui, &process_states, &key);
            return;
        }
        // Compare [og == state] & [node_vec/pool_vec] and enable diff if found.
        // The struct fields are compared directly because [Version]
        // contains Arc<Mutex>'s that cannot be compared easily.
        // They don't need to be compared anyway.
        debug!("App | Checking diff between [og] & [state]");
        let og = app.og.lock().unwrap();
        let diff = og.status != app.state.status
            || og.gupax != app.state.gupax
            || og.node != app.state.node
            || og.p2pool != app.state.p2pool
            || og.xmrig != app.state.xmrig
            || og.xmrig_proxy != app.state.xmrig_proxy
            || og.xvb != app.state.xvb
            || app.og_node_vec != app.node_vec
            || app.og_pool_vec != app.pool_vec;
        drop(og);
        app.diff = diff;

        // replace backup host by custom ones when user is in p2pool advanced sub menu
        // Only if the backup host is different from the custom ones
        if app.state.p2pool.submenu != SubmenuP2pool::Advanced && app.tab == Tab::P2pool {
            let mut backup_hosts = app.backup_hosts.lock().unwrap();
            if app.node_vec.iter().any(|(_, n)| backup_hosts.contains(n)) {
                *backup_hosts = app.node_vec.iter().map(|n| n.1.clone()).collect();
            }
        }

        app.top_panel(ui);
        app.bottom_panel(ui, &key, wants_input, &process_states);
        // xvb_is_alive is not the same for bottom and for middle.
        // for status we don't want to enable the column when it is retrying requests.
        // but also we don't want the user to be able to start it in this case.
        let p_xvb = process_states.find_mut(ProcessName::Xvb);
        p_xvb.alive = p_xvb.state != ProcessState::Dead;
        app.middle_panel(ui, key, &process_states);
    }
}
#[derive(Debug)]
pub struct ProcessStateGui {
    pub name: ProcessName,
    pub state: ProcessState,
    pub alive: bool,
    pub waiting: bool,
}

impl ProcessStateGui {
    pub fn run_middle_msg(&self) -> &str {
        match self.name {
            ProcessName::Node => NODE_MIDDLE,
            ProcessName::P2pool => P2POOL_MIDDLE,
            ProcessName::Xmrig => XMRIG_MIDDLE,
            ProcessName::XmrigProxy => XMRIG_PROXY_MIDDLE,
            ProcessName::Xvb => XVB_MIDDLE,
        }
    }
    pub fn stop(&self, helper: &Arc<Mutex<Helper>>) {
        (self.name.stop_fn())(helper)
    }
}

#[derive(Deref, DerefMut, Debug)]
pub struct ProcessStatesGui(Vec<ProcessStateGui>);

impl ProcessStatesGui {
    // order is important for lock
    pub fn new(app: &App) -> Self {
        let mut process_states = ProcessStatesGui(vec![]);
        for process in [
            &app.node,
            &app.p2pool,
            &app.xmrig,
            &app.xmrig_proxy,
            &app.xvb,
        ] {
            let lock = process.lock().unwrap();
            process_states.push(ProcessStateGui {
                name: lock.name,
                alive: lock.is_alive(),
                waiting: lock.is_waiting(),
                state: lock.state,
            });
        }
        process_states
    }
    pub fn is_alive(&self, name: ProcessName) -> bool {
        self.iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("This vec should always contains all Processes {self:?}"))
            .alive
    }
    pub fn find(&self, name: ProcessName) -> &ProcessStateGui {
        self.iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("This vec should always contains all Processes {self:?}"))
    }
    pub fn find_mut(&mut self, name: ProcessName) -> &mut ProcessStateGui {
        self.iter_mut()
            .find(|p| p.name == name)
            .expect("This vec should always contains all Processes")
    }
}

fn shutdown_screen(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.heading("Stopping Gupax...");
            ui.label("Waiting for the started processes to exit.");
        });
    });
}

/// Helper function to mitigate https://github.com/emilk/egui/issues/7434.
///
/// If this returns true, the app should early return in the `update()` function
/// or call `wgpu::Device::poll()`
fn mitigate_wgpu_mem_leak(ctx: &egui::Context) -> bool {
    let mut is_minimized = false;
    ctx.input(|reader| {
        is_minimized = reader.viewport().minimized.unwrap_or_default();
    });

    is_minimized
}

impl App {
    /// Move the crawled nodes into the ping list, and select the fastest one
    /// when no remote node is selected.
    ///
    /// Called from `GuiApp::logic`, and before the settings are saved on
    /// quit, for a Gupax hidden to the tray with no frames.
    pub fn refresh_remote_nodes(&mut self) {
        if !self.state.gupax.auto.crawl && self.tab != Tab::P2pool {
            return;
        }
        let mut crawler_lock = self.crawler.lock().unwrap();
        let mut ping_lock = self.ping.lock().unwrap();
        let crawling = crawler_lock.crawling;
        let ping_nodes = &mut ping_lock.nodes;
        let crawl_nodes = &mut crawler_lock.nodes;

        if *ping_nodes != *crawl_nodes && !crawl_nodes.is_empty() {
            *ping_nodes = crawl_nodes.clone();
            if !crawling {
                *crawl_nodes = RemoteNodes::default();
            }
        }

        if self.state.p2pool.selected_remote_node.is_none() {
            let fastest = ping_nodes.first().cloned();
            drop(ping_lock);
            drop(crawler_lock);
            self.state.p2pool.selected_remote_node = fastest;
        }
    }

    /// ask the user if he wants gupax to download the required binaries
    /// Will not ask if every path of binaries exist or if he checked the "do not check next time".
    #[cfg(not(feature = "distro"))]
    pub fn ask_download_binaries(&mut self) {
        if !self.ask_download_start_acknowledge && self.state.gupax.updates.ask_download_start {
            let p2pool_exist = self.state.gupax.absolute_p2pool_path.is_file();
            let node_exist = self.state.gupax.absolute_node_path.is_file();
            let xmrig_exist = self.state.gupax.absolute_xmrig_path.is_file();
            let xp_exist = self.state.gupax.absolute_xp_path.is_file();
            if !p2pool_exist || !node_exist || !xmrig_exist || !xp_exist {
                let msg = format!(
                    "Gupax is missing the binary of:\n{}\n{}\n{}\n{}\n\nDo you want it to download them now ?",
                    if !p2pool_exist { "P2Pool" } else { "" },
                    if !node_exist { "Node" } else { "" },
                    if !xmrig_exist { "XMRig" } else { "" },
                    if !xp_exist { "XMRig-Proxy" } else { "" }
                );
                let mut binaries = vec![];
                if !p2pool_exist {
                    binaries.push("p2pool".to_string());
                }
                if !node_exist {
                    binaries.push("monerod".to_string());
                }
                if !xmrig_exist {
                    binaries.push("xmrig".to_string());
                }
                if !xp_exist {
                    binaries.push("xmrig-proxy".to_string());
                }
                self.error_state.set(
                    msg,
                    ErrorFerris::Cute,
                    ErrorButtons::WarnUpdate(WarnUpdateData {
                        yes_button: "Download missing binaries".to_string(),
                        no_button: "No, and do not ask again".to_string(),
                        name: binaries.join(" "),
                    }),
                );
            }
        }
        // only check once at start
        self.ask_download_start_acknowledge = true;
    }
}
