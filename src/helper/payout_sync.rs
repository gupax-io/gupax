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

//! Syncs the payout history: adds the payouts that Gupax did not see in the P2Pool output
//! and removes the payouts of orphaned blocks.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use log::*;
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;

use super::MONERO_BLOCK_TIME_IN_SECONDS;
use crate::{
    components::update::APP_USER_AGENT,
    constants::{P2POOL_PAYOUT_CHECK_DEPTH, SECOND},
    disk::{gupax_p2pool_api::GupaxP2poolApi, node::Node},
    miscs::client_with,
    xmr::AtomicUnit,
};

// Time without data after which a request fails.
// The largest requests of a sync started answering within 0.2 s from a remote node.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

// (date, atomic_unit, height) of each payout found.
type Payouts = Vec<(String, AtomicUnit, u64)>;

// The missing payouts are listed by the observer if set.
pub struct SyncSources {
    pub observer: String,
    // Address P2Pool mines to.
    pub address: String,
    // URL of the RPC of the node P2Pool uses.
    pub node: Option<String>,
    // File of the P2Pool data API listing the last blocks found by the pool.
    pub found_blocks: PathBuf,
}

impl SyncSources {
    pub fn new(api: &GupaxP2poolApi, node: Option<&Node>, observer: String) -> Self {
        Self {
            observer,
            address: api.address.clone(),
            node: node.map(|node| {
                let ip = if node.ip.contains(':') {
                    format!("[{}]", node.ip)
                } else {
                    node.ip.clone()
                };
                format!("http://{ip}:{}", node.rpc)
            }),
            found_blocks: api.found_blocks.clone(),
        }
    }
}

// Address P2Pool pays: [subaddress] if set, else [wallet].
pub fn payout_address(wallet: &str, subaddress: &str) -> String {
    if subaddress.is_empty() {
        wallet.to_string()
    } else {
        subaddress.to_string()
    }
}

// Starts a sync in a new thread, unless one is running.
// Returns whether it started.
pub fn start(
    api: &mut GupaxP2poolApi,
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    sources: SyncSources,
) -> bool {
    if api.syncing {
        return false;
    }
    api.syncing = true;
    api.stop_sync = false;
    api.sync.clear();
    let gupax_p2pool_api = Arc::clone(gupax_p2pool_api);
    std::thread::spawn(move || sync(&gupax_p2pool_api, &sources));
    true
}

#[tokio::main]
async fn sync(gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>, sources: &SyncSources) {
    let mut results = Vec::new();
    tokio::select! {
        () = run(gupax_p2pool_api, sources, &mut results) => (),
        () = stopped(gupax_p2pool_api) => results.push("Stopped".to_string()),
    }
    let mut api = gupax_p2pool_api.lock().unwrap();
    api.sync = results.join(". ");
    info!("Payout sync | {}", api.sync);
    api.syncing = false;
}

// Completes once the user stops the sync.
async fn stopped(gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>) {
    while !gupax_p2pool_api.lock().unwrap().stop_sync {
        tokio::time::sleep(SECOND).await;
    }
}

async fn run(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    sources: &SyncSources,
    results: &mut Vec<String>,
) {
    let client = client();
    if !sources.observer.is_empty() {
        let payouts = observer_payouts(&client, &sources.observer, &sources.address).await;
        results.push(match payouts {
            Ok(payouts) => merge(gupax_p2pool_api, &payouts),
            Err(e) => format!("Sync error: {e:#}"),
        });
    }
    let node = synced_node(&client, sources.node.as_deref()).await;
    results.push(match &node {
        Ok(node) => {
            let found_blocks = &sources.found_blocks;
            match remove_orphaned_payouts(gupax_p2pool_api, &client, node, found_blocks).await {
                Ok(removed) => format!("Orphaned payouts removed: {removed}"),
                Err(e) => format!("Orphan check error: {e:#}"),
            }
        }
        Err(e) => format!("Orphan check error: {e:#}"),
    });
}

// Client failing on a request that stalls.
fn client() -> ClientWithMiddleware {
    client_with(
        reqwest::Client::builder()
            .read_timeout(READ_TIMEOUT)
            .build()
            .unwrap_or_default(),
    )
}

// Adds the missing [payouts] to the history and describes the result.
fn merge(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    payouts: &[(String, AtomicUnit, u64)],
) -> String {
    match gupax_p2pool_api.lock().unwrap().merge_payouts(payouts) {
        Ok(added) => format!("Missing payouts added: {added}"),
        Err(e) => format!("Sync error: payout history write error: {e}"),
    }
}

//---------------------------------------------------------------------------------------------------- Observer
// This matches the payouts returned by the API of a P2Pool observer (excluding most fields).
#[derive(Deserialize)]
struct ObserverPayout {
    main_height: u64,
    timestamp: i64,
    coinbase_reward: u64,
}

// Lists the final payouts of [address] known by [observer].
async fn observer_payouts(
    client: &ClientWithMiddleware,
    observer: &str,
    address: &str,
) -> anyhow::Result<Payouts> {
    let observer = observer.trim().trim_end_matches('/');
    let url = if observer.contains("://") {
        format!("{observer}/api/payouts/{address}?limit=0")
    } else {
        format!("https://{observer}/api/payouts/{address}?limit=0")
    };
    // Blocks found since then may still be orphaned.
    let final_before = chrono::Utc::now().timestamp()
        - (P2POOL_PAYOUT_CHECK_DEPTH * MONERO_BLOCK_TIME_IN_SECONDS) as i64;
    Ok(client
        .get(url)
        .header(USER_AGENT, APP_USER_AGENT)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<ObserverPayout>>()
        .await?
        .into_iter()
        .filter(|payout| payout.timestamp <= final_before)
        .map(|payout| {
            (
                utc_date(payout.timestamp),
                AtomicUnit::from_u64(payout.coinbase_reward),
                payout.main_height,
            )
        })
        .collect())
}

//---------------------------------------------------------------------------------------------------- Orphaned blocks
// This matches the blocks found by the pool in the P2Pool data API (excluding most fields).
#[derive(Deserialize)]
struct FoundBlock {
    height: u64,
    hash: String,
}

// This matches the [get_block_header_by_height] result (excluding most fields).
#[derive(Deserialize)]
struct BlockHeaderResult {
    block_header: BlockHeader,
    status: String,
}

#[derive(Deserialize)]
struct BlockHeader {
    depth: u64,
    hash: String,
}

// Removes the payouts of the blocks found by the pool that were orphaned.
// Returns the number of payouts removed.
async fn remove_orphaned_payouts(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    client: &ClientWithMiddleware,
    node: &str,
    found_blocks: &Path,
) -> anyhow::Result<u64> {
    let found_blocks: Vec<FoundBlock> = match std::fs::read_to_string(found_blocks) {
        Ok(found_blocks) => serde_json::from_str(&found_blocks)?,
        // P2Pool writes it once the pool found a block.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let paid: HashSet<u64> = gupax_p2pool_api
        .lock()
        .unwrap()
        .log
        .lines()
        .filter_map(GupaxP2poolApi::payout_height)
        .collect();
    let mut orphaned = Vec::new();
    let mut kept = HashSet::new();
    for block in found_blocks {
        if !paid.contains(&block.height) {
            continue;
        }
        let params = json!({"height": block.height});
        let result =
            rpc::<BlockHeaderResult>(client, node, "get_block_header_by_height", params).await?;
        ensure_ok(&result.status)?;
        if result.block_header.hash == block.hash {
            kept.insert(block.height);
        } else if result.block_header.depth >= P2POOL_PAYOUT_CHECK_DEPTH {
            info!("Payout sync | Block {} was orphaned", block.height);
            orphaned.push(block.height);
        }
    }
    // Another block of the pool may have replaced the orphaned one.
    orphaned.retain(|height| !kept.contains(height));
    gupax_p2pool_api
        .lock()
        .unwrap()
        .remove_payouts(&orphaned)
        .map_err(|e| anyhow::anyhow!("payout history write error: {e}"))
}

//---------------------------------------------------------------------------------------------------- Monero node
// This matches a JSON-RPC response of the Monero node.
#[derive(Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    message: String,
}

// This matches the [get_info] result (excluding most fields).
#[derive(Deserialize)]
struct Info {
    synchronized: bool,
    status: String,
}

// Returns [node] if synced to the network.
async fn synced_node<'a>(
    client: &ClientWithMiddleware,
    node: Option<&'a str>,
) -> anyhow::Result<&'a str> {
    let node = node.ok_or_else(|| anyhow::anyhow!("P2Pool is not connected to a node"))?;
    let info = rpc::<Info>(client, node, "get_info", json!({})).await?;
    ensure_ok(&info.status)?;
    anyhow::ensure!(info.synchronized, "the node is not synced");
    Ok(node)
}

async fn rpc<T: DeserializeOwned>(
    client: &ClientWithMiddleware,
    node: &str,
    method: &str,
    params: serde_json::Value,
) -> anyhow::Result<T> {
    let body = json!({"jsonrpc": "2.0", "id": "0", "method": method, "params": params});
    let response = client
        .post(format!("{node}/json_rpc"))
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await?
        .error_for_status()?
        .json::<RpcResponse<T>>()
        .await?;
    if let Some(error) = response.error {
        anyhow::bail!("node error: {}", error.message);
    }
    response
        .result
        .ok_or_else(|| anyhow::anyhow!("node error: no result"))
}

fn ensure_ok(status: &str) -> anyhow::Result<()> {
    anyhow::ensure!(status == "OK", "node status: {status}");
    Ok(())
}

//---------------------------------------------------------------------------------------------------- Common
// Formats a timestamp like the dates of the P2Pool output.
fn utc_date(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0).map_or_else(
        || "????-??-?? ??:??:??.????".to_string(),
        |date| date.format("%Y-%m-%d %H:%M:%S.0000").to_string(),
    )
}

//---------------------------------------------------------------------------------------------------- Tests
#[cfg(test)]
mod tests {
    use super::*;

    fn node(ip: &str, rpc: &str) -> Node {
        Node {
            ip: ip.to_string(),
            rpc: rpc.to_string(),
            zmq: String::new(),
        }
    }

    #[test]
    fn url_of_the_node() {
        let api = GupaxP2poolApi::new();
        let url = |ip| SyncSources::new(&api, Some(&node(ip, "18081")), String::new()).node;
        assert_eq!(url("127.0.0.1").as_deref(), Some("http://127.0.0.1:18081"));
        assert_eq!(url("::1").as_deref(), Some("http://[::1]:18081"));
    }

    #[test]
    fn address_paid_by_p2pool() {
        assert_eq!(payout_address("4A", ""), "4A");
        assert_eq!(payout_address("4A", "8B"), "8B");
    }
}
