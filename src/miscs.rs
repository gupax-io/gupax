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

//---------------------------------------------------------------------------------------------------- Misc functions

// Get absolute [Gupax] binary path
use std::fmt::Write;
use std::time::Duration;
#[cold]
#[inline(never)]
pub fn get_exe() -> Result<String, std::io::Error> {
    match std::env::current_exe() {
        Ok(path) => Ok(path.display().to_string()),
        Err(err) => {
            error!("Couldn't get absolute Gupax PATH");
            Err(err)
        }
    }
}

// Get absolute [Gupax] directory path
#[cold]
#[inline(never)]
pub fn get_exe_dir() -> Result<String, std::io::Error> {
    match std::env::current_exe() {
        Ok(mut path) => {
            path.pop();
            Ok(path.display().to_string())
        }
        Err(err) => {
            error!("Couldn't get exe basepath PATH");
            Err(err)
        }
    }
}

// Clean any [gupax_update_.*] directories
// The trailing random bits must be exactly 10 alphanumeric characters
#[cold]
#[inline(never)]
pub fn clean_dir() -> Result<(), anyhow::Error> {
    let regex = Regex::new("^gupax_update_[A-Za-z0-9]{10}$").unwrap();
    for entry in std::fs::read_dir(get_exe_dir()?)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        if Regex::is_match(
            &regex,
            entry
                .file_name()
                .to_str()
                .ok_or_else(|| anyhow::Error::msg("Basename failed"))?,
        ) {
            let path = entry.path();
            match std::fs::remove_dir_all(&path) {
                Ok(_) => info!("Remove [{}] ... OK", path.display()),
                Err(e) => warn!("Remove [{}] ... FAIL ... {}", path.display(), e),
            }
        }
    }
    Ok(())
}

// Print disk files to console
#[cold]
#[inline(never)]
pub fn print_disk_file(path: &PathBuf) {
    match std::fs::read_to_string(path) {
        Ok(string) => {
            print!("{string}");
            exit(0);
        }
        Err(e) => {
            error!("{e}");
            exit(1);
        }
    }
}

// Prints the GupaxP2PoolApi files.
#[cold]
#[inline(never)]
pub fn print_gupax_p2pool_api(gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>) {
    let api = gupax_p2pool_api.lock().unwrap();
    let log = match std::fs::read_to_string(&api.path_log) {
        Ok(string) => string,
        Err(e) => {
            error!("{e}");
            exit(1);
        }
    };
    let payout = match std::fs::read_to_string(&api.path_payout) {
        Ok(string) => string,
        Err(e) => {
            error!("{e}");
            exit(1);
        }
    };
    let xmr = match std::fs::read_to_string(&api.path_xmr) {
        Ok(string) => string,
        Err(e) => {
            error!("{e}");
            exit(1);
        }
    };
    let xmr = match xmr.trim().parse::<u64>() {
        Ok(o) => crate::xmr::AtomicUnit::from_u64(o),
        Err(e) => {
            warn!("GupaxP2poolApi | [xmr] parse error: {e}");
            exit(1);
        }
    };
    println!(
        "{}\nTotal payouts | {}\nTotal XMR     | {} ({} Atomic Units)",
        log,
        payout.trim(),
        xmr,
        xmr.to_u64()
    );
    exit(0);
}

#[inline]
pub fn cmp_f64(a: f64, b: f64) -> std::cmp::Ordering {
    match (a <= b, a >= b) {
        (false, true) => std::cmp::Ordering::Greater,
        (true, false) => std::cmp::Ordering::Less,
        (true, true) => std::cmp::Ordering::Equal,
        _ => std::cmp::Ordering::Less,
    }
}
// Free functions.

use crate::disk::gupax_p2pool_api::GupaxP2poolApi;
use crate::helper::ProcessName;
use chrono::Local;
use egui::TextStyle;
use egui::Ui;
use log::error;
use log::warn;
use regex::Regex;
use reqwest_middleware::ClientWithMiddleware;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;

use log::info;

//---------------------------------------------------------------------------------------------------- Use
use crate::constants::*;

//----------------------------------------------------------------------------------------------------
#[cold]
#[inline(never)]
// Clamp the scaling resolution `f32` to a known good `f32`.
pub fn clamp_scale(scale: f32) -> f32 {
    // Make sure it is finite.
    if !scale.is_finite() {
        return APP_DEFAULT_SCALE;
    }

    // Clamp between valid range.
    scale.clamp(APP_MIN_SCALE, APP_MAX_SCALE)
}
pub fn output_console(output: &mut String, msg: &str, p_name: ProcessName) {
    if let Err(e) = writeln!(output, "{}{msg}", datetimeonsole()) {
        error!("{p_name} Watchdog | GUI status write failed: {e}");
    }
}
pub fn output_console_without_time(output: &mut String, msg: &str, p_name: ProcessName) {
    if let Err(e) = writeln!(output, "{msg}") {
        error!("{p_name} Watchdog | GUI status write failed: {e}");
    }
}
fn datetimeonsole() -> String {
    format!("[{}]  ", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
}

pub fn client() -> ClientWithMiddleware {
    reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
        .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
            reqwest_retry::policies::ExponentialBackoff::builder()
                .retry_bounds(Duration::from_secs(1), Duration::from_secs(4))
                .build_with_total_retry_duration(Duration::from_secs(8)),
        ))
        .build()
}
/// to get the right height that a text must take before a button to be aligned in the center correctly.
pub fn height_txt_before_button(ui: &Ui, style: &TextStyle) -> f32 {
    ui.style().spacing.button_padding.y * 2.0 + ui.text_style_height(style)
}
