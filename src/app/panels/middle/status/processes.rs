use egui::{Label, ScrollArea, Ui, Vec2};
use std::sync::{Arc, Mutex};

use crate::app::eframe_impl::ProcessStatesGui;
use crate::disk::state::Status;
use crate::helper::node::PubNodeApi;
use crate::helper::p2pool::{ImgP2pool, PubP2poolApi};
use crate::helper::xrig::xmrig::{ImgXmrig, PubXmrigApi};
use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
use crate::helper::xvb::{PubXvbApi, nodes::Pool};
use crate::helper::{ProcessName, sys_info::Sys};

use crate::constants::*;
use egui::{RichText, TextStyle};
use log::*;
impl Status {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn processes(
        &mut self,
        show_processes: &[ProcessName],
        sys: &Arc<Mutex<Sys>>,
        ui: &mut egui::Ui,
        node_api: &Arc<Mutex<PubNodeApi>>,
        p2pool_api: &Arc<Mutex<PubP2poolApi>>,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        xmrig_api: &Arc<Mutex<PubXmrigApi>>,
        xmrig_proxy_api: &Arc<Mutex<PubXmrigProxyApi>>,
        xmrig_img: &Arc<Mutex<ImgXmrig>>,
        xvb_api: &Arc<Mutex<PubXvbApi>>,
        max_threads: u16,
        states: &ProcessStatesGui,
    ) {
        let width_column = ui.text_style_height(&TextStyle::Body) * 16.0;
        let height_column = width_column * 2.7;
        let size_column = Vec2::new(width_column, height_column);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ScrollArea::vertical()
            .id_salt("vertical_processes")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ScrollArea::horizontal().show(ui, |ui| {
                        column_process(ui, size_column, true, |ui| {
                            gupax(ui, sys);
                        });
                        column_process(
                            ui,
                            size_column,
                            show_processes.contains(&ProcessName::Node),
                            |ui| {
                                node(ui, states.is_alive(ProcessName::Node), node_api);
                            },
                        );
                        column_process(
                            ui,
                            size_column,
                            show_processes.contains(&ProcessName::P2pool),
                            |ui| {
                                p2pool(
                                    ui,
                                    states.is_alive(ProcessName::P2pool),
                                    p2pool_api,
                                    p2pool_img,
                                );
                            },
                        );
                        column_process(
                            ui,
                            size_column,
                            show_processes.contains(&ProcessName::Xmrig),
                            |ui| {
                                xmrig(
                                    ui,
                                    states.is_alive(ProcessName::Xmrig),
                                    xmrig_api,
                                    xmrig_img,
                                    max_threads,
                                );
                            },
                        );
                        column_process(
                            ui,
                            size_column,
                            show_processes.contains(&ProcessName::XmrigProxy),
                            |ui| {
                                xmrig_proxy(
                                    ui,
                                    states.is_alive(ProcessName::XmrigProxy),
                                    xmrig_proxy_api,
                                );
                            },
                        );
                        column_process(
                            ui,
                            size_column,
                            show_processes.contains(&ProcessName::Xvb),
                            |ui| {
                                xvb(ui, states.is_alive(ProcessName::Xvb), xvb_api);
                            },
                        );
                    });
                });
            });
    }
}

pub fn column_process<R>(
    ui: &mut Ui,
    size_column: Vec2,
    visible: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) {
    if visible {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.set_width(size_column.x);
                ui.set_height(size_column.y);
                ui.vertical_centered(|ui| add_contents(ui))
            });
        });
    }
}

fn gupax(ui: &mut Ui, sys: &Arc<Mutex<Sys>>) {
    ui.label(RichText::new("[Gupax]").text_style(TextStyle::Heading))
        .on_hover_text("Gupax is online");
    let sys = sys.lock().unwrap();
    ui.label(RichText::new("Uptime").underline())
        .on_hover_text(STATUS_GUPAX_UPTIME);
    // put some space for uptime so that when seconds appears every minutes, no label is moved.
    ui.add_sized(
        [
            0.0,
            (ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y) * 1.5,
        ],
        Label::new(sys.gupax_uptime.to_string()),
    );
    ui.label(RichText::new("Gupax CPU").underline())
        .on_hover_text(STATUS_GUPAX_CPU_USAGE);
    ui.label(sys.gupax_cpu_usage.to_string());
    ui.label(RichText::new("Gupax Memory").underline())
        .on_hover_text(STATUS_GUPAX_MEMORY_USAGE);
    ui.label(sys.gupax_memory_used_mb.to_string());
    ui.label(RichText::new("System CPU").underline())
        .on_hover_text(STATUS_GUPAX_SYSTEM_CPU_USAGE);
    ui.label(sys.system_cpu_usage.to_string());
    ui.label(RichText::new("System Memory").underline())
        .on_hover_text(STATUS_GUPAX_SYSTEM_MEMORY);
    ui.label(sys.system_memory.to_string());
    ui.label(RichText::new("System CPU Model").underline())
        .on_hover_text(STATUS_GUPAX_SYSTEM_CPU_MODEL);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.label(sys.system_cpu_model.to_string());
    drop(sys);
}

fn p2pool(
    ui: &mut Ui,
    p2pool_alive: bool,
    p2pool_api: &Arc<Mutex<PubP2poolApi>>,
    p2pool_img: &Arc<Mutex<ImgP2pool>>,
) {
    ui.add_enabled_ui(p2pool_alive, |ui| {
        ui.label(RichText::new("[P2Pool]").text_style(TextStyle::Heading))
            .on_hover_text("P2Pool is online")
            .on_disabled_hover_text("P2Pool is offline");
        ui.style_mut().override_text_style = Some(TextStyle::Small);
        let api = p2pool_api.lock().unwrap();
        ui.label(RichText::new("Uptime").underline())
            .on_hover_text(STATUS_P2POOL_UPTIME);
        // put some space for uptime so that when seconds appears every minutes, no label is moved.
        ui.add_sized(
            [
                0.0,
                (ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y) * 1.5,
            ],
            Label::new(api.uptime.display(true)),
        );
        ui.label(RichText::new("Current Shares").underline())
            .on_hover_text(STATUS_P2POOL_CURRENT_SHARES);
        ui.label(api.sidechain_shares.to_string());
        ui.label(RichText::new("Shares Found").underline())
            .on_hover_text(STATUS_P2POOL_SHARES);
        ui.label(
            (if let Some(s) = api.shares_found {
                s.to_string()
            } else {
                UNKNOWN_DATA.to_string()
            })
            .to_string(),
        );
        ui.label(RichText::new("Payouts").underline())
            .on_hover_text(STATUS_P2POOL_PAYOUTS);
        ui.label(format!("Total: {}", api.payouts));
        ui.label(format!(
            "[{:.7}/hour]\n[{:.7}/day]\n[{:.7}/month]",
            api.payouts_hour, api.payouts_day, api.payouts_month
        ));
        ui.label(RichText::new("XMR Mined").underline())
            .on_hover_text(STATUS_P2POOL_XMR);
        ui.label(format!("Total: {:.13} XMR", api.xmr));
        ui.label(format!(
            "[{:.7}/hour]\n[{:.7}/day]\n[{:.7}/month]",
            api.xmr_hour, api.xmr_day, api.xmr_month
        ));
        ui.label(RichText::new("Hashrate (15m/1h/24h)").underline())
            .on_hover_text(STATUS_P2POOL_HASHRATE);
        ui.label(&api.hashrate);
        ui.label(RichText::new("Miners Connected").underline())
            .on_hover_text(STATUS_P2POOL_CONNECTIONS);
        ui.label(format!("{}", api.connections));
        ui.label(RichText::new("Effort").underline())
            .on_hover_text(STATUS_P2POOL_EFFORT);
        ui.label(format!(
            "[Average: {}] [Current: {}]",
            api.average_effort, api.current_effort
        ));
        let img = p2pool_img.lock().unwrap();
        ui.label(RichText::new("Monero Node").underline())
            .on_hover_text(STATUS_P2POOL_MONERO_NODE);
        let text_node = if let Some(node) = &api.current_node {
            format!("IP: [{}]\n[RPC: {}] [ZMQ: {}]", node.ip, node.rpc, node.zmq)
        } else {
            "Not connected to any node".to_string()
        };
        ui.label(text_node);
        ui.label(RichText::new("Sidechain").underline())
            .on_hover_text(STATUS_P2POOL_POOL);
        ui.label(&img.chain);
        ui.label(RichText::new("Address").underline())
            .on_hover_text(STATUS_P2POOL_ADDRESS);
        ui.label(&img.address);
        drop(img);
        drop(api);
    });
}
#[allow(clippy::too_many_arguments)]
fn xmrig_proxy(
    ui: &mut Ui,
    xmrig_proxy_alive: bool,
    xmrig_proxy_api: &Arc<Mutex<PubXmrigProxyApi>>,
) {
    ui.add_enabled_ui(xmrig_proxy_alive, |ui| {
        ui.label(RichText::new("[XMRig-Proxy]").text_style(TextStyle::Heading))
            .on_hover_text("XMRig-Proxy is online")
            .on_disabled_hover_text("XMRig-Proxy is offline");
        let api = xmrig_proxy_api.lock().unwrap();
        ui.label(RichText::new("Uptime").underline())
            .on_hover_text(STATUS_XMRIG_PROXY_UPTIME);
        // put some space for uptime so that when seconds appears every minutes, no label is moved.
        ui.add_sized(
            [
                0.0,
                (ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y) * 1.5,
            ],
            Label::new(api.uptime.display(true)),
        );
        ui.label(RichText::new("Hashrate\n(1m/10m/1h/12h/24h)").underline())
            .on_hover_text(STATUS_XMRIG_PROXY_HASHRATE);
        ui.label(&api.hashrate);
        ui.label(format!(
            "[Accepted: {}]\n[Rejected: {}]",
            api.accepted, api.rejected
        ));
        ui.label(RichText::new("Miners Connected").underline())
            .on_hover_text(STATUS_PROXY_CONNECTIONS);
        ui.label(api.miners.to_string());

        ui.label(RichText::new("Pool").underline())
            .on_hover_text(STATUS_XMRIG_PROXY_POOL);
        ui.label(api.pool.as_ref().unwrap_or(&Pool::Unknown).to_string());
        drop(api);
    });
}
#[allow(clippy::too_many_arguments)]
fn xmrig(
    ui: &mut Ui,
    xmrig_alive: bool,
    xmrig_api: &Arc<Mutex<PubXmrigApi>>,
    xmrig_img: &Arc<Mutex<ImgXmrig>>,
    max_threads: u16,
) {
    debug!("Status Tab | Rendering [XMRig]");
    ui.add_enabled_ui(xmrig_alive, |ui| {
        // ui.set_min_size(min_size);
        ui.label(
            RichText::new("[XMRig]")
                .text_style(TextStyle::Heading),
        )
        .on_hover_text("XMRig is online")
        .on_disabled_hover_text("XMRig is offline");
        let api = xmrig_api.lock().unwrap();
        ui.label(RichText::new("Uptime").underline())
            .on_hover_text(STATUS_XMRIG_UPTIME);
        // put some space for uptime so that when seconds appears every minutes, no label is moved.
        ui.add_sized(
            [
                0.0,
                (ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y) * 1.5,
            ],
            Label::new(api.uptime.display(true)),
        );
        ui.label(api.resources.to_string()).on_hover_text("Load average\nThe current load for your CPU. It should not be above the number of total threads or it could mean that your CPU is currently overloaded and will slow down.\nXmrig should not be the cause for overloading a CPU, but other tasks on your system might.");
        ui.label(
            RichText::new("Hashrate\n(10s/1m/15m)")
                .underline()
                ,
        )
        .on_hover_text(STATUS_XMRIG_HASHRATE);
        ui.label(api.hashrate.to_string());
        ui.label(RichText::new("Difficulty").underline())
            .on_hover_text(STATUS_XMRIG_DIFFICULTY);
        ui.label(api.diff.to_string());
        ui.label(RichText::new("Shares").underline())
            .on_hover_text(STATUS_XMRIG_SHARES);
        ui.label(format!(
            "[Accepted: {}]\n[Rejected: {}]",
            api.accepted, api.rejected
        ));
        ui.label(RichText::new("Pool").underline())
            .on_hover_text(STATUS_XMRIG_POOL);
        ui.label(api.pool.as_ref().unwrap_or(&Pool::Unknown).to_string());
        ui.label(RichText::new("Threads").underline())
            .on_hover_text(STATUS_XMRIG_THREADS);
        ui.label(format!(
            "{}/{}",
            xmrig_img.lock().unwrap().threads,
            max_threads
        ));
        drop(api);
    });
}

fn xvb(ui: &mut Ui, xvb_alive: bool, xvb_api: &Arc<Mutex<PubXvbApi>>) {
    //
    let api = &xvb_api.lock().unwrap().stats_pub;
    // if block height is at 0, the API has not been retrieved correctly.
    let enabled = xvb_alive && api.block_height != 0;
    debug!("Status Tab | Rendering [XvB]");
    ui.add_enabled_ui(enabled, |ui| {
        // for now there is no API ping or /health, so we verify if the field reward_yearly is empty or not.
        // ui.set_min_size(min_size);
        ui.label(RichText::new("[XvB Raffle]").text_style(TextStyle::Heading))
            .on_hover_text("XvB API stats")
            .on_disabled_hover_text("No data received from XvB API");
        // [Round Type]
        ui.label(RichText::new("Round Type").underline())
            .on_hover_text(STATUS_XVB_ROUND_TYPE);
        ui.label(api.round_type.to_string());
        // [Time Remaining]
        ui.label(RichText::new("Round Time Remaining").underline())
            .on_hover_text(STATUS_XVB_TIME_REMAIN);
        ui.label(format!("{} minutes", api.time_remain));
        // Donated Hashrate
        ui.label(RichText::new("Bonus Hashrate").underline())
            .on_hover_text(STATUS_XVB_DONATED_HR);
        ui.label(format!(
            "{}kH/s\n+\n{}kH/s\ndonated by\n{} donors\n with\n{} miners",
            api.bonus_hr, api.donate_hr, api.donate_miners, api.donate_workers
        ));
        // Players
        ui.label(RichText::new("Players").underline())
            .on_hover_text(STATUS_XVB_PLAYERS);
        ui.label(format!(
            "[Registered: {}]\n[Playing: {}]",
            api.players, api.players_round
        ));
        // Winner
        ui.label(RichText::new("Winner").underline())
            .on_hover_text(STATUS_XVB_WINNER);
        ui.label(&api.winner);
        // Share effort
        ui.label(RichText::new("Share Effort").underline())
            .on_hover_text(STATUS_XVB_SHARE);
        ui.label(api.share_effort.to_string());
        // Block reward
        ui.label(RichText::new("Block Reward").underline())
            .on_hover_text(STATUS_XVB_BLOCK_REWARD);
        ui.label(api.block_reward.to_string());
        // reward yearly
        ui.label(RichText::new("Est. Reward (Yearly)").underline())
            .on_hover_text(STATUS_XVB_YEARLY);
        if api.reward_yearly.is_empty() {
            ui.label("No information".to_string());
        } else {
            let text = api
                .rewards()
                .iter()
                .map(|(round, reward)| format!("{round}: {reward} XMR\n"))
                .collect::<Vec<String>>()
                .join("");

            ui.label(text);
        }
    });
}
#[allow(clippy::too_many_arguments)]
fn node(ui: &mut Ui, node_alive: bool, node_api: &Arc<Mutex<PubNodeApi>>) {
    debug!("Status Tab | Rendering [Node]");
    ui.add_enabled_ui(node_alive, |ui| {
        ui.label(RichText::new("[Node]").text_style(TextStyle::Heading))
            .on_hover_text("Node is online")
            .on_disabled_hover_text("Node is offline");
        let api = node_api.lock().unwrap();
        ui.label(RichText::new("Uptime").underline())
            .on_hover_text(STATUS_NODE_UPTIME);
        // put some space for uptime so that when seconds appears every minutes, no label is moved.
        ui.add_sized(
            [
                0.0,
                (ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y) * 1.5,
            ],
            Label::new(api.uptime.display(true)),
        );

        ui.label(RichText::new("Block Height").underline())
            .on_hover_text(STATUS_NODE_BLOCK_HEIGHT);
        ui.label(api.blockheight.to_string());
        ui.label(RichText::new("Network Difficulty").underline())
            .on_hover_text(STATUS_NODE_DIFFICULTY);
        ui.label(api.difficulty.to_string());
        ui.label(RichText::new("Database size").underline())
            .on_hover_text(STATUS_NODE_DB_SIZE);
        ui.label(api.database_size.to_owned());
        ui.label(RichText::new("Free space").underline())
            .on_hover_text(STATUS_NODE_FREESPACE);
        ui.label(api.free_space.to_owned());
        ui.label(RichText::new("Network Type").underline())
            .on_hover_text(STATUS_NODE_NETTYPE);
        ui.label(api.nettype.to_string());
        ui.label(RichText::new("Outgoing peers").underline())
            .on_hover_text(STATUS_NODE_OUT);
        ui.label(api.outgoing_connections.to_string());
        ui.label(RichText::new("Incoming peers").underline())
            .on_hover_text(STATUS_NODE_IN);
        ui.label(api.incoming_connections.to_string());
        ui.label(RichText::new("Synchronized").underline())
            .on_hover_text(STATUS_NODE_SYNC);
        ui.label(api.synchronized.to_string());
        ui.label(RichText::new("Status").underline())
            .on_hover_text(STATUS_NODE_STATUS);
        ui.label(api.status.to_string());
        drop(api);
    });
}
