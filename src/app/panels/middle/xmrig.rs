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
use egui::{Checkbox, Image, Ui, vec2};
use log::*;

use std::sync::{Arc, Mutex};

use super::common::list_poolnode::PoolNode;
use super::common::state_edit_field::StateTextEdit;

const XMRIG_DAEMON_HTTP: &str = "daemon+http://";
const XMRIG_DAEMON_HTTPS: &str = "daemon+https://";

fn strip_daemon_scheme(ip: &str) -> &str {
    ip.strip_prefix(XMRIG_DAEMON_HTTP)
        .or_else(|| ip.strip_prefix(XMRIG_DAEMON_HTTPS))
        .unwrap_or(ip)
}

fn is_daemon_rpc(ip: &str) -> bool {
    ip.starts_with(XMRIG_DAEMON_HTTP) || ip.starts_with(XMRIG_DAEMON_HTTPS)
}

fn set_daemon_rpc(ip: &mut String, enabled: bool) {
    let host = strip_daemon_scheme(ip).to_string();
    if enabled {
        *ip = format!("{XMRIG_DAEMON_HTTP}{host}");
    } else {
        *ip = host;
    }
}

fn valid_xmrig_endpoint(ip: &str) -> bool {
    let host = strip_daemon_scheme(ip);
    REGEXES.ipv4.is_match(host) || REGEXES.domain.is_match(host)
}

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
            Some("High performant miner for RandomX"),
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
                        XMRIG_INPUT,
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
                    START_OPTIONS_HOVER,
                );
                ui.add_enabled_ui(self.arguments.is_empty(), |ui| {
                    debug!("XMRig Tab | Rendering [Address]");
                    monero_address_field(&mut self.address, ui, XMRIG_ADDRESS);
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
                    &format!("Threads [1-{}]:", self.max_threads),
                    XMRIG_THREADS,
                    &mut self.current_threads,
                    1..=self.max_threads,
                );
                #[cfg(not(target_os = "linux"))] // Pause on active isn't supported on Linux
                slider_state_field(
                    ui,
                    "Pause on active [0-255]:",
                    &format!("{} [{}] seconds.", XMRIG_PAUSE, self.pause),
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
                                    let mut daemon_rpc = is_daemon_rpc(&self.ip);
                                    if ui
                                        .checkbox(&mut daemon_rpc, "Solo mining")
                                        .on_hover_text(
                                            "Connect XMRig directly to a Monero daemon instead of a Stratum pool.",
                                        )
                                        .changed()
                                    {
                                        set_daemon_rpc(&mut self.ip, daemon_rpc);
                                        if daemon_rpc {
                                            self.tls = false;
                                            self.keepalive = false;
                                        }
                                    }

                                    // TLS/Keepalive are pool-specific options. XMRig's
                                    // daemon+http(s) URL scheme selects daemon RPC mode.
                                    ui.add_enabled_ui(!daemon_rpc, |ui| {
                                        ui.horizontal(|ui| {
                                            let width = (ui.available_width() / 2.0) - 11.0;
                                            let height = height_txt_before_button(
                                                ui,
                                                &egui::TextStyle::Button,
                                            ) * 2.0;
                                            let size = vec2(width, height);
                                            ui.add_sized(
                                                size,
                                                Checkbox::new(&mut self.tls, "TLS Connection"),
                                            )
                                            .on_hover_text(XMRIG_TLS);
                                            ui.separator();
                                            ui.add_sized(
                                                size,
                                                Checkbox::new(&mut self.keepalive, "Keepalive"),
                                            )
                                            .on_hover_text(XMRIG_KEEPALIVE);
                                        });
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
            .help_msg(XMRIG_NAME)
            .validations(&[|x| REGEXES.name.is_match(x)])
            .build(ui, &mut self.name)
    }
    fn rpc_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" RPC PORT  ")
            .max_ch(5)
            .help_msg(XMRIG_PORT)
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.port)
    }
    fn ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" IP        ")
            .max_ch(255)
            .help_msg(XMRIG_IP)
            .validations(&[valid_xmrig_endpoint])
            .build(ui, &mut self.ip)
    }
    fn rig_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" RIG ID    ")
            .max_ch(30)
            .help_msg(XMRIG_RIG)
            .build(ui, &mut self.rig)
    }
    fn api_ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" API IP    ")
            .max_ch(255)
            .help_msg(XMRIG_API_IP)
            .validations(&[|x| REGEXES.ipv4.is_match(x) || REGEXES.domain.is_match(x)])
            .build(ui, &mut self.api_ip)
    }
    fn api_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" API PORT  ")
            .max_ch(5)
            .help_msg(XMRIG_API_PORT)
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.api_port)
    }
    fn api_token_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" API TOKEN ")
            .max_ch(255)
            .help_msg(XMRIG_API_TOKEN)
            .build(ui, &mut self.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::Helper;

    #[test]
    fn daemon_rpc_endpoint_round_trip() {
        let mut ip = "127.0.0.1".to_string();
        set_daemon_rpc(&mut ip, true);
        assert_eq!(ip, "daemon+http://127.0.0.1");
        assert!(is_daemon_rpc(&ip));

        set_daemon_rpc(&mut ip, false);
        assert_eq!(ip, "127.0.0.1");
        assert!(!is_daemon_rpc(&ip));
    }

    #[test]
    fn daemon_rpc_endpoint_is_valid() {
        assert!(valid_xmrig_endpoint("daemon+http://127.0.0.1"));
        assert!(valid_xmrig_endpoint("daemon+https://node.example.com"));
        assert!(!valid_xmrig_endpoint("daemon+http://not a host"));
    }

    #[test]
    fn daemon_rpc_scheme_reaches_xmrig_url() {
        let mut state = Xmrig::default();
        state.simple = false;
        state.ip = "daemon+http://127.0.0.1".to_string();
        state.port = "18081".to_string();

        let args = Helper::build_xmrig_args(&state, StartOptionsMode::Advanced, 3333);
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--url" && pair[1] == "daemon+http://127.0.0.1:18081"
        }));
    }
}
