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
use crate::app::eframe_impl::{gui_background_loop, pick_renderer, run_gui, start_in_tray};
use crate::cli::Cli;
use crate::daemon::start_daemon;
use crate::tray::{TrayChannel, TraySlot};
use crate::utils::single_instance;
//---------------------------------------------------------------------------------------------------- Imports
use crate::constants::*;
use crate::inits::{init_auto, init_logger};
use crate::utils::*;
use clap::Parser;
use egui::Vec2;
use log::info;
use log::warn;
use std::rc::Rc;
use std::time::Instant;

mod app;
mod cli;
mod components;
mod daemon;
mod disk;
mod helper;
mod inits;
mod miscs;
mod tray;
mod utils;

// Sudo (dummy values for Windows)
#[cfg(target_family = "unix")]
extern crate sudo as sudo_check;

//---------------------------------------------------------------------------------------------------- Main [App] frame
fn main() {
    #[cfg(target_os = "windows")]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "elevated-helper")
    {
        run_elevated_helper(elevated_helper::cli::Args::parse_from(
            std::env::args_os().skip(1),
        ));
    }
    let args = Cli::parse();
    #[cfg(target_os = "windows")]
    if args.elevated_helper {
        run_elevated_helper(elevated_helper::cli::Args {
            name_pipe_stdin: args.name_stdin_pipe.unwrap(),
            name_pipe_stdout: args.name_stdout_pipe.unwrap(),
            program_path: args.binary_path.unwrap(),
            program_args: args.arguments,
            creation_flags: None,
            priority: None,
        });
    }

    let now = Instant::now();

    // Set custom panic hook.
    crate::panic::set_panic_hook(now);

    // Init logger.
    init_logger(now, args.logfile);
    let tray_channel = Rc::new(TrayChannel::new());
    let tray_slot = TraySlot::default();
    // Before [AppEgui::new], which runs the binaries and writes the state
    // file: a launch that only shows the running window must leave its
    // saved state unchanged.
    if args.runs_gui() {
        tray_channel.set_global();
        if !single_instance::init(tray_channel.sender(), !args.tray) {
            if args.tray {
                info!("Gupax is already running, exiting");
            } else {
                info!("Gupax is already running: told it to show its window, exiting");
            }
            return;
        }
    }
    let app = AppEgui::new(now, &args);
    // Before [init_auto], so a Gupax with no renderer left to try starts no
    // process.
    if args.runs_gui() && !pick_renderer(&app) {
        return;
    }
    let mut app_lock = app.inner.lock();
    init_auto(&mut app_lock);
    drop(app_lock);
    // Gupax folder cleanup.
    #[cfg(target_os = "windows")]
    match crate::miscs::clean_dir() {
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
        let resolution = Vec2::new(selected_width, selected_height);
        let name_version = app.inner.lock().name_version.clone();
        if start_in_tray(&app, &tray_slot, &tray_channel) {
            run_gui(
                &app,
                &tray_slot,
                &tray_channel,
                initial_window_size,
                resolution,
                &name_version,
            );
        }
        gui_background_loop(
            &app,
            &tray_slot,
            &tray_channel,
            initial_window_size,
            resolution,
            &name_version,
        );
    }
}

#[cfg(target_os = "windows")]
fn run_elevated_helper(mut args: elevated_helper::cli::Args) -> ! {
    use windows::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
    args.priority = Some(BELOW_NORMAL_PRIORITY_CLASS.0);
    match elevated_helper::run(args) {
        // Windows exit codes are u32, which this cast keeps bit for bit.
        Ok(code) => std::process::exit(code as i32),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
