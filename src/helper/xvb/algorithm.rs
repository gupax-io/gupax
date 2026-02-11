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

use std::{
    fmt::{Debug, Display},
    sync::{Arc, Mutex},
    time::Duration,
};

use log::info;
use serde::{Deserialize, Serialize};
use strum::EnumCount;

use crate::{
    constants::{
        BLOCK_PPLNS_WINDOW_MAIN_MAX, BLOCK_PPLNS_WINDOW_MINI, BLOCK_PPLNS_WINDOW_NANO,
        SECOND_PER_BLOCK_P2POOL_MAIN, SECOND_PER_BLOCK_P2POOL_MINI, SECOND_PER_BLOCK_P2POOL_NANO,
        XVB_SIDE_MARGIN_1H, XVB_TIME_ALGO,
    },
    disk::state::{P2poolChain, Xvb},
    helper::{
        ProcessName,
        p2pool::{ImgP2pool, PubP2poolApi},
        xrig::HashrateProvider,
        xvb::{
            PubXvbApi,
            distri_algo::{Distributor, Target},
            nodes::Pool,
            rounds::XvbRound,
        },
    },
    miscs::{output_console, output_console_without_time},
};
/// The Algorithm struct will possess every raw values
/// that are needed to make a decision + configuration
/// from the user.
#[derive(Debug)]
pub struct Algorithm {
    /// PPLNS Window block number
    /// The main chain is dynamic
    pws_dynamic: Option<u64>,
    p2pool_difficulty: u64,
    p2pool_chain: P2poolChain,
    shares: u32,
    current_controllable_hr: f32,
    xvb_24h_avg: f32,
    xvb_1h_avg: f32,
    p2pool_avg_last_hour_hashrate: u64,
    p2pool_local_hashrate: u64,
    p2pool_sidechain_hashrate: u64,
    parameters: AlgoParameters,
}

impl Algorithm {
    pub fn new(
        p2pool_api: Arc<Mutex<PubP2poolApi>>,
        p2pool_img: Arc<Mutex<ImgP2pool>>,
        xvb_api: Arc<Mutex<PubXvbApi>>,
        hashrate_provider: HashrateProvider,
    ) -> Self {
        let pws_dynamic = p2pool_api.lock().unwrap().window_length_blocks;
        let p2pool_difficulty = p2pool_api.lock().unwrap().p2pool_difficulty_u64;
        let shares = p2pool_api.lock().unwrap().sidechain_shares;
        let p2pool_avg_last_hour_hashrate = xvb_api
            .lock()
            .unwrap()
            .p2pool_sent_last_hour_samples
            .average() as u64;
        let p2pool_local_hashrate = p2pool_api.lock().unwrap().hashrate_15m;
        let p2pool_sidechain_hashrate = p2pool_api.lock().unwrap().sidechain_ehr as u64;
        let xvb_24h_avg = xvb_api.lock().unwrap().stats_priv.donor_24hr_avg;
        let xvb_1h_avg = xvb_api.lock().unwrap().stats_priv.donor_1hr_avg;
        let algo_cfg = xvb_api.lock().unwrap().algo_config.clone();
        let runtime_mode = xvb_api.lock().unwrap().runtime_mode.clone();
        let p2pool_chain =
            P2poolChain::try_from(p2pool_img.lock().unwrap().chain.clone()).unwrap_or_default();
        Self {
            pws_dynamic,
            p2pool_difficulty,
            p2pool_chain,
            shares,
            current_controllable_hr: hashrate_provider.current_controllable_hr(),
            xvb_24h_avg,
            xvb_1h_avg,
            p2pool_avg_last_hour_hashrate,
            p2pool_local_hashrate,
            p2pool_sidechain_hashrate,
            parameters: AlgoParameters {
                config: algo_cfg,
                envs: AlgoEnvs::new(p2pool_img, xvb_api, hashrate_provider),
                runtime_mode,
            },
        }
    }

    pub async fn run(&self, gui_api_xvb: &Arc<Mutex<PubXvbApi>>) {
        output_console_without_time(
            &mut gui_api_xvb.lock().unwrap().output,
            &format!(
                "\nAlgorithm of HR distribution started for {}s",
                self.parameters.config.timeframe.as_secs()
            ),
            ProcessName::Xvb,
        );
        let decision = self.construct_decision();
        output_console(
            &mut gui_api_xvb.lock().unwrap().output,
            &self.decision_cause_msg(decision.2),
            ProcessName::Xvb,
        );
        let distributor = Distributor {
            fallback_pool: decision.0,
            pools_with_target: if let Some(target) = decision.1 {
                vec![target]
            } else {
                vec![]
            },
            hashrate_provider: self.parameters.envs.hashrate_provider.clone(),
            timeframe: self.parameters.config.timeframe,
        };
        info!("Run of a decision of the distribution algorithm");

        if let Err(e) = distributor.run_decision(gui_api_xvb).await {
            output_console(
                &mut gui_api_xvb.lock().unwrap().output,
                &format!("Error while updating the configuration of the hashrate provider: {e}"),
                ProcessName::Xvb,
            );
        }
    }
    fn decision_cause_msg(&self, cause: DecisionCause) -> String {
        match cause {
            DecisionCause::NotEnoughShares => format!(
                "Your address on the {} P2Pool chain does not have the minimum required shares: {} instead of at least {}",
                self.p2pool_chain, self.shares, self.parameters.config.min_share
            ),
            DecisionCause::TargetNotAchieved(target) => format!(
                "Your target average ({}H/s) is not reached. Trying to catch up by sending all the hashrate to {} so you can reach the target average faster",
                target.target_hr, target.pool
            ),
            DecisionCause::NotEnoughSparableHashrate => format!(
                "You do not have enough sparable hashrate ({}H/s). Sending only on {}",
                self.sparable_hr(),
                self.parameters.envs.p2pool_pool
            ),
            DecisionCause::Normal => {
                "You have enough hashrate to maintain the target average".to_string()
            }
        }
    }
}

#[derive(Debug)]
struct AlgoParameters {
    runtime_mode: XvbMode,
    config: AlgoConfig,
    envs: AlgoEnvs,
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AlgoConfig {
    pub min_share: u32,
    pub p2pool_watch_sidechain: bool,
    pub timeframe: Duration,
    pub p2pool_buffer: i8,
    pub catch_up: bool,
}

impl Default for AlgoConfig {
    fn default() -> Self {
        Self {
            min_share: 1,
            p2pool_watch_sidechain: false,
            timeframe: Duration::from_millis(XVB_TIME_ALGO),
            p2pool_buffer: 25,
            catch_up: true,
        }
    }
}

struct AlgoEnvs {
    p2pool_pool: Pool,
    xvb_pool: Pool,
    hashrate_provider: HashrateProvider,
}

impl AlgoEnvs {
    fn new(
        p2pool_img: Arc<Mutex<ImgP2pool>>,
        xvb_api: Arc<Mutex<PubXvbApi>>,
        hashrate_provider: HashrateProvider,
    ) -> Self {
        let p2pool_pool = Pool::P2pool(p2pool_img.lock().unwrap().stratum_port);
        let xvb_pool = xvb_api.lock().unwrap().stats_priv.pool.clone();
        Self {
            p2pool_pool,
            xvb_pool,
            hashrate_provider,
        }
    }
}

impl Debug for AlgoEnvs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Configuration data")
            .field("P2Pool Pool", &self.p2pool_pool)
            .field("XvB Pool", &self.xvb_pool)
            .field("Hashrate provider", &self.hashrate_provider.to_string())
            .finish()
    }
}

impl Algorithm {
    /// Takes the value, calculate the decision, and return a
    /// tuple of fallback pool and a target that can be used
    /// to build a distributor.
    pub fn construct_decision(&self) -> (Pool, Option<Target>, DecisionCause) {
        if self.shares < self.parameters.config.min_share {
            return (
                self.parameters.envs.p2pool_pool.clone(),
                None,
                DecisionCause::NotEnoughShares,
            );
        }

        let decision = match &self.parameters.runtime_mode {
            XvbMode::Auto => self.mode_auto(),
            XvbMode::Hero => self.mode_hero(),
            XvbMode::ManualDonationLevel(round) => self.mode_round(&round.to_round()),
            XvbMode::ManualP2pool(hr) => self.mode_p2pool(*hr),
            XvbMode::ManualXvb(hr) => self.mode_xvb(*hr),
        };
        if self.parameters.config.catch_up
            && let Some(target) = &decision.1
            && self.fast_mode(target)
        {
            return (
                self.parameters.envs.xvb_pool.clone(),
                None,
                DecisionCause::TargetNotAchieved(target.clone()),
            );
        }
        decision
    }
    fn mode_auto(&self) -> (Pool, Option<Target>, DecisionCause) {
        let mega = XvbRound::XVB_ROUND_DONOR_MEGA_MIN_HR as f32;
        let whale = XvbRound::XVB_ROUND_DONOR_WHALE_MIN_HR as f32;
        let vip_donor = XvbRound::XVB_ROUND_DONOR_VIP_MIN_HR as f32;
        let donor = XvbRound::XVB_ROUND_DONOR_MIN_HR as f32;
        let donation_level = match self.sparable_hr() {
            x if x > mega => mega,
            x if x > whale => whale,
            x if x > vip_donor => vip_donor,
            x if x > donor => donor,
            _ => 0.0,
        };
        if donation_level > 0.0 {
            (
                self.parameters.envs.p2pool_pool.clone(),
                Some(Target {
                    pool: self.parameters.envs.xvb_pool.clone(),
                    target_hr: donation_level,
                }),
                DecisionCause::Normal,
            )
        } else {
            (
                self.parameters.envs.p2pool_pool.clone(),
                None,
                DecisionCause::NotEnoughSparableHashrate,
            )
        }
    }
    fn mode_hero(&self) -> (Pool, Option<Target>, DecisionCause) {
        let target_hr = self.sparable_hr();
        (
            self.parameters.envs.p2pool_pool.clone(),
            Some(Target {
                pool: self.parameters.envs.xvb_pool.clone(),
                target_hr,
            }),
            DecisionCause::Normal,
        )
    }
    fn mode_round(&self, round: &XvbRound) -> (Pool, Option<Target>, DecisionCause) {
        (
            self.parameters.envs.p2pool_pool.clone(),
            Some(Target {
                pool: self.parameters.envs.xvb_pool.clone(),
                target_hr: round.get_hashrate(),
            }),
            DecisionCause::Normal,
        )
    }
    fn mode_p2pool(&self, hashrate: f32) -> (Pool, Option<Target>, DecisionCause) {
        (
            self.parameters.envs.xvb_pool.clone(),
            Some(Target {
                pool: self.parameters.envs.p2pool_pool.clone(),
                target_hr: hashrate,
            }),
            DecisionCause::Normal,
        )
    }
    fn mode_xvb(&self, hashrate: f32) -> (Pool, Option<Target>, DecisionCause) {
        (
            self.parameters.envs.p2pool_pool.clone(),
            Some(Target {
                pool: self.parameters.envs.xvb_pool.clone(),
                target_hr: hashrate,
            }),
            DecisionCause::Normal,
        )
    }

    /// Estimated P2Pool HR for your xmr address
    fn estimate_p2pool_total_hr(&self) -> u64 {
        if self.parameters.config.p2pool_watch_sidechain {
            self.p2pool_sidechain_hashrate
        } else {
            self.p2pool_local_hashrate
        }
    }

    fn estimate_external_p2pool_hr(&self) -> u64 {
        let mut p2pool_external_hashrate =
            self.estimate_p2pool_total_hr() - self.p2pool_avg_last_hour_hashrate;
        // do not take into account very small external hashrate as the estimation has a margin of error.
        if (p2pool_external_hashrate as f32)
            < (self.estimate_p2pool_total_hr() as f32 * Self::MARGIN_EXTERNAL_HR)
        {
            p2pool_external_hashrate = 0;
        }
        p2pool_external_hashrate
    }

    /// Includes the p2pool buffer
    fn estimate_share_minimum_required_hr(&self) -> f32 {
        let pws;
        let second_per_block = match self.p2pool_chain {
            P2poolChain::Main => {
                pws = self.pws_dynamic.unwrap_or(BLOCK_PPLNS_WINDOW_MAIN_MAX);
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
        let minimum_hr = ((self.p2pool_difficulty / (pws * second_per_block)) as f32
            * (1.0 + (self.parameters.config.p2pool_buffer as f32 / 100.0)))
            - self.estimate_external_p2pool_hr() as f32;
        minimum_hr.max(0.0)
    }
    /// HR that would rest if the minimum was given to p2pool
    fn sparable_hr(&self) -> f32 {
        self.current_controllable_hr - self.estimate_share_minimum_required_hr()
    }

    fn fast_mode(&self, target: &Target) -> bool {
        if self.xvb_24h_avg < target.target_hr
            || self.xvb_1h_avg < target.target_hr * (1.0 - XVB_SIDE_MARGIN_1H)
        {
            return true;
        }
        false
    }
    const MARGIN_EXTERNAL_HR: f32 = 0.02;
}
#[derive(Debug)]
pub enum DecisionCause {
    NotEnoughShares,
    TargetNotAchieved(Target),
    NotEnoughSparableHashrate,
    Normal,
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize, Default)]
pub enum ManualDonationMetric {
    #[default]
    Hash,
    Kilo,
    Mega,
}

impl ManualDonationMetric {
    pub fn coeff(&self) -> f32 {
        match self {
            Self::Hash => 1.0,
            Self::Kilo => 1000.0,
            Self::Mega => 1_000_000.0,
        }
    }
}

impl Display for ManualDonationMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Hash => "H/s",
            Self::Kilo => "KH/s",
            Self::Mega => "MH/s",
        };

        write!(f, "{text}")
    }
}
#[derive(Debug, Clone, Deserialize, PartialEq, Default, Serialize, EnumCount)]
pub enum XvbMode {
    #[default]
    Auto,
    ManualXvb(f32),
    ManualP2pool(f32),
    Hero,
    /// Should not include VIP nor MVP
    ManualDonationLevel(ManualDonationLevel),
}

impl From<&Xvb> for XvbMode {
    fn from(state: &Xvb) -> Self {
        if state.simple {
            if state.simple_hero_mode {
                return XvbMode::Hero;
            }
            return XvbMode::Auto;
        }

        match state.mode {
            XvbModeChoice::Auto => XvbMode::Auto,
            XvbModeChoice::Hero => XvbMode::Hero,
            XvbModeChoice::ManualXvb => XvbMode::ManualXvb(
                state.manual_xvb_slider_amount * state.manual_xvb_donation_metric.coeff(),
            ),
            XvbModeChoice::ManualP2pool => XvbMode::ManualP2pool(
                state.manual_p2pool_slider_amount * state.manual_p2pool_donation_metric.coeff(),
            ),
            XvbModeChoice::ManualDonationLevel => {
                XvbMode::ManualDonationLevel(state.manual_donation_level.clone())
            }
        }
    }
}

/// We need to save values for each choice
#[derive(Debug, Clone, Deserialize, PartialEq, Default, Serialize, EnumCount)]
pub enum XvbModeChoice {
    #[default]
    Auto,
    Hero,
    ManualXvb,
    ManualP2pool,
    ManualDonationLevel,
}

impl Display for XvbModeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Auto => "Auto",
            Self::Hero => "Hero",
            Self::ManualXvb => "Manual Xvb",
            Self::ManualP2pool => "Manual P2pool",
            Self::ManualDonationLevel => "Manual Donation Level",
        };

        write!(f, "{text}")
    }
}

impl Display for XvbMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Auto => "Auto".to_string(),
            Self::Hero => "Hero".to_string(),
            Self::ManualXvb(amount) => format!("Manual Xvb: {amount}HR/s"),
            Self::ManualP2pool(amount) => format!("Manual P2pool {amount}HR/s"),
            Self::ManualDonationLevel(round) => format!("Manual Donation Level: {round}"),
        };

        write!(f, "{text}")
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize, Default)]
pub enum ManualDonationLevel {
    #[default]
    Donor,
    DonorVIP,
    DonorWhale,
    DonorMega,
}
impl Display for ManualDonationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Donor => "Donor",
            Self::DonorVIP => "Donor VIP",
            Self::DonorWhale => "Donor Whale",
            Self::DonorMega => "Donor Mega",
        };

        write!(f, "{text}")
    }
}
impl ManualDonationLevel {
    fn to_round(&self) -> XvbRound {
        match self {
            Self::Donor => XvbRound::Donor,
            Self::DonorVIP => XvbRound::DonorVip,
            Self::DonorWhale => XvbRound::DonorWhale,
            Self::DonorMega => XvbRound::DonorMega,
        }
    }
}
#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use crate::{
        helper::{
            p2pool::{ImgP2pool, PubP2poolApi},
            xrig::xmrig::{ImgXmrig, PubXmrigApi},
            xvb::{
                PubXvbApi,
                algorithm::{Algorithm, XvbMode},
                nodes::Pool,
            },
        },
        miscs::client,
    };

    #[test]
    fn test_manual_xvb_mode() {
        let client = client();
        let gui_api_xvb = Arc::new(Mutex::new(PubXvbApi::new()));
        let gui_api_xmrig = Arc::new(Mutex::new(PubXmrigApi::new()));
        let gui_api_p2pool = Arc::new(Mutex::new(PubP2poolApi::new()));
        let img_p2pool = Arc::new(Mutex::new(ImgP2pool::default()));
        let xmrig_img = Arc::new(Mutex::new(ImgXmrig::new()));

        gui_api_p2pool.lock().unwrap().sidechain_shares = 1;
        gui_api_xmrig.lock().unwrap().hashrate_raw_15m = 10_000.0;
        gui_api_xvb.lock().unwrap().stats_priv.donor_24hr_avg = 1000.0;
        gui_api_xvb.lock().unwrap().stats_priv.donor_1hr_avg = 1000.0;
        gui_api_xvb.lock().unwrap().runtime_mode = XvbMode::ManualXvb(1000.0);
        let hashrate_provider =
            crate::helper::xrig::HashrateProvider::Xmrig(gui_api_xmrig, xmrig_img, client);
        let algo = Algorithm::new(gui_api_p2pool, img_p2pool, gui_api_xvb, hashrate_provider);
        assert!(
            algo.construct_decision()
                .1
                .is_some_and(|target| target.target_hr == 1000.0)
        );
    }
    #[test]
    fn test_manual_p2pool_mode() {
        let client = client();
        let gui_api_xvb = Arc::new(Mutex::new(PubXvbApi::new()));
        let gui_api_xmrig = Arc::new(Mutex::new(PubXmrigApi::new()));
        let gui_api_p2pool = Arc::new(Mutex::new(PubP2poolApi::new()));
        let img_p2pool = Arc::new(Mutex::new(ImgP2pool::default()));
        let xmrig_img = Arc::new(Mutex::new(ImgXmrig::new()));

        gui_api_p2pool.lock().unwrap().sidechain_shares = 1;
        gui_api_xmrig.lock().unwrap().hashrate_raw_15m = 10_000.0;
        gui_api_xvb.lock().unwrap().stats_priv.donor_24hr_avg = 1000.0;
        gui_api_xvb.lock().unwrap().stats_priv.donor_1hr_avg = 1000.0;
        gui_api_xvb.lock().unwrap().runtime_mode = XvbMode::ManualP2pool(1000.0);
        let hashrate_provider =
            crate::helper::xrig::HashrateProvider::Xmrig(gui_api_xmrig, xmrig_img, client);
        let algo = Algorithm::new(gui_api_p2pool, img_p2pool, gui_api_xvb, hashrate_provider);
        assert_eq!(
            Some(crate::helper::xvb::distri_algo::Target {
                pool: Pool::P2pool(3333),
                target_hr: 1000.0
            }),
            algo.construct_decision().1
        )
    }
}
