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

use crate::app::panels::middle::common::list_poolnode::{PoolNode, list_poolnode};
use crate::app::panels::middle::common::state_edit_field::{StateTextEdit, slider_state_field};
use crate::disk::state::P2poolChain;
use crate::miscs::height_txt_before_button;
use crate::utils::i18n::t;
use crate::{disk::state::P2pool, utils::regex::REGEXES};

use crate::constants::*;
use egui::{Button, Checkbox, Ui};
use log::*;

impl P2pool {
    pub(super) fn advanced(&mut self, ui: &mut Ui, node_vec: &mut Vec<(String, PoolNode)>) {
        if !self.arguments.is_empty() {
            ui.disable();
        }
        // let height = size.y / 16.0;
        // let space_h = size.y / 128.0;
        debug!("P2Pool Tab | Rendering [Node List] elements");
        let mut incorrect_input = false; // This will disable [Add/Delete] on bad input
        // [Monero node IP/RPC/ZMQ]
        egui::ScrollArea::horizontal()
            .id_salt("p2pool_horizontal")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            if !self.name_field(ui) {
                                incorrect_input = false;
                            }
                            if !self.ip_field(ui) {
                                incorrect_input = false;
                            }
                            if !self.rpc_port_field(ui) {
                                incorrect_input = false;
                            }
                            if !self.zmq_port_field(ui) {
                                incorrect_input = false;
                            }
                            if !self.stratum_port_field(ui) {
                                incorrect_input = false;
                            }
                        });
                        list_poolnode(
                            ui,
                            &mut (&mut self.name, &mut self.ip, &mut self.rpc, &mut self.zmq),
                            &mut self.selected_node,
                            node_vec,
                            incorrect_input,
                        );
                    });
                });

                debug!("P2Pool Tab | Rendering [Main/Mini/Nano/Peers/Log] elements");
                // [Main/Mini]
                ui.horizontal(|ui| {
                    // let height = height / 4.0;
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            let height =
                                height_txt_before_button(ui, &egui::TextStyle::Button) * 1.9;
                            ui.horizontal(|ui| {
                                let width = ((ui.available_width() / 4.0) - SPACE).max(80.0);
                                if ui
                                    .add_sized(
                                        [width, height],
                                        Button::selectable(
                                            self.chain == P2poolChain::Main,
                                            "P2Pool Main",
                                        ),
                                    )
                                    .on_hover_text(p2pool_main())
                                    .clicked()
                                {
                                    self.chain = P2poolChain::Main;
                                }
                                if ui
                                    .add_sized(
                                        [width, height],
                                        Button::selectable(
                                            self.chain == P2poolChain::Mini,
                                            "P2Pool Mini",
                                        ),
                                    )
                                    .on_hover_text(p2pool_mini())
                                    .clicked()
                                {
                                    self.chain = P2poolChain::Mini;
                                }
                                if ui
                                    .add_sized(
                                        [width, height],
                                        Button::selectable(
                                            self.chain == P2poolChain::Nano,
                                            "P2Pool Nano",
                                        ),
                                    )
                                    .on_hover_text(p2pool_nano())
                                    .clicked()
                                {
                                    self.chain = P2poolChain::Nano;
                                }
                            });
                            debug!("P2Pool Tab | Rendering Backup host button");
                            ui.group(|ui| {
                                // [Backup host]
                                ui.add_sized(
                                    [(ui.available_width() / 2.0) - (SPACE * 2.0), height],
                                    Checkbox::new(&mut self.backup_host, t!("Backup host")),
                                )
                                .on_hover_text(p2pool_backup_host_advanced());
                            });
                        });
                    });
                    // [Out/In Peers] + [Log Level]
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.add_space(SPACE);
                            slider_state_field(
                                ui,
                                t!("Out peers [2-450]:"),
                                p2pool_out(),
                                &mut self.out_peers,
                                2..=450,
                            );
                            ui.add_space(SPACE);
                            slider_state_field(
                                ui,
                                t!("In peers  [2-450]:"),
                                p2pool_in(),
                                &mut self.in_peers,
                                2..=450,
                            );
                            ui.add_space(SPACE);
                            slider_state_field(
                                ui,
                                t!("Log level [ 0-6 ]:"),
                                p2pool_log(),
                                &mut self.log_level,
                                0..=6,
                            );
                        })
                    });
                });
            });
    }
    fn name_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description("   Name     ")
            .max_ch(30)
            .help_msg(p2pool_name())
            .validations(&[|x| REGEXES.name.is_match(x)])
            .build(ui, &mut self.name)
    }
    fn rpc_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description("   RPC PORT ")
            .max_ch(5)
            .help_msg(p2pool_rpc_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.rpc)
    }
    fn zmq_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description("   ZMQ PORT ")
            .max_ch(5)
            .help_msg(p2pool_zmq_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.zmq)
    }
    fn ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description("   IP       ")
            .max_ch(255)
            .help_msg(p2pool_node_ip())
            .validations(&[|x| REGEXES.ipv4.is_match(x), |x| REGEXES.domain.is_match(x)])
            .build(ui, &mut self.ip)
    }

    /// TODO: find a better solution to handle settings that are not String ?
    fn stratum_port_field(&mut self, ui: &mut Ui) -> bool {
        let mut port = self.stratum_port.to_string();
        let valid = StateTextEdit::new(ui)
            .description(t!("STRATUM PORT"))
            .max_ch(5)
            .help_msg(help_stratum_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut port);
        if let Ok(port) = port.parse() {
            self.stratum_port = port;
        }
        valid
    }
}
