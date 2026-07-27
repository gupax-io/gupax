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

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::{TrayCmd, TraySender};

pub struct TrayBackend {
    tray: TrayIcon,
    show_hide: MenuItem,
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

        let show_hide = MenuItem::new("Hide Gupax", true, None);
        let quit = MenuItem::new("Quit Gupax", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show_hide, &PredefinedMenuItem::separator(), &quit])?;

        let show_hide_id = show_hide.id().clone();
        let quit_id = quit.id().clone();
        {
            let sender = sender.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if *event.id() == show_hide_id {
                    sender.send(TrayCmd::ToggleShowHide);
                } else if *event.id() == quit_id {
                    sender.send(TrayCmd::Quit);
                }
            }));
        }
        // Left-click on the icon toggles the window (Windows behavior;
        // on macOS tray-icon shows the menu on click instead).
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

        let builder = TrayIconBuilder::new()
            .with_tooltip("Gupax")
            .with_icon(icon)
            .with_menu(Box::new(menu));
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_as_template(true);
        let tray = builder.build()?;

        Ok(Self { tray, show_hide })
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.show_hide
            .set_text(if visible { "Hide Gupax" } else { "Show Gupax" });
    }
}

fn icon_rgba(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let icon = image::load_from_memory(bytes)
        .expect("Failed to read icon bytes")
        .to_rgba8();
    let (width, height) = icon.dimensions();
    (icon.into_raw(), width, height)
}
