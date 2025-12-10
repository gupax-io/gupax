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

use crate::app::panels::middle::common::console::{console, input_args_field, start_options_field};
use crate::app::panels::middle::common::header_tab::header_tab;
use crate::app::panels::middle::common::state_edit_field::{path_db_field, slider_state_field};
use crate::app::panels::middle::{rpc_bind_field, rpc_port_field, zmq_bind_field, zmq_port_field};
use crate::utils::constants::BYTES_MONERO;
use crate::{
    NODE_DNS_BLOCKLIST, NODE_DNS_CHECKPOINT, NODE_FULL_MEM, NODE_INPUT, NODE_PRUNNING, NODE_URL,
    START_OPTIONS_HOVER,
};
use egui::{Image, TextStyle};
use std::sync::{Arc, Mutex};

use log::debug;

use crate::components::gupax::FileWindow;
use crate::disk::state::{Node, StartOptionsMode};
use crate::helper::node::PubNodeApi;
use crate::helper::{Process, ProcessName};
use crate::{P2POOL_IN, P2POOL_LOG, P2POOL_OUT, SPACE};

impl Node {
    #[inline(always)] // called once
    pub fn show(
        &mut self,
        process: &Arc<Mutex<Process>>,
        api: &Arc<Mutex<PubNodeApi>>,
        buffer: &mut String,
        file_window: &Arc<Mutex<FileWindow>>,
        ui: &mut egui::Ui,
    ) {
        ui.style_mut().override_text_style = Some(TextStyle::Body);
        let logo = Some(Image::from_bytes("bytes:/monero.png", BYTES_MONERO));
        header_tab(
            ui,
            logo,
            &[("Monerod", NODE_URL, "")],
            Some("C++ Monero Node"),
            true,
        );
        // console output for log
        debug!("Node Tab | Rendering [Console]");
        egui::ScrollArea::vertical().show(ui, |ui| {
            let text = &api.lock().unwrap().output;
            ui.group(|ui| {
                console(ui, text, &mut self.console_height, ProcessName::Node);
                if !self.simple {
                    ui.separator();
                    input_args_field(
                        ui,
                        buffer,
                        process,
                        r#"Commands: help, status, set_log <level>, diff"#,
                        NODE_INPUT,
                    );
                }
            });
            //---------------------------------------------------------------------------------------------------- [Advanced] Console
            if !self.simple {
                //---------------------------------------------------------------------------------------------------- Arguments
                debug!("Node Tab | Rendering [Arguments]");
                let default_args_simple = self.start_options(StartOptionsMode::Simple);
                let default_args_advanced = self.start_options(StartOptionsMode::Advanced);
                start_options_field(
                    ui,
                    &mut self.arguments,
                    &default_args_simple,
                    &default_args_advanced,
                    Self::process_name().start_options_hint(),
                    START_OPTIONS_HOVER,
                );
                //---------------------------------------------------------------------------------------------------- Prunned checkbox
                if !self.arguments.is_empty() {
                    ui.disable();
                }
                ui.add_space(SPACE);
                debug!("Node Tab | Rendering DNS  and Prunning buttons");
                ui.horizontal(|ui| {
                    ui.group(|ui| {
                        ui.checkbox(&mut self.pruned, "Prunned")
                            .on_hover_text(NODE_PRUNNING);
                        ui.separator();
                        ui.checkbox(&mut self.dns_blocklist, "DNS blocklist")
                            .on_hover_text(NODE_DNS_BLOCKLIST);
                        ui.separator();
                        ui.checkbox(&mut self.disable_dns_checkpoint, "DNS checkpoint")
                            .on_hover_text(NODE_DNS_CHECKPOINT);
                        ui.separator();
                        ui.checkbox(&mut self.full_memory, "Full memory")
                            .on_hover_text(NODE_FULL_MEM);
                    });
                });

                ui.add_space(SPACE);
                //         // idea
                //         // need to warn the user if local firewall is blocking port
                //         // need to warn the user if NAT is blocking port
                //         // need to show local ip address
                //         // need to show public ip
                ui.horizontal(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                rpc_bind_field(&mut self.api_ip, ui);
                                rpc_port_field(&mut self.api_port, ui);
                                ui.add_space(SPACE);
                                zmq_bind_field(&mut self.zmq_ip, ui);
                                zmq_port_field(&mut self.zmq_port, ui);
                            });
                        });

                        //---------------------------------------------------------------------------------------------------- In/Out peers
                        debug!("Node Tab | Rendering sliders elements");
                        ui.vertical(|ui| {
                            ui.group(|ui| {
                                ui.add_space(SPACE);
                                slider_state_field(
                                    ui,
                                    "Out peers [2-450]:",
                                    P2POOL_OUT,
                                    &mut self.out_peers,
                                    2..=450,
                                );
                                ui.add_space(SPACE);
                                slider_state_field(
                                    ui,
                                    "In peers  [2-450]:",
                                    P2POOL_IN,
                                    &mut self.in_peers,
                                    2..=450,
                                );
                                ui.add_space(SPACE);
                                slider_state_field(
                                    ui,
                                    "Log level [ 0-4 ]:",
                                    P2POOL_LOG,
                                    &mut self.log_level,
                                    0..=6,
                                );
                                ui.add_space(SPACE);
                            });
                        });
                    });
                });
                //---------------------------------------------------------------------------------------------------- DB path
                ui.add_space(SPACE);
                ui.group(|ui| {
                    path_db_field(ui, &mut self.path_db, file_window);
                    let mut guard = file_window.lock().unwrap();
                    if guard.picked_nodedb {
                        self.path_db.clone_from(&guard.nodedb_path);
                        guard.picked_nodedb = false;
                    }
                });
                ui.add_space(SPACE);
            }
        });
    }
}
