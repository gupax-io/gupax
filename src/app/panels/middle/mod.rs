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

use crate::app::Tab;
use crate::app::eframe_impl::ProcessStatesGui;
use crate::app::keys::KeyPressed;
use crate::components::gupax::FileWindow;
use crate::helper::ProcessName;
use crate::regex::REGEXES;
use crate::utils::constants::*;
use common::state_edit_field::StateTextEdit;
use egui::*;
use log::debug;
mod about;
pub mod common;
mod gupax;
mod node;
mod p2pool;
mod status;
mod xmrig;
mod xmrig_proxy;
mod xvb;
impl crate::app::App {
    #[allow(clippy::too_many_arguments)]
    pub fn middle_panel(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        key: KeyPressed,
        states: &ProcessStatesGui,
    ) {
        // Middle panel, contents of the [Tab]
        debug!("App | Rendering CENTRAL_PANEL (tab contents)");
        CentralPanel::default().show(ctx, |ui| {
            self.size.x = ui.available_width();
            self.size.y = ui.available_height();
            // This sets the Ui dimensions after Top/Bottom are filled
            ui.style_mut().override_text_style = Some(TextStyle::Body);
            match self.tab {
                Tab::About => self.about_show(key, ui),
                Tab::Status => {
                    debug!("App | Entering [Status] Tab");
                    crate::disk::state::Status::show(
                        &mut self.state.status,
                        &self.state.gupax.show_processes,
                        &self.pub_sys,
                        &self.node_api,
                        &self.p2pool_api,
                        &self.xmrig_api,
                        &self.xmrig_proxy_api,
                        &self.xvb_api,
                        &self.p2pool_img,
                        &self.xmrig_img,
                        states,
                        self.max_threads,
                        &self.gupax_p2pool_api,
                        &self.benchmarks,
                        ctx,
                        ui,
                    );
                }
                Tab::Gupax => {
                    debug!("App | Entering [Gupax] Tab");
                    crate::disk::state::Gupax::show(
                        &mut self.state.gupax,
                        &self.og,
                        &self.state_path,
                        &self.update,
                        &self.file_window,
                        &mut self.error_state,
                        &self.restart,
                        frame,
                        ctx,
                        ui,
                        &mut self.must_resize,
                        &self.notifications_api,
                    );
                }
                Tab::Node => {
                    debug!("App | Entering [Node] Tab");
                    crate::disk::state::Node::show(
                        &mut self.state.node,
                        &self.node,
                        &self.node_api,
                        &mut self.node_stdin,
                        &self.file_window,
                        ui,
                    );
                }
                Tab::P2pool => {
                    let (rpc_port, zmq_port) = self.state.node.ports();
                    debug!("App | Entering [P2Pool] Tab");
                    crate::disk::state::P2pool::show(
                        &mut self.state.p2pool,
                        &mut self.node_vec,
                        &self.og,
                        &self.ping,
                        &self.p2pool,
                        &self.p2pool_api,
                        &mut self.p2pool_stdin,
                        ctx,
                        ui,
                        self.backup_hosts.clone(),
                        &self.state.gupax.absolute_p2pool_path,
                        zmq_port,
                        rpc_port,
                        &self.crawler,
                    );
                }
                Tab::Xmrig => {
                    debug!("App | Entering [XMRig] Tab");
                    crate::disk::state::Xmrig::show(
                        &mut self.state.xmrig,
                        &mut self.pool_vec,
                        &self.xmrig,
                        &self.xmrig_api,
                        &mut self.xmrig_stdin,
                        ctx,
                        ui,
                        self.state.p2pool.stratum_port(),
                    );
                }
                Tab::XmrigProxy => {
                    debug!("App | Entering [XMRig-Proxy] Tab");
                    crate::disk::state::XmrigProxy::show(
                        &mut self.state.xmrig_proxy,
                        &self.xmrig_proxy,
                        &mut self.pool_vec,
                        &self.xmrig_proxy_api,
                        &mut self.xmrig_proxy_stdin,
                        ui,
                        self.state.p2pool.stratum_port(),
                        &self.ip_local,
                        &self.ip_public,
                        &self.proxy_port_reachable,
                        &self.helper,
                    );
                }
                Tab::Xvb => {
                    debug!("App | Entering [XvB] Tab");
                    crate::disk::state::Xvb::show(
                        &mut self.state.xvb,
                        &self.state.p2pool.address,
                        ctx,
                        ui,
                        &self.xvb_api,
                        states.is_alive(ProcessName::Xvb),
                    );
                }
            }
        });
    }
}

// Common widgets that will appears on multiple panels.

// header

// console

// sliders in/out peers/log

// menu node

// premade state edit field
// return boolean to know if the field input is validated.
fn rpc_port_field(field: &mut String, ui: &mut Ui) -> bool {
    StateTextEdit::new(ui)
        .description("   RPC PORT ")
        .max_ch(5)
        .help_msg(NODE_API_PORT)
        .validations(&[|x| REGEXES.port.is_match(x)])
        .build(ui, field)
}
fn zmq_port_field(field: &mut String, ui: &mut Ui) -> bool {
    StateTextEdit::new(ui)
        .description("   ZMQ PORT ")
        .max_ch(5)
        .help_msg(NODE_ZMQ_PORT)
        .validations(&[|x| REGEXES.port.is_match(x)])
        .build(ui, field)
}
fn rpc_bind_field(field: &mut String, ui: &mut Ui) -> bool {
    StateTextEdit::new(ui)
        .description("RPC BIND IP ")
        .max_ch(255)
        .help_msg(NODE_API_BIND)
        .validations(&[|x| REGEXES.ipv4.is_match(x), |x| REGEXES.domain.is_match(x)])
        .build(ui, field)
}

fn zmq_bind_field(field: &mut String, ui: &mut Ui) -> bool {
    StateTextEdit::new(ui)
        .description("API BIND IP ")
        .max_ch(255)
        .help_msg(NODE_ZMQ_BIND)
        .validations(&[|x| REGEXES.ipv4.is_match(x), |x| REGEXES.domain.is_match(x)])
        .build(ui, field)
}
