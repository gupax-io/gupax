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
    sync::{Arc, Mutex},
    time::Duration,
};

use log::{debug, info, warn};
use reqwest_middleware::ClientWithMiddleware as Client;
use serde::Deserialize;
use serde_this_or_that::as_u64;
use strum::IntoEnumIterator;

use crate::{
    XVB_URL_PUBLIC_API,
    helper::{Process, ProcessName, ProcessState, xvb::output_console},
};

use super::{PubXvbApi, rounds::XvbRound};

#[allow(dead_code)] // because deserialize doesn't use all the fields
#[derive(Debug, Clone, Default, Deserialize)]
pub struct XvbPubStats {
    // remaining time of round in minutes
    // this value can be negative if the round did not yet found a share in the main sidechain.
    pub time_remain: i32,
    pub bonus_hr: f64,
    pub donate_hr: f64,      // donated hr from all donors
    pub donate_miners: u32,  // numbers of donors
    pub donate_workers: u32, // numbers of workers from donors
    pub players: u32,
    pub players_round: u32,
    pub winner: String,
    pub share_effort: String,
    pub block_reward: String,
    pub round_type: XvbRound,
    #[serde(deserialize_with = "as_u64")]
    pub block_height: u64,
    pub block_hash: String,
    #[serde(deserialize_with = "as_u64")]
    pub roll_winner: u64,
    #[serde(deserialize_with = "as_u64")]
    pub roll_round: u64,
    pub reward_yearly: Vec<f64>,
}

impl XvbPubStats {
    pub fn rewards(&self) -> Vec<(XvbRound, f64)> {
        let mut rounds = XvbRound::iter();
        let mut vec = vec![];
        for reward in self.reward_yearly.iter() {
            if let Some(round) = rounds.next() {
                vec.push((round, *reward));
            }
        }
        vec
    }
    #[inline]
    // Send an HTTP request to XvB's API, serialize it into [Self] and return it
    pub(in crate::helper) async fn request_api(
        client: &Client,
    ) -> std::result::Result<Self, anyhow::Error> {
        Ok(client
            .get(XVB_URL_PUBLIC_API)
            .timeout(Duration::from_secs(5))
            .send()
            .await?
            .json::<Self>()
            .await?)
    }
    pub async fn update_stats(
        client: &Client,
        gui_api: &Arc<Mutex<PubXvbApi>>,
        pub_api: &Arc<Mutex<PubXvbApi>>,
        process: &Arc<Mutex<Process>>,
    ) {
        debug!("XvB Watchdog | Attempting HTTP public API request...");
        match XvbPubStats::request_api(client).await {
            Ok(new_data) => {
                debug!("XvB Watchdog | HTTP API request OK");
                pub_api.lock().unwrap().stats_pub = new_data;
                let previously_failed = process.lock().unwrap().state == ProcessState::Failed;
                if previously_failed {
                    info!("XvB Watchdog | Public stats are working again");
                    output_console(
                        &mut gui_api.lock().unwrap().output,
                        "requests for public API are now working",
                        ProcessName::Xvb,
                    );
                    process.lock().unwrap().state = ProcessState::Syncing;
                }
            }
            Err(err) => {
                warn!(
                    "XvB Watchdog | Could not send HTTP API request to: {XVB_URL_PUBLIC_API} even after multiples tries\n:{err}"
                );
                // output the error to console
                // if error already present, no need to print it multiple times.
                output_console(
                    &mut gui_api.lock().unwrap().output,
                    &format!(
                        "Failure to retrieve public stats from {XVB_URL_PUBLIC_API}\nWill retry shortly..."
                    ),
                    ProcessName::Xvb,
                );
                // we stop the algo (will be stopped by the check status on next loop) because we can't make the rest work without public stats. (winner in xvb private stats).
                output_console(
                    &mut gui_api.lock().unwrap().output,
                    "request to get public API failed.\nYou won't have any public stats available but the algorithm can continue.",
                    ProcessName::Xvb,
                );
                pub_api.lock().unwrap().stats_pub = XvbPubStats::default();
            }
        }
    }
}
