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

//! Windows/macOS tray backend: the `tray-icon` crate.
//! Events arrive through global callbacks.

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::{TrayCmd, TraySender};

/// Constant menu ids: [`install_handlers`] runs once per process, and its
/// callbacks handle every tray created when the tray settings change.
const SHOW_HIDE_ID: &str = "gupax-show-hide";
const QUIT_ID: &str = "gupax-quit";

pub struct TrayBackend {
    _tray: TrayIcon,
    show_hide: MenuItem,
}

/// Set the process-wide `tray-icon`/`muda` callbacks to send to the tray
/// channel, once: both crates store their handler in a write-once cell, and
/// every [`TraySender`] sends to the one channel of the process.
fn install_handlers(sender: TraySender) {
    static HANDLERS: Once = Once::new();
    HANDLERS.call_once(move || {
        let menu_sender = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id().as_ref() {
            SHOW_HIDE_ID => menu_sender.send(TrayCmd::ToggleShowHide),
            QUIT_ID => menu_sender.send(TrayCmd::Quit),
            _ => {}
        }));
        // Left-click toggles the window on Windows, see [`TrayBackend::new`].
        //
        // A double-click arrives as a release, `DoubleClick`, then a second
        // release: the second release is skipped, so a double-click toggles
        // once.
        let after_double_click = AtomicBool::new(false);
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => after_double_click.store(true, Ordering::Relaxed),
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } if !after_double_click.swap(false, Ordering::Relaxed) => {
                sender.send(TrayCmd::ToggleShowHide);
            }
            _ => {}
        }));
    });
}

impl TrayBackend {
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        #[cfg(target_os = "windows")]
        let (rgba, width, height) = {
            let icon = &crate::inits::ICON;
            (icon.rgba.clone(), icon.width, icon.height)
        };
        #[cfg(target_os = "macos")]
        let (rgba, width, height) =
            crate::miscs::icon_rgba(crate::utils::constants::BYTES_TRAY_ICON_TEMPLATE);
        let icon = Icon::from_rgba(rgba, width, height)?;

        let show_hide = MenuItem::with_id(SHOW_HIDE_ID, "Hide Gupax", true, None);
        let quit = MenuItem::with_id(QUIT_ID, "Quit Gupax", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show_hide, &PredefinedMenuItem::separator(), &quit])?;

        install_handlers(sender);

        let builder = TrayIconBuilder::new()
            .with_tooltip("Gupax")
            .with_icon(icon)
            .with_menu(Box::new(menu));
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_as_template(true);
        // Left-click toggles the window on Windows, where `tray-icon` also
        // opens the menu on it by default. On macOS a click opens the menu,
        // whose tracking loop consumes the click event.
        #[cfg(target_os = "windows")]
        let builder = builder.with_menu_on_left_click(false);
        let tray = builder.build()?;

        Ok(Self {
            _tray: tray,
            show_hide,
        })
    }

    /// Assumed displayed once `build` succeeded; only the Linux backend
    /// checks.
    pub fn icon_visible(&self) -> bool {
        true
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.show_hide
            .set_text(if visible { "Hide Gupax" } else { "Show Gupax" });
    }
}
