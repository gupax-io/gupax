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

use crate::XVB_MIN_TIME_SEND;
use crate::disk::state::P2poolChain;
use crate::helper::Process;
use crate::helper::p2pool::ImgP2pool;
use crate::helper::xrig::current_api_url_xrig;
use crate::helper::xrig::xmrig::ImgXmrig;
use crate::helper::xrig::xmrig_proxy::ImgProxy;
use crate::helper::xrig::xmrig_proxy::PubXmrigProxyApi;
use crate::helper::xvb::current_controllable_hr;
use crate::miscs::output_console;
use crate::miscs::output_console_without_time;
use crate::utils::constants::BLOCK_PPLNS_WINDOW_MAIN_MAX;
use crate::utils::constants::BLOCK_PPLNS_WINDOW_NANO;
use crate::utils::constants::SECOND_PER_BLOCK_P2POOL_MAIN;
use crate::utils::constants::SECOND_PER_BLOCK_P2POOL_MINI;
use crate::utils::constants::SECOND_PER_BLOCK_P2POOL_NANO;
use crate::utils::constants::XVB_SIDE_MARGIN_1H;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use log::{info, warn};
use reqwest_middleware::ClientWithMiddleware as Client;
use tokio::time::sleep;

use crate::{
    BLOCK_PPLNS_WINDOW_MINI, XVB_ROUND_DONOR_MEGA_MIN_HR, XVB_ROUND_DONOR_MIN_HR,
    XVB_ROUND_DONOR_VIP_MIN_HR, XVB_ROUND_DONOR_WHALE_MIN_HR, XVB_TIME_ALGO,
    helper::{
        p2pool::PubP2poolApi,
        xrig::{update_xmrig_config, xmrig::PubXmrigApi},
        xvb::{nodes::Pool, priv_stats::RuntimeMode},
    },
};

use super::{PubXvbApi, SamplesAverageHour, priv_stats::RuntimeDonationLevel};

const MARGIN_EXTERNAL_HR: f32 = 0.02;
#[allow(clippy::too_many_arguments)]
pub(crate) async fn algorithm(
    client: &Client,
    pub_api: &Arc<Mutex<PubXvbApi>>,
    gui_api_xvb: &Arc<Mutex<PubXvbApi>>,
    gui_api_xmrig: &Arc<Mutex<PubXmrigApi>>,
    gui_api_xp: &Arc<Mutex<PubXmrigProxyApi>>,
    gui_api_p2pool: &Arc<Mutex<PubP2poolApi>>,
    state_p2pool: &crate::disk::state::P2pool,
    share: u32,
    time_donated: &Arc<Mutex<u64>>,
    rig: &str,
    xp_alive: bool,
    p2pool_buffer: i8,
    proxy_img: &Arc<Mutex<ImgProxy>>,
    xmrig_img: &Arc<Mutex<ImgXmrig>>,
    p2pool_img: &Arc<Mutex<ImgP2pool>>,
    p2pool_process: &Arc<Mutex<Process>>,
) {
    let token_xmrig = if xp_alive {
        proxy_img.lock().unwrap().token.clone()
    } else {
        xmrig_img.lock().unwrap().token.clone()
    };
    let mut algorithm = Algorithm::new(
        client,
        pub_api,
        gui_api_xvb,
        gui_api_xmrig,
        gui_api_xp,
        gui_api_p2pool,
        &token_xmrig,
        state_p2pool,
        share,
        time_donated,
        rig,
        xp_alive,
        p2pool_buffer,
        proxy_img,
        xmrig_img,
        p2pool_img,
        p2pool_process,
    );
    algorithm.run().await;
}

#[allow(dead_code)]
pub struct Algorithm<'a> {
    client: &'a Client,
    pub_api: &'a Arc<Mutex<PubXvbApi>>,
    gui_api_xvb: &'a Arc<Mutex<PubXvbApi>>,
    gui_api_xmrig: &'a Arc<Mutex<PubXmrigApi>>,
    gui_api_xp: &'a Arc<Mutex<PubXmrigProxyApi>>,
    gui_api_p2pool: &'a Arc<Mutex<PubP2poolApi>>,
    token_xmrig: &'a str,
    state_p2pool: &'a crate::disk::state::P2pool,
    time_donated: &'a Arc<Mutex<u64>>,
    rig: &'a str,
    xp_alive: bool,
    pub stats: Stats,
    p2pool_img: &'a Arc<Mutex<ImgP2pool>>,
    p2pool_process: &'a Arc<Mutex<Process>>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Stats {
    share: u32,
    hashrate_xmrig: f32,
    pub target_donation_hashrate: f32,
    xvb_24h_avg: f32,
    xvb_1h_avg: f32,
    address: String,
    runtime_mode: RuntimeMode,
    runtime_donation_level: RuntimeDonationLevel,
    // manual slider for p2pool and xvb manual
    runtime_amount: f64,
    p2pool_total_hashrate: f32,
    p2pool_avg_last_hour_hashrate: f32,
    p2pool_external_hashrate: f32,
    share_min_hashrate: f32,
    spareable_hashrate: f32,
    needed_time_xvb: u64,
    api_url: String,
    msg_xmrig_or_xp: String,
}

impl<'a> Algorithm<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: &'a Client,
        pub_api: &'a Arc<Mutex<PubXvbApi>>,
        gui_api_xvb: &'a Arc<Mutex<PubXvbApi>>,
        gui_api_xmrig: &'a Arc<Mutex<PubXmrigApi>>,
        gui_api_xp: &'a Arc<Mutex<PubXmrigProxyApi>>,
        gui_api_p2pool: &'a Arc<Mutex<PubP2poolApi>>,
        token_xmrig: &'a str,
        state_p2pool: &'a crate::disk::state::P2pool,
        share: u32,
        time_donated: &'a Arc<Mutex<u64>>,
        rig: &'a str,
        xp_alive: bool,
        p2pool_buffer: i8,
        proxy_img: &Arc<Mutex<ImgProxy>>,
        xmrig_img: &Arc<Mutex<ImgXmrig>>,
        p2pool_img: &'a Arc<Mutex<ImgP2pool>>,
        p2pool_process: &'a Arc<Mutex<Process>>,
    ) -> Self {
        let use_sidechain_hr = gui_api_xvb.lock().unwrap().use_p2pool_sidechain_hr;
        let hashrate_xmrig = current_controllable_hr(xp_alive, gui_api_xp, gui_api_xmrig);

        let address = state_p2pool.address.clone();

        let runtime_mode = gui_api_xvb.lock().unwrap().stats_priv.runtime_mode.clone();
        let runtime_donation_level = gui_api_xvb
            .lock()
            .unwrap()
            .stats_priv
            .runtime_manual_donation_level
            .clone();
        let runtime_amount = gui_api_xvb.lock().unwrap().stats_priv.runtime_manual_amount;

        let p2pool_total_hashrate = if use_sidechain_hr {
            gui_api_p2pool.lock().unwrap().sidechain_ehr
        } else if gui_api_p2pool.lock().unwrap().hashrate_1h > 0 {
            gui_api_p2pool.lock().unwrap().hashrate_1h as f32
        } else {
            gui_api_p2pool.lock().unwrap().hashrate_15m as f32
        };

        let p2pool_avg_last_hour_hashrate = Self::calc_last_hour_avg_hash_rate(
            &gui_api_xvb.lock().unwrap().p2pool_sent_last_hour_samples,
        );
        let mut p2pool_external_hashrate =
            (p2pool_total_hashrate - p2pool_avg_last_hour_hashrate).max(0.0);
        // do not take into account very small external hashrate as the estimation has a margin of error.
        if p2pool_external_hashrate < (p2pool_total_hashrate * MARGIN_EXTERNAL_HR) {
            p2pool_external_hashrate = 0.0;
        }
        info!(
            "p2pool external hashrate({p2pool_external_hashrate}) = p2ool_total_hashrate({p2pool_total_hashrate}) - p2pool_avg_last_hour_hashrate({p2pool_avg_last_hour_hashrate})"
        );

        let difficulty_p2pool = gui_api_p2pool.lock().unwrap().p2pool_difficulty_u64;
        let pws_dynamic = gui_api_p2pool.lock().unwrap().window_length_blocks;
        let share_min_hashrate = Self::minimum_hashrate_share(
            difficulty_p2pool,
            state_p2pool.chain.clone(),
            p2pool_external_hashrate,
            p2pool_buffer,
            pws_dynamic,
        );

        let spareable_hashrate = hashrate_xmrig - share_min_hashrate;

        let api_url = if xp_alive {
            current_api_url_xrig(true, None, Some(&proxy_img.lock().unwrap()))
        } else {
            current_api_url_xrig(true, Some(&xmrig_img.lock().unwrap()), None)
        };

        let msg_xmrig_or_xp = (if xp_alive { "XMRig-Proxy" } else { "XMRig" }).to_string();
        info!("xp alive: {xp_alive:?}");

        let xvb_24h_avg = pub_api.lock().unwrap().stats_priv.donor_24hr_avg * 1000.0;
        let xvb_1h_avg = pub_api.lock().unwrap().stats_priv.donor_1hr_avg * 1000.0;

        let stats = Stats {
            share,
            hashrate_xmrig,
            xvb_24h_avg,
            xvb_1h_avg,
            address,
            target_donation_hashrate: f32::default(),
            runtime_mode,
            runtime_donation_level,
            runtime_amount,
            p2pool_total_hashrate,
            p2pool_avg_last_hour_hashrate,
            p2pool_external_hashrate,
            share_min_hashrate,
            spareable_hashrate,
            needed_time_xvb: 0,
            api_url,
            msg_xmrig_or_xp,
        };

        let mut new_instance = Self {
            client,
            pub_api,
            gui_api_xvb,
            gui_api_xmrig,
            gui_api_xp,
            gui_api_p2pool,
            token_xmrig,
            state_p2pool,
            time_donated,
            rig,
            xp_alive,
            stats,
            p2pool_img,
            p2pool_process,
        };
        // external XvB HR is taken into account with get_target_donation_hashrate so the needed time is calculating how much time is needed from local sparable HR only
        new_instance.stats.target_donation_hashrate =
            new_instance.get_target_donation_hashrate().max(0.0);
        new_instance.stats.needed_time_xvb = Self::get_needed_time_xvb(
            new_instance.stats.target_donation_hashrate,
            new_instance.stats.hashrate_xmrig,
        );

        new_instance
    }

    fn is_share_fulfilled(&self) -> bool {
        let is_criteria_fulfilled = self.stats.share > 0;

        info!(
            "Algorithm | shares({}) > 0 : {}",
            self.stats.share, is_criteria_fulfilled,
        );

        is_criteria_fulfilled
    }

    fn is_xvb_fulfilled(&self) -> bool {
        let runtime_mode = &self.stats.runtime_mode;
        if *runtime_mode != RuntimeMode::Auto && *runtime_mode != RuntimeMode::ManualDonationLevel {
            info!("Algorithm | not running auto or manual round selection, no fast average");
            return true;
        }
        let target_donation_hashrate = self.stats.target_donation_hashrate;

        // For 1h average, there is 20% margin.
        // Since we calculate the time needed for a exact target and hashrate is a bit variable,
        // The fast mode could get triggered without being useful since being a bit under the 1h target is normal
        // and still respect the 20% margin. Trigger the fast mode when we are under the 20% of one hour hashrate
        let is_criteria_fulfilled = self.stats.xvb_24h_avg >= target_donation_hashrate
            && self.stats.xvb_1h_avg >= target_donation_hashrate * (1.0 - XVB_SIDE_MARGIN_1H);
        info!(
            "Algorithm | xvb_24h_avg({}) > target_donation_hashrate({}) && xvb_1h_avg({}) > target_donation_hashrate({}) : {}",
            self.stats.xvb_24h_avg,
            target_donation_hashrate,
            self.stats.xvb_1h_avg,
            target_donation_hashrate,
            is_criteria_fulfilled
        );
        is_criteria_fulfilled
    }

    async fn target_p2pool_node(&self) {
        let node = Pool::P2pool(self.state_p2pool.current_port(
            self.p2pool_process.lock().unwrap().is_alive(),
            &self.p2pool_img.lock().unwrap(),
        ));
        if self.gui_api_xvb.lock().unwrap().current_pool != Some(node.clone()) {
            info!(
                "Algorithm | request {} to mine on p2pool",
                self.stats.msg_xmrig_or_xp
            );
            if let Err(err) = update_xmrig_config(
                self.client,
                &self.stats.api_url,
                self.token_xmrig,
                &node,
                &self.stats.address,
                self.rig,
            )
            .await
            {
                warn!(
                    "Algorithm | Failed request HTTP API {}",
                    self.stats.msg_xmrig_or_xp
                );
                output_console(
                    &mut self.gui_api_xvb.lock().unwrap().output,
                    &format!(
                        "Failure to update {} config with HTTP API.\nError: {}",
                        self.stats.msg_xmrig_or_xp, err
                    ),
                    crate::helper::ProcessName::Xvb,
                );
            } else {
                info!(
                    "Algorithm | {} mining on p2pool pool",
                    self.stats.msg_xmrig_or_xp
                );
            }
        }
    }

    async fn target_xvb_node(&self) {
        let pool = self.gui_api_xvb.lock().unwrap().stats_priv.pool.clone();

        info!(
            "Algorithm | request {} to mine on XvB on server: ({})",
            self.stats.msg_xmrig_or_xp, pool
        );

        if let Err(err) = update_xmrig_config(
            self.client,
            &self.stats.api_url,
            self.token_xmrig,
            &pool,
            &self.stats.address,
            "",
        )
        .await
        {
            // show to console error about updating xmrig config
            warn!(
                "Algorithm | Failed request HTTP API {}",
                self.stats.msg_xmrig_or_xp
            );
            output_console(
                &mut self.gui_api_xvb.lock().unwrap().output,
                &format!(
                    "Failure to update {} config with HTTP API.\nError: {}",
                    self.stats.msg_xmrig_or_xp, err
                ),
                crate::helper::ProcessName::Xvb,
            );
        } else {
            info!(
                "Algorithm | {} mining on XvB pool",
                self.stats.msg_xmrig_or_xp
            );
        }
    }

    async fn send_all_p2pool(&self) {
        self.target_p2pool_node().await;

        info!(
            "Algorithm | algo sleep for {} seconds while mining on P2pool",
            XVB_MIN_TIME_SEND / 1000
        );
        sleep(Duration::from_millis(XVB_TIME_ALGO)).await;
        let hashrate = current_controllable_hr(self.xp_alive, self.gui_api_xp, self.gui_api_xmrig);
        self.gui_api_xvb
            .lock()
            .unwrap()
            .p2pool_sent_last_hour_samples
            .0
            .push_back(hashrate);
    }

    async fn send_all_xvb(&self) {
        self.target_xvb_node().await;

        info!(
            "Algorithm | algo sleep for {} seconds while mining on XvB",
            XVB_TIME_ALGO / 1000
        );
        sleep(Duration::from_millis(XVB_TIME_ALGO)).await;
        self.gui_api_xvb
            .lock()
            .unwrap()
            .p2pool_sent_last_hour_samples
            .0
            .push_back(0.0);
    }

    async fn sleep_then_update_node_xmrig(&self) {
        info!(
            "Algorithm | algo sleep for {} seconds while mining on P2pool",
            (XVB_TIME_ALGO - self.stats.needed_time_xvb) as f32 / 1000.0
        );
        sleep(Duration::from_millis(
            XVB_TIME_ALGO - self.stats.needed_time_xvb,
        ))
        .await;

        // only update xmrig config if it is actually mining.
        self.target_xvb_node().await;

        // will not quit the process until it is really done.
        // xvb process watch this algo handle to see if process is finished or not.

        info!(
            "Algorithm | algo sleep for {} seconds while mining on XvB",
            self.stats.needed_time_xvb as f32 / 1000.0
        );
        sleep(Duration::from_millis(self.stats.needed_time_xvb)).await;
        // HR could be not the same now as the avg sent the last 10mn, will be replaced later by a better history of HR
        let hashrate = current_controllable_hr(self.xp_alive, self.gui_api_xp, self.gui_api_xmrig);
        let hashes = hashrate
            * ((XVB_TIME_ALGO as f32 - self.stats.needed_time_xvb as f32) / XVB_TIME_ALGO as f32);
        // dbg
        info!("hashes p2pool sample: {hashes}");
        self.gui_api_xvb
            .lock()
            .unwrap()
            .p2pool_sent_last_hour_samples
            .0
            .push_back(hashes);
    }

    pub fn get_target_donation_hashrate(&self) -> f32 {
        match self.stats.runtime_mode {
            RuntimeMode::Auto => self.get_auto_mode_target_donation_hashrate(),
            RuntimeMode::Hero => self.get_hero_mode_target_donation_hashrate(),
            RuntimeMode::ManualXvb => {
                info!(
                    "Algorithm | ManualXvBMode target_donation_hashrate=runtime_amount({}H/s)",
                    self.stats.runtime_amount
                );
                self.stats.runtime_amount as f32
            }
            RuntimeMode::ManualP2pool => {
                let target_donation_hashrate =
                    self.stats.hashrate_xmrig - (self.stats.runtime_amount as f32);

                info!(
                    "Algorithm | ManualP2poolMode target_donation_hashrate({})=hashrate_xmrig({})-runtime_amount({})",
                    target_donation_hashrate, self.stats.hashrate_xmrig, self.stats.runtime_amount
                );

                target_donation_hashrate
            }
            // manual donation level will take into account external HR
            RuntimeMode::ManualDonationLevel => {
                let target_donation_hashrate = self.stats.runtime_donation_level.get_hashrate();

                info!(
                    "Algorithm | ManualDonationLevelMode target_donation_hashrate({})={:#?}.get_hashrate()",
                    target_donation_hashrate, self.stats.runtime_donation_level
                );

                target_donation_hashrate
            }
        }
    }

    fn get_auto_mode_target_donation_hashrate(&self) -> f32 {
        let donation_level = match self.stats.spareable_hashrate {
            x if x > (XVB_ROUND_DONOR_MEGA_MIN_HR as f32) => Some(RuntimeDonationLevel::DonorMega),
            x if x > (XVB_ROUND_DONOR_WHALE_MIN_HR as f32) => {
                Some(RuntimeDonationLevel::DonorWhale)
            }
            x if x > (XVB_ROUND_DONOR_VIP_MIN_HR as f32) => Some(RuntimeDonationLevel::DonorVIP),
            x if x > (XVB_ROUND_DONOR_MIN_HR as f32) => Some(RuntimeDonationLevel::Donor),
            _ => None,
        };

        info!("Algorithm | AutoMode target_donation_level detected ({donation_level:#?})");

        let target_donation_hashrate = if let Some(level) = donation_level {
            level.get_hashrate()
        } else {
            0.0
        };

        info!("Algorithm | AutoMode target_donation_hashrate ({target_donation_hashrate})");

        target_donation_hashrate
    }
    // hero mode, send all spareable hashrate to XvB. the targeted hashrate is the spearable hashrate.
    // XvB fast average needs to be disabled in hero mode, or else the min share HR will never get his needed time.
    fn get_hero_mode_target_donation_hashrate(&self) -> f32 {
        info!(
            "Algorithm | HeroMode target_donation_hashrate=spareable_hashrate({})",
            self.stats.spareable_hashrate
        );

        self.stats.spareable_hashrate
    }

    // push new value into samples before executing this calcul
    fn calc_last_hour_avg_hash_rate(samples: &SamplesAverageHour) -> f32 {
        samples.0.iter().sum::<f32>() / samples.0.len() as f32
    }

    fn minimum_hashrate_share(
        difficulty: u64,
        chain: P2poolChain,
        p2pool_external_hashrate: f32,
        p2pool_buffer: i8,
        pws_dynamic: Option<u64>,
    ) -> f32 {
        let pws;
        let second_per_block = match chain {
            P2poolChain::Main => {
                pws = pws_dynamic.unwrap_or(BLOCK_PPLNS_WINDOW_MAIN_MAX);
                SECOND_PER_BLOCK_P2POOL_MAIN
            }
            P2poolChain::Mini => {
                pws = BLOCK_PPLNS_WINDOW_MINI;
                SECOND_PER_BLOCK_P2POOL_MINI
            }
            P2poolChain::Nano => {
                pws = BLOCK_PPLNS_WINDOW_NANO;
                SECOND_PER_BLOCK_P2POOL_NANO
            }
        };
        let minimum_hr = ((difficulty / (pws * second_per_block)) as f32
            * (1.0 + (p2pool_buffer as f32 / 100.0)))
            - p2pool_external_hashrate;

        info!(
            "Algorithm | (difficulty({difficulty}) / (window pplns blocks({pws}) * seconds per p2pool block({second_per_block})) * (BUFFER 1 + ({p2pool_buffer})) / 100) - outside HR({p2pool_external_hashrate}H/s) = minimum HR({minimum_hr}H/s) to keep a share."
        );

        if minimum_hr.is_sign_negative() {
            info!("Algorithm | if minimum HR is negative, it is 0.");
        }

        minimum_hr.max(0.0)
    }

    async fn fulfill_share(&self) {
        output_console(
            &mut self.gui_api_xvb.lock().unwrap().output,
            "There are no shares in p2pool. Sending all hashrate to p2pool!",
            crate::helper::ProcessName::Xvb,
        );

        info!("Algorithm | There are no shares in p2pool. Sending all hashrate to p2pool!");

        self.send_all_p2pool().await
    }

    async fn fulfill_xvb(&self) {
        output_console(
            &mut self.gui_api_xvb.lock().unwrap().output,
            "XvB average target not achieved. Sending all hashrate to XvB!",
            crate::helper::ProcessName::Xvb,
        );

        info!("Algorithm | XvB average target not achieved. Sending all hashrate to XvB!");

        *self.time_donated.lock().unwrap() = XVB_TIME_ALGO;

        self.send_all_xvb().await
    }

    async fn fulfill_normal_cycles(&mut self) {
        // do not switch pool for a very short time, so mine a minimum on XvB with XVB_MIN_TIME_SEND value
        let needed_time = &mut self.stats.needed_time_xvb;
        if *needed_time > 0 && *needed_time < XVB_MIN_TIME_SEND {
            *needed_time = XVB_MIN_TIME_SEND;
            info!(
                "Algorithm | Needed time: {} to send on XvB is less than minimum time to send, sending the minimum {XVB_MIN_TIME_SEND}s to XvB !",
                self.stats.needed_time_xvb
            );
        }
        output_console(
            &mut self.gui_api_xvb.lock().unwrap().output,
            &format!(
                "There is a share in p2pool and XvB average is achieved. Sending {} seconds to XvB!",
                self.stats.needed_time_xvb as f32 / 1000.0
            ),
            crate::helper::ProcessName::Xvb,
        );

        *self.time_donated.lock().unwrap() = self.stats.needed_time_xvb;

        match self.stats.needed_time_xvb.cmp(&0) {
            std::cmp::Ordering::Equal => {
                info!(
                    "Algorithm | Needed time: {}s to send on XvB, sending all HR to p2pool",
                    self.stats.needed_time_xvb as f32 / 1000.0
                );
                self.send_all_p2pool().await;
            }
            std::cmp::Ordering::Greater => {
                info!(
                    "Algorithm | There is a share in p2pool and XvB average is achieved. Sending  {} seconds to XvB!",
                    self.stats.needed_time_xvb as f32 / 1000.0
                );
                if self.stats.needed_time_xvb < XVB_TIME_ALGO {
                    self.target_p2pool_node().await;
                }
                self.sleep_then_update_node_xmrig().await;
            }
            std::cmp::Ordering::Less => {
                // this should not happen since the needed_time_xvb value is u32 so it can not be negative
            }
        };
    }

    pub async fn run(&mut self) {
        output_console(
            &mut self.gui_api_xvb.lock().unwrap().output,
            "Algorithm of HR distribution started for one minute.",
            crate::helper::ProcessName::Xvb,
        );

        info!("Algorithm | Starting...");
        info!("Algorithm | {:#?}", self.stats);

        let external_p2pool_hr = self.stats.p2pool_external_hashrate;
        if external_p2pool_hr > 0.0 {
            output_console(
                &mut self.gui_api_xvb.lock().unwrap().output,
                &format!(
                    "estimated external HR on P2pool: {:.3}kH/s",
                    external_p2pool_hr / 1000.0
                ),
                crate::helper::ProcessName::Xvb,
            );
        }

        if !self.is_share_fulfilled() {
            self.fulfill_share().await
        } else if !self.is_xvb_fulfilled() {
            self.fulfill_xvb().await
        } else {
            self.fulfill_normal_cycles().await
        }

        output_console_without_time(
            &mut self.gui_api_xvb.lock().unwrap().output,
            "",
            crate::helper::ProcessName::Xvb,
        )
    }
    // time needed to send on XvB get to the targeted doner round
    fn get_needed_time_xvb(target_donation_hashrate: f32, hashrate_xmrig: f32) -> u64 {
        // the ceil() is required since we dont' send half seconds, we take the value above to be sure to not undersent.
        // It specially makes a difference for whales for whom one second can mean a lot of hashrate.
        let needed_time =
            (target_donation_hashrate / hashrate_xmrig) * (XVB_TIME_ALGO as f32).ceil();

        info!(
            "Algorithm | Calculating... needed time for XvB ({needed_time}milis)=(target_donation_hashrate({target_donation_hashrate})/hashrate_xmrig({hashrate_xmrig})/1000)*XVB_TIME_ALGO({XVB_TIME_ALGO})"
        );
        // never go above time of algo
        // it could be the case if manual donation level is set
        needed_time.clamp(0.0, XVB_TIME_ALGO as f32) as u64
    }
}
