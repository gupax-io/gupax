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

use crate::app::panels::middle::common::list_poolnode::PoolNode;
use crate::utils::node_latency::port_ping;
use derive_more::{Deref, DerefMut};
use egui::Color32;
use enclose::enc;
use log::*;
use rand::{RngExt, rng};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Serialize, Eq)]
pub struct RemoteNode {
    pub ip: IpAddr,
    pub rpc: u16,
    pub zmq: u16,
    pub ms: u64,
}

// we ignore latency to identify nodes
impl PartialEq for RemoteNode {
    fn eq(&self, other: &Self) -> bool {
        self.ip == other.ip && self.rpc == other.rpc && self.zmq == other.zmq
    }
}

impl PartialEq<PoolNode> for RemoteNode {
    fn eq(&self, other: &PoolNode) -> bool {
        self.ip.to_string() == other.ip()
            && self.rpc.to_string() == other.port()
            && self.zmq.to_string() == other.custom()
    }
}

#[derive(DerefMut, Deref, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct RemoteNodes(Vec<RemoteNode>);

impl RemoteNodes {
    // Returns a default if index is not found in the const array.
    pub fn index_or_random(&self, index: usize) -> Option<&RemoteNode> {
        if index >= self.len() {
            self.get_random_same_ok()
        } else {
            Some(&self[index])
        }
    }
    pub fn find_selected(&self, selected: &RemoteNode) -> Option<&RemoteNode> {
        self.iter().find(|node| *node == selected)
    }
    // Return a random node (that isn't the one already selected).
    // Return a random valid node (no input str).
    pub fn get_random_same_ok(&self) -> Option<&RemoteNode> {
        if self.is_empty() {
            return None;
        }
        let rng = rng().random_range(0..self.len());
        self.index_or_random(rng)
    }
    // Return the node [-1] of this one
    pub fn get_last(&self, current: &RemoteNode) -> RemoteNode {
        let mut found = false;
        let mut last = current;
        for node in self.iter() {
            if found {
                return node.clone();
            }
            if current == node {
                found = true;
            } else {
                last = node;
            }
        }
        last.clone()
    }
    // Return the node [+1] of this one
    pub fn get_next(&self, current: &RemoteNode) -> RemoteNode {
        let mut found = false;
        for node in self.iter() {
            if found {
                return node.clone();
            }
            if current == node {
                found = true;
            }
        }
        current.clone()
    }
}

impl RemoteNode {
    pub fn ping_color(&self) -> Color32 {
        // if it comes from the crawler, the value should always be set.
        // But if it's 0 (for example if in the future this method is used for used for manually inserted nodes), use the gray color
        if self.ms == 0 {
            return Color32::GRAY;
        }
        match self.ms.cmp(&GREEN_NODE_PING) {
            Ordering::Less | Ordering::Equal => Color32::GREEN,
            Ordering::Greater => match self.ms.cmp(&RED_NODE_PING) {
                Ordering::Less => Color32::ORANGE,
                Ordering::Greater | Ordering::Equal => Color32::RED,
            },
        }
    }

    // TODO if ever wanting to show the country
    // Use a database https://github.com/sapics/ip-location-db to show country of discovered node
    // pub fn country(&self) -> String {
    //     "Country Soon".to_string()
    // }
}

impl std::fmt::Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:#?}", self.ip)
    }
}

//---------------------------------------------------------------------------------------------------- Formatting
// 5000 = 4 max length
pub fn format_ms(ms: u64) -> String {
    match ms.to_string().len() {
        1 => format!("{ms}ms   "),
        2 => format!("{ms}ms  "),
        3 => format!("{ms}ms "),
        _ => format!("{ms}ms"),
    }
}

//---------------------------------------------------------------------------------------------------- Node data
pub const GREEN_NODE_PING: u64 = 100;
// yellow is anything in-between green/red
pub const RED_NODE_PING: u64 = 300;
pub const TIMEOUT_NODE_PING: u64 = 1000;

//---------------------------------------------------------------------------------------------------- `/get_info`
// A struct repr of the JSON-RPC we're
// expecting back from the pinged nodes.
//
// This struct leaves out most fields on purpose,
// we only need a few to verify the node is ok.
#[allow(dead_code)] // allow dead code because Deserialize doesn't use all the fields in this program
#[derive(Debug, serde::Deserialize)]
pub struct GetInfo<'a> {
    pub id: &'a str,
    pub jsonrpc: &'a str,
    pub _result: GetInfoResult,
}

#[derive(Debug, serde::Deserialize)]
pub struct GetInfoResult {
    pub _mainnet: bool,
    pub _synchronized: bool,
}

//---------------------------------------------------------------------------------------------------- Ping data
#[derive(Debug)]
pub struct Ping {
    pub nodes: RemoteNodes,
    pub pinging: bool,
    pub msg: String,
    pub prog: f32,
    pub pinged: bool,
    pub auto_selected: bool,
}

impl Ping {
    pub fn new(nodes: RemoteNodes) -> Self {
        Self {
            nodes,
            pinging: false,
            msg: "No ping in progress".to_string(),
            prog: 0.0,
            pinged: false,
            auto_selected: true,
        }
    }

    //---------------------------------------------------------------------------------------------------- Main Ping function
    #[cold]
    #[inline(never)]
    // Intermediate function for spawning thread
    pub fn spawn_thread(ping: &Arc<Mutex<Self>>) {
        info!("Spawning ping thread...");
        let ping = Arc::clone(ping);
        std::thread::spawn(move || {
            let now = Instant::now();
            match Self::ping(&ping) {
                Ok(msg) => {
                    info!("Ping ... OK");
                    ping.lock().unwrap().msg = msg;
                    ping.lock().unwrap().pinged = true;
                    ping.lock().unwrap().auto_selected = false;
                    ping.lock().unwrap().prog = 100.0;
                }
                Err(err) => {
                    error!("Ping ... FAIL ... {err}");
                    ping.lock().unwrap().pinged = false;
                    ping.lock().unwrap().msg = err.to_string();
                }
            }
            info!("Ping ... Took [{}] seconds...", now.elapsed().as_secs_f32());
            ping.lock().unwrap().pinging = false;
        });
    }

    // This is for pinging the remote nodes to
    // find the fastest/slowest one for the user.
    // The process:
    //   - Send [get_info] JSON-RPC request over HTTP to all IPs
    //   - Measure each request in milliseconds
    //   - Timeout on requests over 5 seconds
    //   - Add data to appropriate struct
    //   - Sorting fastest to lowest is automatic (fastest nodes return ... the fastest)
    //
    // This used to be done 3x linearly but after testing, sending a single
    // JSON-RPC call to all IPs asynchronously resulted in the same data.
    #[cold]
    #[inline(never)]
    #[tokio::main]
    pub async fn ping(ping: &Arc<Mutex<Self>>) -> Result<String, anyhow::Error> {
        // Start ping
        ping.lock().unwrap().pinging = true;
        ping.lock().unwrap().prog = 0.0;
        let len = ping.lock().unwrap().nodes.len();
        let percent = Arc::new((100.0 / (len as f32)).floor());

        // Handle vector
        let mut handles = Vec::with_capacity(len);
        let mut nodes = ping.lock().unwrap().nodes.clone();
        let vec_nodes = Arc::new(Mutex::new(Vec::with_capacity(nodes.len())));
        for node in nodes.iter() {
            let handle = tokio::task::spawn(enc!((vec_nodes, node, ping, percent) async move {
                let socket_address = SocketAddr::new(node.ip, node.zmq);

                if let Ok(ms) = port_ping(socket_address, TIMEOUT_NODE_PING).await {
                    let info = format!("{ms}ms ... {}", node.ip);
                    info!("Ping | {ms}ms ... {}", node.ip);

                    let mut ping = ping.lock().unwrap();
                    ping.msg = info;
                    ping.prog += *percent;
                    drop(ping);
                    let mut node = node.clone();
                    node.ms = ms;
                    vec_nodes.lock().unwrap().push(node);
                }
            }));
            handles.push(handle);
        }

        for handle in handles {
            handle.await?;
        }
        nodes = RemoteNodes(vec_nodes.lock().unwrap().to_vec());
        nodes.sort_by(|a, b| a.ms.cmp(&b.ms));
        let fastest_info;
        if let Some(node) = nodes.first() {
            fastest_info = format!("Fastest node: {}ms ... {}", node.ms, node.ip);
        } else {
            fastest_info = "Pinged without any nodes".to_string();
        }

        let info = "Cleaning up connections".to_string();
        info!("Ping | {info}...");
        let mut ping = ping.lock().unwrap();
        ping.msg = info;
        ping.nodes = nodes;
        drop(ping);
        Ok(fastest_info)
    }
    // This returns relative to the ping.
    pub fn get_last_from_ping(&self, current: &RemoteNode) -> RemoteNode {
        let mut found = false;
        let mut last = current;
        for data in self.nodes.iter() {
            if found {
                return last.clone();
            }
            if current == data {
                found = true;
            } else {
                last = data;
            }
        }
        last.clone()
    }

    pub fn get_next_from_ping(&self, current: &RemoteNode) -> RemoteNode {
        let mut found = false;
        for data in self.nodes.iter() {
            if found {
                return data.clone();
            }
            if current == data {
                found = true;
            }
        }
        current.clone()
    }
}
