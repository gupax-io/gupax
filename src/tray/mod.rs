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

//! System tray icon so Gupax can keep running in the background.
//!
//! Two backends behind the same [`TrayManager`] interface:
//! - Windows/macOS: the `tray-icon` crate.
//! - Linux: the `ksni` crate (StatusNotifierItem over DBus, on its own
//!   thread).
//!
//! All events (tray callbacks, second launches) go through one
//! [`TrayChannel`]: senders send a [`TrayCmd`] through a [`TraySender`]
//! and call `Context::request_repaint()`. The main thread handles them in
//! `GuiApp::logic()` while a window exists, or in the background wait loop
//! when none does. Quit and exit are commands too: only the main thread may
//! drop the tray icon, and Windows keeps the icon of an exited process in
//! the tray until it is dropped.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::mutex::Mutex;
use log::info;

use crate::app::AppEgui;

#[cfg(target_os = "linux")]
mod ksni_backend;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod tray_icon_backend;

#[cfg(target_os = "linux")]
use ksni_backend as backend;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tray_icon_backend as backend;

/// On Linux, hiding to the tray destroys the window and showing re-creates
/// it through the background loop (winit can not unmap a Wayland window,
/// and X11 uses the same code). Windows/macOS keep the window and unmap it.
pub const HIDE_BY_CLOSING: bool = cfg!(target_os = "linux");

/// Commands sent to the GUI main thread.
#[derive(Clone, Copy, Debug)]
pub enum TrayCmd {
    ToggleShowHide,
    /// Show and focus the window (sent by a second Gupax launch)
    Show,
    /// Shut Gupax down from the main thread, dropping the tray icon first
    Quit,
    /// Replace Gupax with the binary the updater wrote, removing the tray icon first
    Restart,
    /// Exit with this code, from the main thread, removing the tray icon first
    Exit(i32),
}

/// The egui context the tray callbacks request repaints from, if a window exists.
type CtxSlot = Arc<Mutex<Option<egui::Context>>>;

/// Sending side of the [`TrayChannel`]: sends a command and requests a
/// repaint, which eframe runs for a hidden window too. Used by the tray
/// backends and the single-instance listener, on any thread.
#[derive(Clone)]
pub struct TraySender {
    tx: Sender<TrayCmd>,
    ctx: CtxSlot,
}

impl TraySender {
    pub fn send(&self, cmd: TrayCmd) {
        let _ = self.tx.send(cmd);
        if let Some(ctx) = self.ctx.lock().as_ref() {
            ctx.request_repaint();
        }
    }
}

/// Set by `main()` in GUI mode, so any thread can send commands to the main
/// thread through [`request`]. Empty in daemon mode, which has no main loop.
static GLOBAL_SENDER: std::sync::OnceLock<TraySender> = std::sync::OnceLock::new();

/// Ask the main thread to handle `cmd`. Returns `false` when nothing is
/// listening and the caller has to act by itself.
pub fn request(cmd: TrayCmd) -> bool {
    match GLOBAL_SENDER.get() {
        Some(sender) => {
            sender.send(cmd);
            true
        }
        None => false,
    }
}

/// Receiving side, owned by the main thread for the whole process, while
/// windows and tray icons are created and removed.
pub struct TrayChannel {
    pub rx: Receiver<TrayCmd>,
    sender: TraySender,
}

impl TrayChannel {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            rx,
            sender: TraySender {
                tx,
                ctx: Arc::new(Mutex::new(None)),
            },
        }
    }

    pub fn sender(&self) -> TraySender {
        self.sender.clone()
    }

    /// Make [`request`] send to this channel.
    pub fn set_global(&self) {
        let _ = GLOBAL_SENDER.set(self.sender.clone());
    }

    /// Set the egui context the senders request repaints from.
    /// Must be set again when the window is (re-)created.
    pub fn set_context(&self, ctx: &egui::Context) {
        *self.sender.ctx.lock() = Some(ctx.clone());
    }

    /// Drop the context of a closed window, which holds its fonts and
    /// textures.
    pub fn clear_context(&self) {
        *self.sender.ctx.lock() = None;
    }
}

/// Commands accumulated by [`drain`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drained {
    /// Any number of queued clicks applies a single toggle
    pub toggle: bool,
    pub show: bool,
    pub quit: bool,
    pub restart: bool,
    pub exit: Option<i32>,
}

impl Drained {
    pub fn push(&mut self, cmd: TrayCmd) {
        match cmd {
            TrayCmd::ToggleShowHide => self.toggle = true,
            TrayCmd::Show => self.show = true,
            TrayCmd::Quit => self.quit = true,
            TrayCmd::Restart => self.restart = true,
            TrayCmd::Exit(code) => self.exit = Some(code),
        }
    }
}

/// Take every queued command; returns immediately when there is none.
pub fn drain(rx: &Receiver<TrayCmd>) -> Drained {
    let mut drained = Drained::default();
    while let Ok(cmd) = rx.try_recv() {
        drained.push(cmd);
    }
    drained
}

/// The tray, shared between `main()` and the GUI so it can outlive the
/// window (on Linux the window is destroyed while hidden to the tray).
/// Only accessed from the main thread.
pub type TraySlot = Arc<Mutex<Option<TrayManager>>>;

/// Owns the native tray icon (dropping it removes the icon) and keeps its
/// menu in sync with the window state. Not `Send` on Windows/macOS.
pub struct TrayManager {
    backend: backend::TrayBackend,
    /// Last value sent to the backend, to call it on changes only.
    window_visible: Option<bool>,
}

impl TrayManager {
    /// On Windows/macOS this must be called on the main thread, after the
    /// event loop started (i.e. from the first `logic()` call).
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        let backend = backend::TrayBackend::new(sender)?;
        info!("Tray | icon created");
        Ok(Self {
            backend,
            window_visible: None,
        })
    }

    /// Whether the icon is displayed. On Linux the icon is drawn by the
    /// StatusNotifier host of the desktop, which may be absent, so creating
    /// one can succeed while nothing shows. Checked every frame, as it can
    /// change while Gupax runs.
    pub fn icon_visible(&self) -> bool {
        self.backend.icon_visible()
    }

    /// Adapt the Show/Hide menu entry to the window state.
    pub fn set_window_visible(&mut self, visible: bool) {
        if self.window_visible.replace(visible) != Some(visible) {
            self.backend.set_window_visible(visible);
        }
    }
}

pub fn icon_displayed(tray_slot: &TraySlot) -> bool {
    tray_slot
        .lock()
        .as_ref()
        .is_some_and(TrayManager::icon_visible)
}

/// Remove the tray icon, then stop all processes and exit.
pub fn quit_from_tray(app: &AppEgui, tray_slot: &TraySlot) -> ! {
    info!("Shutdown | removing the tray icon...");
    *tray_slot.lock() = None;
    let wait = app.inner.lock().stop_all();
    wait.wait()
}

/// Restart for the auto-updater, removing the tray icon before the process exits.
pub fn restart_from_tray(tray_slot: &TraySlot) -> ! {
    info!("Tray | restarting into the updated Gupax...");
    *tray_slot.lock() = None;
    crate::components::update::restart_gupax()
}

/// Whether an update is running: quitting now could leave a partly written
/// binary.
fn update_in_progress(app: &AppEgui) -> bool {
    let app = app.inner.lock();
    let updating = app.update.lock().unwrap().updating;
    drop(app);
    updating
}

/// What a Quit among the drained commands asks for.
pub enum QuitRequest {
    Now,
    /// An update is running: show the quit question with its warning.
    Ask,
}

/// Run the commands that end the process, [`TrayCmd::Exit`] and a
/// [`TrayCmd::Restart`] with no Quit, and return what a Quit asks for.
/// `quitting` is a quit already confirmed or running.
pub fn settle(
    drained: &Drained,
    quitting: bool,
    app: &AppEgui,
    tray_slot: &TraySlot,
) -> Option<QuitRequest> {
    if let Some(code) = drained.exit {
        *tray_slot.lock() = None;
        crate::app::quit::goodbye(code);
    }
    if drained.restart && !drained.quit && !quitting {
        restart_from_tray(tray_slot);
    }
    if !drained.quit {
        return None;
    }
    // The tray's Quit skips the confirmation screen unless an update is
    // running, as that screen shows the update warning.
    if !quitting && update_in_progress(app) {
        log::warn!("Tray | Quit while an update is running, asking first");
        return Some(QuitRequest::Ask);
    }
    Some(QuitRequest::Now)
}

/// [--tray] on Windows/macOS: move the first window off-screen, as eframe
/// shows it after the first frame and the hide happens one frame later;
/// [`crate::app::App::window_parked`] moves it back on the first show. On
/// macOS the app also starts with no Dock icon, for the first
/// `eframe::run_native` of the process.
pub fn start_as_background_app(options: &mut eframe::NativeOptions) {
    options.viewport.position = Some(egui::Pos2::new(-32000.0, -32000.0));
    // The show eframe does after the first frame must not take the focus.
    options.viewport.active = Some(false);
    #[cfg(target_os = "macos")]
    macos::start_as_background_app(options);
}

/// Present Gupax as a windowed app, or as a background app with no
/// Dock/taskbar entry that can not become the frontmost application.
///
/// Only macOS needs this: on Windows a hidden window has no taskbar entry,
/// and on Linux the window is destroyed.
pub fn set_windowed_app(windowed: bool) {
    #[cfg(target_os = "macos")]
    macos::set_windowed_app(windowed);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = windowed;
    }
}

/// Show the window when macOS reactivates Gupax while it is hidden to the
/// tray, as opening Gupax.app again does.
pub fn show_on_reopen(sender: TraySender) {
    #[cfg(target_os = "macos")]
    macos::show_on_reopen(sender);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = sender;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn clicks_coalesce_into_one_toggle() {
        let (tx, rx) = channel();
        assert!(!drain(&rx).toggle);
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        assert!(drain(&rx).toggle);
        assert!(!drain(&rx).toggle);
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        assert!(drain(&rx).toggle);
    }

    #[test]
    fn every_command_survives_a_drain() {
        let (tx, rx) = channel();
        tx.send(TrayCmd::Show).unwrap();
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        tx.send(TrayCmd::Quit).unwrap();
        tx.send(TrayCmd::Restart).unwrap();
        tx.send(TrayCmd::Exit(0)).unwrap();
        let drained = drain(&rx);
        assert!(
            drained.show
                && drained.toggle
                && drained.quit
                && drained.restart
                && drained.exit.is_some()
        );
    }

    #[test]
    fn request_reports_when_nobody_listens() {
        // set_global is process-wide and set once; the tests never call it.
        assert!(!request(TrayCmd::Quit));
    }
}
