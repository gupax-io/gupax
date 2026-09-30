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

use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::utils::macros::sleep;

use log::{error, info};

use crate::errors::ErrorButtons;
use crate::errors::ErrorFerris;
use crate::helper::{Process, ProcessName, ProcessSignal, ProcessState, StopFn};

use super::{App, QuitPhase, WindowState};

/// Everything [`close_action`] needs to route a window close request.
struct CloseContext {
    hide_by_closing: bool,
    /// "Close to tray" setting
    hide_to_tray: bool,
    /// A tray icon is displayed
    tray_active: bool,
    asked_close_to_tray: bool,
    /// An error or question screen is displayed
    error_shown: bool,
    ask_before_quit: bool,
    /// The quit confirmation screen is displayed
    quit_confirmed: bool,
}

/// What a window close request must do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CloseAction {
    /// Let the window close and keep running in the tray (Linux)
    HideByClosing,
    /// Cancel the close and unmap the window
    HideByUnmapping,
    /// Cancel the close and ask whether to keep running in the tray
    AskTrayOnClose,
    /// Cancel the close and ask for confirmation
    AskQuit,
    /// Save (if enabled) and quit
    Quit,
}

/// A close from the taskbar can arrive while the window is minimized, and
/// eframe paints nothing then, so un-minimize the window for the question.
fn bring_question_into_view(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Focus);
}

/// Route a window close request. A pure function, so it is unit-tested.
fn close_action(c: &CloseContext) -> CloseAction {
    if c.hide_to_tray && c.tray_active {
        return if c.hide_by_closing {
            CloseAction::HideByClosing
        } else {
            CloseAction::HideByUnmapping
        };
    }
    if !c.asked_close_to_tray && !c.error_shown && c.tray_active {
        return CloseAction::AskTrayOnClose;
    }
    if c.ask_before_quit && !c.quit_confirmed {
        return CloseAction::AskQuit;
    }
    CloseAction::Quit
}

impl App {
    pub(super) fn quit(&mut self, ctx: &egui::Context, tray_active: bool) {
        // Used to be `eframe::App::on_close_event(&mut self) -> bool`.
        use egui::viewport::ViewportCommand;
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        // The close that hides to the tray arrives here a frame after it was
        // sent, and a show can arrive in between. The window is wanted again
        // then, so this close is not a quit.
        if std::mem::take(&mut self.hide_close_pending) {
            if self.window_state != WindowState::HiddenToTray {
                info!("Tray | shown again before the window closed, keeping it");
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            return;
        }
        match close_action(&CloseContext {
            hide_by_closing: crate::tray::HIDE_BY_CLOSING,
            hide_to_tray: self.state.gupax.auto.hide_to_tray,
            tray_active,
            asked_close_to_tray: self.state.gupax.asked_close_to_tray,
            error_shown: self.error_state.error,
            ask_before_quit: self.state.gupax.auto.ask_before_quit,
            quit_confirmed: self.error_state.quit_twice,
        }) {
            CloseAction::HideByClosing => {
                // The window closes: run_native returns and
                // gui_background_loop runs.
                info!("Tray | closing the window to the tray");
                self.window_state = WindowState::HiddenToTray;
                self.notify_hidden_to_tray();
            }
            CloseAction::HideByUnmapping => {
                info!("Tray | hiding the window to the tray, keeping Gupax running");
                self.hide_to_tray(ctx, true);
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            CloseAction::AskTrayOnClose => {
                self.error_state.set(
                    "Gupax can keep running in the system tray when the window is closed.\nKeep Gupax running in the tray when closing the window?\n(You can change this later with the \"Close to tray\" checkbox)",
                    ErrorFerris::Cute,
                    ErrorButtons::TrayOnClose,
                );
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                bring_question_into_view(ctx);
            }
            CloseAction::AskQuit => {
                info!("quit");
                self.ask_quit_confirmation();
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                bring_question_into_view(ctx);
            }
            CloseAction::Quit => {
                info!("quit");
                // The shutdown stops the children and exits the process, so the
                // close is cancelled.
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.request_quit();
            }
        }
    }

    /// Ask for a shutdown, carried out by the GUI thread: only it may drop
    /// the tray icon.
    pub fn request_quit(&mut self) {
        if self.quit_phase != QuitPhase::None {
            return;
        }
        self.quit_phase = QuitPhase::Asked;
        crate::tray::request(crate::tray::TrayCmd::Quit);
    }

    /// Show the quit confirmation screen; a close while it is displayed quits.
    pub fn ask_quit_confirmation(&mut self) {
        self.error_state
            .set("", ErrorFerris::Oops, ErrorButtons::StayQuit);
        self.error_state.quit_twice = true;
    }

    /// Hide the window to the tray. With `notify`, the first hide tells the
    /// user Gupax still runs.
    pub fn hide_to_tray(&mut self, ctx: &egui::Context, notify: bool) {
        self.window_state = WindowState::HiddenToTray;
        if notify {
            self.notify_hidden_to_tray();
        }
        if crate::tray::HIDE_BY_CLOSING {
            self.hide_close_pending = true;
            ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Close);
        } else {
            ctx.send_viewport_cmd(egui::viewport::ViewportCommand::Visible(false));
        }
    }

    /// Tell the user, once, that Gupax still runs in the tray.
    pub fn notify_hidden_to_tray(&mut self) {
        if self.state.gupax.notified_hidden_to_tray {
            return;
        }
        self.persist_gupax_flag(|gupax| gupax.notified_hidden_to_tray = true);
        std::thread::spawn(|| {
            crate::helper::notification::notif(
                "Gupax keeps running in the system tray.\nUse the tray icon to open it again or to quit.",
            );
        });
    }

    pub fn save_tray_on_close_answer(&mut self, enable: bool) {
        self.persist_gupax_flag(|gupax| {
            gupax.auto.hide_to_tray = enable;
            gupax.asked_close_to_tray = true;
        });
    }

    /// Write a setting Gupax decided by itself, and nothing else: the file
    /// gets this one change, so the tabs' unsaved edits stay unsaved, and
    /// [`App::og`] gets it too for [`App::diff`] and the Reset button.
    pub(super) fn persist_gupax_flag(&mut self, set: impl Fn(&mut crate::disk::state::Gupax)) {
        set(&mut self.state.gupax);
        set(&mut self.og.lock().unwrap().gupax);
        let saved = crate::disk::state::State::get(&self.state_path).and_then(|mut on_disk| {
            set(&mut on_disk.gupax);
            crate::disk::state::State::save(&mut on_disk, &self.state_path)
        });
        if let Err(e) = saved {
            error!("State file: {e}");
        }
    }

    /// Save the state if enabled, stop answering second launches and raise
    /// the stop flags. The returned wait holds no lock on the App.
    pub fn stop_all(&mut self) -> ShutdownWait {
        // Daemon mode does not edit the settings: saving them on Ctrl+C could
        // overwrite what the GUI saved.
        if self.state.gupax.auto.save_before_quit && !self.daemon {
            self.refresh_remote_nodes();
            self.save_before_quit();
        }
        // Before the wait, which can take tens of seconds, so a launch during
        // it starts after this Gupax and its children exited.
        crate::utils::single_instance::stop_answering();
        info!("Shutdown | stopping all child processes...");
        let mut stopped = Vec::new();
        for name in SHUTDOWN_ORDER {
            let (process, stop) = self.stoppable(name);
            let mut lock = process.lock().unwrap();
            // A detected Node was started outside of Gupax and keeps running.
            if lock.external || !(lock.is_alive() || lock.state == ProcessState::Waiting) {
                continue;
            }
            if lock.state == ProcessState::Waiting {
                // Cancels a pending restart: its thread starts the process
                // from Waiting only, and a start already past that check
                // gets the Stop once spawned.
                lock.state = ProcessState::Dead;
                lock.signal = ProcessSignal::Stop;
            } else {
                drop(lock);
                stop(&self.helper);
            }
            stopped.push((name, Arc::clone(process)));
        }
        ShutdownWait(stopped)
    }

    fn stoppable(&self, name: ProcessName) -> (&Arc<Mutex<Process>>, StopFn) {
        let process = match name {
            ProcessName::Node => &self.node,
            ProcessName::P2pool => &self.p2pool,
            ProcessName::Xmrig => &self.xmrig,
            ProcessName::XmrigProxy => &self.xmrig_proxy,
            ProcessName::Xvb => &self.xvb,
        };
        (process, name.stop_fn())
    }
}

/// The children [`App::stop_all`] asked to stop.
pub struct ShutdownWait(Vec<(ProcessName, Arc<Mutex<Process>>)>);

impl ShutdownWait {
    pub fn wait(self) -> ! {
        self.wait_children();
        goodbye(0)
    }

    pub fn wait_children(self) {
        // Each stop only raised a flag, and the watchdog threads kill the
        // processes concurrently. So one deadline shared by sequential waits
        // is the same as waiting on the whole set.
        let deadline = Instant::now() + Duration::from_secs(30);
        for (name, process) in self.0 {
            while process.lock().unwrap().is_alive() && Instant::now() < deadline {
                sleep!(100);
            }
            if process.lock().unwrap().is_alive() {
                error!("Shutdown | {name} did not stop in time, leaving it running");
            }
        }
    }
}

pub fn goodbye(code: i32) -> ! {
    info!("Shutdown | goodbye!");
    exit(code);
}

/// The order [`App::stop_all`] raises the stop flags in: every
/// process before the process it uses, so no process keeps working with a
/// service already asked to stop. Each watchdog acts on its next tick, so
/// this orders the requests.
const SHUTDOWN_ORDER: [ProcessName; 5] = [
    ProcessName::Xvb,
    ProcessName::Xmrig,
    ProcessName::XmrigProxy,
    ProcessName::P2pool,
    ProcessName::Node,
];

#[cfg(test)]
mod test {
    use super::{CloseAction, CloseContext, SHUTDOWN_ORDER, close_action};
    use crate::helper::ProcessName;

    #[test]
    fn shutdown_asks_dependents_first() {
        let at = |p: ProcessName| SHUTDOWN_ORDER.iter().position(|q| *q == p).unwrap();
        assert!(
            at(ProcessName::Xvb) < at(ProcessName::Xmrig),
            "XvB re-points XMRig's pool"
        );
        assert!(
            at(ProcessName::Xmrig) < at(ProcessName::P2pool),
            "XMRig mines to P2Pool"
        );
        assert!(
            at(ProcessName::XmrigProxy) < at(ProcessName::P2pool),
            "the proxy forwards to P2Pool"
        );
        assert!(
            at(ProcessName::P2pool) < at(ProcessName::Node),
            "P2Pool talks to the node"
        );
    }

    #[test]
    fn shutdown_covers_every_process() {
        use strum::IntoEnumIterator as _;
        for name in ProcessName::iter() {
            assert!(SHUTDOWN_ORDER.contains(&name), "{name:?} is never stopped");
        }
        assert_eq!(SHUTDOWN_ORDER.len(), ProcessName::iter().count());
    }

    fn base() -> CloseContext {
        CloseContext {
            hide_by_closing: false,
            hide_to_tray: false,
            tray_active: false,
            asked_close_to_tray: true,
            error_shown: false,
            ask_before_quit: false,
            quit_confirmed: false,
        }
    }

    #[test]
    fn close_quits_without_tray() {
        assert_eq!(close_action(&base()), CloseAction::Quit);
    }

    #[test]
    fn close_hides_when_enabled() {
        let c = CloseContext {
            hide_to_tray: true,
            tray_active: true,
            ..base()
        };
        assert_eq!(close_action(&c), CloseAction::HideByUnmapping);
        assert_eq!(
            close_action(&CloseContext {
                hide_by_closing: true,
                ..c
            }),
            CloseAction::HideByClosing
        );
    }

    #[test]
    fn hiding_needs_a_tray_icon() {
        let c = CloseContext {
            hide_to_tray: true,
            tray_active: false,
            ..base()
        };
        assert_eq!(close_action(&c), CloseAction::Quit);
    }

    #[test]
    fn first_close_with_a_tray_asks_once() {
        let c = CloseContext {
            tray_active: true,
            asked_close_to_tray: false,
            ..base()
        };
        assert_eq!(close_action(&c), CloseAction::AskTrayOnClose);
        assert_eq!(
            close_action(&CloseContext {
                error_shown: true,
                ..c
            }),
            CloseAction::Quit
        );
        assert_eq!(
            close_action(&CloseContext {
                asked_close_to_tray: true,
                ..c
            }),
            CloseAction::Quit
        );
    }

    #[test]
    fn quit_confirmation_asked_then_honored() {
        let c = CloseContext {
            ask_before_quit: true,
            ..base()
        };
        assert_eq!(close_action(&c), CloseAction::AskQuit);
        assert_eq!(
            close_action(&CloseContext {
                quit_confirmed: true,
                ..c
            }),
            CloseAction::Quit
        );
    }
}
