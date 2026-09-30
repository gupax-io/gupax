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
//! The callbacks below run on ksni's thread.
//!
//! GNOME needs an AppIndicator extension to display the icon; KDE and most
//! other desktop environments display it by default.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};

use ksni::blocking::TrayMethods;

use super::{TrayCmd, TraySender};

/// The icon in ARGB32 network byte order, as the StatusNotifierItem spec
/// requires. Decoded once, as ksni reads the pixmap on every property
/// update.
fn icon_argb() -> &'static (Vec<u8>, u32, u32) {
    static ICON: OnceLock<(Vec<u8>, u32, u32)> = OnceLock::new();
    ICON.get_or_init(|| {
        let icon = &crate::inits::ICON;
        let mut data = icon.rgba.clone();
        for pixel in data.as_chunks_mut::<4>().0 {
            pixel.rotate_right(1);
        }
        (data, icon.width, icon.height)
    })
}

pub struct TrayBackend {
    /// Menu updates, applied by the thread that registered the icon.
    /// Dropping this sender removes the icon.
    updates: Sender<bool>,
    watcher_present: Arc<AtomicBool>,
    registered: Arc<AtomicBool>,
}

impl TrayBackend {
    /// Registers on a separate thread: registering and every menu update wait
    /// on DBus with no timeout, and a tray host that does not answer would
    /// block the frame loop.
    pub fn new(sender: TraySender) -> anyhow::Result<Self> {
        let watcher_present = Arc::new(AtomicBool::new(true));
        let registered = Arc::new(AtomicBool::new(false));
        let tray = GupaxTray {
            sender,
            window_visible: true,
            watcher_present: watcher_present.clone(),
        };
        let (updates, rx) = channel::<bool>();
        let done = registered.clone();
        std::thread::spawn(move || {
            // Registers even when no StatusNotifierWatcher is on the bus yet,
            // as when Gupax starts before the tray of the desktop: ksni
            // registers the item once a watcher appears.
            let handle = match tray.assume_sni_available(true).spawn() {
                Ok(handle) => handle,
                Err(e) => {
                    log::warn!("Tray | could not register StatusNotifierItem: {e}");
                    return;
                }
            };
            done.store(true, Ordering::Release);
            while let Ok(visible) = rx.recv() {
                handle.update(move |tray| tray.window_visible = visible);
            }
            // The host also removes the item when the bus connection closes,
            // so a bus that stops answering can not block the quit here.
            handle.shutdown();
        });
        Ok(Self {
            updates,
            watcher_present,
            registered,
        })
    }

    pub fn icon_visible(&self) -> bool {
        self.registered.load(Ordering::Acquire) && self.watcher_present.load(Ordering::Relaxed)
    }

    pub fn set_window_visible(&self, visible: bool) {
        let _ = self.updates.send(visible);
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
    /// Returns `true` to keep the service running: a watcher can appear
    /// again, and ksni then registers the item again.
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
    // Distro packages install a themed icon; the portable binary uses the
    // embedded pixels.
    #[cfg(feature = "distro")]
    fn icon_name(&self) -> String {
        "gupax".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let (data, width, height) = icon_argb();
        vec![ksni::Icon {
            width: *width as i32,
            height: *height as i32,
            data: data.clone(),
        }]
    }
    // ksni calls this on a left-click.
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
    use super::icon_argb;
    use crate::utils::constants::BYTES_ICON;

    #[test]
    fn pixmap_is_argb_in_network_byte_order() {
        let (argb, width, height) = icon_argb();
        let (rgba, source_width, source_height) = crate::miscs::icon_rgba(BYTES_ICON);
        assert_eq!((*width, *height), (source_width, source_height));
        assert_eq!(argb.len(), rgba.len());
        for (argb, rgba) in argb.as_chunks::<4>().0.iter().zip(rgba.as_chunks::<4>().0) {
            assert_eq!(*argb, [rgba[3], rgba[0], rgba[1], rgba[2]]);
        }
    }
}
