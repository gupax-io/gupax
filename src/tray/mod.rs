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
//! - Linux: the `ksni` crate (StatusNotifierItem over DBus, no GTK
//!   dependency, runs on its own thread).
//!
//! All events (tray callbacks, second-launch activations) funnel into one
//! [`TrayChannel`]: producers push a [`TrayCmd`] through a [`TraySender`]
//! and wake eframe with `Context::request_repaint()`. The main thread
//! consumes them in `GuiApp::logic()` while a window exists, or in the
//! background wait loop when none does. Quit is also a command, so the
//! tray icon is always dropped by the main thread before exiting
//! (Windows leaves a ghost icon in the tray otherwise).

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

/// The HWND of the main window, set at startup, used by [`show_window_win32`].
#[cfg(target_os = "windows")]
pub static MAIN_WINDOW_HWND: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);

/// On Linux, hiding to the tray destroys the window and showing re-creates
/// it through the background loop (winit can not unmap a Wayland window,
/// and one behavior for both Linux backends is simpler). Windows/macOS
/// hide the window natively instead.
pub const HIDE_BY_CLOSING: bool = cfg!(target_os = "linux");

/// Commands sent to the GUI main thread.
#[derive(Clone, Copy, Debug)]
pub enum TrayCmd {
    ToggleShowHide,
    /// Show and focus the window (sent by a second Gupax launch)
    Show,
    /// Shut Gupax down from the main thread, dropping the tray icon first
    Quit,
}

/// The egui context the tray callbacks wake up, if a window exists.
type CtxSlot = Arc<Mutex<Option<egui::Context>>>;

/// Sending half of the [`TrayChannel`]: pushes a command and wakes eframe.
/// Given to the tray backends and the single-instance listener (any thread).
/// Hidden windows do wake up too: winit-macOS dispatches `RedrawRequested`
/// from its own queue regardless of visibility, and on Windows
/// [`show_window_win32`] force-shows the window first.
#[derive(Clone)]
pub struct TraySender {
    tx: Sender<TrayCmd>,
    ctx: CtxSlot,
}

impl TraySender {
    pub fn send(&self, cmd: TrayCmd) {
        let _ = self.tx.send(cmd);
        #[cfg(target_os = "windows")]
        show_window_win32(cmd);
        if let Some(ctx) = self.ctx.lock().as_ref() {
            ctx.request_repaint();
        }
    }
}

/// Receiving half, owned by the main thread for the whole process life
/// (the window and the tray icon both come and go, the channel does not).
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

    /// Register the egui context the senders wake up.
    /// Must be refreshed when the window is (re-)created.
    pub fn set_context(&self, ctx: &egui::Context) {
        *self.sender.ctx.lock() = Some(ctx.clone());
    }
}

impl Default for TrayChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Commands accumulated by [`drain`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drained {
    /// Any number of queued clicks applies a single toggle
    pub toggle: bool,
    pub show: bool,
    pub quit: bool,
}

/// Drain all queued tray commands without blocking.
pub fn drain(rx: &Receiver<TrayCmd>) -> Drained {
    let mut drained = Drained::default();
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            TrayCmd::ToggleShowHide => drained.toggle = true,
            TrayCmd::Show => drained.show = true,
            TrayCmd::Quit => drained.quit = true,
        }
    }
    drained
}

/// The tray, shared between `main()` and the GUI so it can outlive the
/// window (on Linux the window is destroyed while hidden to the tray).
/// Only ever accessed from the main thread.
pub type TraySlot = Arc<Mutex<Option<TrayManager>>>;

/// Owns the native tray icon (dropping it removes the icon) and keeps its
/// menu in sync with the window state. NOT `Send` on Windows/macOS.
pub struct TrayManager {
    backend: backend::TrayBackend,
    /// Last pushed value, to only touch the backend on changes.
    window_visible: Mutex<Option<bool>>,
}

impl TrayManager {
    /// On Windows/macOS this must be called on the main thread, after the
    /// event loop started (i.e. from the first `logic()` call).
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        let backend = backend::TrayBackend::new(sender)?;
        info!("Tray | icon created");
        Ok(Self {
            backend,
            window_visible: Mutex::new(None),
        })
    }

    /// Whether the icon is actually displayed. Creating one can succeed
    /// without that being true on Linux, where the icon is drawn by the
    /// desktop's StatusNotifier host rather than by Gupax, and there may
    /// not be one. Reconciled every frame, as it can change either way
    /// while Gupax runs.
    pub fn icon_visible(&self) -> bool {
        self.backend.icon_visible()
    }

    /// Adapt the Show/Hide menu entry to the window state.
    pub fn set_window_visible(&self, visible: bool) {
        let mut cached = self.window_visible.lock();
        if *cached == Some(visible) {
            return;
        }
        *cached = Some(visible);
        self.backend.set_window_visible(visible);
    }
}

/// Quit from the tray (or a queued equivalent): remove the icon first,
/// then stop all processes and exit.
pub fn quit_from_tray(app: &AppEgui, tray_slot: &TraySlot) -> ! {
    info!("Tray | Quit selected, shutting down...");
    *tray_slot.lock() = None;
    app.inner.lock().graceful_shutdown()
}

/// [--tray]: launch straight into the background, with no window mapped
/// and no Dock/taskbar entry. [`set_windowed_app`] and the first frame's
/// hiding can only act once a frame has run, which is late enough for a
/// window and a Dock icon to appear and disappear again.
///
/// Only macOS: winit dispatches `RedrawRequested` from its own queue
/// there, so the first frame still runs and still creates the tray. A
/// hidden Windows window gets no `WM_PAINT`, so the frame might never run
/// and the tray would never be created — which is what
/// [`show_window_win32`] works around. Linux never gets here: it creates
/// no window at all ([`HIDE_BY_CLOSING`]).
///
/// The activation policy half only affects the first
/// `eframe::run_native` of the process, as the winit event loop is built
/// once and then reused.
pub fn start_as_background_app(options: &mut eframe::NativeOptions) {
    #[cfg(target_os = "macos")]
    macos::start_as_background_app(options);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
    }
}

/// Present Gupax as a normal windowed app, or as a background app with no
/// Dock/taskbar entry that can not become the frontmost application.
///
/// Only macOS needs this: hiding the window is enough to leave the
/// taskbar on Windows, and to leave it on Linux the window is destroyed.
pub fn set_windowed_app(windowed: bool) {
    #[cfg(target_os = "macos")]
    macos::set_windowed_app(windowed);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = windowed;
    }
}

/// Windows: force-show the window natively so the frame loop resumes even
/// if the repaint request is swallowed for a hidden window. The real
/// visibility state is reconciled in `logic()` right after.
///
/// A hidden window is not merely unpainted: it gets no `WM_PAINT`, so
/// eframe runs no frame at all and nothing would consume the command --
/// including [`TrayCmd::Quit`], which has to be handled by the main thread
/// because it is the only one that may drop the tray icon. Mapping the
/// window is what makes the tray menu work while hidden, so it happens for
/// every command; only how it is mapped depends on which one.
#[cfg(target_os = "windows")]
fn show_window_win32(cmd: TrayCmd) {
    use std::sync::atomic::Ordering;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindowVisible, SW_SHOW, SW_SHOWNOACTIVATE, SetForegroundWindow, ShowWindow,
    };
    let hwnd = MAIN_WINDOW_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                if matches!(cmd, TrayCmd::Quit) {
                    // On the way out: a stale window has nothing to show
                    // and Gupax has no business stealing focus from
                    // whatever the user is doing for the moments it takes
                    // to stop the child processes.
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                } else {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = SetForegroundWindow(hwnd);
                }
            }
        }
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
        assert!(!drain(&rx).toggle); // already drained
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        assert!(drain(&rx).toggle); // a double-click is one toggle
    }

    #[test]
    fn show_and_quit_are_sticky() {
        let (tx, rx) = channel();
        tx.send(TrayCmd::Show).unwrap();
        tx.send(TrayCmd::ToggleShowHide).unwrap();
        tx.send(TrayCmd::Quit).unwrap();
        let drained = drain(&rx);
        assert!(drained.show && drained.toggle && drained.quit);
    }
}
