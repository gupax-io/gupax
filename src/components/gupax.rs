// Gupax - GUI Uniting P2Pool And XMRig
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

use crate::{disk::state::*, utils::macros::arc_mut};
use log::*;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    thread,
};

//---------------------------------------------------------------------------------------------------- FileWindow
// Struct for writing/reading the path state.
// The opened file picker is started in a new
// thread so main() needs to be in sync.
pub struct FileWindow {
    pub thread: bool,             // Is there already a FileWindow thread?
    pub picked_p2pool: bool,      // Did the user pick a path for p2pool?
    pub picked_xmrig: bool,       // Did the user pick a path for xmrig?
    pub picked_xp: bool,          // Did the user pick a path for xmrig-proxy?
    pub picked_node: bool,        // Did the user pick a path for node?
    pub picked_nodedb: bool,      // Did the user pick a path for node?
    pub picked_view_key: bool,    // Did the user pick a file for the private view key?
    pub p2pool_path: String,      // The picked p2pool path
    pub node_path: String,        // The picked node path
    pub nodedb_path: String,      // The picked node path
    pub xmrig_path: String,       // The picked xmrig path
    pub xmrig_proxy_path: String, // The picked xmrig-proxy path
    pub view_key_path: String,    // The picked private view key path
}

impl FileWindow {
    pub fn new() -> Arc<Mutex<Self>> {
        arc_mut!(Self {
            thread: false,
            picked_p2pool: false,
            picked_xmrig: false,
            picked_xp: false,
            picked_node: false,
            picked_nodedb: false,
            picked_view_key: false,
            p2pool_path: String::new(),
            node_path: String::new(),
            nodedb_path: String::new(),
            xmrig_path: String::new(),
            xmrig_proxy_path: String::new(),
            view_key_path: String::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum FileType {
    P2pool,
    Xmrig,
    XmrigProxy,
    Node,
    NodeDB,
    ViewKey,
}

//---------------------------------------------------------------------------------------------------- Ratio Lock
// Enum for the lock ratio in the advanced tab.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub enum Ratio {
    Width,
    Height,
    None,
}

//---------------------------------------------------------------------------------------------------- Gupax
impl Gupax {
    // Checks if a path is a valid path to a file.
    pub fn path_is_file(path: &str) -> bool {
        let path = path.to_string();
        match crate::disk::into_absolute_path(path) {
            Ok(path) => path.is_file(),
            _ => false,
        }
    }
    // Checks if a path is a valid path to a directory.
    pub fn path_is_dir(path: &str) -> bool {
        let path = path.to_string();
        match crate::disk::into_absolute_path(path) {
            Ok(path) => path.is_dir(),
            _ => false,
        }
    }

    #[cold]
    #[inline(never)]
    pub fn spawn_file_window_thread(file_window: &Arc<Mutex<FileWindow>>, file_type: FileType) {
        use FileType::*;
        let name = match file_type {
            P2pool => "P2Pool",
            Xmrig => "XMRig",
            XmrigProxy => "XMRigProxy",
            Node => "Node",
            NodeDB => "Node DB",
            ViewKey => "private view key",
        };
        let file_window = file_window.clone();
        file_window.lock().unwrap().thread = true;
        thread::spawn(move || {
            let path = match file_type {
                NodeDB => rfd::FileDialog::new()
                    .set_title("Select a directory for the DB of your Node")
                    .pick_folder(),
                ViewKey => rfd::FileDialog::new()
                    .set_title("Select the file of your private view key")
                    .pick_file(),
                _ => rfd::FileDialog::new()
                    .set_title(format!("Select {name} Binary for Gupax"))
                    .pick_file(),
            };
            if let Some(path) = path {
                info!("Gupax | Path selected for {} ... {}", name, path.display());
                match file_type {
                    P2pool => {
                        file_window.lock().unwrap().p2pool_path = path.display().to_string();
                        file_window.lock().unwrap().picked_p2pool = true;
                    }
                    Xmrig => {
                        file_window.lock().unwrap().xmrig_path = path.display().to_string();
                        file_window.lock().unwrap().picked_xmrig = true;
                    }
                    XmrigProxy => {
                        file_window.lock().unwrap().xmrig_proxy_path = path.display().to_string();
                        file_window.lock().unwrap().picked_xp = true;
                    }
                    Node => {
                        file_window.lock().unwrap().node_path = path.display().to_string();
                        file_window.lock().unwrap().picked_node = true;
                    }
                    NodeDB => {
                        file_window.lock().unwrap().nodedb_path = path.display().to_string();
                        file_window.lock().unwrap().picked_nodedb = true;
                    }
                    ViewKey => {
                        file_window.lock().unwrap().view_key_path = path.display().to_string();
                        file_window.lock().unwrap().picked_view_key = true;
                    }
                };
            } else {
                info!("Gupax | No path selected for {name}");
            }

            file_window.lock().unwrap().thread = false;
        });
    }
}
