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

#[cfg(test)]
mod test {

    use crate::app::submenu_enum::SubmenuP2pool;
    use crate::disk::state::{StartOptionsMode, XmrigProxy};
    use crate::helper::p2pool::ImgP2pool;
    use crate::helper::xrig::xmrig_proxy::ImgProxy;
    use crate::helper::{
        Helper, Process, ProcessName, ProcessState,
        p2pool::{PrivP2poolLocalApi, PrivP2poolNetworkApi},
    };
    use crate::human::HumanNumber;
    use crate::miscs::client;

    #[test]
    fn get_current_shares() {
        let stdout = "
statusfromgupax
2024-03-25 21:31:21.7919 SideChain status
Monero node               = node2.monerodevs.org:18089:ZMQ:18084 (37.187.74.171)
Main chain height         = 3113042
Main chain hashrate       = 1.985 GH/s
Side chain ID             = mini
Side chain height         = 7230432
Side chain hashrate       = 8.925 MH/s
PPLNS window              = 2160 blocks (+79 uncles, 0 orphans)
PPLNS window duration     = 6h 9m 46s
Your wallet address       = 4A5Dwt2qKwKEQrZfo4aBkSNtvDDAzSFbAJcyFkdW5RwDh9U4WgeZrgKT4hUoE2gv8h6NmsNMTyjsEL8eSLMbABds5rYFWnw
Your shares               = 0 blocks (+0 uncles, 0 orphans)
Block reward share        = 0.000% (0.000000000000 XMR)
2024-03-25 21:31:21.7920 StratumServer status
Hashrate (15m est) = 0 H/s
Hashrate (1h  est) = 0 H/s
Hashrate (24h est) = 0 H/s
Total hashes       = 0
Shares found       = 0
Average effort     = 0.000%
Current effort     = 0.000%
Connections        = 0 (0 incoming)
2024-03-25 21:31:21.7920 P2PServer status
Connections    = 10 (0 incoming)
Peer list size = 1209
Uptime         = 0h 2m 4s
".lines();
        let mut shares = 1;
        let mut status_output = false;
        for line in stdout {
            // if command status is sent by gupax process and not the user, forward it only to update_from_status method.
            // 25 lines after the command are the result of status, with last line finishing by update.
            if line.contains("statusfromgupax") {
                status_output = true;
                continue;
            }
            if status_output {
                if line.contains("Your shares") {
                    // update sidechain shares
                    shares = line.split_once('=').expect("should be = at Your Share, maybe new version of p2pool has different output for status command ?").1.split_once("blocks").expect("should be a 'blocks' at Your Share, maybe new version of p2pool has different output for status command ?").0.trim().parse::<u32>().expect("this should be the number of share");
                }
                if line.contains("Uptime") {
                    // end of status
                    status_output = false;
                }
                continue;
            }
        }
        assert_eq!(shares, 0);
    }
    #[test]
    fn reset_gui_output() {
        let max = crate::helper::GUI_OUTPUT_LEEWAY;
        let mut string = String::with_capacity(max);
        for _ in 0..=max {
            string.push('0');
        }
        Helper::check_reset_gui_output(&mut string, ProcessName::P2pool);
        // Some text gets added, so just check for less than 500 bytes.
        assert!(string.len() < 500);
    }

    #[test]
    fn combine_gui_pub_p2pool_api() {
        use crate::helper::PubP2poolApi;
        let mut gui_api = PubP2poolApi::new();
        let mut pub_api = PubP2poolApi::new();
        pub_api.payouts = 1;
        pub_api.payouts_hour = 2.0;
        pub_api.payouts_day = 3.0;
        pub_api.payouts_month = 4.0;
        pub_api.xmr = 1.0;
        pub_api.xmr_hour = 2.0;
        pub_api.xmr_day = 3.0;
        pub_api.xmr_month = 4.0;
        println!("BEFORE - GUI_API: {:#?}\nPUB_API: {:#?}", gui_api, pub_api);
        assert_ne!(gui_api, pub_api);
        PubP2poolApi::combine_gui_pub_api(&mut gui_api, &mut pub_api);
        println!("AFTER - GUI_API: {:#?}\nPUB_API: {:#?}", gui_api, pub_api);
        assert_eq!(gui_api, pub_api);
        pub_api.xmr = 2.0;
        PubP2poolApi::combine_gui_pub_api(&mut gui_api, &mut pub_api);
        assert_eq!(gui_api, pub_api);
        assert_eq!(gui_api.xmr, 2.0);
        assert_eq!(pub_api.xmr, 2.0);
    }

    #[test]
    fn calc_payouts_and_xmr_from_output_p2pool() {
        use crate::helper::PubP2poolApi;
        use std::sync::{Arc, Mutex};
        let public = Arc::new(Mutex::new(PubP2poolApi::new()));
        let output_parse = Arc::new(Mutex::new(String::from(
            r#"payout of 5.000000000001 XMR in block 1111
			payout of 5.000000000001 XMR in block 1112
			payout of 5.000000000001 XMR in block 1113"#,
        )));
        let output_pub = Arc::new(Mutex::new(String::new()));
        let elapsed = std::time::Duration::from_secs(60);
        let mut public = public.lock().unwrap();
        PubP2poolApi::update_from_output(&mut public, &output_parse, &output_pub, elapsed);
        println!("{:#?}", public);
        assert_eq!(public.payouts, 3);
        assert_eq!(public.payouts_hour, 180.0);
        assert_eq!(public.payouts_day, 4320.0);
        assert_eq!(public.payouts_month, 129600.0);
        assert_eq!(public.xmr, 15.000000000003);
        assert_eq!(public.xmr_hour, 900.00000000018);
        assert_eq!(public.xmr_day, 21600.00000000432);
        assert_eq!(public.xmr_month, 648000.0000001296);
    }

    #[test]
    fn p2pool_synchronized_false_positive() {
        use crate::helper::PubP2poolApi;
        use std::sync::{Arc, Mutex};
        let public = Arc::new(Mutex::new(PubP2poolApi::new()));

        // The SideChain that is "SYNCHRONIZED" in this output is
        // probably not main/mini, but the sidechain started on height 1,
        // so this should _not_ trigger alive state.
        let output_parse = Arc::new(Mutex::new(String::from(
            r#"2024-11-02 17:39:02.6241 SideChain SYNCHRONIZED
            2024-11-02 17:39:02.6242 StratumServer SHARE FOUND: mainchain height 3272685, sidechain height 0, diff 100000, client 127.0.0.1:40874, effort 100.001%
            2024-11-02 17:39:02.6559 StratumServer SHARE FOUND: mainchain height 3272685, sidechain height 0, diff 100000, client 127.0.0.1:40874, effort 200.002%"#,
        )));
        let output_pub = Arc::new(Mutex::new(String::new()));
        let elapsed = std::time::Duration::from_secs(60);
        let process = Arc::new(Mutex::new(Process::new(
            ProcessName::P2pool,
            "".to_string(),
            PathBuf::new(),
        )));

        // It only gets checked if we're `Syncing`.
        process.lock().unwrap().state = ProcessState::Syncing;
        PubP2poolApi::update_from_output(
            &mut public.lock().unwrap(),
            &output_parse,
            &output_pub,
            elapsed,
        );
        println!("{:#?}", process);
        assert!(process.lock().unwrap().state == ProcessState::Syncing); // still syncing
    }

    #[test]
    fn update_pub_p2pool_from_local_network_pool() {
        use crate::helper::PubP2poolApi;
        use crate::helper::p2pool::PoolStatistics;
        use crate::helper::p2pool::PrivP2poolLocalApi;
        use crate::helper::p2pool::PrivP2poolNetworkApi;
        use crate::helper::p2pool::PrivP2poolPoolApi;
        use std::sync::{Arc, Mutex};
        let public = Arc::new(Mutex::new(PubP2poolApi::new()));
        let local = PrivP2poolLocalApi {
            hashrate_15m: 10_000,
            hashrate_1h: 20_000,
            hashrate_24h: 30_000,
            shares_found: 1000,
            average_effort: 100.000,
            current_effort: 200.000,
            connections: 1234,
        };
        let network = PrivP2poolNetworkApi {
            difficulty: 300_000_000_000,
            hash: "asdf".to_string(),
            height: 1234,
            reward: 2345,
            timestamp: 3456,
        };
        let pool = PrivP2poolPoolApi {
            pool_statistics: PoolStatistics {
                hashRate: 1_000_000, // 1 MH/s
                miners: 1_000,
                sidechainHeight: 10_000_000,
                sidechainDifficulty: 10_000_000,
            },
        };
        // Update Local
        let mut p = public.lock().unwrap();
        PubP2poolApi::update_from_local(&mut p, local);
        println!("AFTER LOCAL: {:#?}", p);
        assert_eq!(p.hashrate_15m.to_string(), "10000");
        assert_eq!(p.hashrate_1h.to_string(), "20000");
        assert_eq!(p.hashrate_24h.to_string(), "30000");
        assert_eq!(
            p.shares_found.expect("the value is set").to_string(),
            "1000"
        );
        assert_eq!(p.average_effort.to_string(), "100.00%");
        assert_eq!(p.current_effort.to_string(), "200.00%");
        assert_eq!(p.connections.to_string(), "1,234");
        assert_eq!(p.user_p2pool_hashrate_u64, 20000);
        // Update Network + Pool
        PubP2poolApi::update_from_network_pool(&mut p, network, pool);
        println!("AFTER NETWORK+POOL: {:#?}", p);
        assert_eq!(p.monero_difficulty.to_string(), "300,000,000,000");
        assert_eq!(p.monero_hashrate.to_string(), "2.500 GH/s");
        assert_eq!(p.hash.to_string(), "asdf");
        assert_eq!(HumanNumber::from_u32(p.height).to_string(), "1,234");
        assert_eq!(p.reward.to_u64(), 2345);
        assert_eq!(p.p2pool_difficulty.to_string(), "10,000,000");
        assert_eq!(p.p2pool_hashrate.to_string(), "1.000 MH/s");
        assert_eq!(p.miners.to_string(), "1,000");
        assert_eq!(
            p.solo_block_mean.display(false),
            "5 months, 21 days, 9 hours, 52 minutes"
        );
        assert_eq!(
            p.p2pool_block_mean.display(false),
            "3 days, 11 hours, 20 minutes"
        );
        assert_eq!(p.p2pool_share_mean.display(false), "8 minutes, 20 seconds");
        assert_eq!(p.p2pool_percent.to_string(), "0.040000%");
        assert_eq!(p.user_p2pool_percent.to_string(), "2.000000%");
        assert_eq!(p.user_monero_percent.to_string(), "0.000800%");
        drop(p);
    }

    #[test]
    fn set_xmrig_mining() {
        use crate::helper::PubXmrigApi;
        use std::sync::{Arc, Mutex};
        let public = Arc::new(Mutex::new(PubXmrigApi::new()));
        let output_parse = Arc::new(Mutex::new(String::from(
            "[2022-02-12 12:49:30.311]  net      no active pools, stop mining",
        )));
        let elapsed = std::time::Duration::from_secs(60);
        let process = Arc::new(Mutex::new(Process::new(
            ProcessName::Xmrig,
            "".to_string(),
            PathBuf::new(),
        )));
        let process_p2pool = Arc::new(Mutex::new(Process::new(
            ProcessName::P2pool,
            "".to_string(),
            PathBuf::new(),
        )));
        let process_xp = Arc::new(Mutex::new(Process::new(
            ProcessName::XmrigProxy,
            "".to_string(),
            PathBuf::new(),
        )));
        let img_p2pool = Arc::new(Mutex::new(ImgP2pool::new()));
        let img_proxy = Arc::new(Mutex::new(ImgProxy::new()));
        let proxy_state = XmrigProxy::default();
        let p2pool_state = P2pool::default();

        process.lock().unwrap().state = ProcessState::Alive;
        process.lock().unwrap().output_parse = output_parse;
        PubXmrigApi::update_from_output(
            &mut public.lock().unwrap(),
            elapsed,
            &mut process.lock().unwrap(),
            &process_p2pool.lock().unwrap(),
            &process_xp.lock().unwrap(),
            &img_proxy,
            &img_p2pool,
            &proxy_state,
            &p2pool_state,
        );
        println!("{:#?}", process);
        assert!(process.lock().unwrap().state == ProcessState::NotMining);

        let output_parse = Arc::new(Mutex::new(String::from(
            "[2022-02-12 12:49:30.311]  net      new job from 192.168.2.1:3333 diff 402K algo rx/0 height 2241142 (11 tx)",
        )));
        process.lock().unwrap().output_parse = output_parse;
        PubXmrigApi::update_from_output(
            &mut public.lock().unwrap(),
            elapsed,
            &mut process.lock().unwrap(),
            &process_p2pool.lock().unwrap(),
            &process_xp.lock().unwrap(),
            &img_proxy,
            &img_p2pool,
            &proxy_state,
            &p2pool_state,
        );
        assert!(process.lock().unwrap().state == ProcessState::Alive);
    }

    #[test]
    fn serde_priv_p2pool_local_api() {
        let data = r#"{
				"hashrate_15m": 12,
				"hashrate_1h": 11111,
				"hashrate_24h": 468967,
				"total_hashes": 2019283840922394082390,
				"shares_found": 289037,
				"average_effort": 915.563,
				"current_effort": 129.297,
				"connections": 123,
				"incoming_connections": 96
			}"#;
        let priv_api = PrivP2poolLocalApi::from_str(data).unwrap();
        let json = serde_json::ser::to_string_pretty(&priv_api).unwrap();
        println!("{}", json);
        let data_after_ser = r#"{
  "hashrate_15m": 12,
  "hashrate_1h": 11111,
  "hashrate_24h": 468967,
  "shares_found": 289037,
  "average_effort": 915.563,
  "current_effort": 129.297,
  "connections": 123
}"#;
        assert_eq!(data_after_ser, json)
    }

    #[test]
    fn serde_priv_p2pool_network_api() {
        let data = r#"{
				"difficulty": 319028180924,
				"hash": "22ae1b83d727bb2ff4efc17b485bc47bc8bf5e29a7b3af65baf42213ac70a39b",
				"height": 2776576,
				"reward": 600499860000,
				"timestamp": 1670953659
			}"#;
        let priv_api = PrivP2poolNetworkApi::from_str(data).unwrap();
        let json = serde_json::ser::to_string_pretty(&priv_api).unwrap();
        println!("{}", json);
        let data_after_ser = r#"{
  "difficulty": 319028180924,
  "hash": "22ae1b83d727bb2ff4efc17b485bc47bc8bf5e29a7b3af65baf42213ac70a39b",
  "height": 2776576,
  "reward": 600499860000,
  "timestamp": 1670953659
}"#;
        assert_eq!(data_after_ser, json)
    }

    #[test]
    fn serde_priv_p2pool_pool_api() {
        let data = r#"{
				"pool_list": ["pplns"],
				"pool_statistics": {
					"hashRate": 10225772,
					"miners": 713,
					"totalHashes": 487463929193948,
					"lastBlockFoundTime": 1670453228,
					"lastBlockFound": 2756570,
					"totalBlocksFound": 4,
					"sidechainHeight": 9000000,
					"sidechainDifficulty": 102257720
				}
			}"#;
        let priv_api = crate::helper::p2pool::PrivP2poolPoolApi::from_str(data).unwrap();
        let json = serde_json::ser::to_string_pretty(&priv_api).unwrap();
        println!("{}", json);
        let data_after_ser = r#"{
  "pool_statistics": {
    "hashRate": 10225772,
    "miners": 713,
    "sidechainHeight": 9000000,
    "sidechainDifficulty": 102257720
  }
}"#;
        assert_eq!(data_after_ser, json)
    }

    #[test]
    fn serde_priv_xmrig_api() {
        let data = r#"{
		    "id": "6226e3sd0cd1a6es",
		    "worker_id": "hinto",
		    "uptime": 123,
		    "restricted": true,
		    "resources": {
		        "memory": {
		            "free": 123,
		            "total": 123123,
		            "resident_set_memory": 123123123
		        },
		        "load_average": [10.97, 10.58, 10.47],
		        "hardware_concurrency": 12
		    },
		    "features": ["api", "asm", "http", "hwloc", "tls", "opencl", "cuda"],
		    "results": {
		        "diff_current": 123,
		        "shares_good": 123,
		        "shares_total": 123,
		        "avg_time": 123,
		        "avg_time_ms": 123,
		        "hashes_total": 123,
		        "best": [123, 123, 123, 13, 123, 123, 123, 123, 123, 123],
		        "error_log": []
		    },
		    "algo": "rx/0",
		    "connection": {
		        "pool": "localhost:3333",
		        "ip": "127.0.0.1",
		        "uptime": 123,
		        "uptime_ms": 123,
		        "ping": 0,
		        "failures": 0,
		        "tls": null,
		        "tls-fingerprint": null,
		        "algo": "rx/0",
		        "diff": 123,
		        "accepted": 123,
		        "rejected": 123,
		        "avg_time": 123,
		        "avg_time_ms": 123,
		        "hashes_total": 123,
		        "error_log": []
		    },
		    "version": "6.18.0",
		    "kind": "miner",
		    "ua": "XMRig/6.18.0 (Linux x86_64) libuv/2.0.0-dev gcc/10.2.1",
		    "cpu": {
		        "brand": "blah blah blah",
		        "family": 1,
		        "model": 2,
		        "stepping": 0,
		        "proc_info": 123,
		        "aes": true,
		        "avx2": true,
		        "x64": true,
		        "64_bit": true,
		        "l2": 123123,
		        "l3": 123123,
		        "cores": 12,
		        "threads": 24,
		        "packages": 1,
		        "nodes": 1,
		        "backend": "hwloc/2.8.0a1-git",
		        "msr": "ryzen_19h",
		        "assembly": "ryzen",
		        "arch": "x86_64",
		        "flags": ["aes", "vaes", "avx", "avx2", "bmi2", "osxsave", "pdpe1gb", "sse2", "ssse3", "sse4.1", "popcnt", "cat_l3"]
		    },
		    "donate_level": 0,
		    "paused": false,
		    "algorithms": ["cn/1", "cn/2", "cn/r", "cn/fast", "cn/half", "cn/xao", "cn/rto", "cn/rwz", "cn/zls", "cn/double", "cn/ccx", "cn-lite/1", "cn-heavy/0", "cn-heavy/tube", "cn-heavy/xhv", "cn-pico", "cn-pico/tlo", "cn/upx2", "rx/0", "rx/wow", "rx/arq", "rx/graft", "rx/sfx", "rx/keva", "argon2/chukwa", "argon2/chukwav2", "argon2/ninja", "astrobwt", "astrobwt/v2", "ghostrider"],
		    "hashrate": {
		        "total": [111.11, 111.11, 111.11],
		        "highest": 111.11,
		        "threads": [
		            [111.11, 111.11, 111.11]
		        ]
		    },
		    "hugepages": true
		}"#;
        use crate::helper::xrig::xmrig::PrivXmrigApi;
        let priv_api = serde_json::from_str::<PrivXmrigApi>(data).unwrap();
        let json = serde_json::ser::to_string_pretty(&priv_api).unwrap();
        println!("{}", json);
        let data_after_ser = r#"{
  "worker_id": "hinto",
  "resources": {
    "load_average": [
      10.97,
      10.58,
      10.47
    ]
  },
  "connection": {
    "diff": 123,
    "accepted": 123,
    "rejected": 123
  },
  "hashrate": {
    "total": [
      111.11,
      111.11,
      111.11
    ]
  }
}"#;
        assert_eq!(data_after_ser, json)
    }

    use std::path::Path;
    use std::{path::PathBuf, thread};

    use crate::disk::state::P2pool;

    use crate::helper::xvb::public_stats::XvbPubStats;
    use reqwest_middleware::ClientWithMiddleware as Client;

    #[test]
    fn public_api_deserialize() {
        let client = client();
        let new_data = thread::spawn(move || corr(&client)).join().unwrap();
        assert!(!new_data.reward_yearly.is_empty());
    }
    #[tokio::main]
    async fn corr(client: &Client) -> XvbPubStats {
        XvbPubStats::request_api(client).await.unwrap()
    }

    #[test]
    fn custom_args_p2pool() {
        // check that custom args are parsed correctly.
        let arguments = "--wallet 4A5Dwt2qKwKEQrZfo4aBkSNtvDDAzSFbAJcyFkdW5RwDh9U4WgeZrgKT4hUoE2gv8h6NmsNMTyjsEL8eSLMbABds5rYFWnw --host node2.monerodevs.org --rpc-port 18089 --zmq-port 18084 --data-api /home/lm/Téléchargements/gupax-v1.5.4-rc3-linux-x64/p2pool --local-api --no-color --mini --light-mode".to_string();
        let state = P2pool {
            submenu: SubmenuP2pool::Advanced,
            arguments: arguments.clone(),
            ..Default::default()
        };
        let args = Helper::build_p2pool_args(
            &state,
            Path::new(""),
            &Vec::new(),
            false,
            18083,
            18081,
            StartOptionsMode::Custom,
        );
        assert_eq!(
            arguments
                .split(" ")
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
            args
        );
    }

    #[test]
    fn nodes_of_p2pool() {
        use crate::disk::node::Node;
        use crate::helper::p2pool::{P2poolHost, P2poolNodes};
        let nodes = |arguments: &str| {
            P2poolNodes::from_args(&arguments.split(' ').map(String::from).collect::<Vec<_>>())
        };
        let host = |ip: &str, rpc: &str, zmq: &str| P2poolHost {
            node: Node {
                ip: ip.to_string(),
                rpc: rpc.to_string(),
                zmq: zmq.to_string(),
            },
            ..Default::default()
        };
        let backup = "--host 127.0.0.1 --rpc-port 18089 --zmq-port 18084 --host node2.monerodevs.org --rpc-port 18081 --zmq-port 18083";
        assert_eq!(
            nodes(backup).hosts,
            [
                host("127.0.0.1", "18089", "18084"),
                host("node2.monerodevs.org", "18081", "18083")
            ]
        );
        // P2Pool connects to its default node without --host.
        assert_eq!(
            nodes("--mini --rpc-port 18089").hosts,
            [host("127.0.0.1", "18089", "18083")]
        );
        assert_eq!(
            nodes("--rpc-port 18089 --zmq-port 18084 --host 192.168.1.5").hosts,
            [
                host("127.0.0.1", "18089", "18084"),
                host("192.168.1.5", "18089", "18084")
            ]
        );
        let tor = nodes("--socks5 127.0.0.1:9050 --host xyz.onion --rpc-login user:pass --rpc-ssl");
        assert_eq!(
            tor,
            P2poolNodes {
                hosts: vec![P2poolHost {
                    rpc_login: "user:pass".to_string(),
                    rpc_ssl: true,
                    ..host("xyz.onion", "18081", "18083")
                }],
                socks5: "127.0.0.1:9050".to_string(),
            }
        );
    }

    #[test]
    fn arguments_of_a_params_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("p2pool.conf"),
            "# P2Pool parameters\n\
             wallet = 4AAA\n\
             host 192.168.1.5 # node on the LAN\n\
             rpc-login \"user:p\\\"a ss\"\n\
             rpc-ssl\n\
             socks5=127.0.0.1:9050\n",
        )
        .unwrap();
        let args = |arguments: &[&str]| {
            arguments
                .iter()
                .map(|arg| arg.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            Helper::read_params_file(&args(&["--params-file", "p2pool.conf"]), dir.path()),
            args(&[
                "--wallet",
                "4AAA",
                "--host",
                "192.168.1.5",
                "--rpc-login",
                "user:p\"a ss",
                "--rpc-ssl",
                "--socks5",
                "127.0.0.1:9050"
            ])
        );
        assert_eq!(
            Helper::read_params_file(&args(&["--mini"]), dir.path()),
            args(&["--mini"])
        );
    }

    use crate::helper::p2pool::PubP2poolApi;
    use crate::helper::xrig::xmrig::PubXmrigApi;
    use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
    use crate::helper::xvb::PubXvbApi;
    use crate::helper::{
        ProcessSignal, check_died, ended_output, exited, not_alive, output, run_in_folder_of,
        shell_command, signal_end, sleeping, spawn_piped, started, text_lines, within,
    };
    use crate::utils::constants::HORI_CONSOLE;
    use std::io::{BufRead, Read, Write};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[test]
    fn a_process_that_could_not_start_shows_why() {
        let process = not_alive(ProcessName::P2pool);
        process.lock().unwrap().signal = ProcessSignal::Restart;
        let mut gui_output = String::new();
        let start = spawn_piped(Command::new("/missing/p2pool"));
        assert!(started(start, &mut process.lock().unwrap(), &mut gui_output).is_none());
        assert_eq!(process.lock().unwrap().state, ProcessState::Failed);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
        assert!(
            gui_output.contains("P2pool could not start.\nError: /missing/p2pool: "),
            "{gui_output}"
        );
    }

    #[test]
    fn a_binary_at_the_root_runs_in_no_folder() {
        let mut cmd = Command::new("/");
        run_in_folder_of(&mut cmd, Path::new("/"));
        assert_eq!(cmd.get_current_dir(), None);
    }

    // What `cmd` wrote once its output ended.
    fn output_of(cmd: Command) -> String {
        let (mut child, _stdin, mut stdout) = spawn_piped(cmd).unwrap();
        let output = within(move || {
            let mut output = String::new();
            stdout.read_to_string(&mut output).unwrap();
            output
        });
        child.wait().unwrap();
        output
    }

    #[test]
    fn the_output_of_a_process_ends_with_it() {
        assert_eq!(
            output_of(shell_command("echo last", "echo last")).trim_end(),
            "last"
        );
    }

    #[test]
    fn errors_of_a_process_reach_its_output() {
        let cmd = shell_command("echo err 1>&2", "echo err >&2");
        assert_eq!(output_of(cmd).trim_end(), "err");
    }

    fn api() -> Mutex<PubP2poolApi> {
        Mutex::new(PubP2poolApi::new())
    }

    #[test]
    fn a_run_ends_its_console_with_its_name_uptime_and_exit_status() {
        let gui_api = api();
        assert!(check_died(
            &mut exited(0),
            &ended_output(),
            &not_alive(ProcessName::Xmrig),
            &(Instant::now() - Duration::from_secs(61)),
            &gui_api,
            &api(),
        ));
        assert_eq!(
            gui_api.lock().unwrap().output,
            format!(
                "{HORI_CONSOLE}\nXMRig stopped | Uptime: [1 minute, 1 second] | Exit status: [Successful]\n{HORI_CONSOLE}\n\n\n\n\n"
            )
        );
    }

    #[test]
    fn last_lines_of_a_process_come_before_its_exit_status() {
        let process = not_alive(ProcessName::P2pool);
        process.lock().unwrap().state = ProcessState::Alive;
        *process.lock().unwrap().output_pub.lock().unwrap() = "read\n".to_string();
        let (gui_api, pub_api) = (api(), api());
        gui_api.lock().unwrap().output = "start\n".to_string();
        pub_api.lock().unwrap().output = "combined\n".to_string();
        assert!(check_died(
            &mut exited(1),
            &ended_output(),
            &process,
            &Instant::now(),
            &gui_api,
            &pub_api,
        ));
        let gui_output = &gui_api.lock().unwrap().output;
        assert!(
            gui_output.starts_with(&format!(
                "start\ncombined\nread\n{HORI_CONSOLE}\nP2pool stopped | "
            )),
            "{gui_output}"
        );
        assert!(gui_output.contains("Exit status: [Failed]"), "{gui_output}");
        assert_eq!(process.lock().unwrap().state, ProcessState::Failed);
    }

    // A process that its watchdog has to end.
    fn signalled(signal: ProcessSignal) -> Arc<Mutex<Process>> {
        let process = not_alive(ProcessName::P2pool);
        process.lock().unwrap().state = ProcessState::Middle;
        process.lock().unwrap().signal = signal;
        process
    }

    #[test]
    fn a_stopped_process_shows_its_last_lines() {
        let process = signalled(ProcessSignal::Stop);
        *process.lock().unwrap().output_pub.lock().unwrap() = "last\n".to_string();
        let (gui_api, pub_api) = (api(), api());
        pub_api.lock().unwrap().output = "earlier\n".to_string();
        assert!(signal_end(
            &process,
            Some(&mut exited(0)),
            &ended_output(),
            &Instant::now(),
            &gui_api,
            &pub_api,
        ));
        let gui_output = &gui_api.lock().unwrap().output;
        assert!(gui_output.starts_with("earlier\nlast\n"), "{gui_output}");
    }

    // Stores the lines of `stdout` as the reader of a watchdog does, and returns once `until` is
    // stored.
    fn read_until(stdout: Box<dyn Read + Send>, process: &Mutex<Process>, until: &str) {
        let output_pub = Arc::clone(&process.lock().unwrap().output_pub);
        std::thread::spawn({
            let output_pub = Arc::clone(&output_pub);
            move || {
                for line in std::io::BufReader::new(stdout).lines() {
                    output_pub.lock().unwrap().push_str(&(line.unwrap() + "\n"));
                }
            }
        });
        let reading = Instant::now();
        while !output_pub.lock().unwrap().contains(until) {
            assert!(
                reading.elapsed() < Duration::from_secs(5),
                "{until:?} not read"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_stopped_process_shows_a_last_line_cut_before_its_newline() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo first; printf last; exec sleep 30"]);
        let (mut child, _stdin, stdout) = spawn_piped(cmd).unwrap();
        let (stdout, output_end) = output(Box::new(stdout));
        let process = signalled(ProcessSignal::Stop);
        read_until(stdout, &process, "first");
        let gui_api = api();
        assert!(signal_end(
            &process,
            Some(&mut child),
            &output_end,
            &Instant::now(),
            &gui_api,
            &api(),
        ));
        let gui_output = &gui_api.lock().unwrap().output;
        assert!(gui_output.starts_with("first\nlast\n"), "{gui_output}");
    }

    #[test]
    fn lines_written_to_a_log_after_its_end_was_read_are_shown() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("xmrig_output");
        std::fs::write(&log, "first\n").unwrap();
        let (stdout, output_end) = output(Box::new(std::fs::File::open(&log).unwrap()));
        let process = not_alive(ProcessName::P2pool);
        read_until(stdout, &process, "first");
        // As XMRig writes to its macOS log, where a killed XMRig leaves its last line without a
        // newline.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(b"last")
            .unwrap();
        let gui_api = api();
        assert!(check_died(
            &mut exited(0),
            &output_end,
            &process,
            &Instant::now(),
            &gui_api,
            &api(),
        ));
        let gui_output = &gui_api.lock().unwrap().output;
        assert!(gui_output.starts_with("first\nlast\n"), "{gui_output}");
    }

    #[test]
    fn exit_status_waits_for_a_stuck_reader_until_the_limit() {
        let (_stdout, output_end) = output(Box::new(std::io::empty()));
        let waiting = Instant::now();
        within(move || output_end.wait(Duration::from_millis(10)));
        assert!(waiting.elapsed() >= Duration::from_millis(10));
    }

    #[test]
    fn a_stopped_process_stops_successfully() {
        let mut child = sleeping();
        let process = signalled(ProcessSignal::Stop);
        let gui_api = api();
        assert!(signal_end(
            &process,
            Some(&mut child),
            &ended_output(),
            &Instant::now(),
            &gui_api,
            &api(),
        ));
        assert_eq!(process.lock().unwrap().state, ProcessState::Dead);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
        let gui_output = &gui_api.lock().unwrap().output;
        assert!(
            gui_output.contains("Exit status: [Successful]"),
            "{gui_output}"
        );
    }

    #[test]
    fn a_restart_leaves_the_process_waiting_for_its_start() {
        let process = signalled(ProcessSignal::Restart);
        let gui_api = api();
        assert!(signal_end(
            &process,
            Some(&mut exited(0)),
            &ended_output(),
            &Instant::now(),
            &gui_api,
            &api(),
        ));
        assert_eq!(process.lock().unwrap().state, ProcessState::Waiting);
        let gui_output = &gui_api.lock().unwrap().output;
        assert!(gui_output.contains("P2pool stopped | "), "{gui_output}");
    }

    #[test]
    fn a_restart_raised_before_a_process_exited_still_restarts_it() {
        let process = signalled(ProcessSignal::Restart);
        assert!(check_died(
            &mut exited(0),
            &ended_output(),
            &process,
            &Instant::now(),
            &api(),
            &api(),
        ));
        assert_eq!(process.lock().unwrap().state, ProcessState::Waiting);
    }

    #[test]
    fn a_restarting_process_never_shows_as_stopped() {
        let process = signalled(ProcessSignal::Restart);
        let (mut stdout, output_end) = output(Box::new(std::io::empty()));
        // Reads the state while the watchdog waits for the end of the output.
        let reader = std::thread::spawn({
            let process = Arc::clone(&process);
            move || {
                stdout.read_to_end(&mut Vec::new()).unwrap();
                process.lock().unwrap().state
            }
        });
        assert!(signal_end(
            &process,
            Some(&mut exited(0)),
            &output_end,
            &Instant::now(),
            &api(),
            &api(),
        ));
        assert_eq!(reader.join().unwrap(), ProcessState::Middle);
    }

    #[test]
    fn a_raised_signal_leaves_the_process_stopping() {
        let process = not_alive(ProcessName::P2pool);
        process.lock().unwrap().state = ProcessState::Alive;
        Process::raise(&process, ProcessSignal::Stop);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::Stop);
        assert_eq!(process.lock().unwrap().state, ProcessState::Middle);
    }

    #[test]
    fn a_stop_of_a_process_that_exited_leaves_it_exited() {
        let process = not_alive(ProcessName::P2pool);
        process.lock().unwrap().state = ProcessState::Failed;
        Process::raise(&process, ProcessSignal::Stop);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
        assert_eq!(process.lock().unwrap().state, ProcessState::Failed);
    }

    #[test]
    fn a_restart_of_a_process_that_exited_starts_it() {
        let process = not_alive(ProcessName::P2pool);
        Process::raise(&process, ProcessSignal::Restart);
        assert_eq!(process.lock().unwrap().state, ProcessState::Waiting);
    }

    #[test]
    fn a_stop_reaches_xvb_while_its_requests_fail() {
        let process = not_alive(ProcessName::Xvb);
        process.lock().unwrap().state = ProcessState::Failed;
        Process::raise(&process, ProcessSignal::Stop);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::Stop);
        assert_eq!(process.lock().unwrap().state, ProcessState::Middle);
    }

    #[test]
    fn a_restart_of_a_node_gupax_did_not_start_is_left_to_it() {
        let process = not_alive(ProcessName::Node);
        process.lock().unwrap().state = ProcessState::Alive;
        process.lock().unwrap().external = true;
        assert!(!Process::raise(&process, ProcessSignal::Restart));
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
        assert_eq!(process.lock().unwrap().state, ProcessState::Alive);
    }

    #[test]
    fn a_stop_in_progress_keeps_its_state() {
        let process = signalled(ProcessSignal::Stop);
        process
            .lock()
            .unwrap()
            .state
            .set_running(ProcessState::Alive);
        assert_eq!(process.lock().unwrap().state, ProcessState::Middle);
    }

    #[test]
    fn lines_after_a_line_in_another_encoding_are_read() {
        let lines = text_lines(b"data dir C:\\Jos\xe9\r\nlast\n".as_slice())
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(lines, ["data dir C:\\Jos\u{fffd}", "last"]);
    }

    #[test]
    fn xmrig_console_shows_the_lines_left_to_show() {
        let process = not_alive(ProcessName::Xmrig);
        *process.lock().unwrap().output_pub.lock().unwrap() = "to show\n".to_string();
        *process.lock().unwrap().output_parse.lock().unwrap() = "to parse\n".to_string();
        let mut public = PubXmrigApi::new();
        PubXmrigApi::update_from_output(
            &mut public,
            Duration::ZERO,
            &mut process.lock().unwrap(),
            &not_alive(ProcessName::P2pool).lock().unwrap(),
            &not_alive(ProcessName::XmrigProxy).lock().unwrap(),
            &Arc::new(Mutex::new(ImgProxy::new())),
            &Arc::new(Mutex::new(ImgP2pool::new())),
            &XmrigProxy::default(),
            &P2pool::default(),
        );
        assert_eq!(public.output, "to show\n");
        assert!(
            process
                .lock()
                .unwrap()
                .output_parse
                .lock()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn xmrig_proxy_console_shows_the_lines_left_to_show() {
        let process = not_alive(ProcessName::XmrigProxy);
        *process.lock().unwrap().output_pub.lock().unwrap() = "to show\n".to_string();
        *process.lock().unwrap().output_parse.lock().unwrap() = "to parse\n".to_string();
        let mut public = PubXmrigProxyApi::new();
        PubXmrigProxyApi::update_from_output(
            &mut public,
            Duration::ZERO,
            &mut process.lock().unwrap(),
            &not_alive(ProcessName::P2pool).lock().unwrap(),
            &Arc::new(Mutex::new(ImgP2pool::new())),
            &P2pool::default(),
            &XmrigProxy::default(),
        );
        assert_eq!(public.output, "to show\n");
        assert!(
            process
                .lock()
                .unwrap()
                .output_parse
                .lock()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn xmrig_proxy_lines_show_as_text() {
        let output_pub = Arc::new(Mutex::new(String::new()));
        futures::executor::block_on(Helper::read_pty_xp(
            Arc::new(Mutex::new(String::new())),
            Arc::clone(&output_pub),
            std::io::Cursor::new(concat!(
                "\x1b[?25l\x1b[2J\x1b[H\r\n",
                " * ABOUT XMRig-Proxy/6.22\r\n",
                "\x1b[1;37mnew job\x1b[0m from pool\r\n",
                "\x1b[?25h\r\n",
                "end\r\n"
            )),
            not_alive(ProcessName::Xvb),
            &Arc::new(Mutex::new(PubXvbApi::new())),
            not_alive(ProcessName::P2pool),
            &P2pool::default(),
            &Arc::new(Mutex::new(ImgP2pool::new())),
            &XmrigProxy::default(),
        ));
        assert_eq!(
            *output_pub.lock().unwrap(),
            " * ABOUT XMRig-Proxy/6.22\nnew job from pool\nend\n"
        );
    }

    #[test]
    fn a_cancelled_start_leaves_the_process_dead() {
        let process = signalled(ProcessSignal::Stop);
        process.lock().unwrap().cancel_start();
        assert_eq!(process.lock().unwrap().state, ProcessState::Dead);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
    }

    #[test]
    fn a_stop_raised_before_a_start_is_kept() {
        let process = signalled(ProcessSignal::Stop);
        process.lock().unwrap().set_starting();
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::Stop);
    }

    #[test]
    fn a_failing_local_api_is_asked_once() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        std::thread::spawn({
            let requests = Arc::clone(&requests);
            move || {
                for connection in listener.incoming() {
                    requests.fetch_add(1, Ordering::Relaxed);
                    // Closed unanswered.
                    drop(connection);
                }
            }
        });
        let request = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(
                crate::miscs::local_client()
                    .get(format!("http://{address}"))
                    .send(),
            );
        assert!(request.is_err());
        assert_eq!(requests.load(Ordering::Relaxed), 1);
    }
}
