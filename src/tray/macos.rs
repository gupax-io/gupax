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

//! macOS half of [`super::start_as_background_app`] and
//! [`super::set_windowed_app`]: the `Regular` activation policy is a
//! windowed app, `Accessory` a menu bar app with no Dock icon.

use std::cell::Cell;
use std::ptr::NonNull;

use log::debug;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDidBecomeActiveNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS as _};

use super::{TrayCmd, TraySender};

/// Whether Gupax is running from an app bundle, which is how it ships.
///
/// A binary outside a bundle can not switch the activation policy back and
/// forth: measured on macOS 14, going `Accessory` and back to `Regular`
/// leaves an unbundled process with `ownsMenuBar == false` permanently, so
/// the menu bar keeps showing the previous app and the Dock shows a generic
/// icon. The same code in a bundle gets the menu bar and its own icon back.
/// So outside a bundle Gupax stays a windowed app, and keeps its Dock icon
/// while in the tray.
///
/// winit makes the same distinction for the same reason, see its
/// `applicationDidFinishLaunching`.
fn bundled() -> bool {
    static BUNDLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *BUNDLED.get_or_init(|| {
        // Read from the layout on disk: the first call comes from
        // [`start_as_background_app`], before the process is registered
        // with LaunchServices.
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let mut up = exe.ancestors().skip(1);
        let is = |name: &str, part: Option<&std::path::Path>| {
            part.and_then(|p| p.file_name()).is_some_and(|n| n == name)
        };
        is("MacOS", up.next())
            && is("Contents", up.next())
            && up
                .next()
                .and_then(|p| p.extension())
                .is_some_and(|e| e == "app")
    })
}

thread_local! {
    /// Last policy applied. AppKit's `activationPolicy` waits for a
    /// LaunchServices round trip (~66us measured), and
    /// [`crate::app::eframe_impl::GuiApp`] sets the policy every frame. This
    /// process is the only one changing its activation policy.
    static WINDOWED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// See [`super::start_as_background_app`].
///
/// The launch policy goes through winit's builder, which winit applies in
/// `applicationDidFinishLaunching`, and `activate_ignoring_other_apps(false)`
/// keeps the launch from activating the app.
pub fn start_as_background_app(options: &mut eframe::NativeOptions) {
    if !bundled() {
        return;
    }
    options.event_loop_builder = Some(Box::new(|builder| {
        builder
            .with_activation_policy(ActivationPolicy::Accessory)
            .with_activate_ignoring_other_apps(false);
    }));
}

/// See [`super::set_windowed_app`].
///
/// `ViewportCommand::Focus` brings the app to the foreground: the show path
/// sends it once the window is on screen. Runs on the main thread only, as
/// AppKit requires.
pub fn set_windowed_app(windowed: bool) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if !bundled() {
        return;
    }
    if WINDOWED.replace(Some(windowed)) == Some(windowed) {
        return;
    }
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(if windowed {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    });
    debug!(
        "Tray | macOS activation policy set to {}",
        if windowed { "Regular" } else { "Accessory" }
    );
}

/// See [`super::show_on_reopen`]. Opening Gupax.app again reactivates the
/// running process, so there is no second launch for the single-instance
/// guard to answer.
pub fn show_on_reopen(sender: TraySender) {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| {
        let block = block2::RcBlock::new(move |_: NonNull<NSNotification>| {
            if WINDOWED.get() == Some(false) {
                sender.send(TrayCmd::Show);
            }
        });
        // SAFETY: the name is an AppKit constant; with no object filter and
        // no queue the block runs on the posting thread, the main one for
        // this notification, and it only holds a `TraySender`, which is
        // `Send` and `Sync`.
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidBecomeActiveNotification),
                None,
                None,
                &block,
            )
        };
        // The observer stays registered until the process exits.
        std::mem::forget(observer);
    });
}
