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
//! windowed app, `Accessory` is a menu bar app with neither a Dock icon
//! nor the ability to be the frontmost application.
//!
//! The two go through different APIs because they act at different times.
//! The launch policy has to be handed to winit's event loop builder,
//! which is its only hook for it; later toggles go straight to
//! `NSApplication`, since the event loop is built once and then reused.
//! Both reach the same process-wide singleton, so the `objc2` version
//! here does not have to match the one winit links.

use std::cell::Cell;

use log::debug;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS as _};

/// Whether Gupax is running from an app bundle, which is how it ships.
///
/// A loose binary can not do the activation policy dance: measured on
/// macOS 14, going `Accessory` and back to `Regular` leaves an unbundled
/// process with `ownsMenuBar == false` for good, so the menu bar keeps
/// showing whichever app was there before and the Dock tile falls back to
/// a generic icon. The same code in a bundle comes back with the menu bar
/// and its own icon. So outside a bundle -- `cargo run`, or the binary on
/// its own -- Gupax simply stays a normal windowed app: it keeps a Dock
/// icon while in the tray, which is much better than losing the menu bar.
///
/// winit makes the same distinction for the same reason, see its
/// `applicationDidFinishLaunching`.
fn bundled() -> bool {
    static BUNDLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *BUNDLED.get_or_init(|| {
        // Asking LaunchServices (`NSRunningApplication::bundleIdentifier`)
        // is no good here: the first call comes from
        // [`start_as_background_app`], before the event loop exists, when
        // the process is not registered yet and it answers "not bundled"
        // even inside a bundle. The layout on disk is the same answer and
        // is knowable straight away.
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
    /// Last policy applied. Asking AppKit for it instead is not an option
    /// on this path: `activationPolicy` forwards to `NSRunningApplication`
    /// and blocks on a LaunchServices round trip (~66us measured, against
    /// ~2ns for other getters on the same object), and
    /// [`crate::app::eframe_impl::GuiApp`] reconciles this every frame.
    /// A local copy can not drift, as this process is the only writer of
    /// its own activation policy.
    static WINDOWED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// See [`super::start_as_background_app`].
///
/// The policy has to go through winit's builder because winit picks the
/// launch policy in `applicationDidFinishLaunching` and only leaves it
/// alone when one was given there: for an unbundled binary it otherwise
/// forces `Regular`, and a bundled one falls back to `Info.plist`, which
/// has no `LSUIElement`. Either way a Dock icon would appear and stay
/// until [`set_windowed_app`] runs on the first frame.
///
/// `activate_ignoring_other_apps(false)` goes with it: winit activates
/// the app on launch otherwise, which is the other half of not being
/// frontmost.
pub fn start_as_background_app(options: &mut eframe::NativeOptions) {
    // the window is never mapped either way, which is what stops the
    // [--tray] flash; only the policy needs a bundle
    options.viewport.visible = Some(false);
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
/// Coming back to the foreground is left to `ViewportCommand::Focus`,
/// which the show path sends right after `Visible(true)`: winit only
/// activates once the window is on screen (`focus_window` checks
/// `isVisible` first), and this runs before the queued viewport commands
/// are applied, so activating here would be too early.
///
/// Only does something on the main thread, which is where eframe drives
/// the GUI, and where AppKit requires this to happen.
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
