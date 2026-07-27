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
#![feature(path_is_empty)]
// Only (windows|macos|linux) + (x64|arm64) are supported.
#[cfg(not(target_pointer_width = "64"))]
compile_error!("gupax is only compatible with 64-bit CPUs");

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux",)))]
compile_error!("gupax is only built for windows/macos/linux");

use crate::app::AppEgui;
use crate::app::eframe_impl::{gui_background_loop, run_gui, start_in_tray};
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
    let args = Cli::parse();
    #[cfg(target_os = "windows")]
    if args.elevated_helper {
        use windows::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;
        let args_elevated_helper = elevated_helper::cli::Args {
            name_pipe_stdin: args.name_stdin_pipe.unwrap(),
            name_pipe_stdout: args.name_stdout_pipe.unwrap(),
            program_path: args.binary_path.unwrap(),
            program_args: args.arguments,
            creation_flags: None,
            priority: Some(BELOW_NORMAL_PRIORITY_CLASS.0),
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
    let app = AppEgui::new(now, &args);
    // The command channel between the tray/second launches and the GUI;
    // it lives for the whole process (windows and tray icons come and go).
    let tray_channel = Rc::new(TrayChannel::new());
    let tray_slot = TraySlot::default();
    // A second GUI launch shows the window of the running instance
    // instead of starting a duplicate (and its auto-started services).
    if !args.daemon {
        let name_version = app.inner.lock().name_version.clone();
        if !single_instance::init(tray_channel.sender(), &name_version) {
            info!("Gupax is already running: told it to show its window, exiting");
            return;
        }
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
        // [--tray] on Linux starts waiting in the tray, without any window
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
        // On Linux, hiding to the tray closes the window: keep running in
        // the background and re-create the window when asked from the tray.
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
