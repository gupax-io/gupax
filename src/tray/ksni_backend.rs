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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ksni::blocking::{Handle, TrayMethods};

use super::{TrayCmd, TraySender};
use crate::utils::constants::{BYTES_TRAY_ICON_ARGB, TRAY_ICON_SIZE};

pub struct TrayBackend {
    handle: Handle<GupaxTray>,
    watcher_present: Arc<AtomicBool>,
}

impl TrayBackend {
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        let watcher_present = Arc::new(AtomicBool::new(true));
        let tray = GupaxTray {
            sender,
            window_visible: true,
            watcher_present: watcher_present.clone(),
        };
        // Register even when no StatusNotifierWatcher is on the bus yet,
        // and let ksni attach once one shows up: without this a missing
        // watcher is a hard error, which is the normal case when Gupax is
        // autostarted at login and beats the desktop's tray to the bus --
        // and [`crate::app::eframe_impl::GuiApp`] never retries, so the
        // icon would stay missing for the rest of the session.
        //
        // The cost is that registering no longer proves the icon can be
        // seen by anyone, so [`Self::icon_visible`] answers that instead.
        let handle = tray
            .assume_sni_available(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("could not register StatusNotifierItem: {e}"))?;
        Ok(Self {
            handle,
            watcher_present,
        })
    }

    /// Whether anything is drawing the icon right now.
    ///
    /// Sound to read straight after [`Self::new`]: the blocking `spawn`
    /// runs the whole registration, [`ksni::Tray::watcher_offline`]
    /// included, before it returns.
    pub fn icon_visible(&self) -> bool {
        self.watcher_present.load(Ordering::Relaxed)
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
    /// Shared with [`TrayBackend::icon_visible`]; written from ksni's
    /// thread, read from the GUI thread.
    watcher_present: Arc<AtomicBool>,
}

impl ksni::Tray for GupaxTray {
    fn id(&self) -> String {
        "io.gupax.Gupax".into()
    }
    /// No watcher means no icon anywhere, and hiding the window destroys
    /// it on Linux, so Gupax would end up somewhere the user can not
    /// reach it. Everything that hides keys off
    /// [`crate::app::App::tray_active`], which follows this.
    ///
    /// Returns `true` to keep the service running regardless: a watcher
    /// that went away can come back (a shell restart, or the user enabling
    /// the tray extension), and ksni re-registers the item when it does.
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        log::warn!("Tray | no StatusNotifierWatcher, the icon is not displayed: {reason:?}");
        self.watcher_present.store(false, Ordering::Relaxed);
        true
    }
    fn watcher_online(&self) {
        log::info!("Tray | a StatusNotifierWatcher is back, the icon is displayed again");
        self.watcher_present.store(true, Ordering::Relaxed);
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
        let (mut data, width, height) = crate::miscs::icon_rgba(BYTES_ICON);
        assert_eq!((width, height), (TRAY_ICON_SIZE, TRAY_ICON_SIZE));
        for pixel in data.chunks_exact_mut(4) {
            pixel.rotate_right(1);
        }
        assert!(
            data.as_slice() == BYTES_TRAY_ICON_ARGB,
            "stale tray-icon.argb: regenerate it from icon.png"
        );
    }
}
