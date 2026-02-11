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

pub mod xmrig;
pub mod xmrig_proxy;

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
        // dbg!(&request);
        let mut config = request.send().await?.json::<Value>().await?;
        // modify node configuration
        let uri = format!("{}:{}", target_pool.url(), target_pool.port());
        info!("replacing {self} config from api url {url_api} config with node {target_pool}");
        let pointer_base = "/pools/0/";
        let pointer_url = [pointer_base, "url"].concat();
        // dbg!(&config);
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
