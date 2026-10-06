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

// This handles reading/writing the disk files:
//     - [state.toml] -> [App] state
//     - [nodes.toml] -> [Manual Nodes] list
// The TOML format is used. This struct hierarchy
// directly translates into the TOML parser:
//   State/
//   ├─ Gupax/
//   │  ├─ ...
//   ├─ P2pool/
//   │  ├─ ...
//   ├─ Xmrig/
//   │  ├─ ...
//   ├─ Version/
//      ├─ ...

use crate::disk::consts::*;
use crate::{app::Tab, components::gupax::Ratio, constants::*, human::*, macros::*, xmr::*};
use figment::Figment;
use figment::providers::{Format, Toml};
use log::*;
use serde::{Deserialize, Serialize};
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::{
    fmt::Display,
    fmt::Write,
    fs,
    path::PathBuf,
    result::Result,
    sync::{Arc, Mutex},
};

use self::errors::TomlError;

pub mod consts;
pub mod errors;
pub mod gupax_p2pool_api;
pub mod node;
pub mod pool;
pub mod state;
pub mod status;
pub mod tests;
//---------------------------------------------------------------------------------------------------- General functions for all [File]'s
// get_file_path()      | Return absolute path to OS data path + filename
// read_to_string()     | Convert the file at a given path into a [String]
// create_new()         | Write a default TOML Struct into the appropriate file (in OS data path)
// into_absolute_path() | Convert relative -> absolute path

pub fn get_gupax_data_path() -> Result<PathBuf, TomlError> {
    // Get OS data folder
    // Linux   | $XDG_DATA_HOME or $HOME/.local/share/gupax  | /home/alice/.local/state/gupax
    // macOS   | $HOME/Library/Application Support/Gupax     | /Users/Alice/Library/Application Support/Gupax
    // Windows | {FOLDERID_RoamingAppData}\Gupax             | C:\Users\Alice\AppData\Roaming\Gupax
    match dirs::data_dir() {
        Some(mut path) => {
            path.push(DIRECTORY);
            info!("OS | Data path ... {}", path.display());
            Ok(path)
        }
        None => {
            error!("OS | Data path ... FAIL");
            Err(TomlError::Path(PATH_ERROR.to_string()))
        }
    }
}
#[cfg(target_family = "unix")]
pub fn set_unix_750_perms(path: &PathBuf) -> Result<(), TomlError> {
    match fs::set_permissions(path, fs::Permissions::from_mode(0o750)) {
        Ok(_) => {
            info!(
                "OS | Unix 750 permissions on path [{}] ... OK",
                path.display()
            );
            Ok(())
        }
        Err(e) => {
            error!(
                "OS | Unix 750 permissions on path [{}] ... FAIL ... {}",
                path.display(),
                e
            );
            Err(TomlError::Io(e))
        }
    }
}

pub fn get_gupax_p2pool_path(os_data_path: &Path) -> PathBuf {
    let mut gupax_p2pool_dir = os_data_path.to_path_buf();
    gupax_p2pool_dir.push(GUPAX_P2POOL_API_DIRECTORY);
    gupax_p2pool_dir
}

pub fn create_gupax_dir(path: &PathBuf) -> Result<(), TomlError> {
    // Create Gupax directory
    match fs::create_dir_all(path) {
        Ok(_) => info!("OS | Create data path ... OK"),
        Err(e) => {
            error!("OS | Create data path ... FAIL ... {e}");
            return Err(TomlError::Io(e));
        }
    }
    #[cfg(target_os = "windows")]
    return Ok(());
    #[cfg(target_family = "unix")]
    set_unix_750_perms(path)
}

pub fn create_gupax_p2pool_dir(path: &PathBuf) -> Result<(), TomlError> {
    // Create Gupax directory
    match fs::create_dir_all(path) {
        Ok(_) => {
            info!(
                "OS | Create Gupax-P2Pool API path [{}] ... OK",
                path.display()
            );
            Ok(())
        }
        Err(e) => {
            error!(
                "OS | Create Gupax-P2Pool API path [{}] ... FAIL ... {}",
                path.display(),
                e
            );
            Err(TomlError::Io(e))
        }
    }
}

// Convert a [File] path to a [String]
pub fn read_to_string(file: File, path: &PathBuf) -> Result<String, TomlError> {
    match fs::read_to_string(path) {
        Ok(string) => {
            info!("{file:?} | Read ... OK");
            Ok(string)
        }
        Err(err) => {
            warn!("{file:?} | Read ... FAIL");
            Err(TomlError::Io(err))
        }
    }
}

// Write str to console with [info!] surrounded by "---"
pub fn print_dash(toml: &str) {
    info!("{HORIZONTAL}");
    for i in toml.lines() {
        info!("{i}");
    }
    info!("{HORIZONTAL}");
}

// Turn relative paths into absolute paths
pub fn into_absolute_path(path: String) -> Result<PathBuf, TomlError> {
    let path = PathBuf::from(path);
    if path.is_relative() {
        let mut dir = std::env::current_exe()?;
        dir.pop();
        dir.push(path);
        Ok(dir)
    } else {
        Ok(path)
    }
}

//---------------------------------------------------------------------------------------------------- [File] Enum (for matching which file)
#[derive(Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub enum File {
    // State files
    State, // state.toml        | Gupax state
    Node,  // node.toml         | P2Pool manual node selector
    Pool,  // pool.toml         | XMRig manual pool selector
    Crawl, // found_nodes.toml | Crawler saved results

    // Gupax-P2Pool API
    Log,  // log  | Raw log lines of P2Pool payouts received
    Scan, // scan | Block from which to sync the payouts, per address
}
