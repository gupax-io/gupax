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

use crate::{
    app::{Benchmark, eframe_impl::ProcessStatesGui, submenu_enum::SubmenuStatus},
    components::gupax::FileWindow,
    disk::{gupax_p2pool_api::GupaxP2poolApi, state::Status},
    helper::{
        ProcessName, ProcessState,
        node::PubNodeApi,
        p2pool::{ImgP2pool, PubP2poolApi},
        sys_info::Sys,
        xrig::{
            xmrig::{ImgXmrig, PubXmrigApi},
            xmrig_proxy::PubXmrigProxyApi,
        },
        xvb::PubXvbApi,
    },
};
use std::sync::{Arc, Mutex};

mod benchmarks;
mod p2pool;
mod processes;

impl Status {
    #[inline(always)] // called once
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        show_process: &[ProcessName],
        sys: &Arc<Mutex<Sys>>,
        node_api: &Arc<Mutex<PubNodeApi>>,
        p2pool_api: &Arc<Mutex<PubP2poolApi>>,
        xmrig_api: &Arc<Mutex<PubXmrigApi>>,
        xmrig_proxy_api: &Arc<Mutex<PubXmrigProxyApi>>,
        xvb_api: &Arc<Mutex<PubXvbApi>>,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
        xmrig_img: &Arc<Mutex<ImgXmrig>>,
        states: &ProcessStatesGui,
        max_threads: u16,
        gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
        benchmarks: &[Benchmark],
        observer: &mut String,
        file_window: &Arc<Mutex<FileWindow>>,
        ui: &mut egui::Ui,
    ) {
        //---------------------------------------------------------------------------------------------------- [Processes]
        if self.submenu == SubmenuStatus::Processes {
            self.processes(
                show_process,
                sys,
                ui,
                node_api,
                p2pool_api,
                p2pool_img,
                xmrig_api,
                xmrig_proxy_api,
                xmrig_img,
                xvb_api,
                max_threads,
                states,
            );
        //---------------------------------------------------------------------------------------------------- [P2Pool]
        } else if self.submenu == SubmenuStatus::P2pool {
            self.p2pool(
                ui,
                gupax_p2pool_api,
                states.find(ProcessName::P2pool).state == ProcessState::Alive,
                p2pool_api,
                observer,
                file_window,
            );
        //---------------------------------------------------------------------------------------------------- [Benchmarks]
        } else if self.submenu == SubmenuStatus::Benchmarks {
            self.benchmarks(
                ui,
                benchmarks,
                states.is_alive(ProcessName::Xmrig),
                xmrig_api,
            )
        }
    }
}
