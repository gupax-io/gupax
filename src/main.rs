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

use crate::app::App;
use crate::cli::Cli;
use crate::daemon::start_daemon;
//---------------------------------------------------------------------------------------------------- Imports
use crate::constants::*;
use crate::inits::{init_auto, init_logger, init_options};
use crate::miscs::clean_dir;
use crate::utils::*;
use clap::Parser;
use egui::Vec2;
use log::info;
use log::warn;
use std::sync::Arc;
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
    #[cfg(target_os = "windows")]
    if args.elevated_helper {
        use windows::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
        let args_elevated_helper = elevated_helper::cli::Args {
            name_pipe_stdin: args.name_stdin_pipe.unwrap(),
            name_pipe_stdout: args.name_stdout_pipe.unwrap(),
            program_path: args.binary_path.unwrap(),
            program_args: args.arguments,
            creation_flags: Some(BELOW_NORMAL_PRIORITY_CLASS.0),
        };
        if let Err(e) = elevated_helper::run(args_elevated_helper) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        use sysinfo::System;
        let mut sys = System::new();
        let process = sys.processes_by_exact_name("xmrig.exe".as_ref()).next();

        if let Some(process) = process {
            let pid = process.pid();
            loop {
                use crate::utils::macros::sleep;

                sleep!(5000);
                sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                // check if xmrig is still alive
                if sys.process(pid).is_none() {
                    break;
                }
            }
        }
        std::process::exit(0);
    }

    let now = Instant::now();

    // Set custom panic hook.
    crate::panic::set_panic_hook(now);

    // Init logger.
    init_logger(now, args.logfile);
    let mut app = App::new(now, &args);
    init_auto(&mut app);
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
        let app = Arc::new(app);
        start_daemon(&app);
    } else {
        // Init GUI stuff.
        let selected_width = app.state.gupax.selected_width as f32;
        let selected_height = app.state.gupax.selected_height as f32;
        let initial_window_size = if selected_width > APP_MAX_WIDTH
            || selected_height > APP_MAX_HEIGHT
        {
            warn!(
                "App | Set width or height was greater than the maximum! Starting with the default resolution..."
            );
            Some(Vec2::new(APP_DEFAULT_WIDTH, APP_DEFAULT_HEIGHT))
        } else {
            Some(Vec2::new(
                app.state.gupax.selected_width as f32,
                app.state.gupax.selected_height as f32,
            ))
        };
        let options = init_options(initial_window_size);

        let resolution = Vec2::new(selected_width, selected_height);

        // Run Gupax.
        eframe::run_native(
            &app.name_version.clone(),
            options,
            Box::new(move |cc| {
                egui_extras::install_image_loaders(&cc.egui_ctx);
                Ok(Box::new(App::cc(cc, resolution, app)))
            }),
        )
        .unwrap();
    }
}
