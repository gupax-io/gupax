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

// Hide the Window console for release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Only (windows|macos|linux) + (x64|arm64) are supported.
#[cfg(not(target_pointer_width = "64"))]
compile_error!("gupax is only compatible with 64-bit CPUs");

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux",)))]
compile_error!("gupax is only built for windows/macos/linux");

use crate::app::AppEgui;
use crate::cli::Cli;
use crate::daemon::start_daemon;
//---------------------------------------------------------------------------------------------------- Imports
use crate::constants::*;
use crate::inits::{init_auto, init_logger, init_options};
use crate::miscs::clean_dir;
use crate::utils::*;
use clap::Parser;
use egui::Vec2;
use log::warn;
use log::{error, info};
use std::time::Instant;

mod app;
mod cli;
mod components;
mod daemon;
mod disk;
mod helper;
mod inits;
mod miscs;
mod utils;

// Sudo (dummy values for Windows)
#[cfg(target_family = "unix")]
extern crate sudo as sudo_check;

//---------------------------------------------------------------------------------------------------- Main [App] frame
fn main() {
    let args = Cli::parse();
    let now = Instant::now();

    // Set custom panic hook.
    crate::panic::set_panic_hook(now);

    // Init logger.
    init_logger(now, args.logfile);
    let app = AppEgui::new(now, &args);
    let mut app_lock = app.inner.lock();
    init_auto(&mut app_lock);
    drop(app_lock);
    // Gupax folder cleanup.
    match clean_dir() {
        Ok(_) => info!("Temporary folder cleanup ... OK"),
        Err(e) => warn!("Could not cleanup [gupax_tmp] folders: {e}"),
    }
    info!(
        "/*************************************/ Init ... OK /*************************************/"
    );

    // if Gupax is started as a daemon, stay here and do not load the GUI

    if args.daemon {
        // if the app receives Ctrl+C, make sure to terminate all services
        start_daemon(app.clone());
    } else {
        // Init GUI stuff.
        let selected_width = app.inner.lock().state.gupax.selected_width as f32;
        let selected_height = app.inner.lock().state.gupax.selected_height as f32;
        let initial_window_size = if selected_width > APP_MAX_WIDTH
            || selected_height > APP_MAX_HEIGHT
        {
            warn!(
                "App | Set width or height was greater than the maximum! Starting with the default resolution..."
            );
            Some(Vec2::new(APP_DEFAULT_WIDTH, APP_DEFAULT_HEIGHT))
        } else {
            Some(Vec2::new(selected_width, selected_height))
        };
        info!("after daemon");
        let mut options = init_options(initial_window_size);
        options.renderer = app.inner.lock().current_renderer();

        let resolution = Vec2::new(selected_width, selected_height);

        let name_version = app.inner.lock().name_version.clone();

        if let Err(e) = eframe::run_native(
            &name_version,
            options.clone(),
            Box::new({
                info!(
                    "starting Gupax with renderer: {}",
                    app.inner.lock().current_renderer()
                );
                let app = app.clone();
                move |cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    Ok(Box::new(AppEgui::cc(cc, resolution, app)))
                }
            }),
        ) {
            let mut guard = app.inner.lock();
            error!(
                "eframe crashed using the renderer: {}.Error: {e}",
                guard.current_renderer()
            );

            warn!(
                "Use the other renderer temporarily, the new renderer will be used at next startup if the settings are saved"
            );
            guard.state.gupax.renderer_use_glow = !guard.state.gupax.renderer_use_glow;

            options.renderer = guard.current_renderer();

            warn!(
                "Restarting with Gupax with renderer {}",
                guard.current_renderer()
            );
            drop(guard);
            // app.
            if let Err(e) = eframe::run_native(
                &name_version,
                options,
                Box::new({
                    info!(
                        "starting Gupax with renderer: {}",
                        app.inner.lock().current_renderer()
                    );
                    let app = app.clone();
                    move |cc| {
                        egui_extras::install_image_loaders(&cc.egui_ctx);
                        Ok(Box::new(AppEgui::cc(cc, resolution, app)))
                    }
                }),
            ) {
                error!(
                    "eframe crashed using the renderer: {}.Error: {e}",
                    app.inner.lock().current_renderer()
                );
                error!(
                    "crashed with both renderer: Please open an issue on https://github.com/gupax-io/gupax/issues"
                );
            }
        }
    }
}
