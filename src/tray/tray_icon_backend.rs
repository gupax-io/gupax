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

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::{TrayCmd, TraySender};
use crate::miscs::icon_rgba;

/// Fixed menu ids: [`install_handlers`] runs once for the whole process,
/// so its callbacks can not close over the ids of one particular tray —
/// a tray dropped and re-created (toggling the tray settings) gets fresh
/// [`MenuItem`]s, and callbacks holding the old ids would leave its menu
/// dead.
const SHOW_HIDE_ID: &str = "gupax-show-hide";
const QUIT_ID: &str = "gupax-quit";

pub struct TrayBackend {
    /// Held only for its `Drop`, which removes the icon from the tray.
    _tray: TrayIcon,
    show_hide: MenuItem,
}

/// Point the process-wide `tray-icon`/`muda` callbacks at the tray channel.
///
/// Both crates keep their handler in a write-once `OnceCell`, so only the
/// first call has any effect and there is no way to uninstall them. The
/// [`Once`] makes that explicit and stops every later tray icon boxing
/// two closures for the callee to silently discard.
///
/// Outliving the tray icon costs nothing: every [`TraySender`] is a clone
/// of the one channel and context slot that live as long as the process.
fn install_handlers(sender: TraySender) {
    static HANDLERS: Once = Once::new();
    HANDLERS.call_once(move || {
        let menu_sender = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id().as_ref() {
            SHOW_HIDE_ID => menu_sender.send(TrayCmd::ToggleShowHide),
            QUIT_ID => menu_sender.send(TrayCmd::Quit),
            _ => {}
        }));
        // Left-click on the icon toggles the window. Only Windows gets
        // here: on macOS a click opens the menu instead, and the event is
        // then never delivered (see [`TrayBackend::new`]).
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                sender.send(TrayCmd::ToggleShowHide);
            }
        }));
    });
}

impl TrayBackend {
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        #[cfg(target_os = "windows")]
        let (rgba, width, height) = icon_rgba(crate::utils::constants::BYTES_ICON);
        // macOS menu bar convention: a monochrome template image that
        // adapts to the light/dark bar and accent colors.
        #[cfg(target_os = "macos")]
        let (rgba, width, height) = icon_rgba(crate::utils::constants::BYTES_TRAY_ICON_TEMPLATE);
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
        // Windows convention: left-click acts, right-click opens the menu.
        // `tray-icon` pops the menu on either by default *and* still emits
        // the click event, so one left-click would do both.
        //
        // macOS keeps the default, where a click on a menu bar item is
        // supposed to open its menu. The click event does not double up
        // there: the menu is opened from `mouseDown:` through
        // `performClick`, whose modal tracking loop consumes the mouse-up
        // that would have been reported.
        #[cfg(target_os = "windows")]
        let builder = builder.with_menu_on_left_click(false);
        let tray = builder.build()?;

        Ok(Self {
            _tray: tray,
            show_hide,
        })
    }

    /// The tray icon exists, or `build` above failed: nothing can take it
    /// away afterwards. Only the Linux backend can answer no.
    pub fn icon_visible(&self) -> bool {
        true
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.show_hide
            .set_text(if visible { "Hide Gupax" } else { "Show Gupax" });
    }
}
