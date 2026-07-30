use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::app::submenu_enum::SubmenuP2pool;
use crate::app::{App, AppEgui, Tab, WindowState};
use crate::components::node::RemoteNodes;
#[cfg(not(feature = "distro"))]
use crate::errors::{ErrorButtons, ErrorFerris};
use crate::helper::{Helper, ProcessName, ProcessState};
use crate::inits::init_text_styles;
#[cfg(not(feature = "distro"))]
use crate::utils::errors::WarnUpdateData;
use crate::{NODE_MIDDLE, P2POOL_MIDDLE, SECOND, XMRIG_MIDDLE, XMRIG_PROXY_MIDDLE, XVB_MIDDLE};
use derive_more::derive::{Deref, DerefMut};
use log::{debug, error, info, warn};

/// The eframe shell around [`AppEgui`], and the owner of everything that
/// must stay on the main thread: the tray icon is not `Send` on
/// Windows/macOS, so it can not live inside [`App`]/[`AppEgui`], which
/// other threads hold (e.g. the Ctrl+C handler in daemon mode).
/// Dropped and re-created with the window on Linux, while its fields'
/// shared contents live on in `main()`.
pub struct GuiApp {
    pub app: AppEgui,
    /// Filled lazily on the first frame, emptied when disabled.
    /// Shared with `main()` so the tray outlives the window on Linux.
    tray_slot: crate::tray::TraySlot,
    /// Command channel between tray/single-instance senders and the GUI.
    tray_channel: Rc<crate::tray::TrayChannel>,
    /// Tray creation failed: don't retry every frame, and never hide the
    /// window (the app would become unreachable).
    tray_failed: bool,
    /// Windows: a frame has seen the window really unmapped, so finding it
    /// mapped again means something outside Gupax did it.
    #[cfg(target_os = "windows")]
    window_seen_hidden: bool,
}

impl GuiApp {
    pub fn cc(
        cc: &eframe::CreationContext<'_>,
        resolution: egui::Vec2,
        app: AppEgui,
        tray_slot: crate::tray::TraySlot,
        tray_channel: Rc<crate::tray::TrayChannel>,
    ) -> Self {
        let app = AppEgui::cc(cc, resolution, app);
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = cc.window_handle()
                && let RawWindowHandle::Win32(h) = handle.as_raw()
            {
                crate::tray::MAIN_WINDOW_HWND
                    .store(h.hwnd.get(), std::sync::atomic::Ordering::Relaxed);
            }
        }
        // Point the wake-up callbacks to the (re-)created window's context.
        tray_channel.set_context(&cc.egui_ctx);
        Self {
            app,
            tray_slot,
            tray_channel,
            tray_failed: false,
            #[cfg(target_os = "windows")]
            window_seen_hidden: false,
        }
    }

    /// Create or remove the tray icon depending on the current settings.
    fn tray_sync(&mut self) {
        let wants_tray = {
            let mut app = self.app.inner.lock();
            // [--tray] forces a tray only for as long as it is the reason
            // the window is hidden. Once the user has the window back the
            // settings decide again, so unchecking both can remove the
            // icon instead of the flag pinning it for the whole session.
            if app.window_state == WindowState::Visible {
                app.start_in_tray_flag = false;
            }
            app.state.gupax.auto.hide_to_tray
                || app.state.gupax.auto.start_with_tray
                || app.start_in_tray_flag
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
        // Reconciled every frame instead of on creation: on Linux the icon
        // is drawn by the desktop's StatusNotifier host, which can come
        // and go while Gupax runs, so having created a tray icon does not
        // mean there is one to see. Everything that hides the window keys
        // off `tray_active`, and a tray nobody draws must not read as one
        // that is there -- Gupax would hide itself out of reach.
        let displayed = slot.as_ref().is_some_and(|tray| tray.icon_visible());
        let mut app = self.app.inner.lock();
        app.tray_active = displayed;
        // abandon a pending [--tray] hiding, it needs an icon to hide into
        if !displayed && app.window_state == WindowState::StartingInTray {
            app.window_state = WindowState::Visible;
        }
    }

    /// Keep the tray's Show/Hide menu entry, and whether Gupax is a
    /// windowed app at all, in sync with the window state (both
    /// deduplicated by the callee).
    fn tray_refresh(&self) {
        let visible = self.app.inner.lock().window_state == WindowState::Visible;
        let slot = self.tray_slot.lock();
        // Sitting in the tray means background app. Never without a tray
        // icon though, or the only way left to reach Gupax would be to
        // launch it again.
        crate::tray::set_windowed_app(visible || slot.is_none());
        if let Some(tray) = slot.as_ref() {
            tray.set_window_visible(visible);
        }
    }

    /// Take the window back when something outside Gupax un-hid it.
    ///
    /// Windows has no channel between launches: a second `gupax.exe` finds
    /// the running window by title and maps it with `ShowWindow` itself
    /// ([`crate::utils::single_instance`]), so no [`crate::tray::TrayCmd`]
    /// is ever queued and nothing else would leave
    /// [`WindowState::HiddenToTray`] -- leaving the user a window that
    /// paints nothing, since [`Self::ui`] returns early while hidden.
    ///
    /// The latch is what keeps this from re-showing the window Gupax is
    /// itself in the middle of hiding: `ViewportCommand::Visible(false)` is
    /// only applied after the frame that sends it, so the window is still
    /// mapped for one frame while the state already says hidden.
    #[cfg(target_os = "windows")]
    fn resync_native_visibility(&mut self, ctx: &egui::Context) {
        use std::sync::atomic::Ordering;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
        let hwnd = crate::tray::MAIN_WINDOW_HWND.load(Ordering::Relaxed);
        if hwnd == 0 {
            return;
        }
        if self.app.inner.lock().window_state != WindowState::HiddenToTray {
            self.window_seen_hidden = false;
            return;
        }
        let mapped = unsafe { IsWindowVisible(HWND(hwnd as *mut core::ffi::c_void)).as_bool() };
        if !mapped {
            self.window_seen_hidden = true;
        } else if self.window_seen_hidden {
            info!("Tray | the window was shown from outside Gupax, taking it back");
            self.window_seen_hidden = false;
            self.app.inner.lock().window_state = WindowState::Visible;
            ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Focus);
            ctx.request_repaint();
        }
    }
}

/// [--tray] on Linux: create the tray before any window, so not even a
/// hidden one exists until asked for; [`gui_background_loop`] then waits.
/// Returns whether a window must be created right away, which is the case
/// everywhere else: Windows/macOS need a running event loop for their tray
/// and hide the first window in [`GuiApp::logic`] instead.
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
            // Registering an icon does not mean anything displays it, and
            // this is the one path [`GuiApp::tray_sync`] can not correct
            // later: it starts with no window at all, so an icon nobody
            // draws would leave nothing to click and no window to come
            // back to. Keep the icon either way -- a StatusNotifier host
            // can still show up, and the reconcile in `tray_sync` picks
            // that up -- but start with a window.
            let displayed = tray.icon_visible();
            *tray_slot.lock() = Some(tray);
            let mut app = app.inner.lock();
            app.tray_active = displayed;
            app.window_state = if displayed {
                WindowState::HiddenToTray
            } else {
                warn!("Tray | the tray icon is not displayed, starting with a window");
                WindowState::Visible
            };
            !displayed
        }
        Err(e) => {
            warn!("Tray | creation failed, starting with a window: {e}");
            app.inner.lock().window_state = WindowState::Visible;
            true
        }
    }
}

/// Block until the tray (or a second Gupax launch) asks to show the
/// window. A Quit command shuts Gupax down right here; a dead channel
/// shows the window, because without one it would be unreachable.
fn wait_for_show(
    app: &AppEgui,
    tray_slot: &crate::tray::TraySlot,
    tray_channel: &crate::tray::TrayChannel,
) {
    match tray_channel.rx.recv() {
        Ok(crate::tray::TrayCmd::Quit) => crate::tray::quit_from_tray(app, tray_slot),
        Ok(crate::tray::TrayCmd::ToggleShowHide | crate::tray::TrayCmd::Show) => {
            // coalesce queued commands into a single "show"; Quit wins
            if crate::tray::drain(&tray_channel.rx).quit {
                crate::tray::quit_from_tray(app, tray_slot);
            }
        }
        Err(_) => warn!("Tray | command channel is gone, showing the window"),
    }
}

/// On Linux hiding to the tray closes the window instead of unmapping it
/// (`eframe::run_native` returns while Gupax keeps running), and this loop
/// re-creates it on the next tray activation, until a real quit.
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
        if let Some(tray) = tray_slot.lock().as_ref() {
            tray.set_window_visible(false);
        }
        wait_for_show(app, tray_slot, tray_channel);
        info!("Tray | creating the window");
        app.inner.lock().window_state = WindowState::Visible;
        run_gui(
            app,
            tray_slot,
            tray_channel,
            initial_window_size,
            resolution,
            name_version,
        );
    }
}

/// Run the eframe event loop until the window closes. If the configured
/// renderer crashes, flip to the other one and retry once (the flipped
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
    // Built fresh per run rather than cloned: `NativeOptions::clone`
    // deliberately drops builder hooks, and the icon behind
    // [`crate::inits::init_options`] is decoded once and shared.
    let options = |renderer| {
        let mut options = crate::inits::init_options(initial_window_size);
        options.renderer = renderer;
        if starting_in_tray {
            crate::tray::start_as_background_app(&mut options);
        }
        options
    };
    let renderer = app.inner.lock().current_renderer();
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
        }
    }
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
    // eframe calls this even while the window is hidden, unlike [ui].
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use egui::viewport::ViewportCommand;
        self.tray_sync();
        let drained = crate::tray::drain(&self.tray_channel.rx);
        if drained.quit {
            crate::tray::quit_from_tray(&self.app, &self.tray_slot);
        }
        if drained.show || drained.toggle {
            let mut app = self.app.inner.lock();
            let hidden = app.window_state == WindowState::HiddenToTray;
            // A toggle with no icon left can only show: hiding would put
            // Gupax where nothing can reach it. [tray_sync] has already
            // reconciled `tray_active` for this frame, so an icon that
            // stopped being displayed counts here too.
            if drained.show || hidden || !app.tray_active {
                app.window_state = WindowState::Visible;
                drop(app);
                debug!("Tray | showing the window");
                if !crate::tray::HIDE_BY_CLOSING {
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                }
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            } else {
                app.window_state = WindowState::HiddenToTray;
                app.notify_hidden_to_tray();
                drop(app);
                debug!("Tray | hiding the window to the tray");
                // a window can not be hidden on Linux: destroy it, the
                // background loop re-creates it on the next activation
                ctx.send_viewport_cmd(if crate::tray::HIDE_BY_CLOSING {
                    ViewportCommand::Close
                } else {
                    ViewportCommand::Visible(false)
                });
            }
            ctx.request_repaint();
        }
        #[cfg(target_os = "windows")]
        self.resync_native_visibility(ctx);
        // Routed from here rather than from [ui], which eframe skips
        // whenever the viewport is not `visible()` -- minimized, or
        // occluded on macOS. A close request arriving on such a frame
        // would go unanswered, and eframe takes that for consent: the root
        // viewport closes, skipping hide-to-tray, the quit confirmation
        // and save-on-exit alike, and leaving the child processes behind.
        // [App::quit] returns right away unless a close was requested, so
        // calling it every frame costs nothing.
        //
        // Before [tray_refresh] so that a close which hides to the tray
        // gets its menu entry and activation policy updated in the same
        // frame, like the hide above.
        self.app.inner.lock().quit(ctx);
        self.tray_refresh();
        // [--tray] on Windows/macOS: hide on the very first frame. The
        // command is processed right after eframe's forced first show,
        // within the same event-loop iteration, so the window never
        // appears. (On Linux no window is created at all: [start_in_tray])
        let mut app = self.app.inner.lock();
        if app.window_state == WindowState::StartingInTray && app.tray_active {
            app.window_state = WindowState::HiddenToTray;
            drop(app);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut app = self.app.inner.lock();
        if mitigate_wgpu_mem_leak(ui.ctx()) {
            return;
        }
        // Nothing is on screen while hidden to the tray. Returning also
        // skips the once-a-second `request_repaint_after` below, which is
        // what was keeping the frame loop running for an invisible
        // window. [logic] still runs every frame and a tray command still
        // wakes the loop through `Context::request_repaint`.
        //
        // eframe can not skip [ui] on its own: it gates it on
        // `ViewportInfo::visible()`, which is derived from minimized and
        // occluded only and so never reflects `ViewportCommand::Visible`.
        // An unmapped window raises no occlusion event on macOS, and
        // winit does not emit one on Windows at all.
        if app.window_state == WindowState::HiddenToTray {
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

        let mut selected_nodes = None;
        // crawl/pinged/selected remote node refresh
        if app.state.gupax.auto.crawl || app.tab == Tab::P2pool {
            let mut crawler_lock = app.crawler.lock().unwrap();
            let mut ping_lock = app.ping.lock().unwrap();
            let crawling = crawler_lock.crawling;
            let ping_nodes = &mut ping_lock.nodes;
            let crawl_nodes = &mut crawler_lock.nodes;

            if *ping_nodes != *crawl_nodes && !crawl_nodes.is_empty() {
                *ping_nodes = crawl_nodes.clone();
                if !crawling {
                    *crawl_nodes = RemoteNodes::default();
                }
            }

            // refresh the selected node with the fastest from the pinged nodes if it was empty
            if app.state.p2pool.selected_remote_node.is_none() {
                selected_nodes = ping_nodes.first().cloned();
            }
        }
        if (app.state.gupax.auto.crawl || app.tab == Tab::P2pool)
            && app.state.p2pool.selected_remote_node.is_none()
        {
            app.state.p2pool.selected_remote_node = selected_nodes;
        }
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
        match self.name {
            ProcessName::Node => Helper::stop_node(helper),
            ProcessName::P2pool => Helper::stop_p2pool(helper),
            ProcessName::Xmrig => Helper::stop_xmrig(helper),
            ProcessName::XmrigProxy => Helper::stop_xp(helper),
            ProcessName::Xvb => Helper::stop_xvb(helper),
        }
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
