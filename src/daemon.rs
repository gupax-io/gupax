use std::{io, process::exit, thread::sleep, time::Duration};

use crate::{
    app::{App, AppEgui},
    helper::{Helper, xvb::nodes::Pool},
};

pub fn start_daemon(app: AppEgui) {
    // if the app receives Ctrl+C, make sure to terminate all services
    let app_ctrlc = app.clone();
    ctrlc::set_handler(move || {
        Helper::stop_xvb(&app_ctrlc.inner.lock().helper);
        Helper::stop_xmrig(&app_ctrlc.inner.lock().helper);
        Helper::stop_xp(&app_ctrlc.inner.lock().helper);
        Helper::stop_p2pool(&app_ctrlc.inner.lock().helper);
        Helper::stop_node(&app_ctrlc.inner.lock().helper);
        exit(0);
    })
    .expect("Error setting Ctrl-C handler");
    loop {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => sleep(Duration::from_secs(10)),
            Ok(_) => {
                if input.contains("s") {
                    print_all_services(&app.inner.lock());
                } else {
                    println!("Press s then Enter to print the Status of started services");
                }
            }
            Err(e) => {
                eprintln!("Could not understand input: {e}");
                println!("Press s then Enter to print the Status of started services");
            }
        }
    }
}

fn print_all_services(app: &App) {
    println!("{}", status_gupax(app));
    if app.node.lock().unwrap().is_alive()
        && app
            .state
            .gupax
            .show_processes
            .contains(&crate::helper::ProcessName::Node)
    {
        println!("{}", status_node(app));
    }
    if app.p2pool.lock().unwrap().is_alive()
        && app
            .state
            .gupax
            .show_processes
            .contains(&crate::helper::ProcessName::P2pool)
    {
        println!("{}", status_p2pool(app));
    }
    if app.xmrig.lock().unwrap().is_alive()
        && app
            .state
            .gupax
            .show_processes
            .contains(&crate::helper::ProcessName::Xmrig)
    {
        println!("{}", status_xmrig(app));
    }
    if app.xmrig_proxy.lock().unwrap().is_alive()
        && app
            .state
            .gupax
            .show_processes
            .contains(&crate::helper::ProcessName::XmrigProxy)
    {
        println!("{}", status_xp(app));
    }
    if app.xvb.lock().unwrap().is_alive()
        && app.xvb_api.lock().unwrap().stats_pub.block_height != 0
        && app
            .state
            .gupax
            .show_processes
            .contains(&crate::helper::ProcessName::Xvb)
    {
        println!("{}", status_xvb(app));
    }
}

fn status_gupax(app: &App) -> String {
    let sys = app.pub_sys.lock().unwrap();
    format!(
        "
[Gupax]
Uptime:        {}
Gupax CPU:    {}
Gupax Memory: {}
System CPU:    {}
System Memory: {}
CPU model:     {}
        ",
        sys.gupax_uptime,
        sys.gupax_cpu_usage,
        sys.gupax_memory_used_mb,
        sys.system_cpu_usage,
        sys.system_memory,
        sys.system_cpu_model
    )
}
fn status_node(app: &App) -> String {
    let api = app.node_api.lock().unwrap();
    format!(
        "
[Node]
Uptime:             {}
Block Height:       {}
Network Difficulty: {}
Database size:      {}
Free space:         {}
Network Type:       {}
Outgoing peers:     {}
Incoming peers:     {}
Synchronized:       {}
Status:             {}
        ",
        api.uptime.display(false),
        api.blockheight,
        api.difficulty,
        api.database_size,
        api.free_space,
        api.nettype,
        api.outgoing_connections,
        api.incoming_connections,
        api.synchronized,
        api.status,
    )
}
fn status_p2pool(app: &App) -> String {
    let api = app.p2pool_api.lock().unwrap();
    let img = app.p2pool_img.lock().unwrap();
    let text_node = if let Some(node) = &api.current_node {
        format!("IP: [{}]\n[RPC: {}] [ZMQ: {}]", node.ip, node.rpc, node.zmq)
    } else {
        "Not connected to any node".to_string()
    };
    format!(
        "
[P2Pool]
Uptime:                   {}
Current Shares:           {}
Shares Found:             {}
Payouts:                  {}
XMR Mined:                {:.13}
Hashrate (15m/1h/24h):    \n{}
Miners connected:         {}
Effort (average/current): {}/{}
Monero Node:              \n{}
Sidechain:                {}
Address:                   {}

        ",
        api.uptime.display(false),
        api.sidechain_shares,
        api.shares_found.unwrap_or_default(),
        api.payouts,
        api.xmr,
        api.hashrate,
        api.connections,
        api.average_effort,
        api.current_effort,
        text_node,
        &img.chain,
        &img.address
    )
}
fn status_xmrig(app: &App) -> String {
    let api = app.xmrig_api.lock().unwrap();
    let img = app.xmrig_img.lock().unwrap();
    format!(
        "
[XMRig]
Uptime:                      {}
Load Average:                \n{}
Hashrate (10s/1m/15m):       {}
Difficulty:                  {}
Shares: (accepted/rejected): {}/{}
Pool:                        {}
Threads:                     {}/{}
        ",
        api.uptime.display(false),
        api.resources,
        api.hashrate,
        api.diff,
        api.accepted,
        api.rejected,
        &api.pool.as_ref().unwrap_or(&Pool::Unknown),
        img.threads,
        &app.max_threads
    )
}
fn status_xp(app: &App) -> String {
    let api = app.xmrig_proxy_api.lock().unwrap();
    format!(
        "
[Proxy]
Uptime:                      {}
Hashrate (1m/10m/1h/12h/24h):       {}
Shares: (accepted/rejected): {}/{}
Miners connected:         {}
Pool:                        {}
",
        api.uptime.display(false),
        api.hashrate,
        api.accepted,
        api.rejected,
        api.miners,
        &api.pool.as_ref().unwrap_or(&Pool::Unknown),
    )
}
fn status_xvb(app: &App) -> String {
    let api = app.xvb_api.lock().unwrap();
    let estimations = api
        .stats_pub
        .rewards()
        .iter()
        .map(|(round, reward)| format!("{round}: {reward} XMR\n"))
        .collect::<Vec<String>>()
        .join("");
    format!(
        "
[XvB Raffle]
Round Type:           {}
Round Time Remaining: {}m
Bonus Hashrate:
{}
Players:              {}/{}
Winner:               {}
Share Effort:         {}
Block Reward:         {}
Est. Reward:
{}
        ",
        api.stats_pub.round_type,
        api.stats_pub.time_remain,
        api.stats_pub.bonus_hr,
        api.stats_pub.players,
        api.stats_pub.players_round,
        api.stats_pub.winner,
        api.stats_pub.share_effort,
        api.stats_pub.block_reward,
        estimations
    )
}
