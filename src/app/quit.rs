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

use log::{error, info};

use crate::errors::ErrorButtons;
use crate::errors::ErrorFerris;
use crate::helper::{Helper, Process};

use super::{App, WindowState};

/// Everything [`close_action`] needs to route a window close request.
struct CloseContext {
    /// The platform hides to the tray by closing the window (Linux)
    hide_by_closing: bool,
    /// The window is already being closed to the tray
    hidden_to_tray: bool,
    /// "Close to tray" setting
    hide_to_tray: bool,
    /// A tray icon exists
    tray_active: bool,
    /// The one-time close-to-tray question was already answered
    asked_close_to_tray: bool,
    /// An error/question screen is currently displayed
    error_shown: bool,
    /// "Ask before quit" setting
    ask_before_quit: bool,
    /// The quit confirmation is already on screen
    quit_confirmed: bool,
}

/// What a window close request must do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CloseAction {
    /// Let the window close, Gupax keeps running in the tray (Linux,
    /// where the window is destroyed and re-created instead of unmapped)
    HideByClosing,
    /// Cancel the close and unmap the window instead
    HideByUnmapping,
    /// Cancel the close and ask whether to keep running in the tray
    AskTrayOnClose,
    /// Cancel the close and ask for confirmation
    AskQuit,
    /// Save (if enabled) and quit
    Quit,
}

/// Route a window close request. Pure, so the trickiest platform logic of
/// the tray feature stays unit-testable without a window.
fn close_action(c: &CloseContext) -> CloseAction {
    if (c.hide_to_tray && c.tray_active) || (c.hide_by_closing && c.hidden_to_tray) {
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
    pub(super) fn quit(&mut self, ctx: &egui::Context) {
        // Used to be `eframe::App::on_close_event(&mut self) -> bool`.
        use egui::viewport::ViewportCommand;
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        match close_action(&CloseContext {
            hide_by_closing: crate::tray::HIDE_BY_CLOSING,
            hidden_to_tray: self.window_state == WindowState::HiddenToTray,
            hide_to_tray: self.state.gupax.auto.hide_to_tray,
            tray_active: self.tray_active,
            asked_close_to_tray: self.state.gupax.asked_close_to_tray,
            error_shown: self.error_state.error,
            ask_before_quit: self.state.gupax.auto.ask_before_quit,
            quit_confirmed: self.error_state.quit_twice,
        }) {
            CloseAction::HideByClosing => {
                // nothing to send: the close proceeds, run_native returns
                // and gui_background_loop takes over
                if self.window_state != WindowState::HiddenToTray {
                    info!("Tray | closing the window to the tray");
                    self.window_state = WindowState::HiddenToTray;
                    self.notify_hidden_to_tray();
                }
            }
            CloseAction::HideByUnmapping => {
                info!("Tray | hiding the window to the tray instead of quitting");
                self.window_state = WindowState::HiddenToTray;
                self.notify_hidden_to_tray();
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
            CloseAction::AskTrayOnClose => {
                self.error_state.set(
                    "Gupax can keep running in the system tray when the window is closed.\nKeep Gupax running in the tray when closing the window?\n(You can change this later with the \"Close to tray\" checkbox)",
                    ErrorFerris::Cute,
                    ErrorButtons::TrayOnClose,
                );
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            CloseAction::AskQuit => {
                info!("quit");
                self.error_state
                    .set("", ErrorFerris::Oops, ErrorButtons::StayQuit);
                self.error_state.quit_twice = true;
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            CloseAction::Quit => {
                info!("quit");
                if self.state.gupax.auto.save_before_quit {
                    self.save_before_quit();
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    /// One-time notification the first time Gupax hides to the tray, so
    /// the user knows it is still running.
    pub fn notify_hidden_to_tray(&mut self) {
        if self.state.gupax.notified_hidden_to_tray {
            return;
        }
        self.state.gupax.notified_hidden_to_tray = true;
        match crate::disk::state::State::save(&mut self.state, &self.state_path) {
            Ok(_) => self.og.lock().unwrap().gupax = self.state.gupax.clone(),
            Err(e) => error!("State file: {e}"),
        }
        std::thread::spawn(|| {
            crate::helper::notification::notif(
                "Gupax keeps running in the system tray.\nUse the tray icon to open it again or to quit.",
            );
        });
    }

    /// Persist the answer of the one-time close-to-tray question.
    pub fn save_tray_on_close_answer(&mut self, enable: bool) {
        self.state.gupax.auto.hide_to_tray = enable;
        self.state.gupax.asked_close_to_tray = true;
        match crate::disk::state::State::save(&mut self.state, &self.state_path) {
            Ok(_) => self.og.lock().unwrap().gupax = self.state.gupax.clone(),
            Err(e) => error!("State file: {e}"),
        }
    }

    /// Stop all child processes, wait for them to die (with a timeout),
    /// save the state if enabled, and exit the program.
    pub fn graceful_shutdown(&mut self) -> ! {
        info!("Shutdown | stopping all child processes...");
        type StopFn = fn(&Arc<Mutex<Helper>>);
        let processes: [(&Arc<Mutex<Process>>, StopFn); 5] = [
            (&self.node, Helper::stop_node),
            (&self.p2pool, Helper::stop_p2pool),
            (&self.xmrig, Helper::stop_xmrig),
            (&self.xmrig_proxy, Helper::stop_xp),
            (&self.xvb, Helper::stop_xvb),
        ];
        for (process, stop) in &processes {
            if process.lock().unwrap().is_alive() {
                stop(&self.helper);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        for (process, _) in &processes {
            while process.lock().unwrap().is_alive() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        if self.state.gupax.auto.save_before_quit {
            self.save_before_quit();
        }
        info!("Shutdown | goodbye!");
        exit(0);
    }
}

#[cfg(test)]
mod test {
    use super::{CloseAction, CloseContext, close_action};

    /// Tray disabled, nothing asked yet: the base case of each test.
    fn base() -> CloseContext {
        CloseContext {
            hide_by_closing: false,
            hidden_to_tray: false,
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
    fn close_already_routed_to_tray_proceeds() {
        let c = CloseContext {
            hide_by_closing: true,
            hidden_to_tray: true,
            ..base()
        };
        assert_eq!(close_action(&c), CloseAction::HideByClosing);
    }

    #[test]
    fn first_close_with_a_tray_asks_once() {
        let c = CloseContext {
            tray_active: true,
            asked_close_to_tray: false,
            ..base()
        };
        assert_eq!(close_action(&c), CloseAction::AskTrayOnClose);
        // never over an existing error screen, and only once
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
