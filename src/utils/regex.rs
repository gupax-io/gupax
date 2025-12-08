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

// Some regexes used throughout Gupax.

use crate::{disk::node::Node, helper::xvb::nodes::Pool};
use log::warn;
use once_cell::sync::Lazy;
use regex::Regex;

//---------------------------------------------------------------------------------------------------- Lazy
pub static REGEXES: Lazy<Regexes> = Lazy::new(Regexes::new);
pub static P2POOL_REGEX: Lazy<P2poolRegex> = Lazy::new(P2poolRegex::new);
pub static XMRIG_REGEX: Lazy<XmrigRegex> = Lazy::new(XmrigRegex::new);

//---------------------------------------------------------------------------------------------------- [Regexes] struct
// General purpose Regexes, mostly used in the GUI.
#[derive(Clone, Debug)]
pub struct Regexes {
    pub name: Regex,
    pub address: Regex,
    pub ipv4: Regex,
    pub domain: Regex,
    pub port: Regex,
}

impl Regexes {
    #[cold]
    #[inline(never)]
    fn new() -> Self {
        Self {
			name: Regex::new("^[A-Za-z0-9-_.]+( [A-Za-z0-9-_.]+)*$").unwrap(),
			address: Regex::new("^4[A-Za-z1-9]+$").unwrap(), // This still needs to check for (l, I, o, 0)
			ipv4: Regex::new(r#"^((25[0-5]|(2[0-4]|1\d|[1-9]|)\d)\.?\b){4}$"#).unwrap(),
			domain: Regex::new(r#"^[A-Za-z0-9-.]+[A-Za-z0-9-]+$"#).unwrap(),
			port: Regex::new(r#"^([1-9][0-9]{0,3}|[1-5][0-9]{4}|6[0-4][0-9]{3}|65[0-4][0-9]{2}|655[0-2][0-9]|6553[0-5])$"#).unwrap(),
		}
    }

    #[inline]
    // Check if a Monero address is correct.
    // This actually only checks for length & Base58, and doesn't do any checksum validation
    // (the last few bytes of a Monero address are a Keccak hash checksum) so some invalid addresses can trick this function.
    pub fn addr_ok(address: &str) -> bool {
        address.len() == 95
            && REGEXES.address.is_match(address)
            && !address.contains('0')
            && !address.contains('O')
            && !address.contains('l')
    }
}

//---------------------------------------------------------------------------------------------------- [P2poolRegex]
// Meant for parsing the output of P2Pool and finding payouts and total XMR found.
// Why Regex instead of the standard library?
//    1. I'm already using Regex
//    2. It's insanely faster
//
// The following STDLIB implementation takes [0.003~] seconds to find all matches given a [String] with 30k lines:
//     let mut n = 0;
//     for line in P2POOL_OUTPUT.lines() {
//         if line.contains("payout of [0-9].[0-9]+ XMR") { n += 1; }
//     }
//
// This regex function takes [0.0003~] seconds (10x faster):
//     let regex = Regex::new("payout of [0-9].[0-9]+ XMR").unwrap();
//     let n = regex.find_iter(P2POOL_OUTPUT).count();
//
// Both are nominally fast enough where it doesn't matter too much but meh, why not use regex.
#[derive(Clone, Debug)]
pub struct P2poolRegex {
    pub date: Regex,
    pub payout: Regex,
    pub payout_float: Regex,
    pub block: Regex,
    pub block_int: Regex,
    pub block_comma: Regex,
}

impl P2poolRegex {
    #[cold]
    #[inline(never)]
    fn new() -> Self {
        Self {
            date: Regex::new("[0-9]+-[0-9]+-[0-9]+ [0-9]+:[0-9]+:[0-9]+.[0-9]+").unwrap(),
            payout: Regex::new("payout of [0-9].[0-9]+ XMR").unwrap(), // Assumes 12 digits after the dot.
            payout_float: Regex::new("[0-9].[0-9]{12}").unwrap(), // Assumes 12 digits after the dot.
            block: Regex::new("block [0-9]{7}").unwrap(), // Monero blocks will be 7 digits for... the next 10,379 years
            block_int: Regex::new("[0-9]{7}").unwrap(),
            block_comma: Regex::new("[0-9],[0-9]{3},[0-9]{3}").unwrap(),
        }
    }
}

//---------------------------------------------------------------------------------------------------- XMRig regex.
#[derive(Debug)]
pub struct XmrigRegex {
    pub not_mining: Regex,
    pub new_job: Regex,
    pub timeout: Regex,
    pub valid_conn: Regex,
    pub invalid_conn: Regex,
    pub error: Regex,
}

impl XmrigRegex {
    fn new() -> Self {
        Self {
            not_mining: Regex::new("no active pools, stop mining").unwrap(),
            timeout: Regex::new("timeout").unwrap(),
            new_job: Regex::new("new job").unwrap(),
            valid_conn: Regex::new("upstreams active: 1").unwrap(),
            invalid_conn: Regex::new("error: 1").unwrap(),
            // we don't want to include connections status from xmrig-proxy that show the number of errors
            error: Regex::new(r"error: \D").unwrap(),
        }
    }
}

// count the lines without consuming.
pub fn num_lines(s: &str) -> usize {
    static LINE_BREAKS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\r?\n").unwrap());
    LINE_BREAKS.captures_iter(s).count() + 1
}
// get the number of current shares
pub fn nb_current_shares(s: &str) -> Option<u32> {
    static CURRENT_SHARE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"Your shares               = (?P<nb>\d+) blocks").unwrap());
    if let Some(c) = CURRENT_SHARE.captures(s)
        && let Some(m) = c.name("nb")
    {
        return Some(m.as_str().parse::<u32>().unwrap_or_else(|_| {
            panic!(
                "{}",
                [
                    "the number of shares should have been a unit number but is :\n",
                    m.as_str(),
                ]
                .concat()
            )
        }));
    }
    None
}
// get the number of current shares
pub fn p2pool_monero_node(s: &str) -> Option<Node> {
    static CURRENT_NODE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?P<ip>\S+):RPC (?P<rpc>\d+):ZMQ (?P<zmq>\d+)").unwrap());
    if let Some(c) = CURRENT_NODE.captures(s)
        && let Some(m_ip) = c.name("ip")
        && let Some(m_rpc) = c.name("rpc")
        && let Ok(rpc) = m_rpc.as_str().parse::<u16>()
        && let Some(m_zmq) = c.name("zmq")
        && let Ok(zmq) = m_zmq.as_str().parse::<u16>()
    {
        return Some(Node {
            ip: m_ip.as_str().to_string(),
            rpc: rpc.to_string(),
            zmq: zmq.to_string(),
        });
    }
    None
}
pub fn detect_pool_xmrig(s: &str, proxy_port: u16, p2pool_port: u16) -> Option<Pool> {
    static CURRENT_SHARE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(use pool|new job from) (?P<pool>.*:\d{1,5})(| diff)").unwrap());
    if let Some(c) = CURRENT_SHARE.captures(s)
        && let Some(m) = c.name("pool")
    {
        match m.as_str() {
            // if user change address of local p2pool, it could create issue
            x if x.contains("127.0.0.1") => {
                let port = x.split_once(":").unwrap_or_default().1.parse::<u16>();
                if let Ok(port) = port {
                    if port == proxy_port {
                        return Some(Pool::XmrigProxy(port));
                    }
                    if port == p2pool_port {
                        return Some(Pool::P2pool(port));
                    }
                    return Some(Pool::Custom("127.0.0.1".to_string(), port));
                }
            }
            "eu.xmrvsbeast.com:4247" => {
                return Some(Pool::XvBEurope);
            }
            "na.xmrvsbeast.com:4247" => {
                return Some(Pool::XvBNorthAmerica);
            }
            x => {
                let (ip, port) = x.split_once(":").unwrap_or_default();
                if let Ok(port) = port.parse() {
                    return Some(Pool::Custom(ip.to_string(), port));
                }
            }
        }
    }
    warn!(
        "a line on xmrig console was detected as using a new pool but the syntax was not recognized or it was not a pool useable for the algorithm."
    );
    None
}
pub fn estimated_hr(s: &str) -> Option<f32> {
    static CURRENT_SHARE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?P<nb>[-+]?[0-9]*\.?[0-9]+([eE][-+]?[0-9]+)?) (?P<unit>.*)H/s").unwrap()
    });
    if let Some(c) = CURRENT_SHARE.captures(s) {
        let coeff = if let Some(unit) = c.name("unit") {
            match unit.as_str() {
                "K" | "k" => 1000,
                "M" | "m" => 1000 * 1000,
                "G" | "g" => 1000 * 1000 * 1000,
                _ => 1,
            }
        } else {
            1
        } as f32;
        if let Some(m) = c.name("nb") {
            return Some(
                m.as_str().parse::<f32>().unwrap_or_else(|_| {
                    panic!(
                        "{}",
                        [
                            "the number of shares should have been a float number but is :\n",
                            m.as_str(),
                        ]
                        .concat()
                    )
                }) * coeff,
            );
        }
    }
    None
}

pub fn pplns_window_nb_blocks(l: &str) -> Option<u64> {
    static LINE_SHARE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"Your shares               = (?P<nb>\d+) blocks").unwrap());
    if let Some(captures) = LINE_SHARE.captures(l)
        && let Some(blocks) = captures.name("nb")
        && let Ok(nb) = blocks.as_str().parse::<u64>()
    {
        return Some(nb);
    }
    None
}
pub fn contains_node(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(Monero node|host )").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_timeout(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| Regex::new(r"timeout").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_error(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| Regex::new(r"error").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_usepool(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| Regex::new(r"use pool").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_statuscommand(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^statusfromgupax").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_yourshare(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^Your shares               = ").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_window_nb_blocks(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^PPLNS window              = ").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_yourhashrate(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^Your hashrate \(pool-side\) = ").unwrap());
    LINE_SHARE.is_match(l)
}
pub fn contains_end_status(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^Uptime         ").unwrap());
    LINE_SHARE.is_match(l)
}
// P2Pool
/// if the node is disconnected
/// this error will be present if log > 1 and Node is disconnected
pub fn contains_zmq_failure(l: &str) -> bool {
    static LINE_SHARE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(p2pool with offline node: failed: error Error (empty response)|ZMQReader failed to connect to|P2Pool Couldn't restart ZMQ reader: exception Operation cannot be accomplished in current state)").unwrap()
    });
    LINE_SHARE.is_match(l)
}

//---------------------------------------------------------------------------------------------------- TEST
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn build_regexes() {
        let r = Regexes::new();
        assert!(Regex::is_match(&r.name, "_this_ is... a n-a-m-e."));
        assert!(Regex::is_match(
            &r.address,
            "44hintoFpuo3ugKfcqJvh5BmrsTRpnTasJmetKC4VXCt6QDtbHVuixdTtsm6Ptp7Y8haXnJ6j8Gj2dra8CKy5ewz7Vi9CYW"
        ));
        assert!(Regex::is_match(&r.ipv4, "192.168.1.2"));
        assert!(Regex::is_match(&r.ipv4, "127.0.0.1"));
        assert!(Regex::is_match(&r.domain, "sub.domain.com"));
        assert!(Regex::is_match(&r.domain, "sub.domain.longtld"));
        assert!(Regex::is_match(&r.domain, "sub.sub.domain.longtld"));
        assert!(Regex::is_match(&r.domain, "my.node.com"));
        assert!(Regex::is_match(&r.domain, "my.node.longtld"));
        assert!(Regex::is_match(&r.domain, "my.monero-node123.net"));
        assert!(Regex::is_match(&r.domain, "www.my-node.org"));
        assert!(Regex::is_match(&r.domain, "www.my-monero-node123.io"));
        assert!(Regex::is_match(&r.domain, "www.my-monero-node123.longtld"));
        assert!(Regex::is_match(&r.domain, "www.my-monero-node123.org"));
        for i in 1..=65535 {
            assert!(Regex::is_match(&r.port, &i.to_string()));
        }
        assert!(!Regex::is_match(&r.port, "0"));
        assert!(!Regex::is_match(&r.port, "65536"));
    }

    #[test]
    fn build_p2pool_regex() {
        let r = P2poolRegex::new();
        let text = "NOTICE  2022-11-11 11:11:11.1111 P2Pool You received a payout of 0.111111111111 XMR in block 1111111";
        let text2 = "2022-11-11 11:11:11.1111 | 0.111111111111 XMR | Block 1,111,111";
        assert_eq!(
            r.payout.find(text).unwrap().as_str(),
            "payout of 0.111111111111 XMR"
        );
        assert_eq!(
            r.payout_float.find(text).unwrap().as_str(),
            "0.111111111111"
        );
        assert_eq!(
            r.date.find(text).unwrap().as_str(),
            "2022-11-11 11:11:11.1111"
        );
        assert_eq!(r.block.find(text).unwrap().as_str(), "block 1111111");
        assert_eq!(r.block_int.find(text).unwrap().as_str(), "1111111");
        assert_eq!(r.block_comma.find(text2).unwrap().as_str(), "1,111,111");
    }

    #[test]
    fn build_xmrig_regex() {
        let r = XmrigRegex::new();
        let text = "[2022-02-12 12:49:30.311]  net      no active pools, stop mining";
        let text2 = "[2022-02-12 12:49:30.311]  net      new job from 192.168.2.1:3333 diff 402K algo rx/0 height 2241142 (11 tx)";
        assert_eq!(
            r.not_mining.find(text).unwrap().as_str(),
            "no active pools, stop mining"
        );
        assert_eq!(r.new_job.find(text2).unwrap().as_str(), "new job");
    }
}
