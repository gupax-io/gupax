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

use crate::utils::i18n::{t, t_f};
use egui::{Checkbox, TextStyle, Ui, vec2};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

use log::debug;

use crate::app::panels::middle::common::console::{console, input_args_field, start_options_field};
use crate::app::panels::middle::common::header_tab::header_tab;
use crate::app::panels::middle::common::list_poolnode::list_poolnode;
use crate::disk::state::{StartOptionsMode, XmrigProxy};
use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
use crate::helper::{Helper, Process, ProcessName};
use crate::miscs::height_txt_before_button;
use crate::regex::REGEXES;
use crate::utils::constants::ip_not_found;
use crate::{
    SPACE, XMRIG_PROXY_URL, start_options_hover, xmrig_api_ip, xmrig_api_port, xmrig_ip,
    xmrig_keepalive, xmrig_name, xmrig_port, xmrig_proxy_input, xmrig_proxy_redirect, xmrig_rig,
    xmrig_tls,
};

use super::common::list_poolnode::PoolNode;
use super::common::state_edit_field::StateTextEdit;
use super::{help_stratum_ip, help_stratum_port, xmrig_api_token};

impl XmrigProxy {
    #[inline(always)] // called once
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        process: &Arc<Mutex<Process>>,
        pool_vec: &mut Vec<(String, PoolNode)>,
        api: &Arc<Mutex<PubXmrigProxyApi>>,
        buffer: &mut String,
        ui: &mut egui::Ui,
        stratum_port: u16,
        local_ip: &Arc<Mutex<Option<IpAddr>>>,
        public_ip: &Arc<Mutex<Option<Ipv4Addr>>>,
        port_reachable: &Arc<Mutex<bool>>,
        helper: &Arc<Mutex<Helper>>,
    ) {
        header_tab(
            ui,
            None,
            &[("XMRig-Proxy", XMRIG_PROXY_URL, "")],
            Some(t!("High performant proxy for your miners")),
            true,
        );
        // console output for log
        debug!("Xmrig-Proxy Tab | Rendering [Console]");
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                let text = &api.lock().unwrap().output;
                console(ui, text, &mut self.console_height, ProcessName::XmrigProxy);
                //---------------------------------------------------------------------------------------------------- [Advanced] Console
                if !self.simple {
                    ui.separator();
                    input_args_field(
                        ui,
                        buffer,
                        process,
                        r#"Commands: [h]ashrate, [c]onnections, [v]erbose, [w]orkers"#,
                        xmrig_proxy_input(),
                    );
                }
            });
            if !self.simple {
                //---------------------------------------------------------------------------------------------------- Arguments
                debug!("XMRig-Proxy Tab | Rendering [Arguments]");
                let default_args_simple =
                    self.start_options(StartOptionsMode::Simple, stratum_port);
                let default_args_advanced =
                    self.start_options(StartOptionsMode::Advanced, stratum_port);
                start_options_field(
                    ui,
                    &mut self.arguments,
                    &default_args_simple,
                    &default_args_advanced,
                    Self::process_name().start_options_hint(),
                    start_options_hover(),
                );
                if !self.arguments.is_empty() {
                    ui.disable();
                }
                ui.add_space(SPACE);
                // ui.style_mut().spacing.icon_width_inner = width / 45.0;
                // ui.style_mut().spacing.icon_width = width / 35.0;
                // ui.style_mut().spacing.icon_spacing = space_h;
                ui.checkbox(
                    &mut self.redirect_local_xmrig,
                    t!("Auto Redirect local Xmrig to Xmrig-Proxy"),
                )
                .on_hover_text(xmrig_proxy_redirect());

                // idea
                // need to warn the user if local firewall is blocking port
                // need to warn the user if NAT is blocking port
                // need to show local ip address
                // need to show public ip

                debug!("XMRig-Proxy Tab | Rendering [Pool List] elements");
                // let width = ui.available_width() - 10.0;
                let mut incorrect_input = false; // This will disable [Add/Delete] on bad input
                // [Pool IP/Port]
                egui::ScrollArea::horizontal()
                    .id_salt("proxy_horizontal")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.group(|ui| {
                                // let width = width / 10.0;
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
                                            &mut self.p2pool_ip,
                                            &mut self.p2pool_port,
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

                        debug!("XMRig-Proxy Tab | Rendering [API/BIND] TextEdits");
                        // [HTTP API IP/Port]
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    // HTTP API
                                    self.api_ip_field(ui);
                                    self.api_port_field(ui);
                                    self.api_token_field(ui);
                                    self.stratum_ip_field(ui);
                                    self.stratum_port_field(ui);
                                });

                                ui.separator();

                                debug!("XMRig-Proxy Tab | Rendering [TLS/Keepalive] buttons");
                                ui.vertical(|ui| {
                                    // TLS/Keepalive
                                    ui.horizontal(|ui| {
                                        let width = (ui.available_width() / 2.0) - 11.0;
                                        let height =
                                            height_txt_before_button(ui, &TextStyle::Button) * 2.0;
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
        // show instructions to connect miners to the proxy
        ui.add_space(SPACE);
        ui.label(t!("Instructions:"));
        ui.label(t!("To connect a miner to this proxy, start xmrig with the option '-o IP:PORT':"));
        ui.label(t!("If the miner is on the same local network as the proxy, use the local IP.\nIf not, use the public IP. You may have to open xmrig-proxy port in your firewall and or network router."));
        ui.add_space(SPACE);
        // local ip and port
        // public ip (way to hide)
        // example of startup options for xmrig

        // button to refresh public ip address lookup
        // Do not fetch public ip automatically since it could take some time depending on the connection and the user might not want to fetch the dns server.
        if ui.button(t!("refresh ip")).clicked() {
            // fetch ip
            Helper::spawn_ip_fetch(helper);
        }
        ui.label(t!("Local IP: "));
        if let Some(ip) = *local_ip.lock().unwrap() {
            ui.label(ip.to_string());
        } else {
            ui.label(ip_not_found());
        }
        ui.label(t!("Public IP: "));
        if let Some(ip) = *public_ip.lock().unwrap() {
            ui.label(ip.to_string());
        } else {
            ui.label(ip_not_found());
        }
        ui.label(t_f!("PORT of xmrig-proxy: {}", self.bind_port()));

        ui.add_enabled_ui(process.lock().unwrap().is_alive(), |ui| {
            if ui
                .button(t!("check if XMRig-Proxy is accessible outside of the local network"))
                .clicked()
            {
                // check port
                Helper::spawn_proxy_port_reachable(helper, self.bind_port());
            }
        })
        .response
        .on_disabled_hover_text(t!("Start the proxy to enable this button"));
        if *port_reachable.lock().unwrap() {
            ui.label(t!("Proxy port is reachable from outside the local network"));
        } else {
            ui.label(t!("Proxy port is not reachable from outside the local network"));
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
            .description(" PORT      ")
            .max_ch(5)
            .help_msg(xmrig_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.p2pool_port)
    }
    fn ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" IP        ")
            .max_ch(255)
            .help_msg(xmrig_ip())
            .validations(&[|x| REGEXES.ipv4.is_match(x) || REGEXES.domain.is_match(x)])
            .build(ui, &mut self.p2pool_ip)
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
    fn stratum_ip_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" BIND IP   ")
            .max_ch(255)
            .help_msg(help_stratum_ip())
            .validations(&[|x| REGEXES.ipv4.is_match(x) || REGEXES.domain.is_match(x)])
            .build(ui, &mut self.ip)
    }
    fn stratum_port_field(&mut self, ui: &mut Ui) -> bool {
        StateTextEdit::new(ui)
            .description(" BIND PORT ")
            .max_ch(5)
            .help_msg(help_stratum_port())
            .validations(&[|x| REGEXES.port.is_match(x)])
            .build(ui, &mut self.port)
    }
}
