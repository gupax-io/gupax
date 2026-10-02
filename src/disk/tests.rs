//---------------------------------------------------------------------------------------------------- TESTS
#[cfg(test)]
mod test {
    use crate::disk::node::Node;
    use crate::disk::pool::Pool;
    use crate::disk::state::State;
    #[test]
    fn state_of_the_previous_release() {
        let state = State::to_string(&State::new()).unwrap();
        let previous = state.replace("observer = \"\"\n", "");
        assert_ne!(previous, state);
        // The version check reads the state before any merge.
        assert!(State::from_str(&previous).is_ok());
    }

    #[test]
    fn serde_default_state() {
        let state = State::new();
        let string = State::to_string(&state).unwrap();
        State::from_str(&string).unwrap();
    }
    #[test]
    fn serde_default_node() {
        let node = Node::new_vec();
        let string = Node::to_string(&node).unwrap();
        Node::from_str_to_vec(&string).unwrap();
    }
    #[test]
    fn serde_default_pool() {
        let pool = Pool::new_vec();
        let string = Pool::to_string(&pool).unwrap();
        Pool::from_str_to_vec(&string).unwrap();
    }

    #[test]
    fn serde_custom_node() {
        let node = r#"
			['Local Monero Node']
			ip = "localhost"
			rpc = "18081"
			zmq = "18083"

			['asdf-_. ._123']
			ip = "localhost"
			rpc = "11"
			zmq = "1234"

			['aaa     bbb']
			ip = "192.168.2.333"
			rpc = "1"
			zmq = "65535"
		"#;
        let node = Node::from_str_to_vec(node).unwrap();
        Node::to_string(&node).unwrap();
    }

    #[test]
    fn serde_custom_pool() {
        let pool = r#"
			['Local P2Pool']
			rig = "Gupax_v1.0.0"
			ip = "localhost"
			port = "3333"

			['aaa xx .. -']
			rig = "Gupax"
			ip = "192.168.22.22"
			port = "1"

			['           a']
			rig = "Gupax_v1.0.0"
			ip = "127.0.0.1"
			port = "65535"
		"#;
        let pool = Pool::from_str_to_vec(pool).unwrap();
        Pool::to_string(&pool).unwrap();
    }

    // Make sure we keep the user's old values that are still
    // valid but discard the ones that don't exist anymore.
    #[test]
    fn merge_state() {
        let bad_state = r#"
			[gupax]
			SETTING_THAT_DOESNT_EXIST_ANYMORE = 123123
			simple = false
			auto_update = true
			auto_p2pool = false
			auto_xmrig = false
			ask_before_quit = true
			save_before_quit = true
			p2pool_path = "p2pool/p2pool"
			xmrig_path = "xmrig/xmrig"
			absolute_p2pool_path = ""
			absolute_xmrig_path = ""
			selected_width = 0
			selected_height = 0
			tab = "About"
			ratio = "Width"

			[p2pool]
			SETTING_THAT_DOESNT_EXIST_ANYMORE = "String"
			simple = true
			mini = true
			auto_ping = true
			auto_select = true
			out_peers = 10
			in_peers = 450
			log_level = 6
			arguments = ""
			address = "44hintoFpuo3ugKfcqJvh5BmrsTRpnTasJmetKC4VXCt6QDtbHVuixdTtsm6Ptp7Y8haXnJ6j8Gj2dra8CKy5ewz7Vi9CYW"
			name = "Local Monero Node"
			ip = "localhost"
			rpc = "18081"
			zmq = "18083"
			selected_index = 0
			selected_name = "Local Monero Node"
			selected_ip = "localhost"
			selected_rpc = "18081"
			selected_zmq = "18083"

			[xmrig]
			SETTING_THAT_DOESNT_EXIST_ANYMORE = true
			simple = true
			pause = 0
			simple_rig = ""
			arguments = ""
			tls = false
			keepalive = false
			max_threads = 32
			current_threads = 16
			address = ""
			api_ip = "localhost"
			api_port = "18088"
			name = "Local P2Pool"
			rig = "Gupax_v1.0.0"
			ip = "localhost"
			port = "3333"
			selected_index = 0
			selected_name = "Local P2Pool"
			selected_rig = "Gupax_v1.0.0"
			selected_ip = "localhost"
			selected_port = "3333"

            [xvb]
            token = ""
			[version]
			gupax = "v1.0.0"
			p2pool = "v2.5"
			xmrig = "v6.18.0"
		"#.to_string();
        let merged_state = State::merge(&bad_state).unwrap();
        let merged_state = State::to_string(&merged_state).unwrap();
        println!("{}", merged_state);
        assert!(merged_state.contains("in_peers = 450"));
        assert!(merged_state.contains("log_level = 6"));
        assert!(!merged_state.contains("SETTING_THAT_DOESNT_EXIST_ANYMORE"));
        assert!(merged_state.contains("44hintoFpuo3ugKfcqJvh5BmrsTRpnTasJmetKC4VXCt6QDtbHVuixdTtsm6Ptp7Y8haXnJ6j8Gj2dra8CKy5ewz7Vi9CYW"));
        assert!(merged_state.contains("backup_host = true"));
    }

    #[test]
    fn create_and_serde_gupax_p2pool_api() {
        use crate::disk::gupax_p2pool_api::GupaxP2poolApi;
        use crate::xmr::AtomicUnit;
        use crate::xmr::PayoutOrd;

        // Create the files.
        let mut api = GupaxP2poolApi::temporary("create_and_serde_gupax_p2pool_api");
        println!("{:#?}", api);

        // Write some fake data.
        api.log        = "NOTICE  2022-01-27 01:30:23.1377 P2Pool You received a payout of 0.000000000001 XMR in block 2642816".to_string();
        api.payout_u64 = 1;
        api.xmr = AtomicUnit::from_u64(2);
        let (date, atomic_unit, height) = PayoutOrd::parse_raw_payout_line(&api.log);
        let block = crate::human::HumanNumber::from_u64(height.unwrap());
        let formatted_log_line = GupaxP2poolApi::format_payout(&date, &atomic_unit, &block);
        GupaxP2poolApi::write_to_all_files(&api, &formatted_log_line).unwrap();
        println!("AFTER WRITE: {:#?}", api);

        // Read
        GupaxP2poolApi::read_all_files_and_update(&mut api).unwrap();
        println!("AFTER READ: {:#?}", api);

        // Assert that the file read mutated the internal struct correctly.
        assert_eq!(api.payout_u64, 1);
        assert_eq!(api.xmr.to_u64(), 2);
        assert!(!api.payout_ord.is_empty());
        assert!(
            api.log
                .contains("2022-01-27 01:30:23.1377 | 0.000000000001 XMR | Block 2,642,816")
        );
        std::fs::remove_dir_all(api.path_log.parent().unwrap()).unwrap();
    }

    #[test]
    fn ports_of_custom_arguments() {
        use crate::app::submenu_enum::SubmenuP2pool;
        use crate::disk::state::{Node as NodeState, P2pool, StartOptionsMode, XmrigProxy};
        use crate::helper::node::ImgNode;

        // The ZMQ RPC port is another socket than the ZMQ publisher P2Pool uses.
        let node = NodeState {
            simple: false,
            arguments:
                "--zmq-pub tcp://127.0.0.1:18084 --rpc-bind-port 18089 --zmq-rpc-bind-port 18082"
                    .to_string(),
            ..NodeState::default()
        };
        assert_eq!(node.ports(), (18089, 18084));
        let img = ImgNode::new(&node, &StartOptionsMode::Custom);
        assert_eq!((img.rpc_port, img.zmq_port), (18089, 18084));
        let node = NodeState {
            simple: false,
            api_port: String::new(),
            zmq_port: "18084".to_string(),
            ..NodeState::default()
        };
        assert_eq!(node.ports().1, 18084);

        let p2pool = P2pool {
            submenu: SubmenuP2pool::Advanced,
            arguments: "--stratum 0.0.0.0:3334 --mini".to_string(),
            ..P2pool::default()
        };
        assert_eq!(p2pool.stratum_port(), 3334);

        let proxy = XmrigProxy {
            simple: false,
            arguments: "--bind 0.0.0.0:3356 --http-host 127.0.0.1 --http-port 18089".to_string(),
            ..XmrigProxy::default()
        };
        assert_eq!((proxy.bind_port(), proxy.api_port()), (3356, 18089));
    }

    #[test]
    fn local_node_ports_in_p2pool_default_arguments() {
        use crate::disk::state::{P2pool, StartOptionsMode};

        let p2pool = P2pool {
            local_node: true,
            ..P2pool::default()
        };
        let backup_nodes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let path = std::path::Path::new("");
        let args =
            p2pool.start_options(path, &backup_nodes, StartOptionsMode::Simple, 18084, 18089);
        assert!(args.contains("--host 127.0.0.1 --rpc-port 18089 --zmq-port 18084"));
    }

    #[test]
    fn remove_orphaned_payouts() {
        use crate::disk::gupax_p2pool_api::GupaxP2poolApi;
        use crate::human::HumanNumber;
        use crate::xmr::PayoutOrd;

        let mut api = GupaxP2poolApi::temporary("remove_orphaned_payouts");
        let add = |api: &mut GupaxP2poolApi, xmr: &str, height: u64| {
            let line = format!(
                "NOTICE  2026-10-01 10:00:00.0000 P2Pool Your wallet 4AAA got a payout of {xmr} XMR in block {height}"
            );
            let (date, atomic_unit, height) = PayoutOrd::parse_raw_payout_line(&line);
            let block = HumanNumber::from_u64(height.unwrap());
            let formatted_log_line = GupaxP2poolApi::format_payout(&date, &atomic_unit, &block);
            api.add_payout(&formatted_log_line, date, atomic_unit, block);
            api.write_to_all_files(&formatted_log_line).unwrap();
        };
        add(&mut api, "0.000496318620", 3500000);
        add(&mut api, "0.000000000002", 3500001);
        assert_eq!(api.xmr.to_u64(), 496318622);
        // Another Gupax adds a payout.
        add(&mut api.clone(), "0.000000000003", 3500002);

        // The first block was orphaned, the other one has no payout.
        assert_eq!(api.remove_payouts(&[3500000, 3400000]).unwrap(), 1);
        assert_eq!(api.remove_payouts(&[3400000]).unwrap(), 0);

        api.read_all_files_and_update().unwrap();
        assert_eq!(api.payout_u64, 2);
        assert_eq!(api.xmr.to_u64(), 5);
        assert_eq!(
            api.log,
            "2026-10-01 10:00:00.0000 | 0.000000000002 XMR | Block 3,500,001\n\
             2026-10-01 10:00:00.0000 | 0.000000000003 XMR | Block 3,500,002\n"
        );
        std::fs::remove_dir_all(api.path_log.parent().unwrap()).unwrap();
    }

    #[test]
    fn merge_synced_payouts() {
        use crate::disk::gupax_p2pool_api::GupaxP2poolApi;
        use crate::human::HumanNumber;
        use crate::xmr::AtomicUnit;

        let mut api = GupaxP2poolApi::temporary("merge_synced_payouts");
        // Payout seen in the P2Pool output.
        let date = "2026-10-01 10:00:00.1234".to_string();
        let atomic_unit = AtomicUnit::from_u64(2);
        let block = HumanNumber::from_u64(3500001);
        let formatted_log_line = GupaxP2poolApi::format_payout(&date, &atomic_unit, &block);
        api.add_payout(&formatted_log_line, date, atomic_unit, block);
        api.write_to_all_files(&formatted_log_line).unwrap();
        assert!(api.has_payout(3500001));

        // The synced payouts include it and an older one.
        let payouts = [
            (
                "2026-10-01 10:00:00.0000".to_string(),
                AtomicUnit::from_u64(2),
                3500001,
            ),
            (
                "2026-09-30 10:00:00.0000".to_string(),
                AtomicUnit::from_u64(5),
                3499000,
            ),
        ];
        assert_eq!(api.merge_payouts(&payouts).unwrap(), 1);
        assert_eq!(api.merge_payouts(&payouts).unwrap(), 0);

        api.read_all_files_and_update().unwrap();
        assert_eq!(api.payout_u64, 2);
        assert_eq!(api.xmr.to_u64(), 7);
        assert_eq!(
            api.log,
            "2026-09-30 10:00:00.0000 | 0.000000000005 XMR | Block 3,499,000\n\
             2026-10-01 10:00:00.1234 | 0.000000000002 XMR | Block 3,500,001\n"
        );
        std::fs::remove_dir_all(api.path_log.parent().unwrap()).unwrap();
    }

    #[test]
    fn convert_hash() {
        use crate::disk::status::Hash;
        let hash = 1.0;
        assert_eq!(Hash::convert(hash, Hash::Hash, Hash::Hash), 1.0);
        assert_eq!(Hash::convert(hash, Hash::Hash, Hash::Kilo), 0.001);
        assert_eq!(Hash::convert(hash, Hash::Hash, Hash::Mega), 0.000_001);
        assert_eq!(Hash::convert(hash, Hash::Hash, Hash::Giga), 0.000_000_001);
        let hash = 1.0;
        assert_eq!(Hash::convert(hash, Hash::Kilo, Hash::Hash), 1_000.0);
        assert_eq!(Hash::convert(hash, Hash::Kilo, Hash::Kilo), 1.0);
        assert_eq!(Hash::convert(hash, Hash::Kilo, Hash::Mega), 0.001);
        assert_eq!(Hash::convert(hash, Hash::Kilo, Hash::Giga), 0.000_001);
        let hash = 1.0;
        assert_eq!(Hash::convert(hash, Hash::Mega, Hash::Hash), 1_000_000.0);
        assert_eq!(Hash::convert(hash, Hash::Mega, Hash::Kilo), 1_000.0);
        assert_eq!(Hash::convert(hash, Hash::Mega, Hash::Mega), 1.0);
        assert_eq!(Hash::convert(hash, Hash::Mega, Hash::Giga), 0.001);
        let hash = 1.0;
        assert_eq!(Hash::convert(hash, Hash::Giga, Hash::Hash), 1_000_000_000.0);
        assert_eq!(Hash::convert(hash, Hash::Giga, Hash::Kilo), 1_000_000.0);
        assert_eq!(Hash::convert(hash, Hash::Giga, Hash::Mega), 1_000.0);
        assert_eq!(Hash::convert(hash, Hash::Giga, Hash::Giga), 1.0);
    }
}
