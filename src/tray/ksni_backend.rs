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

//! Linux tray backend: StatusNotifierItem over DBus via `ksni`.
//! The callbacks below run on ksni's own thread.
//!
//! GNOME needs an AppIndicator extension for the icon to be visible;
//! KDE and most other DEs work out of the box.

use ksni::blocking::{Handle, TrayMethods};

use super::{TrayCmd, TraySender};
use crate::utils::constants::{BYTES_TRAY_ICON_ARGB, TRAY_ICON_SIZE};

pub struct TrayBackend {
    handle: Handle<GupaxTray>,
}

impl TrayBackend {
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        let tray = GupaxTray {
            sender,
            window_visible: true,
        };
        let handle = tray
            .spawn()
            .map_err(|e| anyhow::anyhow!("could not register StatusNotifierItem: {e}"))?;
        Ok(Self { handle })
    }

    pub fn set_window_visible(&self, visible: bool) {
        let _ = self
            .handle
            .update(move |tray| tray.window_visible = visible);
    }
}

impl Drop for TrayBackend {
    fn drop(&mut self) {
        self.handle.shutdown();
    }
}

struct GupaxTray {
    sender: TraySender,
    window_visible: bool,
}

impl ksni::Tray for GupaxTray {
    fn id(&self) -> String {
        "io.gupax.Gupax".into()
    }
    fn title(&self) -> String {
        "Gupax".into()
    }
    // Distro packages install a themed icon; the portable binary can only
    // rely on the embedded pixels.
    #[cfg(feature = "distro")]
    fn icon_name(&self) -> String {
        "gupax".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: TRAY_ICON_SIZE as i32,
            height: TRAY_ICON_SIZE as i32,
            data: BYTES_TRAY_ICON_ARGB.to_vec(),
        }]
    }
    // Left-click on the icon
    fn activate(&mut self, _x: i32, _y: i32) {
        self.sender.send(TrayCmd::ToggleShowHide);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        let show_hide = if self.window_visible {
            "Hide Gupax"
        } else {
            "Show Gupax"
        };
        vec![
            StandardItem {
                label: show_hide.into(),
                activate: Box::new(|tray: &mut Self| {
                    tray.sender.send(TrayCmd::ToggleShowHide);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit Gupax".into(),
                activate: Box::new(|tray: &mut Self| {
                    tray.sender.send(TrayCmd::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(test)]
mod test {
    use crate::utils::constants::{BYTES_ICON, BYTES_TRAY_ICON_ARGB, TRAY_ICON_SIZE};

    /// [tray-icon.argb] is icon.png pre-converted to ARGB32 network byte
    /// order; if icon.png changes, the file must be regenerated (decode to
    /// RGBA8 and rotate each pixel right by one byte, as done below).
    #[test]
    fn argb_file_matches_source_icon() {
        let icon = image::load_from_memory(BYTES_ICON).unwrap().to_rgba8();
        assert_eq!(icon.dimensions(), (TRAY_ICON_SIZE, TRAY_ICON_SIZE));
        let mut data = icon.into_raw();
        for pixel in data.chunks_exact_mut(4) {
            pixel.rotate_right(1);
        }
        assert!(
            data.as_slice() == BYTES_TRAY_ICON_ARGB,
            "stale tray-icon.argb: regenerate it from icon.png"
        );
    }
}
