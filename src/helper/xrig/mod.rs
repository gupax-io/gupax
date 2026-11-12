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

use std::sync::Arc;
use std::sync::Mutex;

use crate::XMRIG_API_CONFIG_ENDPOINT;
use crate::helper::Pool;
use crate::helper::xrig::xmrig::PubXmrigApi;
use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
use anyhow::Result;
use derive_more::Display;
use log::info;
use reqwest::header::AUTHORIZATION;
use reqwest_middleware::ClientWithMiddleware as Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use xmrig::ImgXmrig;
use xmrig_proxy::ImgProxy;

#[cfg(any(target_os = "windows", test))]
mod elevated;
pub mod xmrig;
pub mod xmrig_proxy;

// Waits for what `child` starts, until `ready` returns something else than WouldBlock. A Stop
// kills `child` and ends the wait with None.
fn wait_ready<T>(
    child: &mut std::process::Child,
    mut ready: impl FnMut() -> std::io::Result<T>,
    stopped: impl Fn() -> bool,
    program: &str,
    awaited: &str,
) -> std::io::Result<Option<T>> {
    let mut exited = None;
    loop {
        match ready() {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            ready => return ready.map(Some),
        }
        // The kill fails on a child that got more rights than Gupax, whose wait goes on.
        if stopped()
            && child
                .kill()
                .inspect_err(|e| log::warn!("XMRig | Could not kill {program}: {e}"))
                .is_ok()
        {
            child.wait()?;
            return Ok(None);
        }
        if let Some(status) = exited {
            return Err(std::io::Error::other(format!(
                "{program} exited ({status}) before {awaited}"
            )));
        }
        // Seen one pass before it fails the wait, so that `ready` finds what the child left.
        exited = child.try_wait()?;
        crate::utils::macros::sleep!(100);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Hashrate {
    total: [Option<f32>; 3],
}

#[derive(Display, Clone, Debug)]
pub enum HashrateProvider {
    #[display("Xmrig")]
    Xmrig(Arc<Mutex<PubXmrigApi>>, Arc<Mutex<ImgXmrig>>, Client),
    #[display("Proxy")]
    Proxy(Arc<Mutex<PubXmrigProxyApi>>, Arc<Mutex<ImgProxy>>, Client),
}

impl HashrateProvider {
    // get the current HR of xmrig or xmrig-proxy
    // will get a longer average HR since it will be more accurate. Shorter timeframe can induce volatility.
    pub fn current_controllable_hr(&self) -> f32 {
        match self {
            Self::Xmrig(api, _, _) => {
                let guard = api.lock().unwrap();
                if guard.hashrate_raw_15m > 0.0 {
                    guard.hashrate_raw_15m
                } else if guard.hashrate_raw_1m > 0.0 {
                    guard.hashrate_raw_1m
                } else {
                    guard.hashrate_raw
                }
            }
            Self::Proxy(api, _, _) => {
                let guard = api.lock().unwrap();
                if guard.hashrate_10m > 0.0 {
                    guard.hashrate_10m
                } else {
                    guard.hashrate_1m
                }
            }
        }
    }

    fn url_api_config(&self) -> String {
        match self {
            Self::Xmrig(_, img, _) => {
                let port = img.lock().unwrap().api_port;
                url_api_xrig_config(port)
            }
            Self::Proxy(_, img, _) => {
                let port = img.lock().unwrap().api_port;
                url_api_xrig_config(port)
            }
        }
    }
    fn client(&self) -> &Client {
        match self {
            Self::Xmrig(_, _, client) => client,
            Self::Proxy(_, _, client) => client,
        }
    }

    pub fn current_pool(&self) -> Option<Pool> {
        match self {
            Self::Xmrig(api, _, _) => api.lock().unwrap().pool.clone(),
            Self::Proxy(api, _, _) => api.lock().unwrap().pool.clone(),
        }
    }

    fn token(&self) -> String {
        match self {
            Self::Xmrig(_, img, _) => img.lock().unwrap().token.clone(),
            Self::Proxy(_, img, _) => img.lock().unwrap().token.clone(),
        }
    }

    /// Update config of the hashrate provider using it's HTTP API
    pub async fn update_config(&self, target_pool: &Pool) -> Result<(), WorkerError> {
        // get config
        let url_api = self.url_api_config();
        let authorization_value = format!("Bearer {}", self.token());
        let request = self
            .client()
            .get(&url_api)
            .header(AUTHORIZATION, &authorization_value);
        let mut config = request.send().await?.json::<Value>().await?;
        // modify node configuration
        let uri = format!("{}:{}", target_pool.url(), target_pool.port());
        info!("replacing {self} config from api url {url_api} config with node {target_pool}");
        let pointer_base = "/pools/0/";
        let pointer_url = [pointer_base, "url"].concat();
        *config
            .pointer_mut(&pointer_url)
            .ok_or_else(|| WorkerError::MissingField(pointer_url))? = uri.into();
        let pointer_user = [pointer_base, "user"].concat();
        *config
            .pointer_mut(&pointer_user)
            .ok_or_else(|| WorkerError::MissingField(pointer_user))? = target_pool.user().into();
        let pointer_rig = [pointer_base, "rig-id"].concat();
        *config
            .pointer_mut(&pointer_rig)
            .ok_or_else(|| WorkerError::MissingField(pointer_rig))? = target_pool.user().into();
        let pointer_tls = [pointer_base, "tls"].concat();
        *config
            .pointer_mut(&pointer_tls)
            .ok_or_else(|| WorkerError::MissingField(pointer_tls))? = target_pool.tls().into();
        let pointer_keepalive = [pointer_base, "keepalive"].concat();
        *config
            .pointer_mut(&pointer_keepalive)
            .ok_or_else(|| WorkerError::MissingField(pointer_keepalive))? =
            target_pool.keepalive().into();
        // send new config
        self.client()
            .put(url_api)
            .header("Authorization", authorization_value)
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(5))
            .body(config.to_string())
            .send()
            .await?;
        info!("{self} | Successfully updated the {self} config");
        Ok(())
    }
}

/// The url is the same for Xmrig and Proxy, apart from the port
fn url_api_xrig_config(api_port: u16) -> String {
    format!("http://127.0.0.1:{api_port}/{XMRIG_API_CONFIG_ENDPOINT}")
}
#[derive(Error, Debug)]
pub enum WorkerError {
    #[error(transparent)]
    Json(#[from] reqwest::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest_middleware::Error),
    #[error("Path {0} does not exist in worker config")]
    MissingField(String),
}

#[cfg(test)]
mod tests {
    use super::wait_ready;
    use crate::helper::{exited, sleeping, within};

    fn not_ready() -> std::io::Result<()> {
        Err(std::io::ErrorKind::WouldBlock.into())
    }

    #[test]
    fn a_start_waits_until_it_is_ready() {
        let mut tries = [Err(std::io::ErrorKind::WouldBlock.into()), Ok(2)].into_iter();
        let mut child = sleeping();
        let ready = || tries.next().unwrap();
        let started = wait_ready(
            &mut child,
            ready,
            || false,
            "PowerShell",
            "the helper connected",
        );
        assert_eq!(started.unwrap(), Some(2));
        child.kill().unwrap();
    }

    #[test]
    fn a_stop_ends_the_wait_of_a_start_with_its_child() {
        let mut child = sleeping();
        let (started, mut child) = within(move || {
            let started = wait_ready(
                &mut child,
                not_ready,
                || true,
                "PowerShell",
                "the helper connected",
            );
            (started, child)
        });
        assert_eq!(started.unwrap(), None);
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn a_start_fails_once_its_child_exited() {
        let mut child = exited(3);
        let status = child.wait().unwrap();
        let started = within(move || {
            wait_ready(
                &mut child,
                not_ready,
                || false,
                "PowerShell",
                "the helper connected",
            )
        });
        assert_eq!(
            started.unwrap_err().to_string(),
            format!("PowerShell exited ({status}) before the helper connected")
        );
    }

    #[test]
    fn a_start_finds_what_its_child_left_at_its_exit() {
        let mut tries = [Err(std::io::ErrorKind::WouldBlock.into()), Ok(2)].into_iter();
        let ready = || tries.next().unwrap();
        let started = wait_ready(
            &mut exited(0),
            ready,
            || false,
            "pkexec",
            "XMRig wrote a line",
        );
        assert_eq!(started.unwrap(), Some(2));
    }
}
