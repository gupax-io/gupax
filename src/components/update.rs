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
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// This file contains all (most) of the code for updating.
// The main [Update] struct contains meta update information.
// It is held by the top [App] struct. Each package also gets
// a [Pkg] struct that only lasts as long as the download.
//
// An update is triggered by either:
//     a. user clicks update on [Gupax] tab
//     b. auto-update at startup

//---------------------------------------------------------------------------------------------------- Imports
#[cfg(target_family = "windows")]
use crate::miscs::get_exe_dir;
use crate::{
    app::BinariesVersion,
    disk::state::Gupax,
    helper::{ProcessName, notification::notif},
};
use bytes::Bytes;
use derive_more::{Deref, Display};
#[cfg(target_family = "unix")]
use flate2::bufread::GzDecoder;
use log::*;
use regex::Regex;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::{fs::create_dir_all, path::Path, process::exit, thread};
use std::{
    process::Command,
    sync::{Arc, Mutex},
};
use thiserror::Error;

//---------------------------------------------------------------------------------------------------- Constants
// Package naming schemes:
// gupax  | gupax-vX.X.X-(windows|macos|linux)-(x64|arm64).(zip|tar.gz)
// Download link = PREFIX + Version (found at runtime) + SUFFIX + Version + EXT
// Example: https://github.com/hinto-janai/gupax/releases/download/v0.0.1/gupax-v0.0.1-linux-standalone-x64.tar.gz
//

cfg_if::cfg_if! {
     if #[cfg(target_family = "unix")] {
    pub const P2POOL_BINARY: &str = "p2pool";
    pub const NODE_BINARY: &str = "monerod";
    pub const XMRIG_BINARY: &str = "xmrig";
    pub const XMRIG_PROXY_BINARY: &str = "xmrig-proxy";
     }
}
cfg_if::cfg_if! {
     if #[cfg(target_os = "windows")] {
    pub(super) const OS_TARGET: &str = "windows";
    pub(super) const ARCHIVE_EXT: &str = "zip";
    pub const P2POOL_BINARY: &str = "p2pool.exe";
    pub const NODE_BINARY: &str = "monerod.exe";
    pub const XMRIG_BINARY: &str = "xmrig.exe";
    pub const XMRIG_PROXY_BINARY: &str = "xmrig-proxy.exe";
     } else if #[cfg(target_os = "linux")] {
    pub(super) const OS_TARGET: &str = "linux";
    pub(super) const ARCHIVE_EXT: &str = "tar.gz";
     } else if #[cfg(target_os = "macos")] {
    pub(super) const OS_TARGET: &str = "macos";
    pub(super) const ARCHIVE_EXT: &str = "tar.gz";
     }
}

#[cfg(target_arch = "x86_64")]
pub(super) const ARCH_TARGET: &str = "x64";
#[cfg(target_arch = "aarch64")]
pub(super) const ARCH_TARGET: &str = "arm64";
// https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api?apiVersion=2022-11-28#user-agent
const APP_USER_AGENT: &str = "GUPAX";
const MSG_NONE: &str = "No update in progress";

//---------------------------------------------------------------------------------------------------- General functions
pub fn check_binary_path(path: &str, process: ProcessName) -> bool {
    let path = match crate::disk::into_absolute_path(path.to_string()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let filename = match path.file_name() {
        Some(p) => p,
        None => {
            error!("Couldn't get {process} file name");
            return false;
        }
    };
    filename == process.binary_name()
}

#[derive(Clone, Deref)]
pub struct Update {
    inner: Arc<Mutex<InnerUpdate>>,
}

#[derive(Clone)]
pub struct InnerUpdate {
    pub updating: bool, // Is an update in progress?
    pub prog: f32,      // Holds the 0-100% progress bar number
    pub msg: String,    // Message to display on [Gupax] tab while updating
    pub gupax_versions: Vec<Release>,
    pub p2pool_versions: Vec<Release>,
    pub xmrig_versions: Vec<Release>,
    pub xp_versions: Vec<Release>,
    pub node_versions: Vec<Release>,
    pub client: Client,
}

pub const BINARIES_NAME: [&str; 5] = ["gupax", "monerod", "p2pool", "xmrig", "xmrig-proxy"];
impl Update {
    // Takes in current paths from [State]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerUpdate {
                updating: false,
                prog: 0.0,
                msg: MSG_NONE.to_string(),
                gupax_versions: vec![],
                p2pool_versions: vec![],
                xmrig_versions: vec![],
                xp_versions: vec![],
                node_versions: vec![],
                client: ClientBuilder::new()
                    .user_agent(APP_USER_AGENT)
                    .build()
                    .unwrap(),
            })),
        }
    }

    fn update_version_with_latest_version(version: &mut String, releases: &[Release], beta: bool) {
        if let Some(latest) = releases
            .iter()
            .find(|r| if beta { r.prerelease } else { !r.prerelease })
        {
            *version = latest.to_string();
        }
    }
    pub fn update_all(
        &self,
        mut gupax_settings: Gupax,
        binaries_version: BinariesVersion,
        restart: Arc<Mutex<bool>>,
    ) {
        let update = self.clone();
        thread::spawn(move || {
            update.lock().unwrap().updating = true;
            let binaries = BINARIES_NAME.into_iter().map(|s| s.to_string()).collect();
            if let Err(e) =
                update.spawn_refresh_versions(&binaries, &gupax_settings, &binaries_version)
            {
                notif(&e.to_string());
                update.lock().unwrap().msg = format!("Refresh of metadata failed: {e}");
                update.lock().unwrap().updating = false;
                return;
            }
            // update config to latest version
            Self::update_version_with_latest_version(
                &mut gupax_settings.updates.gupax_version,
                &update.lock().unwrap().gupax_versions,
                gupax_settings.updates.beta,
            );
            Self::update_version_with_latest_version(
                &mut gupax_settings.updates.node_version,
                &update.lock().unwrap().node_versions,
                gupax_settings.updates.beta,
            );
            Self::update_version_with_latest_version(
                &mut gupax_settings.updates.p2pool_version,
                &update.lock().unwrap().p2pool_versions,
                gupax_settings.updates.beta,
            );
            Self::update_version_with_latest_version(
                &mut gupax_settings.updates.xmrig_version,
                &update.lock().unwrap().xmrig_versions,
                gupax_settings.updates.beta,
            );
            Self::update_version_with_latest_version(
                &mut gupax_settings.updates.proxy_version,
                &update.lock().unwrap().xp_versions,
                gupax_settings.updates.beta,
            );
            if update.is_update_available(&binaries_version) {
                match update.spawn_update_versions(&binaries, &gupax_settings, &binaries_version) {
                    Ok(_) => {
                        if !update.lock().unwrap().msg.contains("already up to date") {
                            if !gupax_settings.updates.automatic_restart {
                                notif(
                                    "A binary has been updated, you need to restart Gupax to apply the change",
                                );
                                *restart.lock().unwrap() = true;
                            } else {
                                restart_gupax();
                            }
                        }
                    }
                    Err(e) => {
                        update.lock().unwrap().msg = format!("Update failed: {:?}", e);
                        notif(&format!("{:?}", e));
                    }
                }
            }
            update.lock().unwrap().updating = false;
        });
    }
    pub fn is_update_available(&self, binaries_version: &BinariesVersion) -> bool {
        let update = self.lock().unwrap();
        for name in BINARIES_NAME {
            let available = match name {
                "gupax" => !update
                    .gupax_versions
                    .iter()
                    .any(|r| r.tag_name == binaries_version.gupax_version),
                "p2pool" => !update
                    .p2pool_versions
                    .iter()
                    .any(|r| r.tag_name == binaries_version.p2pool_version),
                "xmrig" => !update
                    .xmrig_versions
                    .iter()
                    .any(|r| r.tag_name == binaries_version.xmrig_version),
                "xmrig-proxy" => !update
                    .xp_versions
                    .iter()
                    .any(|r| r.tag_name == binaries_version.proxy_version),
                "monerod" => !update
                    .node_versions
                    .iter()
                    .any(|r| r.tag_name == binaries_version.node_version),
                _ => panic!("unknown name"),
            };
            if available {
                return true;
            }
        }
        false
    }
    pub fn refresh_versions(
        &self,
        binaries: Vec<String>,
        gupax_settings: Gupax,
        binaries_version: BinariesVersion,
    ) {
        let update = self.clone();
        thread::spawn(move || {
            update.lock().unwrap().updating = true;
            if let Err(e) =
                update.spawn_refresh_versions(&binaries, &gupax_settings, &binaries_version)
            {
                notif(&e.to_string());
            }
            update.lock().unwrap().updating = false;
        });
    }

    /// TODO, msg in case of failure
    #[tokio::main]
    async fn spawn_refresh_versions(
        &self,
        binaries: &Vec<String>,
        gupax_settings: &Gupax,
        binaries_version: &BinariesVersion,
    ) -> Result<(), reqwest::Error> {
        let client = self.lock().unwrap().client.clone();
        for name in binaries {
            let source = match name.as_str() {
                "gupax" => &gupax_settings.updates.gupax_source,
                "p2pool" => &gupax_settings.updates.p2pool_source,
                "xmrig" => &gupax_settings.updates.xmrig_source,
                "xmrig-proxy" => &gupax_settings.updates.proxy_source,
                "monerod" => &gupax_settings.updates.node_source,
                _ => panic!("unknown name"),
            };

            let mut url = format!("https://{source}");
            if source.contains("github.com") {
                url = url.replace("github.com", "api.github.com/repos");
                url.push_str("/releases");
            }
            let updated_versions = client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json::<Vec<Release>>()
                .await?;

            if let Some(v) = updated_versions.first()
                && v.tag_name != binaries_version.version_by_name(name)
            {
                // there is a new version, send a notification
                notif(&format!(
                    "New version available for {name}:\n{} published at {}",
                    v.tag_name,
                    v.published_at.date_naive()
                ));
            }
            *self.lock().unwrap().releases_by_name(name) = updated_versions;
        }
        Ok(())
    }
    // service should be stopped by UI when clicking the button update
    #[cfg(not(feature = "distro"))]
    pub fn update_version(
        &mut self,
        binaries: Vec<String>,
        gupax_settings: Gupax,
        binaries_version: BinariesVersion,
        restart: Arc<Mutex<bool>>,
    ) {
        let update = self.clone();
        thread::spawn(move || {
            update.lock().unwrap().updating = true;
            match update.spawn_update_versions(&binaries, &gupax_settings, &binaries_version) {
                Ok(_) => {
                    if !update.lock().unwrap().msg.contains("already up to date") {
                        if !gupax_settings.updates.automatic_restart {
                            notif(
                                "A binary has been updated, you need to restart Gupax to apply the change",
                            );
                            *restart.lock().unwrap() = true;
                        } else {
                            restart_gupax();
                        }
                    }
                }
                Err(e) => {
                    update.lock().unwrap().msg = format!("Update failed: {:?}", e);
                    notif(&format!("{:?}", e))
                }
            }
            update.lock().unwrap().updating = false;
        });
    }

    #[tokio::main]
    async fn spawn_update_versions(
        &self,
        binaries: &Vec<String>,
        gupax_settings: &Gupax,
        binaries_version: &BinariesVersion,
    ) -> Result<(), UpdateError> {
        let client = self.lock().unwrap().client.clone();
        let part_progress = 100.0 / binaries.len() as f32;
        self.lock().unwrap().prog = 0.0;
        for name in binaries {
            let source = gupax_settings.updates.source_by_name(name);
            let selected_version = gupax_settings.updates.selected_version_by_name(name);
            let current_version = binaries_version.version_by_name(name);
            let binary_path = if name == "gupax" {
                &std::env::current_exe()?
            } else {
                gupax_settings.path_by_name(name)
            };
            if selected_version == current_version {
                // binary is already at the selected version
                self.lock().unwrap().prog += part_progress;
                self.lock().unwrap().msg =
                    format!("The current version of {name} is already up to date");
                continue;
            }
            // create path only if path is valid and it doesn't exist
            if binary_path.is_empty() {
                return Err(UpdateError::EmptyPath(name.to_string()));
            }
            if binary_path.is_dir() {
                return Err(UpdateError::PathIsDir(name.to_string()));
            }
            if let Some(dir) = binary_path.parent() {
                create_dir_all(dir)?;
            }

            // download selected_version
            self.lock().unwrap().msg = format!("Downloading {name}");
            let (bytes, extension) =
                Self::get_binary(&client, name, selected_version, source).await?;
            self.lock().unwrap().prog += part_progress / 2.0;
            self.lock().unwrap().msg = format!("Extracting {name} binary from the archive");
            // On windows, move current binary if it does exist
            #[cfg(target_os = "windows")]
            {
                if binary_path.exists() {
                    let tmp_dir = Self::get_tmp_dir()?;
                    create_dir_all(&tmp_dir)?;
                    let tmp_windows = tmp_dir + &format!("{name}.exe");
                    std::fs::rename(binary_path, tmp_windows)?;
                }
            }
            match extension.as_str() {
                #[cfg(target_family = "unix")]
                "bz2" => {
                    let mut archive =
                        tar::Archive::new(bzip2_rs::DecoderReader::new(bytes.as_ref()));
                    archive
                        .entries()?
                        .into_iter()
                        .find(|entry| {
                            let path = entry.as_ref().unwrap().path().unwrap();
                            if let Some(filename) = path.file_name()
                                && filename == name.as_str()
                            {
                                return true;
                            }
                            false
                        })
                        .unwrap()?
                        .unpack(binary_path)?;
                }
                #[cfg(target_family = "unix")]
                "gz" => {
                    let mut archive = tar::Archive::new(GzDecoder::new(bytes.as_ref()));
                    for mut entry in archive.entries().unwrap().filter_map(|e| e.ok()) {
                        if entry.path()?.ends_with(name) {
                            entry.unpack(binary_path)?;
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                "zip" => {
                    use std::{fs::File, io::Cursor};
                    let mut archive = zip::ZipArchive::new(Cursor::new(bytes.as_ref()))?;
                    for i in 0..archive.len() {
                        let mut entry = archive.by_index(i)?;
                        let file = entry.name();

                        if let Some(file_name) =
                            Path::new(file).file_name().and_then(|n| n.to_str())
                            && (file_name.eq_ignore_ascii_case(&format!("{name}.exe"))
                                || (name == "xmrig" && file_name.eq("WinRing0x64.sys")))
                        {
                            if file_name.eq("WinRing0x64.sys") {
                                let mut path = binary_path.parent().unwrap().to_path_buf();
                                path.push("WinRing0x64.sys");

                                let mut out = File::create(path)?;
                                std::io::copy(&mut entry, &mut out)?;
                            } else {
                                let mut out = File::create(binary_path)?;
                                std::io::copy(&mut entry, &mut out)?;
                            }
                        }
                    }
                }
                _ => panic!("unsupported format"),
            };
            self.lock().unwrap().prog += part_progress / 2.0;
            self.lock().unwrap().msg = format!("Done updating {name}");
        }
        Ok(())
    }

    pub fn get_version_binary(path: &Path) -> Result<String, std::io::Error> {
        let mut cmd = Command::new(path);
        cmd.arg("--version");
        let output = cmd.output()?;
        let first_line = str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        let re = Regex::new(r"\b(v?\d+\.\d+(?:\.\d+){0,2})\b").unwrap();
        let mut version = re.captures(first_line).unwrap()[0].to_string();
        if !version.starts_with('v') {
            version.insert(0, 'v');
        }
        Ok(version)
    }

    async fn get_binary(
        client: &Client,
        name: &str,
        version: &str,
        source: &str,
    ) -> Result<(Bytes, String), reqwest::Error> {
        let url = match name {
            "gupax" => Self::standard_download_url(source, name, version),
            "p2pool" => Self::standard_download_url(source, name, version),
            "xmrig" => Self::standard_download_url(source, name, version)
                .replace("-v", "-")
                .replace("linux", "linux-static"),
            "xmrig-proxy" => Self::standard_download_url(source, name, version)
                .replace("-v", "-")
                .replace("linux", "linux-static"),
            // node does not have the binaries in the release but on getmonero.org
            // If the given source is github.com/monero-project/monero, download on getmonero.org
            "monerod" => {
                #[cfg(target_os = "linux")]
                let os_target = "linux";
                #[cfg(target_os = "windows")]
                let os_target = "win";
                #[cfg(target_os = "macos")]
                let os_target = "mac";
                #[cfg(target_family = "windows")]
                let ext = "zip";
                #[cfg(target_family = "unix")]
                let ext = "tar.bz2";
                if source == "github.com/monero-project/monero" {
                    [
                        "https://downloads.getmonero.org/cli/monero-",
                        os_target,
                        "-",
                        ARCH_TARGET,
                        "-",
                        version,
                        ".",
                        ext,
                    ]
                    .concat()
                } else {
                    [
                        "https://",
                        source,
                        "releases/download/",
                        "monero-",
                        os_target,
                        "-",
                        ARCH_TARGET,
                        "-",
                        version,
                        ".",
                        ext,
                    ]
                    .concat()
                }
            }
            _ => panic!("unknown name"),
        };
        let extension = url.split('.').next_back().unwrap();
        let bytes = client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        Ok((bytes, extension.to_owned()))
    }

    fn standard_download_url(source: &str, name: &str, version: &str) -> String {
        [
            "https://",
            source,
            "/releases/download/",
            version,
            "/",
            name,
            "-",
            version,
            "-",
            OS_TARGET,
            "-",
            ARCH_TARGET,
            ".",
            ARCHIVE_EXT,
        ]
        .concat()
    }

    // Get a temporary random folder for package download contents
    // This used to use [std::env::temp_dir()] but there were issues
    // using [std::fs::rename()] on tmpfs -> disk (Invalid cross-device link (os error 18)).
    // So, uses the [Gupax] binary directory as a base, something like [/home/hinto/gupax/gupax_update_SG4xsDdVmr]
    // Rename must be used on the same filesystem, but temp_dir could use a different filesystem than gupax.
    #[cfg(target_os = "windows")]
    pub fn get_tmp_dir() -> Result<String, std::io::Error> {
        use rand::{RngExt, distr::Alphanumeric, rng};
        let rand_string: String = rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();
        let base = get_exe_dir()?;
        let tmp_dir = format!("{}{}{}{}", base, r"\gupax_update_", rand_string, r"\");
        info!("Update | Temporary directory ... {tmp_dir}");
        Ok(tmp_dir)
    }
}

impl InnerUpdate {
    pub fn releases_by_name(&mut self, name: &str) -> &mut Vec<Release> {
        match name {
            "gupax" => &mut self.gupax_versions,
            "p2pool" => &mut self.p2pool_versions,
            "xmrig" => &mut self.xmrig_versions,
            "xmrig-proxy" => &mut self.xp_versions,
            "monerod" => &mut self.node_versions,
            _ => panic!("unknown name"),
        }
    }
}

use chrono::{DateTime, Utc};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Display)]
#[display("{}", tag_name)]
pub struct Release {
    pub tag_name: String,
    pub prerelease: bool,
    pub body: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("Path of {0} is empty. Check the advanced submenu of the Settings tab")]
    EmptyPath(String),
    #[error(
        "Path of {0} is a directory but it should be a file. Check the advanced submenu of the Settings tab"
    )]
    PathIsDir(String),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[cfg(target_family = "windows")]
    #[error(transparent)]
    ZipArchive(#[from] zip::result::ZipError),
}

fn restart_gupax() {
    warn!("Restarting Gupax after upgrading !");
    let gupax_path = std::env::current_exe().unwrap();
    let gupax_args = std::env::args();
    let args = gupax_args.skip(1).collect::<Vec<String>>();
    let mut cmd = Command::new(gupax_path);
    cmd.args(args);
    // The successor is spawned while this process is still alive, so hand
    // over the single-instance guard first or it can find it still held
    // and exit on the spot, leaving no Gupax running at all.
    crate::utils::single_instance::release();
    cmd.spawn().unwrap();
    exit(0)
}
