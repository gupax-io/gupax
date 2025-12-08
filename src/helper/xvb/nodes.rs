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

use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::{Arc, Mutex},
};

use derive_more::Display;
use log::{info, warn};
use serde::Deserialize;
use tokio::spawn;

use crate::{
    GUPAX_VERSION_UNDERSCORE, XVB_NODE_EU, XVB_NODE_NA, XVB_NODE_PORT,
    components::node::TIMEOUT_NODE_PING,
    disk::state::{P2pool, Xvb},
    helper::{Process, ProcessName, ProcessState, p2pool::ImgP2pool, xvb::output_console},
    utils::node_latency::port_ping,
};

use super::PubXvbApi;
#[derive(Clone, Debug, Default, PartialEq, Display, Deserialize)]
pub enum Pool {
    #[display("XvB North America Pool")]
    XvBNorthAmerica,
    #[default]
    #[display("XvB European Pool")]
    XvBEurope,
    #[display("Local P2pool")]
    P2pool(u16),
    #[display("Xmrig Proxy")]
    XmrigProxy(u16),
    #[display("Custom Pool")]
    Custom(String, u16),
    #[display("Not connected to any pool")]
    Unknown,
}
impl Pool {
    pub fn url(&self) -> String {
        match self {
            Self::XvBNorthAmerica => String::from(XVB_NODE_NA),
            Self::XvBEurope => String::from(XVB_NODE_EU),
            Self::P2pool(_) => String::from("127.0.0.1"),
            Self::XmrigProxy(_) => String::from("127.0.0.1"),
            Self::Custom(url, _) => url.clone(),
            _ => "???".to_string(),
        }
    }
    pub fn port(&self) -> String {
        match self {
            Self::XvBNorthAmerica | Self::XvBEurope => String::from(XVB_NODE_PORT),
            Self::P2pool(port) => port.to_string(),
            Self::XmrigProxy(port) => port.to_string(),
            Self::Custom(_, port) => port.to_string(),
            _ => "???".to_string(),
        }
    }
    pub fn user(&self, address: &str) -> String {
        match self {
            Self::XvBNorthAmerica => address.chars().take(8).collect(),
            Self::XvBEurope => address.chars().take(8).collect(),
            _ => GUPAX_VERSION_UNDERSCORE.to_string(),
        }
    }
    pub fn tls(&self) -> bool {
        match self {
            Self::XvBNorthAmerica => true,
            Self::XvBEurope => true,
            Self::P2pool(_) => false,
            Self::XmrigProxy(_) => false,
            Self::Custom(_, _) => false,
            _ => false,
        }
    }
    pub fn keepalive(&self) -> bool {
        match self {
            Self::XvBNorthAmerica => true,
            Self::XvBEurope => true,
            Self::P2pool(_) => false,
            Self::XmrigProxy(_) => false,
            Self::Custom(_, _) => false,
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_fastest_pool(
        pub_api_xvb: &Arc<Mutex<PubXvbApi>>,
        gui_api_xvb: &Arc<Mutex<PubXvbApi>>,
        process_xvb: &Arc<Mutex<Process>>,
        process_p2pool: &Arc<Mutex<Process>>,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        p2pool_state: &P2pool,
        xvb_state: &Xvb,
    ) {
        // ping XvB nodes, or only one if set manual
        let xvb_pools_to_ping = if xvb_state.manual_pool_enabled {
            if xvb_state.manual_pool_eu {
                vec![Pool::XvBEurope]
            } else {
                vec![Pool::XvBNorthAmerica]
            }
        } else {
            vec![Pool::XvBNorthAmerica, Pool::XvBEurope]
        };

        // prepare the ping job
        let mut handles = vec![];
        for pool in xvb_pools_to_ping.clone() {
            info!("XvB | ping {pool} XvB pool");
            handles.push(spawn(async move {
                let socket_address = format!("{}:{}", pool.url(), pool.port())
                    .to_socket_addrs()
                    .expect("hardcored valued should always convert to SocketAddr")
                    .collect::<Vec<SocketAddr>>()[0];
                (port_ping(socket_address, TIMEOUT_NODE_PING).await, pool)
            }));
        }
        // ping pools at the same time
        let mut results = vec![];
        for handle in handles {
            let result = handle
                .await
                .ok()
                .unwrap_or_else(|| (Err(anyhow::Error::msg("")), Pool::default()));
            results.push((result.0.ok(), result.1));
        }

        // filter and return the lowest latency pool
        let pool = results
            .into_iter()
            .filter_map(|(ms, pool)| ms.map(|ms| (ms, pool)))
            .min_by_key(|(ms, _)| *ms);

        let chosen_pool = if let Some(fastest_pool) = pool {
            fastest_pool.1
        } else {
            Pool::P2pool(p2pool_state.current_port(
                process_p2pool.lock().unwrap().is_alive(),
                &p2pool_img.lock().unwrap(),
            ))
        };

        if chosen_pool
            == Pool::P2pool(p2pool_state.current_port(
                process_p2pool.lock().unwrap().is_alive(),
                &p2pool_img.lock().unwrap(),
            ))
        {
            xvb_pools_to_ping
                .iter()
                .for_each(|p| warn!("ping for {p} failed !"));
            // if both nodes are dead, then the state of the process must be NodesOffline
            warn!("XvB node ping, all offline or ping failed, switching back to local p2pool",);
            output_console(
                &mut gui_api_xvb.lock().unwrap().output,
                "XvB node ping, all offline or ping failed, switching back to local p2pool",
                ProcessName::Xvb,
            );
            process_xvb.lock().unwrap().state = ProcessState::OfflinePoolsAll;
        } else {
            info!("XvB pool ping, chosen pool is {}", chosen_pool.url());
            // set a different message if the user manually selected the pool
            if xvb_state.manual_pool_enabled {
                output_console(
                    &mut gui_api_xvb.lock().unwrap().output,
                    &format!(
                        "XvB Pool ping, {chosen_pool} has been manually selected and is online."
                    ),
                    ProcessName::Xvb,
                );
            } else {
                output_console(
                    &mut gui_api_xvb.lock().unwrap().output,
                    &format!("XvB Pool ping, {chosen_pool} is selected as the fastest."),
                    ProcessName::Xvb,
                );
            }
            info!("ProcessState to Syncing after finding joinable node");
            // could be used by xmrig who signal that a node is not joignable
            // or by the start of xvb
            // next iteration of the loop of XvB process will verify if all conditions are met to be alive.
            if process_xvb.lock().unwrap().state != ProcessState::Syncing {
                process_xvb.lock().unwrap().state = ProcessState::Syncing;
            }
        }
        pub_api_xvb.lock().unwrap().stats_priv.pool = chosen_pool;
    }
}
