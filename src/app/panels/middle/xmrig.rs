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
use crate::app::panels::middle::common::list_poolnode::list_poolnode;
use crate::app::panels::middle::common::state_edit_field::{
    monero_address_field, slider_state_field,
};
use crate::constants::*;
use crate::disk::state::{StartOptionsMode, Xmrig};
use crate::helper::xrig::xmrig::PubXmrigApi;
use crate::helper::{Process, ProcessName};
use crate::miscs::height_txt_before_button;
use crate::regex::REGEXES;
use crate::utils::i18n::{t, t_f};
use egui::{Checkbox, Image, Ui, vec2};
use log::*;

use std::sync::{Arc, Mutex};

use super::common::list_poolnode::PoolNode;
use super::common::state_edit_field::StateTextEdit;

impl Xmrig {
    #[inline(always)] // called once
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        pool_vec: &mut Vec<(String, PoolNode)>,
        process: &Arc<Mutex<Process>>,
        api: &Arc<Mutex<PubXmrigApi>>,
        buffer: &mut String,
        ui: &mut egui::Ui,
        p2pool_stratum_port: u16,
    ) {
        let logo = Some(Image::from_bytes("bytes:/xmrig.png", BYTES_XMRIG));
        header_tab(
            ui,
            logo,
            &[("XMRig", XMRIG_URL, "")],
            Some(t!("High performant miner for RandomX")),
            true,
        );
        debug!("XMRig Tab | Rendering [Console]");
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                let text = &api.lock().unwrap().output;
                console(ui, text, &mut self.console_height, ProcessName::Xmrig);
                if !self.simple {
                    ui.separator();
                    input_args_field(
                        ui,
                        buffer,
                        process,
                        r#"Commands: [h]ashrate, [p]ause, [r]esume, re[s]ults, [c]onnection"#,
                        xmrig_input(),
                    );
                }
            });
            if !self.simple {
                debug!("XMRig Tab | Rendering [Arguments]");
                let default_args_simple =
                    self.start_options(StartOptionsMode::Simple, p2pool_stratum_port);
                let default_args_advanced =
                    self.start_options(StartOptionsMode::Advanced, p2pool_stratum_port);
                start_options_field(
                    ui,
                    &mut self.arguments,
                    &default_args_simple,
                    &default_args_advanced,
                    Self::process_name().start_options_hint(),
                    start_options_hover(),
                );
                ui.add_enabled_ui(self.arguments.is_empty(), |ui| {
                    debug!("XMRig Tab | Rendering [Address]");
                    monero_address_field(&mut self.address, ui, xmrig_address());
                });
            }
            if self.simple {
                ui.add_space(SPACE);
            }
            debug!("XMRig Tab | Rendering [Threads]");
            ui.vertical_centered(|ui| {
                ui.set_max_width(ui.available_width() * 0.75);
                slider_state_field(
                    ui,
                    &t_f!("Threads [1-{}]:", self.max_threads),
                    xmrig_threads(),
                    &mut self.current_threads,
                    1..=self.max_threads,
                );
                #[cfg(not(target_os = "linux"))] // Pause on active isn't supported on Linux
                slider_state_field(
                    ui,
                    t!("Pause on active [0-255]:"),
                    &t_f!("{} [{}] seconds.", xmrig_pause(), self.pause),
                    &mut self.pause,
                    0..=255,
                );
            });
            if !self.simple {
                if !self.arguments.is_empty() {
                    ui.disable();
                }
                egui::ScrollArea::horizontal()
                    .id_salt("xmrig_horizontal")
                    .show(ui, |ui| {
                        debug!("XMRig Tab | Rendering [Pool List] elements");
                        let mut incorrect_input = false; // This will disable [Add/Delete] on bad input
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
                                    if !self.rig_field(ui) {
                                        incorrect_input = false;
                                    }
                                });
                                ui.vertical(|ui| {
                                    list_poolnode(
                                        ui,
                                        &mut (
                                            &mut self.name,
                                            &mut self.ip,
                                            &mut self.port,
                                            &mut self.rig,
                                        ),
                                        &mut self.selected_pool,
                                        pool_vec,
                                        incorrect_input,
                                    );
                                });
                            });
                        });
                        ui.add_space(5.0);
                        debug!("XMRig Tab | Rendering [API] TextEdits");
                        // [HTTP API IP/Port]
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    self.api_ip_field(ui);
                                    self.api_port_field(ui);
                                    self.api_token_field(ui);
                                });
                                ui.separator();
                                debug!("XMRig Tab | Rendering [TLS/Keepalive] buttons");
                                ui.vertical(|ui| {
                                    // TLS/Keepalive
                                    ui.horizontal(|ui| {
                                        let width = (ui.available_width() / 2.0) - 11.0;
                                        let height =
                                            height_txt_before_button(ui, &egui::TextStyle::Button)
                                                * 2.0;
                                        let size = vec2(width, height);
                                        ui.add_sized(
                                            size,
                                            Checkbox::new(&mut self.tls, t!("TLS Connection")),
                                        )
                                        .on_hover_text(xmrig_tls());
                                        ui.separator();
                                        ui.add_sized(
                                            size,
                                            Checkbox::new(&mut self.keepalive, t!("Keepalive")),
                                        )
                                        .on_hover_text(xmrig_keepalive());
                                    });
                                });
                            });
                        });
                    });
            }
        });
    }
    fn name_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" Name      ")
            .max_ch(30)
            .help_msg(xmrig_name())
            .validations(&[|x| REGEXES.name.is_match(x)])
            .build(ui, &mut self.name)
    }
    fn rpc_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" RPC PORT  ")
            .max_ch(5)
            .help_msg(xmrig_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.port)
    }
    fn ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" IP        ")
            .max_ch(255)
            .help_msg(xmrig_ip())
            .validations(&[|x| REGEXES.ipv4.is_match(x) || REGEXES.domain.is_match(x)])
            .build(ui, &mut self.ip)
    }
    fn rig_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" RIG ID    ")
            .max_ch(30)
            .help_msg(xmrig_rig())
            .build(ui, &mut self.rig)
    }
    fn api_ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" API IP    ")
            .max_ch(255)
            .help_msg(xmrig_api_ip())
            .validations(&[|x| REGEXES.ipv4.is_match(x) || REGEXES.domain.is_match(x)])
            .build(ui, &mut self.api_ip)
    }
    fn api_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" API PORT  ")
            .max_ch(5)
            .help_msg(xmrig_api_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.api_port)
    }
    fn api_token_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" API TOKEN ")
            .max_ch(255)
            .help_msg(xmrig_api_token())
            .build(ui, &mut self.token)
    }
}
