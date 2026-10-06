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
    io::Read,
    net::IpAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{StreamExt, TryStreamExt};
use log::*;
use monero::{
    Address, AddressType, PrivateKey, PublicKey, Transaction, ViewPair,
    blockdata::transaction::SubField, consensus::deserialize,
    cryptonote::onetime_key::KeyGenerator,
};
use reqwest::{
    StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT, WWW_AUTHENTICATE},
};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;

use super::MONERO_BLOCK_TIME_IN_SECONDS;
use crate::{
    components::update::APP_USER_AGENT,
    constants::{P2POOL_PAYOUT_CHECK_DEPTH, SECOND},
    disk::{gupax_p2pool_api::GupaxP2poolApi, node::Node},
    helper::p2pool::P2poolNodes,
    miscs::client_with,
    xmr::{AtomicUnit, PayoutOrd},
};

// Time without data after which a request fails, so a stalled sync ends and the automatic syncs
// go on. The largest requests of a sync started answering within 0.2 s from a remote node.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

// (date, atomic_unit, height) of each payout found.
type Payouts = Vec<(String, AtomicUnit, u64)>;

// Blocks scanned by the first scan of an address, about 30 days.
const FIRST_SCAN_BLOCKS: u64 = 21_600;
// Maximum headers and transactions returned by a restricted node per request.
const NODE_MAX_HEADERS: u64 = 1000;
const NODE_MAX_TRANSACTIONS: usize = 100;
// Connections to the node, leaving P2Pool one of the 3 a public node accepts per IP.
const NODE_MAX_CONNECTIONS: usize = 2;
// Requests to a node with a login until one is accepted.
const NODE_LOGIN_ATTEMPTS: usize = 3;
// Interval of the automatic syncs, the time for a block to become final.
pub const SYNC_INTERVAL: Duration =
    Duration::from_secs(P2POOL_PAYOUT_CHECK_DEPTH * MONERO_BLOCK_TIME_IN_SECONDS);
// Interval of the automatic syncs after a failed one.
pub const RETRY_INTERVAL: Duration = Duration::from_secs(10 * 60);
// Bytes read from a private view key file, which holds a few lines.
const VIEW_KEY_FILE_MAX: u64 = 64 * 1024;

// The missing payouts are found with the view key if filled, else with the observer if set.
struct SyncSources {
    view_key: String,
    observer: String,
    // Address P2Pool mines to.
    address: String,
    // RPC of the node P2Pool uses.
    node: NodeRpc,
    // File of the P2Pool data API listing the last blocks found by the pool.
    found_blocks: PathBuf,
}

impl SyncSources {
    fn new(api: &GupaxP2poolApi, node: &Node) -> Self {
        Self {
            view_key: api.view_key.clone(),
            observer: api.observer.clone(),
            address: api.address.clone(),
            node: NodeRpc::new(node, &api.nodes),
            found_blocks: api.found_blocks.clone(),
        }
    }
}

// RPC of a node, reached like P2Pool does.
struct NodeRpc {
    url: String,
    // user:password
    login: String,
    // SOCKS5 proxy, empty for a direct connection.
    proxy: String,
    // Login challenge of the node, valid on the connection that got it.
    challenge: Mutex<Option<http_auth::PasswordClient>>,
}

impl NodeRpc {
    // RPC of [node], with the options P2Pool has for it in [nodes].
    fn new(node: &Node, nodes: &P2poolNodes) -> Self {
        let host = nodes
            .hosts
            .iter()
            .find(|host| host.node.ip == node.ip && host.node.rpc == node.rpc);
        let scheme = if host.is_some_and(|host| host.rpc_ssl) {
            "https"
        } else {
            "http"
        };
        let ip = if node.ip.contains(':') {
            format!("[{}]", node.ip)
        } else {
            node.ip.clone()
        };
        // P2Pool connects to the private addresses directly.
        let private = node.ip.parse().is_ok_and(|ip| match ip {
            IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
            IpAddr::V6(ip) => {
                ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
            }
        });
        Self {
            url: format!("{scheme}://{ip}:{}", node.rpc),
            login: host.map(|host| host.rpc_login.clone()).unwrap_or_default(),
            proxy: if private {
                String::new()
            } else {
                nodes.socks5.clone()
            },
            challenge: Mutex::new(None),
        }
    }

    // Client of the RPC, accepting any certificate since a node serves a self-signed one.
    fn client(&self) -> anyhow::Result<ClientWithMiddleware> {
        let mut client = reqwest::Client::builder()
            .read_timeout(READ_TIMEOUT)
            .tls_danger_accept_invalid_certs(true);
        if !self.proxy.is_empty() {
            client = client.proxy(reqwest::Proxy::all(format!("socks5h://{}", self.proxy))?);
        }
        Ok(client_with(client.build()?))
    }

    // Concurrent requests to the node: a node with a login gets one connection, for its challenge.
    fn connections(&self) -> usize {
        if self.login.is_empty() {
            NODE_MAX_CONNECTIONS
        } else {
            1
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

// Starts a sync with [node] in a new thread, unless one is running.
// Returns whether it started.
pub fn start(
    api: &mut GupaxP2poolApi,
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    node: &Node,
) -> bool {
    if api.syncing {
        return false;
    }
    let sources = SyncSources::new(api, node);
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
    let failed = tokio::select! {
        failed = run(gupax_p2pool_api, sources, &mut results) => failed,
        () = stopped(gupax_p2pool_api) => {
            results.push("Stopped".to_string());
            false
        }
    };
    let mut api = gupax_p2pool_api.lock().unwrap();
    api.sync = results.join(". ");
    api.sync_failed = failed;
    info!("Payout sync | {}", api.sync);
    api.syncing = false;
}

// Completes once the user stops the sync.
async fn stopped(gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>) {
    while !gupax_p2pool_api.lock().unwrap().stop_sync {
        tokio::time::sleep(SECOND).await;
    }
}

// Syncs the payouts with [sources], describes the result in [results] and returns whether it
// failed.
async fn run(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    sources: &SyncSources,
    results: &mut Vec<String>,
) -> bool {
    let mut failed = false;
    let node = &sources.node;
    let (node_client, height) = match synced_node(node).await {
        Ok(synced) => synced,
        Err(e) => {
            results.push(format!("Node error: {e:#}"));
            return true;
        }
    };
    let found_blocks = read_found_blocks(&sources.found_blocks);
    // Final payouts of the address listed by the observer.
    let mut listed = Payouts::new();
    if sources.view_key.is_empty() && !sources.observer.is_empty() {
        let added = async {
            let (position, path_scan) = {
                let api = gupax_p2pool_api.lock().unwrap();
                (api.read_scan(&sources.address), api.path_scan.clone())
            };
            // The payouts before [position] are in the history, the orphan check needs the ones
            // of the blocks found by the pool.
            let from_height = position.map(|position| {
                let found = found_blocks.iter().flatten().map(|block| block.height);
                found.fold(position, u64::min)
            });
            let last = last_final_block(height);
            let observer = &sources.observer;
            let address = &sources.address;
            let payouts =
                observer_payouts(&client()?, observer, address, from_height, last).await?;
            let write_error = |e| anyhow::anyhow!("payout history write error: {e}");
            let added = gupax_p2pool_api
                .lock()
                .unwrap()
                .merge_payouts(&payouts)
                .map_err(write_error)?;
            if position.is_none_or(|position| last + 1 > position) {
                GupaxP2poolApi::write_scan(&path_scan, last + 1, address).map_err(write_error)?;
            }
            listed = payouts;
            anyhow::Ok(added)
        }
        .await;
        match added {
            Ok(added) => results.push(format!("Missing payouts added: {added}")),
            Err(e) => {
                results.push(format!("Sync error: {e:#}"));
                failed = true;
            }
        }
    }
    let pair = if sources.view_key.is_empty() {
        None
    } else {
        view_pair(&sources.address, &sources.view_key)
            .inspect_err(|e| {
                results.push(format!("Sync error: {e:#}"));
                failed = true;
            })
            .ok()
    };
    if let Some(pair) = &pair {
        let scanned = scan(
            gupax_p2pool_api,
            &node_client,
            node,
            height,
            &sources.address,
            pair,
            results,
        )
        .await;
        if let Err(e) = scanned {
            results.push(format!("Sync error: {e:#}"));
            failed = true;
        }
    }
    let checked = async {
        let found_blocks = found_blocks?;
        let pair = pair.as_ref();
        remove_orphaned_payouts(
            gupax_p2pool_api,
            &node_client,
            node,
            &found_blocks,
            pair,
            &listed,
            results,
        )
        .await
    }
    .await;
    if let Err(e) = checked {
        results.push(format!("Orphan check error: {e:#}"));
        failed = true;
    }
    failed
}

// Client failing on a request that stalls.
fn client() -> anyhow::Result<ClientWithMiddleware> {
    let client = reqwest::Client::builder()
        .read_timeout(READ_TIMEOUT)
        .build()?;
    Ok(client_with(client))
}

//---------------------------------------------------------------------------------------------------- Observer
// This matches the payouts returned by the API of a P2Pool observer (excluding most fields).
#[derive(Deserialize)]
struct ObserverPayout {
    main_height: u64,
    timestamp: i64,
    coinbase_reward: u64,
}

// This matches an error returned by the API of a P2Pool observer.
#[derive(Deserialize)]
struct ObserverError {
    error: String,
}

// Lists the payouts of [address] known by [observer], from the block [from_height] if set
// to the block [last].
async fn observer_payouts(
    client: &ClientWithMiddleware,
    observer: &str,
    address: &str,
    from_height: Option<u64>,
    last: u64,
) -> anyhow::Result<Payouts> {
    let observer = observer.trim().trim_end_matches('/');
    let scheme = if observer.contains("://") {
        ""
    } else {
        "https://"
    };
    let from = from_height.map_or_else(String::new, |height| format!("&from_height={height}"));
    let url = format!("{scheme}{observer}/api/payouts/{address}?limit=0{from}");
    let response = client
        .get(url)
        .header(USER_AGENT, APP_USER_AGENT)
        .send()
        .await?;
    if let Some(e) = response.error_for_status_ref().err() {
        // The observer knows no share of the address.
        let error = response.json::<ObserverError>().await;
        if error.is_ok_and(|error| error.error == "not_found") {
            return Ok(Payouts::new());
        }
        return Err(e.into());
    }
    Ok(response
        .json::<Vec<ObserverPayout>>()
        .await?
        .into_iter()
        .filter(|payout| payout.main_height <= last)
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

// Removes the payouts of the blocks found by the pool that were orphaned, except the payouts of
// the main chain blocks [listed] or found with [pair], and describes the result.
// Fails with the error of the first block left unchecked.
async fn remove_orphaned_payouts(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    client: &ClientWithMiddleware,
    node: &NodeRpc,
    found_blocks: &[FoundBlock],
    pair: Option<&ViewPair>,
    listed: &Payouts,
    results: &mut Vec<String>,
) -> anyhow::Result<()> {
    let paid: HashSet<u64> = gupax_p2pool_api
        .lock()
        .unwrap()
        .log
        .lines()
        .filter_map(PayoutOrd::payout_height)
        .collect();
    let found_blocks: Vec<&FoundBlock> = found_blocks
        .iter()
        .filter(|block| paid.contains(&block.height))
        .collect();
    let headers: Vec<_> = futures::stream::iter(&found_blocks)
        .map(|block| block_header(client, node, block.height))
        .buffered(node.connections())
        .collect()
        .await;
    let mut orphaned = Vec::new();
    let mut kept = HashSet::new();
    // (height, amount) of the payouts of main chain blocks.
    let mut main_chain: Vec<(u64, u64)> = listed
        .iter()
        .map(|(_, atomic_unit, height)| (*height, atomic_unit.to_u64()))
        .collect();
    let mut unchecked = None;
    for (block, header) in found_blocks.into_iter().zip(headers) {
        let header = match header {
            Ok(header) => header,
            // The payouts at this height are kept until checked.
            Err(e) => {
                kept.insert(block.height);
                unchecked.get_or_insert(e);
                continue;
            }
        };
        if header.hash == block.hash {
            kept.insert(block.height);
        } else if header.depth >= P2POOL_PAYOUT_CHECK_DEPTH {
            info!("Payout sync | Block {} was orphaned", block.height);
            orphaned.push(block.height);
            // The block that replaced it may pay the user too.
            if let Some(pair) = pair {
                let amount = coinbases(client, node, std::slice::from_ref(&header))
                    .await
                    .and_then(|txs| txs.iter().map(|tx| coinbase_payout(tx, pair)).sum());
                match amount {
                    Ok(0) => (),
                    Ok(amount) => main_chain.push((block.height, amount)),
                    Err(e) => {
                        kept.insert(block.height);
                        unchecked.get_or_insert(e);
                    }
                }
            }
        }
    }
    // Another block of the pool may have replaced the orphaned one.
    orphaned.retain(|height| !kept.contains(height));
    let removed = gupax_p2pool_api
        .lock()
        .unwrap()
        .remove_payouts(&orphaned, &main_chain)
        .map_err(|e| anyhow::anyhow!("payout history write error: {e}"))?;
    results.push(format!("Orphaned payouts removed: {removed}"));
    unchecked.map_or(Ok(()), Err)
}

// Blocks found by the pool, listed by P2Pool in the file [path].
fn read_found_blocks(path: &Path) -> anyhow::Result<Vec<FoundBlock>> {
    match std::fs::read_to_string(path) {
        Ok(found_blocks) => Ok(serde_json::from_str(&found_blocks)?),
        // P2Pool writes it once the pool found a block.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

// Header of the main chain block at [height].
async fn block_header(
    client: &ClientWithMiddleware,
    node: &NodeRpc,
    height: u64,
) -> anyhow::Result<BlockHeader> {
    let params = json!({"height": height});
    let result =
        rpc::<BlockHeaderResult>(client, node, "get_block_header_by_height", params).await?;
    ensure_ok(&result.status)?;
    Ok(result.block_header)
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

// Scans the final blocks since the last scan of [address], else of the last 30 days, and adds
// the payouts to [pair] found after each range of blocks.
async fn scan(
    gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
    client: &ClientWithMiddleware,
    node: &NodeRpc,
    height: u64,
    address: &str,
    pair: &ViewPair,
    results: &mut Vec<String>,
) -> anyhow::Result<()> {
    let tip = height.saturating_sub(1);
    let (start, path_scan) = {
        let api = gupax_p2pool_api.lock().unwrap();
        let start = api.read_scan(address);
        (
            start.unwrap_or(tip.saturating_sub(FIRST_SCAN_BLOCKS)),
            api.path_scan.clone(),
        )
    };
    let end = last_final_block(height);
    let mut added = 0;
    results.push(format!("Missing payouts added: {added}"));
    for first in (start..=end).step_by(NODE_MAX_HEADERS as usize) {
        let last = (first + NODE_MAX_HEADERS - 1).min(end);
        gupax_p2pool_api.lock().unwrap().sync = format!("Scanning block {first} of {end}");
        let payouts = scan_blocks(client, node, pair, first, last).await?;
        let merged = gupax_p2pool_api.lock().unwrap().merge_payouts(&payouts);
        added += merged
            .and_then(|added| {
                GupaxP2poolApi::write_scan(&path_scan, last + 1, address).map(|()| added)
            })
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

// The first 64 hexadecimal characters of the file at [path], as in the output of
// monero-wallet-cli where the private key comes first.
pub fn read_view_key(path: &Path) -> Result<String, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(VIEW_KEY_FILE_MAX).read_to_end(&mut bytes))
        .map_err(|e| format!("View key file read error: {e}"))?;
    bytes
        .split(|byte| !byte.is_ascii_hexdigit())
        .find(|word| word.len() == 64)
        .map(|word| word.iter().copied().map(char::from).collect())
        .ok_or_else(|| "View key file: no key found".to_string())
}

// Finds the payouts to [pair] in the coinbase transactions of the blocks from [start] to [end].
async fn scan_blocks(
    client: &ClientWithMiddleware,
    node: &NodeRpc,
    pair: &ViewPair,
    start: u64,
    end: u64,
) -> anyhow::Result<Payouts> {
    let params = json!({"start_height": start, "end_height": end});
    let headers = rpc::<BlockHeaders>(client, node, "get_block_headers_range", params).await?;
    ensure_ok(&headers.status)?;
    let requests = futures::stream::iter(headers.headers.chunks(NODE_MAX_TRANSACTIONS))
        .map(|headers| coinbases(client, node, headers))
        .buffered(node.connections());
    let mut payouts = Payouts::new();
    for tx in requests
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
    {
        let amount = coinbase_payout(&tx, pair)?;
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
    node: &NodeRpc,
    headers: &[BlockHeader],
) -> anyhow::Result<Vec<Tx>> {
    let hashes: Vec<&str> = headers.iter().map(|h| h.miner_tx_hash.as_str()).collect();
    let body = json!({"txs_hashes": hashes, "prune": true});
    let txs = post(client, node, "/get_transactions", &body.to_string())
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

// Amount paid to [pair] by the coinbase transaction [tx], if it is the one of a P2Pool block,
// which holds a merge mining tag.
fn coinbase_payout(tx: &Tx, pair: &ViewPair) -> anyhow::Result<u64> {
    let tx: Transaction = deserialize(&hex::decode(&tx.pruned_as_hex)?)?;
    let extra = tx.prefix.extra.try_parse();
    let p2pool = extra
        .0
        .iter()
        .any(|field| matches!(field, SubField::MergeMining(..)));
    let Some(tx_pubkey) = extra.tx_pubkey().filter(|_| p2pool) else {
        return Ok(0);
    };
    // The shared key is derived once, the view tags rule out most outputs.
    let keygen = KeyGenerator::from_key(pair, tx_pubkey);
    Ok(tx
        .prefix
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
        .sum())
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

// Last block of a chain of [height] blocks that can no longer be orphaned.
fn last_final_block(height: u64) -> u64 {
    height.saturating_sub(1 + P2POOL_PAYOUT_CHECK_DEPTH)
}

// Client of [node] and the height of its chain, if [node] is synced to the network.
async fn synced_node(node: &NodeRpc) -> anyhow::Result<(ClientWithMiddleware, u64)> {
    let client = node.client()?;
    let info = rpc::<Info>(&client, node, "get_info", json!({})).await?;
    ensure_ok(&info.status)?;
    anyhow::ensure!(info.synchronized, "the node is not synced");
    Ok((client, info.height))
}

async fn rpc<T: DeserializeOwned>(
    client: &ClientWithMiddleware,
    node: &NodeRpc,
    method: &str,
    params: serde_json::Value,
) -> anyhow::Result<T> {
    let body = json!({"jsonrpc": "2.0", "id": "0", "method": method, "params": params});
    let response = post(client, node, "/json_rpc", &body.to_string())
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

// Posts the JSON [body] to [path] of [node], with the answer to its login challenge.
async fn post(
    client: &ClientWithMiddleware,
    node: &NodeRpc,
    path: &str,
    body: &str,
) -> anyhow::Result<reqwest::Response> {
    let url = format!("{}{path}", node.url);
    let (username, password) = node.login.split_once(':').unwrap_or((&node.login, ""));
    let params = http_auth::PasswordParams {
        username,
        password,
        uri: path,
        method: "POST",
        body: Some(body.as_bytes()),
    };
    for _ in 0..NODE_LOGIN_ATTEMPTS {
        let mut request = client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string());
        if let Some(challenge) = node.challenge.lock().unwrap().as_mut() {
            let answer = challenge.respond(&params).map_err(anyhow::Error::msg)?;
            request = request.header(AUTHORIZATION, answer);
        }
        let response = request.send().await?;
        if node.login.is_empty() || response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }
        let challenge = response.headers().get_all(WWW_AUTHENTICATE);
        let challenge =
            http_auth::PasswordClient::try_from(challenge).map_err(anyhow::Error::msg)?;
        *node.challenge.lock().unwrap() = Some(challenge);
        // The connection is free for the answer once the response is read.
        response.bytes().await?;
    }
    anyhow::bail!("the node rejected the RPC login")
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
    // on 2026-10-01, 2026-10-02 and 2026-10-04.
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
            .match_query(Matcher::Exact("limit=0".to_string()))
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

    // Synced node whose main chain holds the blocks 3,654,400 and 3,654,401, below 99,999,999.
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
            (
                99999999,
                include_str!("../../tests/fixtures/payouts/node_block_header_99999999.json"),
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

    // RPC at [url] with the default options of P2Pool.
    fn rpc_at(url: String) -> NodeRpc {
        NodeRpc {
            url,
            login: String::new(),
            proxy: String::new(),
            challenge: Mutex::new(None),
        }
    }

    #[test]
    fn rpc_of_the_node() {
        let mut api = GupaxP2poolApi::new();
        let args = "--socks5 127.0.0.1:9050 --host xyz.onion --rpc-login user:pass --rpc-ssl --host 192.168.1.5";
        api.nodes = P2poolNodes::from_args(&args.split(' ').map(String::from).collect::<Vec<_>>());
        let rpc = |ip| {
            let node = SyncSources::new(&api, &node(ip, "18081")).node;
            (node.url, node.login, node.proxy)
        };
        let strings = |url: &str, login: &str, proxy: &str| {
            (url.to_string(), login.to_string(), proxy.to_string())
        };
        assert_eq!(
            rpc("xyz.onion"),
            strings("https://xyz.onion:18081", "user:pass", "127.0.0.1:9050")
        );
        assert_eq!(
            rpc("192.168.1.5"),
            strings("http://192.168.1.5:18081", "", "")
        );
        assert_eq!(rpc("::1"), strings("http://[::1]:18081", "", ""));
    }

    // Challenges of monerod started with --rpc-login.
    fn mock_login_challenge(server: &mut mockito::ServerGuard) -> mockito::Mock {
        let mut challenge = server.mock("POST", "/json_rpc").with_status(401);
        for algorithm in ["SHA-256", "SHA-256-sess", "MD5", "MD5-sess"] {
            let value = format!(
                r#"Digest qop="auth",algorithm={algorithm},realm="monero-rpc",nonce="5ofHiw6hDiYxFtTBTnqXyQ==",stale=false"#
            );
            challenge = challenge.with_header("www-authenticate", &value);
        }
        challenge
    }

    #[test]
    fn answers_the_login_challenge_of_the_node() {
        let mut server = mockito::Server::new();
        let challenge = mock_login_challenge(&mut server)
            .match_header("authorization", Matcher::Missing)
            .expect(1)
            .create();
        let answer = r#"^Digest username="user", realm="monero-rpc", uri="/json_rpc", nonce="5ofHiw6hDiYxFtTBTnqXyQ==", algorithm=SHA-256, nc=0000000[12], cnonce="[^"]+", qop=auth, response="[0-9a-f]{64}"$"#;
        let answers = server
            .mock("POST", "/json_rpc")
            .match_header("authorization", Matcher::Regex(answer.to_string()))
            .with_body(include_str!("../../tests/fixtures/payouts/node_info.json"))
            .expect(2)
            .create();
        let mut node = rpc_at(server.url());
        node.login = "user:pass".to_string();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client().unwrap();

        // The next request answers the same challenge.
        for _ in 0..2 {
            let info = runtime
                .block_on(rpc::<Info>(&client, &node, "get_info", json!({})))
                .unwrap();
            assert!(info.synchronized);
        }
        challenge.assert();
        answers.assert();
    }

    #[test]
    fn stops_when_the_node_rejects_the_login() {
        let mut server = mockito::Server::new();
        let challenge = mock_login_challenge(&mut server)
            .expect(NODE_LOGIN_ATTEMPTS)
            .create();
        let mut node = rpc_at(server.url());
        node.login = "user:wrong".to_string();

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(rpc::<serde_json::Value>(
                &client().unwrap(),
                &node,
                "get_info",
                json!({}),
            ))
            .unwrap_err();
        assert_eq!(error.to_string(), "the node rejected the RPC login");
        challenge.assert();
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
        let dir = tempfile::tempdir().unwrap();
        let mut api = GupaxP2poolApi::temporary(dir.path());
        let found_blocks = write_found_blocks(
            dir.path(),
            json!([
                {"height": 99999999, "hash": HASH_ORPHANED},
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
            (
                "2026-04-17 14:00:00.0000".to_string(),
                AtomicUnit::from_u64(8),
                99999999,
            ),
        ];
        api.merge_payouts(&payouts).unwrap();
        let api = Arc::new(Mutex::new(api));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client().unwrap();
        let node = rpc_at(server.url());

        // The other blocks are checked after the block unknown to the node.
        let mut results = Vec::new();
        let error = runtime
            .block_on(remove_orphaned_payouts(
                &api,
                &client,
                &node,
                &read_found_blocks(&found_blocks).unwrap(),
                None,
                &Payouts::new(),
                &mut results,
            ))
            .unwrap_err();
        assert_eq!(results, ["Orphaned payouts removed: 1"]);
        assert!(
            error
                .to_string()
                .contains("greater than current top block height")
        );
        // P2Pool did not find a block yet.
        let missing = dir.path().join("missing");
        let mut results = Vec::new();
        runtime
            .block_on(remove_orphaned_payouts(
                &api,
                &client,
                &node,
                &read_found_blocks(&missing).unwrap(),
                None,
                &Payouts::new(),
                &mut results,
            ))
            .unwrap();
        assert_eq!(results, ["Orphaned payouts removed: 0"]);

        let api = api.lock().unwrap();
        assert_eq!(api.payout_u64, 3);
        assert_eq!(api.xmr.to_u64(), 274561866);
        assert!(!api.has_payout(3654400));
    }

    #[test]
    fn keeps_a_payout_paid_again_at_the_same_height() {
        let mut server = mockito::Server::new();
        mock_node(&mut server);
        let dir = tempfile::tempdir().unwrap();
        let mut api = GupaxP2poolApi::temporary(dir.path());
        let found_blocks = write_found_blocks(
            dir.path(),
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

        let mut results = Vec::new();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(remove_orphaned_payouts(
                &api,
                &client().unwrap(),
                &rpc_at(server.url()),
                &read_found_blocks(&found_blocks).unwrap(),
                None,
                &Payouts::new(),
                &mut results,
            ))
            .unwrap();

        assert_eq!(results, ["Orphaned payouts removed: 0"]);
        assert!(api.lock().unwrap().has_payout(3654400));
    }

    #[test]
    fn keeps_the_payout_of_the_block_replacing_an_orphaned_one() {
        let mut server = mockito::Server::new();
        mock_node(&mut server);
        // Coinbase transaction of the main chain block 3,654,401.
        let mut coinbase: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/payouts/node_coinbases_3654400_3654402.json"
        ))
        .unwrap();
        coinbase["txs"]
            .as_array_mut()
            .unwrap()
            .retain(|tx| tx["block_height"] == 3654401);
        server
            .mock("POST", "/get_transactions")
            .with_body(coinbase.to_string())
            .create();
        let dir = tempfile::tempdir().unwrap();
        let api = GupaxP2poolApi::temporary(dir.path());
        let found_blocks = write_found_blocks(
            dir.path(),
            json!([{"height": 3654401, "hash": HASH_ORPHANED}]),
        );
        let orphaned = (
            "2026-04-17 11:00:00.1234".to_string(),
            AtomicUnit::from_u64(7),
            3654401,
        );
        let api = Arc::new(Mutex::new(api));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client().unwrap();
        let node = rpc_at(server.url());
        let pair = view_pair(ADDRESS, VIEW_KEY).unwrap();

        // The main chain block is found with the view key, or listed by the observer.
        for (pair, listed) in [(Some(&pair), vec![]), (None, vec![payout_3654401()])] {
            let payouts = [orphaned.clone(), payout_3654401()];
            api.lock().unwrap().merge_payouts(&payouts).unwrap();
            let mut results = Vec::new();
            runtime
                .block_on(remove_orphaned_payouts(
                    &api,
                    &client,
                    &node,
                    &read_found_blocks(&found_blocks).unwrap(),
                    pair,
                    &listed,
                    &mut results,
                ))
                .unwrap();
            assert_eq!(results, ["Orphaned payouts removed: 1"]);
            assert_eq!(
                api.lock().unwrap().log,
                "2026-04-17 11:01:06.0000 | 0.000274561854 XMR | Block 3,654,401\n"
            );
        }
    }

    #[test]
    fn counts_the_payouts_of_p2pool_blocks() {
        let coinbases: Transactions = serde_json::from_str(include_str!(
            "../../tests/fixtures/payouts/node_coinbases_3654400_3654402.json"
        ))
        .unwrap();
        let tx = coinbases
            .txs
            .into_iter()
            .find(|tx| tx.block_height == 3654401)
            .unwrap();
        let pair = view_pair(ADDRESS, VIEW_KEY).unwrap();
        assert_eq!(coinbase_payout(&tx, &pair).unwrap(), 274561854);

        // The same outputs paid by a block without merge mining tag, which P2Pool adds.
        let mut transaction: Transaction =
            deserialize(&hex::decode(&tx.pruned_as_hex).unwrap()).unwrap();
        let mut extra = transaction.prefix.extra.try_parse();
        extra
            .0
            .retain(|field| !matches!(field, SubField::MergeMining(..)));
        transaction.prefix.extra = extra.into();
        let solo = Tx {
            pruned_as_hex: hex::encode(monero::consensus::serialize(&transaction)),
            ..tx
        };
        assert_eq!(coinbase_payout(&solo, &pair).unwrap(), 0);
    }

    #[test]
    fn observer_payouts_of_an_address() {
        let mut server = mockito::Server::new();
        let general_fund =
            include_str!("../../tests/fixtures/payouts/observer_payouts_general_fund.json");
        let unknown = include_str!("../../tests/fixtures/payouts/observer_payouts_unknown.json");
        mock_observer(&mut server, ADDRESS, 200, general_fund);
        mock_observer(&mut server, "4AAAA", 404, unknown);
        mock_observer(&mut server, "4BBBB", 404, "404 page not found");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client().unwrap();

        // A pasted URL may end with a slash and a line break.
        let observer = format!("{}/ ", server.url());
        let payouts = runtime
            .block_on(observer_payouts(
                &client,
                &observer,
                ADDRESS,
                None,
                u64::MAX,
            ))
            .unwrap();
        assert_eq!(payouts.len(), 21);
        assert_eq!(payouts[0], payout_3654401());
        let total: u64 = payouts.iter().map(|payout| payout.1.to_u64()).sum();
        assert_eq!(total, 12828731759);

        // The observer knows no share of this address.
        let payouts = runtime
            .block_on(observer_payouts(
                &client,
                &server.url(),
                "4AAAA",
                None,
                u64::MAX,
            ))
            .unwrap();
        assert!(payouts.is_empty());
        let error = runtime
            .block_on(observer_payouts(
                &client,
                &server.url(),
                "4BBBB",
                None,
                u64::MAX,
            ))
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
        let dir = tempfile::tempdir().unwrap();
        let api = GupaxP2poolApi::temporary(dir.path());
        GupaxP2poolApi::write_scan(&api.path_scan, 3700000, "4AAAA").unwrap();
        GupaxP2poolApi::write_scan(&api.path_scan, 3654400, ADDRESS).unwrap();
        let api = Arc::new(Mutex::new(api));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = client().unwrap();
        let node = rpc_at(server.url());
        let pair = view_pair(ADDRESS, VIEW_KEY).unwrap();
        // A node at height 3,654,463, so the block 3,654,402 is final.
        let height = 3654463;

        let mut results = Vec::new();
        let scanned = scan(&api, &client, &node, height, ADDRESS, &pair, &mut results);
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
        let scanned = scan(&api, &client, &node, height, ADDRESS, &pair, &mut results);
        runtime.block_on(scanned).unwrap();
        assert_eq!(results, ["Missing payouts added: 0"]);
        headers.assert();
    }

    #[test]
    fn observer_sync_resumes_after_the_final_blocks() {
        let mut server = mockito::Server::new();
        let general_fund =
            include_str!("../../tests/fixtures/payouts/observer_payouts_general_fund.json");
        let full = mock_observer(&mut server, ADDRESS, 200, general_fund).expect(1);
        let mut recent: Vec<serde_json::Value> = serde_json::from_str(general_fund).unwrap();
        recent.retain(|payout| payout["main_height"].as_u64() >= Some(3654400));
        let resumed = server
            .mock("GET", format!("/api/payouts/{ADDRESS}").as_str())
            .match_query(Matcher::Exact("limit=0&from_height=3654400".to_string()))
            .with_body(serde_json::to_string(&recent).unwrap())
            .expect(1)
            .create();
        mock_node(&mut server);
        let dir = tempfile::tempdir().unwrap();
        let mut api = GupaxP2poolApi::temporary(dir.path());
        api.address = ADDRESS.to_string();
        api.observer = server.url();
        api.found_blocks = write_found_blocks(
            dir.path(),
            json!([{"height": 3654400, "hash": HASH_3654400}]),
        );
        let host = server.host_with_port();
        let (ip, rpc) = host.split_once(':').unwrap();
        let sources = SyncSources::new(&api, &node(ip, rpc));
        let api = Arc::new(Mutex::new(api));

        sync(&api, &sources);
        assert_eq!(
            api.lock().unwrap().sync,
            "Missing payouts added: 21. Orphaned payouts removed: 0"
        );
        // The node is at height 3,774,988, so the block 3,774,927 is the last final one.
        assert_eq!(api.lock().unwrap().read_scan(ADDRESS), Some(3774928));
        // The oldest block found by the pool comes before the final blocks of the first sync.
        sync(&api, &sources);
        assert_eq!(
            api.lock().unwrap().sync,
            "Missing payouts added: 0. Orphaned payouts removed: 0"
        );
        full.assert();
        resumed.assert();
    }

    #[test]
    fn sync_sources_priority() {
        let mut server = mockito::Server::new();
        let general_fund =
            include_str!("../../tests/fixtures/payouts/observer_payouts_general_fund.json");
        let observer = mock_observer(&mut server, ADDRESS, 200, general_fund).expect(1);
        mock_node(&mut server);
        let dir = tempfile::tempdir().unwrap();
        let mut api = GupaxP2poolApi::temporary(dir.path());
        api.address = ADDRESS.to_string();
        api.found_blocks = write_found_blocks(
            dir.path(),
            json!([{"height": 3654400, "hash": HASH_ORPHANED}]),
        );
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
            let mut api = api.lock().unwrap();
            api.view_key = view_key.to_string();
            api.observer = observer.to_string();
            SyncSources::new(&api, &node)
        };

        // The observer adds the missing payouts, then the orphaned one is removed.
        sync(&api, &sources("", &server.url()));
        assert_eq!(
            api.lock().unwrap().sync,
            "Missing payouts added: 21. Orphaned payouts removed: 1"
        );
        assert_eq!(api.lock().unwrap().payout_u64, 21);
        assert!(!api.lock().unwrap().sync_failed);

        // A filled view key is used before the observer.
        let view_key = format!("01{}", "00".repeat(31));
        sync(&api, &sources(&view_key, &server.url()));
        assert_eq!(
            api.lock().unwrap().sync,
            "Sync error: the private view key does not belong to the P2Pool address. Orphaned payouts removed: 0"
        );
        assert!(api.lock().unwrap().sync_failed);
        observer.assert();

        // Without view key and observer, only the orphaned payouts are checked.
        sync(&api, &sources("", ""));
        assert_eq!(api.lock().unwrap().sync, "Orphaned payouts removed: 0");
        assert!(!api.lock().unwrap().sync_failed);
    }

    #[test]
    fn stop_a_sync_waiting_for_the_node() {
        // This node accepts connections and never answers.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let dir = tempfile::tempdir().unwrap();
        let mut api = GupaxP2poolApi::temporary(dir.path());
        api.address = ADDRESS.to_string();
        api.view_key = VIEW_KEY.to_string();
        let node = node("127.0.0.1", &port.to_string());
        let api = Arc::new(Mutex::new(api));

        assert!(start(&mut api.lock().unwrap(), &api, &node));
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
    }

    #[test]
    fn view_key_in_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("view_key");
        // Output of the viewkey command of monero-wallet-cli.
        let public = "a".repeat(64);
        std::fs::write(&file, format!("secret: {VIEW_KEY}\npublic: {public}\n")).unwrap();
        assert_eq!(read_view_key(&file).as_deref(), Ok(VIEW_KEY));

        std::fs::write(&file, "secret: 1234").unwrap();
        assert_eq!(
            read_view_key(&file),
            Err("View key file: no key found".to_string())
        );
        std::fs::remove_file(&file).unwrap();
        assert!(
            read_view_key(&file)
                .unwrap_err()
                .starts_with("View key file read error")
        );
    }
}
