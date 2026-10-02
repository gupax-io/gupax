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
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{StreamExt, TryStreamExt};
use log::*;
use monero::{
    Address, AddressType, PrivateKey, PublicKey, Transaction, ViewPair, consensus::deserialize,
    cryptonote::onetime_key::KeyGenerator,
};
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

// Blocks scanned at least by the first scan of an address, about 30 days.
const FIRST_SCAN_BLOCKS: u64 = 21_600;
// Maximum headers and transactions returned by a restricted node per request.
const NODE_MAX_HEADERS: u64 = 1000;
const NODE_MAX_TRANSACTIONS: usize = 100;
// Maximum connections per IP to a public node.
const NODE_MAX_CONNECTIONS: usize = 3;

// The missing payouts are found with the view key if filled, else with the observer if set.
pub struct SyncSources {
    pub view_key: String,
    pub observer: String,
    // Address P2Pool mines to.
    pub address: String,
    // URL of the RPC of the node P2Pool uses.
    pub node: Option<String>,
    // File of the P2Pool data API listing the last blocks found by the pool.
    pub found_blocks: PathBuf,
}

impl SyncSources {
    pub fn new(
        api: &GupaxP2poolApi,
        node: Option<&Node>,
        view_key: String,
        observer: String,
    ) -> Self {
        Self {
            view_key,
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

// Address P2Pool pays: [subaddress] if valid, else [wallet].
pub fn payout_address(wallet: &str, subaddress: &str) -> String {
    if Address::from_str(subaddress).is_ok_and(|a| a.addr_type == AddressType::SubAddress) {
        subaddress.to_string()
    } else {
        wallet.to_string()
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
    if sources.view_key.is_empty() && !sources.observer.is_empty() {
        let payouts = observer_payouts(&client, &sources.observer, &sources.address).await;
        results.push(match payouts {
            Ok(payouts) => merge(gupax_p2pool_api, &payouts),
            Err(e) => format!("Sync error: {e:#}"),
        });
    }
    let (node, height) = match synced_node(&client, sources.node.as_deref()).await {
        Ok(node) => node,
        Err(e) => {
            results.push(format!("Node error: {e:#}"));
            return;
        }
    };
    if !sources.view_key.is_empty() {
        let scanned = scan(
            gupax_p2pool_api,
            &client,
            node,
            height,
            &sources.address,
            &sources.view_key,
            results,
        )
        .await;
        if let Err(e) = scanned {
            results.push(format!("Sync error: {e:#}"));
        }
    }
    let found_blocks = &sources.found_blocks;
    results.push(
        match remove_orphaned_payouts(gupax_p2pool_api, &client, node, found_blocks).await {
            Ok(removed) => format!("Orphaned payouts removed: {removed}"),
            Err(e) => format!("Orphan check error: {e:#}"),
        },
    );
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

// This matches a block header of the Monero node (excluding most fields).
#[derive(Deserialize)]
struct BlockHeader {
    depth: u64,
    hash: String,
    miner_tx_hash: String,
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

//---------------------------------------------------------------------------------------------------- Private view key
// These match the responses of the Monero node (excluding most fields).
#[derive(Deserialize)]
struct BlockHeaders {
    headers: Vec<BlockHeader>,
    status: String,
}

#[derive(Deserialize)]
struct Transactions {
    #[serde(default)]
    txs: Vec<Tx>,
    status: String,
}

#[derive(Deserialize)]
struct Tx {
    pruned_as_hex: String,
    block_height: u64,
    block_timestamp: i64,
}

// Scans the final blocks since the last scan of [address], else since the oldest payout or
// 30 days ago, whichever is older, and adds the payouts found after each range of blocks.
async fn scan(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    client: &ClientWithMiddleware,
    node: &str,
    height: u64,
    address: &str,
    view_key: &str,
    results: &mut Vec<String>,
) -> anyhow::Result<()> {
    let pair = view_pair(address, view_key)?;
    let tip = height.saturating_sub(1);
    let start = {
        let api = gupax_p2pool_api.lock().unwrap();
        api.read_scan(address).unwrap_or_else(|| {
            let first = tip.saturating_sub(FIRST_SCAN_BLOCKS);
            api.log
                .lines()
                .filter_map(GupaxP2poolApi::payout_height)
                .fold(first, u64::min)
        })
    };
    let end = tip.saturating_sub(P2POOL_PAYOUT_CHECK_DEPTH);
    let mut added = 0;
    results.push(format!("Missing payouts added: {added}"));
    for first in (start..=end).step_by(NODE_MAX_HEADERS as usize) {
        let last = (first + NODE_MAX_HEADERS - 1).min(end);
        gupax_p2pool_api.lock().unwrap().sync = format!("Scanning block {first} of {end}");
        let payouts = scan_blocks(client, node, &pair, first, last).await?;
        let mut api = gupax_p2pool_api.lock().unwrap();
        added += api
            .merge_payouts(&payouts)
            .and_then(|added| api.write_scan(last + 1, address).map(|()| added))
            .map_err(|e| anyhow::anyhow!("payout history write error: {e}"))?;
        if let Some(result) = results.last_mut() {
            *result = format!("Missing payouts added: {added}");
        }
    }
    Ok(())
}

// Keys to find the outputs paid to [address].
fn view_pair(address: &str, view_key: &str) -> anyhow::Result<ViewPair> {
    let address = Address::from_str(address)?;
    let view = PrivateKey::from_str(view_key.trim())?;
    // The public view key of a subaddress derives from its public spend key.
    let public_view = match address.addr_type {
        AddressType::SubAddress => view * &address.public_spend,
        _ => PublicKey::from_private_key(&view),
    };
    anyhow::ensure!(
        public_view == address.public_view,
        "the private view key does not belong to the P2Pool address"
    );
    Ok(ViewPair {
        view,
        spend: address.public_spend,
    })
}

// Finds the payouts to [pair] in the coinbase transactions of the blocks from [start] to [end].
async fn scan_blocks(
    client: &ClientWithMiddleware,
    node: &str,
    pair: &ViewPair,
    start: u64,
    end: u64,
) -> anyhow::Result<Payouts> {
    let params = json!({"start_height": start, "end_height": end});
    let headers = rpc::<BlockHeaders>(client, node, "get_block_headers_range", params).await?;
    ensure_ok(&headers.status)?;
    let requests = futures::stream::iter(headers.headers.chunks(NODE_MAX_TRANSACTIONS))
        .map(|headers| coinbases(client, node, headers))
        .buffered(NODE_MAX_CONNECTIONS);
    let mut payouts = Payouts::new();
    for tx in requests
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
    {
        let transaction: Transaction = deserialize(&hex::decode(&tx.pruned_as_hex)?)?;
        let amount = coinbase_payout(&transaction, pair);
        if amount > 0 {
            payouts.push((
                utc_date(tx.block_timestamp),
                AtomicUnit::from_u64(amount),
                tx.block_height,
            ));
        }
    }
    Ok(payouts)
}

// Coinbase transactions of the blocks of [headers].
async fn coinbases(
    client: &ClientWithMiddleware,
    node: &str,
    headers: &[BlockHeader],
) -> anyhow::Result<Vec<Tx>> {
    let hashes: Vec<&str> = headers.iter().map(|h| h.miner_tx_hash.as_str()).collect();
    let txs = client
        .post(format!("{node}/get_transactions"))
        .header(CONTENT_TYPE, "application/json")
        .body(json!({"txs_hashes": hashes, "prune": true}).to_string())
        .send()
        .await?
        .error_for_status()?
        .json::<Transactions>()
        .await?;
    ensure_ok(&txs.status)?;
    anyhow::ensure!(
        txs.txs.len() == hashes.len(),
        "missing coinbase transactions"
    );
    Ok(txs.txs)
}

// Amount paid to [pair] by the coinbase transaction [tx].
fn coinbase_payout(tx: &Transaction, pair: &ViewPair) -> u64 {
    let Some(tx_pubkey) = tx.prefix.extra.try_parse().tx_pubkey() else {
        return 0;
    };
    // The shared key is derived once, the view tags rule out most outputs.
    let keygen = KeyGenerator::from_key(pair, tx_pubkey);
    tx.prefix
        .outputs
        .iter()
        .enumerate()
        .filter(|(index, output)| {
            output.target.check_view_tag(keygen.rv, *index)
                && output
                    .target
                    .as_one_time_key()
                    .is_some_and(|key| keygen.check(*index, key))
        })
        .map(|(_, output)| output.amount.0)
        .sum()
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
    height: u64,
    synchronized: bool,
    status: String,
}

// Returns [node] if synced to the network, and the height of its chain.
async fn synced_node<'a>(
    client: &ClientWithMiddleware,
    node: Option<&'a str>,
) -> anyhow::Result<(&'a str, u64)> {
    let node = node.ok_or_else(|| anyhow::anyhow!("P2Pool is not connected to a node"))?;
    let info = rpc::<Info>(client, node, "get_info", json!({})).await?;
    ensure_ok(&info.status)?;
    anyhow::ensure!(info.synchronized, "the node is not synced");
    Ok((node, info.height))
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
    // Responses recorded from node2.monerodevs.org:18089 and mini.p2pool.observer
    // on 2026-10-01 and 2026-10-02.
    use super::*;
    use crate::constants::P2POOL_API_PATH_BLOCKS;
    use mockito::Matcher;
    use std::time::Instant;

    // Monero General Fund, which publishes its private view key.
    const ADDRESS: &str = "44AFFq5kSiGBoZ4NMDwYtN18obc8AemS33DBLWs3H7otXft3XjrpDtQGv7SqSsaBYBb98uNbr2VBBEt7f2wfn3RVGQBEP3A";
    const VIEW_KEY: &str = "f359631075708155cc3d92a32b75a7d02a5dcf27756707b47a2b31b21c389501";
    const HASH_3654400: &str = "ccdc3dada00723a548338fb86588999eb03a9ab0d6fb9bb74de1e9ec594e6808";
    const HASH_3654401: &str = "5313b6628b27c6d48420393a86b941b4416f0cfa8b83af47b53edf46d6a11ed0";
    // Block of the pool missing from the main chain.
    const HASH_ORPHANED: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn payout_3654401() -> (String, AtomicUnit, u64) {
        (
            "2026-04-17 11:01:06.0000".to_string(),
            AtomicUnit::from_u64(274561854),
            3654401,
        )
    }

    fn mock_observer(
        server: &mut mockito::ServerGuard,
        address: &str,
        status: usize,
        body: &str,
    ) -> mockito::Mock {
        server
            .mock("GET", format!("/api/payouts/{address}").as_str())
            .match_query(Matcher::UrlEncoded("limit".to_string(), "0".to_string()))
            .match_header("user-agent", APP_USER_AGENT)
            .with_status(status)
            .with_body(body)
            .create()
    }

    fn mock_rpc(
        server: &mut mockito::ServerGuard,
        method: &str,
        params: serde_json::Value,
        body: &str,
    ) -> mockito::Mock {
        server
            .mock("POST", "/json_rpc")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(
                json!({"method": method, "params": params}),
            ))
            .with_body(body)
            .create()
    }

    // Synced node whose main chain holds the blocks 3,654,400 and 3,654,401.
    fn mock_node(server: &mut mockito::ServerGuard) {
        let info = include_str!("../../tests/fixtures/payouts/node_info.json");
        mock_rpc(server, "get_info", json!({}), info);
        for (height, body) in [
            (
                3654400,
                include_str!("../../tests/fixtures/payouts/node_block_header_3654400.json"),
            ),
            (
                3654401,
                include_str!("../../tests/fixtures/payouts/node_block_header_3654401.json"),
            ),
        ] {
            mock_rpc(
                server,
                "get_block_header_by_height",
                json!({"height": height}),
                body,
            );
        }
    }

    // Writes the blocks found by the pool in the P2Pool data API directory [dir].
    fn write_found_blocks(dir: &Path, blocks: serde_json::Value) -> PathBuf {
        let found_blocks = dir.join(P2POOL_API_PATH_BLOCKS);
        std::fs::create_dir_all(found_blocks.parent().unwrap()).unwrap();
        std::fs::write(&found_blocks, blocks.to_string()).unwrap();
        found_blocks
    }

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
        let url = |ip| {
            SyncSources::new(&api, Some(&node(ip, "18081")), String::new(), String::new()).node
        };
        assert_eq!(url("127.0.0.1").as_deref(), Some("http://127.0.0.1:18081"));
        assert_eq!(url("::1").as_deref(), Some("http://[::1]:18081"));
    }

    // A subaddress of the General Fund.
    fn subaddress() -> Address {
        let address = Address::from_str(ADDRESS).unwrap();
        let pair = view_pair(ADDRESS, VIEW_KEY).unwrap();
        let index = monero::cryptonote::subaddress::Index { major: 0, minor: 1 };
        let spend = monero::cryptonote::subaddress::get_spend_public_key(&pair, index);
        Address::subaddress(address.network, spend, pair.view * &spend)
    }

    #[test]
    fn address_paid_by_p2pool() {
        assert_eq!(payout_address("4A", ""), "4A");
        let subaddress = subaddress().to_string();
        assert_eq!(payout_address("4A", &subaddress), subaddress);
        // P2Pool mines to the wallet when the subaddress is invalid.
        assert_eq!(payout_address("4A", "8B"), "4A");
    }

    #[test]
    fn view_key_of_the_address() {
        let address = Address::from_str(ADDRESS).unwrap();
        let pair = view_pair(ADDRESS, VIEW_KEY).unwrap();
        assert_eq!(pair.spend, address.public_spend);
        let subaddress = subaddress();
        let pair = view_pair(&subaddress.to_string(), VIEW_KEY).unwrap();
        assert_eq!(pair.spend, subaddress.public_spend);
        let error = view_pair(ADDRESS, &format!("01{}", "00".repeat(31))).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the private view key does not belong to the P2Pool address"
        );
    }

    #[test]
    fn removes_the_payouts_of_orphaned_blocks() {
        let mut server = mockito::Server::new();
        mock_node(&mut server);
        let mut api = GupaxP2poolApi::temporary("removes_the_payouts_of_orphaned_blocks");
        let dir = api.path_log.parent().unwrap().to_path_buf();
        let found_blocks = write_found_blocks(
            &dir,
            json!([
                {"height": 3654401, "hash": HASH_3654401},
                {"height": 3654400, "hash": HASH_ORPHANED},
            ]),
        );
        let payouts = [
            (
                "2026-04-17 10:59:38.0000".to_string(),
                AtomicUnit::from_u64(1),
                3654400,
            ),
            payout_3654401(),
            // Found while P2Pool was stopped.
            (
                "2026-04-17 13:00:00.0000".to_string(),
                AtomicUnit::from_u64(4),
                3654500,
            ),
        ];
        api.merge_payouts(&payouts).unwrap();
        let api = Arc::new(Mutex::new(api));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client();
        let node = server.url();

        let removed = runtime
            .block_on(remove_orphaned_payouts(&api, &client, &node, &found_blocks))
            .unwrap();
        assert_eq!(removed, 1);
        // P2Pool did not find a block yet.
        let missing = dir.join("missing");
        let removed = runtime
            .block_on(remove_orphaned_payouts(&api, &client, &node, &missing))
            .unwrap();
        assert_eq!(removed, 0);

        let api = api.lock().unwrap();
        assert_eq!(api.payout_u64, 2);
        assert_eq!(api.xmr.to_u64(), 274561858);
        assert!(!api.has_payout(3654400));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_a_payout_paid_again_at_the_same_height() {
        let mut server = mockito::Server::new();
        mock_node(&mut server);
        let mut api = GupaxP2poolApi::temporary("keeps_a_payout_paid_again_at_the_same_height");
        let dir = api.path_log.parent().unwrap().to_path_buf();
        let found_blocks = write_found_blocks(
            &dir,
            json!([
                {"height": 3654400, "hash": HASH_ORPHANED},
                {"height": 3654400, "hash": HASH_3654400},
            ]),
        );
        let payout = (
            "2026-04-17 10:59:38.0000".to_string(),
            AtomicUnit::from_u64(1),
            3654400,
        );
        api.merge_payouts(&[payout]).unwrap();
        let api = Arc::new(Mutex::new(api));

        let removed = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(remove_orphaned_payouts(
                &api,
                &client(),
                &server.url(),
                &found_blocks,
            ))
            .unwrap();

        assert_eq!(removed, 0);
        assert!(api.lock().unwrap().has_payout(3654400));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn observer_payouts_of_an_address() {
        let mut server = mockito::Server::new();
        let general_fund =
            include_str!("../../tests/fixtures/payouts/observer_payouts_general_fund.json");
        let unknown = include_str!("../../tests/fixtures/payouts/observer_payouts_unknown.json");
        mock_observer(&mut server, ADDRESS, 200, general_fund);
        mock_observer(&mut server, "4AAAA", 404, unknown);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client();

        // A pasted URL may end with a slash and a line break.
        let observer = format!("{}/ ", server.url());
        let payouts = runtime
            .block_on(observer_payouts(&client, &observer, ADDRESS))
            .unwrap();
        assert_eq!(payouts.len(), 21);
        assert_eq!(payouts[0], payout_3654401());
        let total: u64 = payouts.iter().map(|payout| payout.1.to_u64()).sum();
        assert_eq!(total, 12828731759);

        let error = runtime
            .block_on(observer_payouts(&client, &server.url(), "4AAAA"))
            .unwrap_err();
        assert!(format!("{error:#}").contains("404 Not Found"));
    }

    #[test]
    fn view_key_scan_resumes_after_the_last_block() {
        let mut server = mockito::Server::new();
        let headers = mock_rpc(
            &mut server,
            "get_block_headers_range",
            json!({"start_height": 3654400, "end_height": 3654402}),
            include_str!("../../tests/fixtures/payouts/node_block_headers_3654400_3654402.json"),
        )
        .expect(1);
        server
            .mock("POST", "/get_transactions")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "txs_hashes": [
                    "a8c5318ba8a1b8531ff24bb4b79e2d4b7904269f04419543c6db93b773f6da54",
                    "9a9adc7a5d78b3a0e96d61fcd035d0fe82ba7df85532000e3b9536cbf6134750",
                    "6f79bacb782f43cdbe011074e5179c3aacf4ceaee5f7abb7e4d0c5fd6ebefc1d"
                ],
                "prune": true
            })))
            .with_body(include_str!(
                "../../tests/fixtures/payouts/node_coinbases_3654400_3654402.json"
            ))
            .create();
        let api = GupaxP2poolApi::temporary("view_key_scan_resumes_after_the_last_block");
        api.write_scan(3700000, "4AAAA").unwrap();
        api.write_scan(3654400, ADDRESS).unwrap();
        let api = Arc::new(Mutex::new(api));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client();
        let node = server.url();
        // A node at height 3,654,463, so the block 3,654,402 is final.
        let height = 3654463;

        let mut results = Vec::new();
        let scanned = scan(
            &api,
            &client,
            &node,
            height,
            ADDRESS,
            VIEW_KEY,
            &mut results,
        );
        runtime.block_on(scanned).unwrap();
        assert_eq!(results, ["Missing payouts added: 1"]);
        {
            let api = api.lock().unwrap();
            assert_eq!(
                api.log,
                "2026-04-17 11:01:06.0000 | 0.000274561854 XMR | Block 3,654,401\n"
            );
            assert_eq!(api.read_scan(ADDRESS), Some(3654403));
            assert_eq!(api.read_scan("4AAAA"), Some(3700000));
        }

        // The blocks are scanned once.
        let mut results = Vec::new();
        let scanned = scan(
            &api,
            &client,
            &node,
            height,
            ADDRESS,
            VIEW_KEY,
            &mut results,
        );
        runtime.block_on(scanned).unwrap();
        assert_eq!(results, ["Missing payouts added: 0"]);
        headers.assert();
        std::fs::remove_dir_all(api.lock().unwrap().path_log.parent().unwrap()).unwrap();
    }

    #[test]
    fn sync_sources_priority() {
        let mut server = mockito::Server::new();
        let general_fund =
            include_str!("../../tests/fixtures/payouts/observer_payouts_general_fund.json");
        let observer = mock_observer(&mut server, ADDRESS, 200, general_fund).expect(1);
        mock_node(&mut server);
        let mut api = GupaxP2poolApi::temporary("sync_sources_priority");
        let dir = api.path_log.parent().unwrap().to_path_buf();
        api.address = ADDRESS.to_string();
        api.found_blocks =
            write_found_blocks(&dir, json!([{"height": 3654400, "hash": HASH_ORPHANED}]));
        let orphaned = (
            "2026-04-17 10:59:38.0000".to_string(),
            AtomicUnit::from_u64(1),
            3654400,
        );
        api.merge_payouts(&[orphaned]).unwrap();
        let api = Arc::new(Mutex::new(api));
        let host = server.host_with_port();
        let (ip, rpc) = host.split_once(':').unwrap();
        let node = node(ip, rpc);
        let sources = |view_key: &str, observer: &str| {
            let api = api.lock().unwrap();
            SyncSources::new(
                &api,
                Some(&node),
                view_key.to_string(),
                observer.to_string(),
            )
        };

        // The observer adds the missing payouts, then the orphaned one is removed.
        sync(&api, &sources("", &server.url()));
        assert_eq!(
            api.lock().unwrap().sync,
            "Missing payouts added: 21. Orphaned payouts removed: 1"
        );
        assert_eq!(api.lock().unwrap().payout_u64, 21);

        // A filled view key is used before the observer.
        let view_key = format!("01{}", "00".repeat(31));
        sync(&api, &sources(&view_key, &server.url()));
        assert_eq!(
            api.lock().unwrap().sync,
            "Sync error: the private view key does not belong to the P2Pool address. Orphaned payouts removed: 0"
        );
        observer.assert();

        // Without view key and observer, only the orphaned payouts are checked.
        sync(&api, &sources("", ""));
        assert_eq!(api.lock().unwrap().sync, "Orphaned payouts removed: 0");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stop_a_sync_waiting_for_the_node() {
        // This node accepts connections and never answers.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut api = GupaxP2poolApi::temporary("stop_a_sync_waiting_for_the_node");
        api.address = ADDRESS.to_string();
        let node = node("127.0.0.1", &port.to_string());
        let sources = SyncSources::new(&api, Some(&node), VIEW_KEY.to_string(), String::new());
        let api = Arc::new(Mutex::new(api));

        assert!(start(&mut api.lock().unwrap(), &api, sources));
        // Let the request reach the node.
        std::thread::sleep(Duration::from_millis(200));
        api.lock().unwrap().stop_sync = true;
        let deadline = Instant::now() + Duration::from_secs(10);
        while api.lock().unwrap().syncing {
            assert!(Instant::now() < deadline, "the sync did not stop");
            std::thread::sleep(Duration::from_millis(10));
        }

        let api = api.lock().unwrap();
        assert_eq!(api.sync, "Stopped");
        std::fs::remove_dir_all(api.path_log.parent().unwrap()).unwrap();
    }
}
