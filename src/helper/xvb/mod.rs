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

use crate::helper::xrig::HashrateProvider;
use crate::helper::xvb::algorithm::{AlgoConfig, Algorithm, XvbMode};
use crate::helper::xvb::priv_stats::XvbPrivStats;
use crate::helper::xvb::public_stats::XvbPubStats;
use crate::helper::{ProcessName, sleep_end_loop};
use crate::miscs::{client, output_console};
use bounded_vec_deque::BoundedVecDeque;
use enclose::enc;
use log::{debug, info, warn};
use readable::up::Uptime;
use reqwest_middleware::ClientWithMiddleware as Client;
use std::time::Duration;
use std::{
    sync::{Arc, Mutex},
    thread,
};
use tokio::spawn;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

use crate::utils::constants::{XVB_PUBLIC_ONLY, XVB_TIME_ALGO};
use crate::{
    helper::{ProcessSignal, ProcessState},
    utils::macros::sleep,
};

use self::nodes::Pool;

use super::p2pool::{ImgP2pool, PubP2poolApi};
use super::xrig::xmrig::{ImgXmrig, PubXmrigApi};
use super::xrig::xmrig_proxy::{ImgProxy, PubXmrigProxyApi};
use super::{Helper, Process};

pub mod algorithm;
pub mod distri_algo;
pub mod nodes;
pub mod priv_stats;
pub mod public_stats;
pub mod rounds;

impl Helper {
    // Just sets some signals for the watchdog thread to pick up on.
    pub fn stop_xvb(helper: &Arc<Mutex<Self>>) {
        info!("XvB | Attempting to stop...");
        helper.lock().unwrap().xvb.lock().unwrap().signal = ProcessSignal::Stop;
        helper.lock().unwrap().xvb.lock().unwrap().state = ProcessState::Middle;
    }
    pub fn restart_xvb(
        helper: &Arc<Mutex<Self>>,
        state_xvb: &crate::disk::state::Xvb,
        state_p2pool: &crate::disk::state::P2pool,
        state_xmrig: &crate::disk::state::Xmrig,
        state_xp: &crate::disk::state::XmrigProxy,
    ) {
        info!("XvB | Attempting to restart...");
        helper.lock().unwrap().xvb.lock().unwrap().signal = ProcessSignal::Restart;
        helper.lock().unwrap().xvb.lock().unwrap().state = ProcessState::Middle;
        let helper = helper.clone();
        let state_xvb = state_xvb.clone();
        let state_p2pool = state_p2pool.clone();
        let state_xmrig = state_xmrig.clone();
        let state_xp = state_xp.clone();
        // This thread lives to wait, start xmrig then die.
        thread::spawn(move || {
            while helper.lock().unwrap().xvb.lock().unwrap().state != ProcessState::Waiting {
                warn!("XvB | Want to restart but process is still alive, waiting...");
                sleep!(1000);
            }
            // Ok, process is not alive, start the new one!
            info!("XvB | Old process seems dead, starting new one!");
            Self::start_xvb(&helper, &state_xvb, &state_p2pool, &state_xmrig, &state_xp);
        });
        info!("XMRig | Restart ... OK");
    }
    pub fn start_xvb(
        helper: &Arc<Mutex<Self>>,
        state_xvb: &crate::disk::state::Xvb,
        state_p2pool: &crate::disk::state::P2pool,
        state_xmrig: &crate::disk::state::Xmrig,
        state_xp: &crate::disk::state::XmrigProxy,
    ) {
        // 1. Clone Arc value from Helper
        // pub for writing new values that will show up on UI after helper thread update. (every seconds.)
        // gui for reading values from other thread and writing directly without waiting one second (terminal output).
        // if only gui was used, values would update on UI at different time which is not a good user experience.
        info!("XvB | cloning helper arc fields");
        // without xmrig alive, it doesn't make sense to use XvB.
        // needed to see if it is alive. For XvB process to function completely, p2pool node must be alive to check the shares in the pplns window.
        let gui_api = Arc::clone(&helper.lock().unwrap().gui_api_xvb);
        let pub_api = Arc::clone(&helper.lock().unwrap().pub_api_xvb);
        let process = Arc::clone(&helper.lock().unwrap().xvb);
        let process_p2pool = Arc::clone(&helper.lock().unwrap().p2pool);
        let gui_api_p2pool = Arc::clone(&helper.lock().unwrap().gui_api_p2pool);
        let process_xmrig = Arc::clone(&helper.lock().unwrap().xmrig);
        let process_xp = Arc::clone(&helper.lock().unwrap().xmrig_proxy);
        // Generally, gui is to read data, pub is to update.
        // ex: read hashrate values from gui, update node to pub.
        let gui_api_xmrig = Arc::clone(&helper.lock().unwrap().gui_api_xmrig);
        let gui_api_xp = Arc::clone(&helper.lock().unwrap().gui_api_xp);
        // get the value with which xmrig and proxy started.
        let img_xmrig = Arc::clone(&helper.lock().unwrap().img_xmrig);
        let img_proxy = Arc::clone(&helper.lock().unwrap().img_proxy);
        let img_p2pool = Arc::clone(&helper.lock().unwrap().img_p2pool);

        // Reset before printing to output.
        // Need to reset because values of stats would stay otherwise which could bring confusion even if panel is with a disabled theme.
        // at the start of a process, values must be default.
        info!(
            "XvB | resetting pub and gui but keep current node as it is updated by xmrig console."
        );
        reset_data_xvb(&pub_api, &gui_api);
        // we reset the console output because it is complete start.
        gui_api.lock().unwrap().output.clear();
        // 2. Set process state
        // XvB has not yet decided if it can continue.
        // it could fail if XvB server is offline or start partially if address is invalid or if p2pool or xmrig are offline.
        // this state will be received accessed by the UI directly and put the status on yellow.
        info!("XvB | Setting process state...");
        {
            let mut lock = process.lock().unwrap();
            lock.state = ProcessState::Middle;
            lock.signal = ProcessSignal::None;
            lock.start = std::time::Instant::now();
        }
        // verify if address is existent on XvB server

        info!("XvB | spawn watchdog");
        thread::spawn(
            enc!((state_xvb, state_p2pool, state_xmrig, state_xmrig,state_xp, img_xmrig, img_proxy, img_p2pool) move || {
                // thread priority, else there are issue on windows but it is also good for other OS
                    Self::spawn_xvb_watchdog(
                    &gui_api,
                    &pub_api,
                    &process,
                    &state_xvb,
                    &state_p2pool,
                    &state_xmrig,
                    &state_xp,
                    &gui_api_p2pool,
                    &process_p2pool,
                    &gui_api_xmrig,
                    &process_xmrig,
                    &gui_api_xp,
                    &process_xp,
                    &img_xmrig,
                    &img_proxy,
                    &img_p2pool,

                );
            }),
        );
    }
    // need the helper so we can restart the thread after getting a signal not caused by a restart.
    #[allow(clippy::too_many_arguments)]
    #[tokio::main]
    async fn spawn_xvb_watchdog(
        gui_api: &Arc<Mutex<PubXvbApi>>,
        pub_api: &Arc<Mutex<PubXvbApi>>,
        process: &Arc<Mutex<Process>>,
        state_xvb: &crate::disk::state::Xvb,
        state_p2pool: &crate::disk::state::P2pool,
        state_xmrig: &crate::disk::state::Xmrig,
        state_xp: &crate::disk::state::XmrigProxy,
        gui_api_p2pool: &Arc<Mutex<PubP2poolApi>>,
        process_p2pool: &Arc<Mutex<Process>>,
        gui_api_xmrig: &Arc<Mutex<PubXmrigApi>>,
        process_xmrig: &Arc<Mutex<Process>>,
        gui_api_xp: &Arc<Mutex<PubXmrigProxyApi>>,
        process_xp: &Arc<Mutex<Process>>,
        // get the current http port of xmrig
        // This port could be changed by the user without restarting xvb
        xmrig_img: &Arc<Mutex<ImgXmrig>>,
        proxy_img: &Arc<Mutex<ImgProxy>>,
        p2pool_img: &Arc<Mutex<ImgP2pool>>,
    ) {
        // create uniq client that is going to be used for during the life of the thread.
        let client = client();
        // checks confition to start XvB, will set proper state of XvB.
        // if state is middle (everything fine here),set which xvb node could be used.
        // should wait for it, because algo needs to not be started if at least one node of XvB are not responsive.
        // if no node respond, state will be AllNodeOffline.
        // state could be offline nodes or alive at this point
        // if offlines nodes, state must not let private api and algo run. only public info, so state must not be alive.
        // in that case, a spawn would retry and change state if they are available again.
        check_conditions_for_start(
            &client,
            gui_api,
            process_p2pool,
            process_xmrig,
            process_xp,
            process,
            state_p2pool,
        )
        .await;
        let mut xp_alive = false;
        let hashrate_provider = if xp_alive {
            HashrateProvider::Proxy(gui_api_xp.clone(), proxy_img.clone(), client.clone())
        } else {
            HashrateProvider::Xmrig(gui_api_xmrig.clone(), xmrig_img.clone(), client.clone())
        };
        // uptime for log of signal check ?
        let start = process.lock().unwrap().start;
        // uptime of last run of algo
        let last_algorithm = Arc::new(Mutex::new(tokio::time::Instant::now()));
        // uptime of last request (public and private)
        let last_request = Arc::new(Mutex::new(tokio::time::Instant::now()));
        // algo check if his behavior must be like the first time or second time. It can reset those values to re-act like first time even if it's not the case.
        let mut first_loop = true;
        // retry will be accessed from the 1m spawn, it can influence the start of algo.
        let retry = Arc::new(Mutex::new(false));
        // let handles;
        let handle_algo = Arc::new(Mutex::new(None));
        let handle_request = Arc::new(Mutex::new(None));
        let mut msg_retry_done = false;

        // let's create the memory of last hour average sent to p2pool and XvB
        // tuple (p2pool, xvb)
        // need to keep it alive even if algo is down, and push values of hashrate sent to p2pool and 0 for XvB.
        // spawn a task to keep the values updated, looking at hr and pool direction.

        info!("XvB | Entering Process mode... ");
        loop {
            debug!("XvB Watchdog | ----------- Start of loop -----------");
            // Set timer of loop
            let start_loop = std::time::Instant::now();
            {
                // check if first loop the state of Xmrig-Proxy
                if first_loop {
                    xp_alive = process_xp.lock().unwrap().state == ProcessState::Alive;
                    msg_retry_done = false;
                    *retry.lock().unwrap() = false;
                }
                // verify if p2pool and xmrig are running, else XvB must be reloaded with another address to start verifying the other process.
                if check_state_outcauses_xvb(
                    &client,
                    gui_api,
                    pub_api,
                    process,
                    process_xmrig,
                    process_xp,
                    process_p2pool,
                    &mut first_loop,
                    &handle_algo,
                    state_p2pool,
                    xp_alive,
                    xmrig_img,
                    proxy_img,
                    p2pool_img,
                    &hashrate_provider,
                )
                .await
                {
                    continue;
                }
                // check signal
                debug!("XvB | check signal");
                if signal_interrupt(
                    process,
                    if xp_alive { process_xp } else { process_xmrig },
                    process_p2pool,
                    start.into(),
                    &client,
                    pub_api,
                    gui_api,
                    state_p2pool,
                    state_xmrig,
                    state_xp,
                    state_xvb,
                    xp_alive,
                    xmrig_img,
                    proxy_img,
                    p2pool_img,
                    &hashrate_provider,
                ) {
                    info!("XvB Watchdog | Signal has stopped the loop");
                    break;
                }
                // let handle_algo_c = handle_algo.lock().unwrap();
                let is_algo_started_once = handle_algo.lock().unwrap().is_some();
                let is_algo_finished = handle_algo
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|algo| algo.is_finished());
                let is_request_finished = handle_request
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|request: &JoinHandle<()>| request.is_finished())
                    || handle_request.lock().unwrap().is_none();
                // Send an HTTP API request only if one minute is passed since the last request or if first loop or if algorithm need to retry or if request is finished and algo is finished or almost finished (only public and private stats). We make sure public and private stats are refreshed before doing another run of the algo.
                // We make sure algo or request are not rerun when they are not over.
                // in the case of quick refresh before new run of algo, make sure it doesn't happen multiple times.
                let last_request_expired =
                    last_request.lock().unwrap().elapsed() >= Duration::from_secs(60);
                let should_refresh_before_next_algo = is_algo_started_once
                    && last_algorithm.lock().unwrap().elapsed()
                        >= Duration::from_secs((XVB_TIME_ALGO as f32 * 0.95) as u64)
                    && last_request.lock().unwrap().elapsed() >= Duration::from_secs(25);
                let process_alive = process.lock().unwrap().state == ProcessState::Alive;
                if ((last_request_expired || first_loop)
                    || (*retry.lock().unwrap()
                        || is_algo_finished
                        || should_refresh_before_next_algo)
                        && process_alive)
                    && is_request_finished
                {
                    // do not wait for the request to finish so that they are retrieved at exactly one minute interval and not block the thread.
                    // Private API will also use this instant if XvB is Alive.
                    // first_loop is false here but could be changed to true under some conditions.
                    // will send a stop signal if public stats failed or update data with new one.
                    *handle_request.lock().unwrap() = Some(spawn(
                        enc!((client, pub_api, gui_api, gui_api_p2pool, gui_api_xmrig, gui_api_xp,  state_xvb, state_p2pool, state_xmrig,  process, last_algorithm, retry, handle_algo,  last_request, proxy_img, xmrig_img, process_p2pool, p2pool_img, hashrate_provider) async move {
                                // needs to wait here for public stats to get private stats.
                                if last_request_expired || first_loop || should_refresh_before_next_algo {
                                XvbPubStats::update_stats(&client, &gui_api, &pub_api, &process).await;
                                    *last_request.lock().unwrap() = Instant::now();
                                }
                                // private stats needs a valid address.
                                // other stats needs everything to be alive, so just require alive here for now.
                                // maybe later differentiate to add a way to get private stats without running the algo ?
                                if process.lock().unwrap().state == ProcessState::Alive {
                                    // get current share to know if we are in a round and this is a required data for algo.
                                    let share = gui_api_p2pool.lock().unwrap().sidechain_shares;
                                    debug!("XvB | Number of current shares: {share}");
                                // private stats can be requested every minute or first loop or if the have almost finished.
                                if last_request_expired || first_loop || should_refresh_before_next_algo {
                                    debug!("XvB Watchdog | Attempting HTTP private API request...");
                                    // reload private stats, it send a signal if error that will be captured on the upper thread.
                                    XvbPrivStats::update_stats(
                                        &client, &state_p2pool.address,  &gui_api, &process,
                                    )
                                    .await;
                                    *last_request.lock().unwrap() = Instant::now();

                                    // verify in which round type we are
                                    let round = pub_api.lock().unwrap().stats_priv.round_type(share > 0);
                                    // refresh the round we participate in.
                                    debug!("XvB | Round type: {round:#?}");
                                    pub_api.lock().unwrap().stats_priv.round_participate = round;
                                    // verify if we are the winner of the current round
                                    let win_current = pub_api.lock().unwrap().stats_pub.winner == Helper::head_tail_of_monero_address(&state_p2pool.address).as_str();                                    pub_api.lock().unwrap().stats_priv.win_current = win_current;
                                }
                                let hashrate = hashrate_provider.current_controllable_hr();
                                let difficulty_data_is_ready = gui_api_p2pool.lock().unwrap().p2pool_difficulty_u64 > 100_000;
                                    if (first_loop || *retry.lock().unwrap()|| is_algo_finished) && hashrate > 0.0 && process.lock().unwrap().state == ProcessState::Alive && difficulty_data_is_ready && pub_api.lock().unwrap().stats_priv.pool != Pool::Unknown
                                    {
                                        // if algo was started, it must not retry next loop.
                                        *retry.lock().unwrap() = false;
                                        // reset instant because algo will start.
                                        *last_algorithm.lock().unwrap() = Instant::now();
                                        *handle_algo.lock().unwrap() = Some(spawn(enc!((client, gui_api_xmrig, gui_api_xp, state_xmrig,   state_xvb, proxy_img, xmrig_img, p2pool_img, process_p2pool) async move {
                        let algorithm = Algorithm::new(gui_api_p2pool, p2pool_img, gui_api.clone(),  hashrate_provider);
                        info!("Algorithm data: {:#?}", &algorithm);
                        algorithm.run(&gui_api).await;
                        info!("Done running algorithm");
                                        })));
                                    } else {
                                        // if xmrig is still at 0 HR but is alive and algorithm is skipped, recheck first 10s of xmrig inside algorithm next time (in one minute). Don't check if algo failed to start because state was not alive after getting private stats.

                                        if (hashrate == 0.0 || !difficulty_data_is_ready) && process.lock().unwrap().state == ProcessState::Alive {
                                            *retry.lock().unwrap() = true
                                        }
                                    }

                                }
                            }),
                    ));
                }
                // if retry is false, next time the message about waiting for xmrig HR can be shown.
                if !*retry.lock().unwrap() {
                    msg_retry_done = false;
                }
                // inform user that algorithm has not yet started because it is waiting for xmrig HR.
                // show this message only once before the start of algo
                if *retry.lock().unwrap() && !msg_retry_done {
                    let msg = if xp_alive {
                        "Algorithm is waiting for 1 minute average HR of XMRig-Proxy or p2pool data"
                    } else {
                        "Algorithm is waiting for 10 seconds average HR of XMRig or p2pool data"
                    };
                    output_console(&mut gui_api.lock().unwrap().output, msg, ProcessName::Xvb);
                    msg_retry_done = true;
                }
                // update indicator (time before switch and mining location) in private stats
                // if algo not running, second message.
                // will update countdown every second.
                // verify current node which is set by algo or circonstances (failed node).
                // verify given time set by algo and start time of current algo.
                // will run only if XvB is alive.
                // let algo time to start, so no countdown is shown.
                update_indicator_algo(
                    is_algo_started_once,
                    is_algo_finished,
                    process,
                    pub_api,
                    gui_api,
                    &last_algorithm,
                );
                // first_loop is done, but maybe retry will allow the algorithm to retry again.
                if first_loop {
                    first_loop = false;
                }
                // Sleep (only if 900ms hasn't passed)
            }
            sleep_end_loop(start_loop, ProcessName::Xvb).await;
        }
    }
}
//---------------------------------------------------------------------------------------------------- Public XvB API

#[derive(Debug, Clone, Default)]
pub struct PubXvbApi {
    pub output: String,
    pub _uptime: u64,
    pub p2pool_sent_last_hour_samples: SamplesAverageHour,
    pub stats_pub: XvbPubStats,
    pub stats_priv: XvbPrivStats,
    // where xmrig is mining right now (or trying to).
    // will be updated by output of xmrig.
    // could also be retrieved by fetching current config.
    pub current_pool: Option<Pool>,
    pub algo_config: AlgoConfig,
    // updated by UI
    pub runtime_mode: XvbMode,
}
#[derive(Debug, Clone)]
pub struct SamplesAverageHour(BoundedVecDeque<f32>);
impl Default for SamplesAverageHour {
    fn default() -> Self {
        let capacity = (3600 / (XVB_TIME_ALGO / 1000)) as usize;
        let mut vec = BoundedVecDeque::new(capacity);
        for _ in 0..capacity {
            vec.push_back(0.0f32);
        }
        SamplesAverageHour(vec)
    }
}

impl SamplesAverageHour {
    pub fn average(&self) -> f32 {
        self.0.iter().sum::<f32>() / self.0.len() as f32
    }
}

impl PubXvbApi {
    pub fn new() -> Self {
        Self::default()
    }
    // The issue with just doing [gui_api = pub_api] is that values get overwritten.
    // This doesn't matter for any of the values EXCEPT for the output,  so we must
    // manually append it instead of overwriting.
    // This is used in the "helper" thread.
    pub(super) fn combine_gui_pub_api(gui_api: &mut Self, pub_api: &mut Self) {
        let mut output = std::mem::take(&mut gui_api.output);
        let buf = std::mem::take(&mut pub_api.output);

        let algo_config = std::mem::take(&mut gui_api.algo_config);
        let runtime_mode = std::mem::take(&mut gui_api.runtime_mode);
        let time_donated = std::mem::take(&mut gui_api.stats_priv.time_donated);
        let fails = std::mem::take(&mut gui_api.stats_priv.fails);
        let donor_1hr_avg = std::mem::take(&mut gui_api.stats_priv.donor_1hr_avg);
        let donor_24hr_avg = std::mem::take(&mut gui_api.stats_priv.donor_24hr_avg);
        if !buf.is_empty() {
            output.push_str(&buf);
        }
        *gui_api = Self {
            output,
            algo_config,
            runtime_mode,
            // current_pool,
            stats_priv: XvbPrivStats {
                // pool,
                time_donated,
                fails,
                donor_1hr_avg,
                donor_24hr_avg,
                ..pub_api.stats_priv.clone()
            },
            ..pub_api.clone()
        };
    }
}
#[allow(clippy::too_many_arguments)]
async fn check_conditions_for_start(
    client: &Client,
    gui_api: &Arc<Mutex<PubXvbApi>>,
    process_p2pool: &Arc<Mutex<Process>>,
    process_xmrig: &Arc<Mutex<Process>>,
    process_xp: &Arc<Mutex<Process>>,
    process_xvb: &Arc<Mutex<Process>>,
    state_p2pool: &crate::disk::state::P2pool,
) {
    let state = if let Err(err) = XvbPrivStats::request_api(client, &state_p2pool.address).await {
        info!("XvB | verify address");
        warn!(
            "Xvb | Start ... Partially failed because address is not registered on XvB server: {err}\n"
        );
        output_console(
            &mut gui_api.lock().unwrap().output,
            &format!(
                "Address is not valid on XvB API.\nCheck if you are registered.\nError: {err}"
            ),
            ProcessName::Xvb,
        );
        ProcessState::NotMining
    } else if process_p2pool.lock().unwrap().state != ProcessState::Alive {
        info!("XvB | verify p2pool pool");
        // send to console: p2pool process is not running
        warn!("Xvb | Start ... Partially failed because P2pool instance is not ready.");
        let msg = if process_p2pool.lock().unwrap().state == ProcessState::Syncing {
            "P2pool process is not ready.\nCheck the P2pool Tab"
        } else {
            "P2pool process is not running.\nCheck the P2pool Tab"
        };
        output_console(&mut gui_api.lock().unwrap().output, msg, ProcessName::Xvb);
        ProcessState::Syncing
    } else if process_xmrig.lock().unwrap().state != ProcessState::Alive
        && process_xp.lock().unwrap().state != ProcessState::Alive
    {
        // send to console: xmrig process is not running
        warn!(
            "Xvb | Start ... Partially failed because Xmrig or Xmrig-Proxy instance is not running."
        );
        // output the error to console
        output_console(
            &mut gui_api.lock().unwrap().output,
            "XMRig or Xmrig-Proxy process is not running.\nCheck the Xmrig or Xmrig-Proxy Tab. One of them must be running to start the XvB algorithm.",
            ProcessName::Xvb,
        );
        ProcessState::Syncing
    } else {
        // all test passed, so it can be Alive
        // stay at middle, updatePools will finish by syncing or offlinenodes and check_status in loop will change state accordingly.
        ProcessState::Middle
    };
    if state != ProcessState::Middle {
        // while waiting for xmrig and p2pool or getting right address, it can get public stats
        info!("XvB | print to console state");
        output_console(
            &mut gui_api.lock().unwrap().output,
            &["XvB partially started.\n", XVB_PUBLIC_ONLY].concat(),
            ProcessName::Xvb,
        );
    }
    // will update the preferred pool for the first loop, even if partially started.
    process_xvb.lock().unwrap().signal =
        ProcessSignal::UpdatePools(Pool::XvBEurope(state_p2pool.address.clone()));

    process_xvb.lock().unwrap().state = state;
}
/// return a bool to continue to next loop if needed.
#[allow(clippy::too_many_arguments)]
async fn check_state_outcauses_xvb(
    client: &Client,
    gui_api: &Arc<Mutex<PubXvbApi>>,
    pub_api: &Arc<Mutex<PubXvbApi>>,
    process: &Arc<Mutex<Process>>,
    process_xmrig: &Arc<Mutex<Process>>,
    process_xp: &Arc<Mutex<Process>>,
    process_p2pool: &Arc<Mutex<Process>>,
    first_loop: &mut bool,
    handle_algo: &Arc<Mutex<Option<JoinHandle<()>>>>,
    state_p2pool: &crate::disk::state::P2pool,
    xp_start_alive: bool,
    xmrig_img: &Arc<Mutex<ImgXmrig>>,
    proxy_img: &Arc<Mutex<ImgProxy>>,
    p2pool_img: &Arc<Mutex<ImgP2pool>>,
    hashrate_provider: &HashrateProvider,
) -> bool {
    // will check if the state can stay as it is.
    // p2pool and xmrig are alive if ready and running (syncing is not alive).
    let state = process.lock().unwrap().state;

    let xp_is_alive = process_xp.lock().unwrap().state == ProcessState::Alive;
    let msg_xmrig_or_proxy = if xp_is_alive { "Xmrig-Proxy" } else { "Xmrig" };
    // if state is not alive, the algo should stop if it was running and p2pool should be used by xmrig.

    if let Some(handle) = handle_algo.lock().unwrap().as_ref() {
        // XvB should stop the algo if the state of xp is different from the start.
        if xp_is_alive != xp_start_alive && !handle.is_finished() {
            warn!("XvB Process | stop the algorithm because Xmrig-Proxy state changed");
            output_console(
                &mut gui_api.lock().unwrap().output,
                "Algorithm stopped because Xmrig-Proxy state changed",
                ProcessName::Xvb,
            );
            handle.abort();
        }
        if state != ProcessState::Alive && !handle.is_finished() {
            handle.abort();
            output_console(
                &mut gui_api.lock().unwrap().output,
                "XvB process can not completely continue, algorithm of distribution of HR is stopped.",
                ProcessName::Xvb,
            );
            // only update xmrig if it is alive and wasn't on p2pool already.
            if pub_api.lock().unwrap().current_pool
                != Some(Pool::P2pool(state_p2pool.current_port(
                    process_p2pool.lock().unwrap().is_alive(),
                    &p2pool_img.lock().unwrap(),
                )))
                && (process_xmrig.lock().unwrap().state == ProcessState::Alive || xp_is_alive)
            {
                spawn(
                    enc!((client,  gui_api, xmrig_img, proxy_img, process_p2pool, p2pool_img, state_p2pool, hashrate_provider) async move {

                                    let pool = Pool::P2pool(state_p2pool.current_port(
                                        process_p2pool.lock().unwrap().is_alive(),
                                        &p2pool_img.lock().unwrap(),
                                    ));
                                    if let Err(err) = hashrate_provider.update_config(&pool).await
                                            {
                                                // show to console error about updating xmrig config
                                                output_console(
                                                    &mut gui_api.lock().unwrap().output,
                                                    &format!(
                                                        "Failure to update {msg_xmrig_or_proxy} config with HTTP API.\nError: {err}"
                                                    ),
                                                    ProcessName::Xvb
                                                );
                                            } else {
                                                let pool =
                    Pool::P2pool(state_p2pool.current_port(
                                        process_p2pool.lock().unwrap().is_alive(),
                                        &p2pool_img.lock().unwrap(),
                                    ));
                                                output_console(
                                                    &mut gui_api.lock().unwrap().output,
                                                    &format!("XvB process can not completely continue, falling back to {pool}"),
                                                    ProcessName::Xvb
                                                );
                                            }

                                    }),
                );
            }
        }
    }
    // if state of Xmrig-Proxy changed, go back to first loop
    if xp_start_alive != xp_is_alive {
        *first_loop = true;
        return true;
    }

    let is_xmrig_alive = process_xp.lock().unwrap().state == ProcessState::Alive
        || process_xmrig.lock().unwrap().state == ProcessState::Alive;
    // XvB should not restart until p2pool values are updated for the first time since it's alive
    // let is_p2pool_alive = process_p2pool.lock().unwrap().state == ProcessState::Alive;
    let is_p2pool_alive = true;

    let p2pool_xmrig_alive = is_xmrig_alive && is_p2pool_alive;
    // if state is middle because start is not finished yet, it will not do anything.
    match state {
        ProcessState::Alive if !p2pool_xmrig_alive => {
            // they are not both alives, so state will be at syncing and data reset, state of loop also.
            warn!(
                "XvB | stopped partially because P2pool pool or xmrig/xmrig-proxy are not reachable."
            );
            // stats must be empty put to default so the UI reflect that XvB private is not running.
            reset_data_xvb(pub_api, gui_api);
            // request from public API must be executed at next loop, do not wait for 1 minute.
            *first_loop = true;
            output_console(
                &mut gui_api.lock().unwrap().output,
                "XvB is now partially stopped because p2pool pool or XMRig/XMRig-Proxy came offline.\nCheck P2pool and Xmrig/Xmrig-Proxy Tabs",
                ProcessName::Xvb,
            );
            output_console(
                &mut gui_api.lock().unwrap().output,
                XVB_PUBLIC_ONLY,
                ProcessName::Xvb,
            );
            process.lock().unwrap().state = ProcessState::Syncing;
        }
        ProcessState::Syncing if p2pool_xmrig_alive => {
            info!("XvB | started this time with p2pool and xmrig");
            // will put state on middle and update pools
            process.lock().unwrap().state = ProcessState::Alive;
            reset_data_xvb(pub_api, gui_api);
            *first_loop = true;

            // if p2pool just came alive, it is possible that the fresh values are not yet active.
            // waiting a full second guarantees that the p2pool thread have the time to complete.
            sleep!(1000);
            output_console(
                &mut gui_api.lock().unwrap().output,
                &[
                    "XvB is now started because p2pool and ",
                    msg_xmrig_or_proxy,
                    " came online.",
                ]
                .concat(),
                ProcessName::Xvb,
            );
        }
        // nothing to do, we don't want to change other state
        _ => {}
    };
    false
}
#[allow(clippy::too_many_arguments)]
fn signal_interrupt(
    process: &Arc<Mutex<Process>>,
    process_xrig: &Arc<Mutex<Process>>,
    process_p2pool: &Arc<Mutex<Process>>,
    start: Instant,
    client: &Client,
    pub_api: &Arc<Mutex<PubXvbApi>>,
    gui_api: &Arc<Mutex<PubXvbApi>>,
    state_p2pool: &crate::disk::state::P2pool,
    state_xmrig: &crate::disk::state::Xmrig,
    state_xp: &crate::disk::state::XmrigProxy,
    state_xvb: &crate::disk::state::Xvb,
    xp_alive: bool,
    xmrig_img: &Arc<Mutex<ImgXmrig>>,
    proxy_img: &Arc<Mutex<ImgProxy>>,
    p2pool_img: &Arc<Mutex<ImgP2pool>>,
    hashrate_provider: &HashrateProvider,
) -> bool {
    // Check SIGNAL
    // check if STOP or RESTART Signal is given.
    // if STOP, will put Signal to None, if Restart to Wait
    // in either case, will break from loop.
    let signal = process.lock().unwrap().signal.clone();
    match signal {
        ProcessSignal::Stop => {
            debug!("P2Pool Watchdog | Stop SIGNAL caught");
            // Wait to get the exit status
            let uptime = start.elapsed();
            info!(
                "Xvb Watchdog | Stopped ... Uptime was: [{}]",
                Uptime::from(uptime)
            );
            // insert the signal into output of XvB
            // This is written directly into the GUI API, because sometimes the 900ms event loop can't catch it.
            output_console(
                &mut gui_api.lock().unwrap().output,
                "\n\n\nXvB stopped\n\n\n",
                ProcessName::Xvb,
            );
            debug!("XvB Watchdog | Stop SIGNAL done, breaking");
            process.lock().unwrap().signal = ProcessSignal::None;
            process.lock().unwrap().state = ProcessState::Dead;
            // reset stats
            reset_data_xvb(pub_api, gui_api);
            return true;
        }
        ProcessSignal::Restart => {
            debug!("XvB Watchdog | Restart SIGNAL caught");
            let uptime = Uptime::from(start.elapsed());
            info!("XvB Watchdog | Stopped ... Uptime was: [{uptime}]");
            // no output to console because service will be started with fresh output.
            debug!("XvB Watchdog | Restart SIGNAL done, breaking");
            process.lock().unwrap().state = ProcessState::Waiting;
            reset_data_xvb(pub_api, gui_api);
            return true;
        }
        ProcessSignal::UpdatePools(pool) => {
            if process.lock().unwrap().state != ProcessState::Waiting {
                warn!("received the UpdatePool signal");
                let token_xmrig = if xp_alive {
                    state_xp.token.clone()
                } else {
                    state_xmrig.token.clone()
                };
                let address = state_p2pool.address.clone();
                // check if state is alive. If it is and it is receiving such a signal, it means something a pool (XvB or P2Pool) has failed.
                // if XvB, xmrig needs to be switch to the other pool (both will be checked though to be sure).
                // if both XvB pools fail after checking, process will be partially stopped and a new spawn will verify if pools are again online and so will continue the process completely if that's the case.
                // if P2pool, the process has to stop the algo and continue partially. The process will continue completely if the confitions are met again.
                // if XvB was not alive, then if it is for XvB pools, it will check and update preferred pool and set XMRig to P2pool if that's not the case.
                let was_alive = process.lock().unwrap().state == ProcessState::Alive;
                // so it won't execute another signal of update pools if it is already doing it.
                process.lock().unwrap().state = ProcessState::Waiting;
                process.lock().unwrap().signal = ProcessSignal::None;
                spawn(
                    enc!((pool, process, client, gui_api, pub_api, was_alive, address, token_xmrig, process_xrig, xmrig_img, proxy_img, process_p2pool, state_p2pool, p2pool_img, state_xvb, hashrate_provider) async move {
                    match pool {
                        Pool::XvBNorthAmerica(_)|Pool::XvBEurope(_) if was_alive => {
                            // a pool is failing. We need to first verify if a pool is available
                        Pool::update_fastest_pool(&pub_api, &gui_api, &process, &process_p2pool, &p2pool_img, &state_p2pool, &state_xvb).await;
                            if process.lock().unwrap().state == ProcessState::OfflinePoolsAll {
                                // No available pools, so launch a process to verify periodically.
                    sleep(Duration::from_secs(10)).await;
                    warn!("pool fail, set spawn that will retry pools and update state.");
                    while process.lock().unwrap().state == ProcessState::OfflinePoolsAll {
                        // this spawn will stay alive until pools are joignable or XvB process is stopped or failed.
                        Pool::update_fastest_pool( &pub_api, &gui_api, &process, &process_p2pool, &p2pool_img, &state_p2pool, &state_xvb).await;
                        sleep(Duration::from_secs(10)).await;
                    }
                                
                            }
                                // a good pool is found, so the next check of the loop should be good and the algo will update XMRig with the good one.

                            
                        },
                        Pool::XvBNorthAmerica(_)|Pool::XvBEurope(_) if !was_alive => {
                        // Probably a start. We don't consider XMRig using XvB pools without algo.
                        // can update xmrig and check status of state in the same time.
                        // update prefred pool
                        Pool::update_fastest_pool(&pub_api, &gui_api, &process, &process_p2pool, &p2pool_img, &state_p2pool, &state_xvb).await;
                        // Need to set XMRig to P2Pool if it wasn't. XMRig should have populated this value at his start.
                        // but if xmrig didn't start, don't update it.
                
                    let p2pool_pool  = Pool::P2pool(state_p2pool.current_port(
                                        process_p2pool.lock().unwrap().is_alive(),
                                        &p2pool_img.lock().unwrap(),
                                    ));
                if process_xrig.lock().unwrap().state == ProcessState::Alive && pub_api.lock().unwrap().current_pool != Some(p2pool_pool.clone()) {
                            spawn(enc!((client, token_xmrig, address,  xmrig_img, proxy_img, gui_api, hashrate_provider) async move{
                if let Err(err) = hashrate_provider.update_config(&p2pool_pool)
                .await {
                                let msg_xmrig_or_proxy = if xp_alive {
                                    "XMRig-Proxy"
                                } else {
                                    "XMRig"
                                };
                    output_console(
                        &mut gui_api.lock().unwrap().output,
                        &format!(
                            "Failure to update {msg_xmrig_or_proxy} config with HTTP API.\nError: {err}"
                        ), ProcessName::Xvb
                    );
                        }
                        }
                            ));}
            },
                        _ => {}
                } } ),
                );
            }
        }
        _ => {}
    }

    false
}
/// Keep values that are dynamically loaded from interaction with the UI
fn reset_data_xvb(pub_api: &Arc<Mutex<PubXvbApi>>, gui_api: &Arc<Mutex<PubXvbApi>>) {
    let mut guard = gui_api.lock().unwrap();
    let algo_config = std::mem::take(&mut guard.algo_config);
    let runtime_mode = std::mem::take(&mut guard.runtime_mode);
    let current_pool = std::mem::take(&mut pub_api.lock().unwrap().current_pool);
    let pool = std::mem::take(&mut pub_api.lock().unwrap().stats_priv.pool);
    let time_donated = std::mem::take(&mut guard.stats_priv.time_donated);
    let fails = std::mem::take(&mut guard.stats_priv.fails);
    let donor_1hr_avg = std::mem::take(&mut guard.stats_priv.donor_1hr_avg);
    let donor_24hr_avg = std::mem::take(&mut guard.stats_priv.donor_24hr_avg);

    *pub_api.lock().unwrap() = PubXvbApi::new();
    pub_api.lock().unwrap().stats_priv.pool = pool;
    pub_api.lock().unwrap().current_pool = current_pool;

    guard.runtime_mode = runtime_mode;
    guard.algo_config = algo_config;
    guard.stats_priv.time_donated = time_donated;
    guard.stats_priv.fails = fails;
    guard.stats_priv.donor_1hr_avg = donor_1hr_avg;
    guard.stats_priv.donor_24hr_avg = donor_24hr_avg;
}
// print date time to console output in same format than xmrig
fn update_indicator_algo(
    is_algo_started_once: bool,
    is_algo_finished: bool,
    process: &Arc<Mutex<Process>>,
    pub_api: &Arc<Mutex<PubXvbApi>>,
    gui_api: &Arc<Mutex<PubXvbApi>>,
    last_algorithm: &Arc<Mutex<Instant>>,
) {
    if is_algo_started_once
        && !is_algo_finished
        && process.lock().unwrap().state == ProcessState::Alive
    {
        let pool = pub_api.lock().unwrap().current_pool.clone();
        let time_donated = gui_api.lock().unwrap().stats_priv.time_donated;
        let msg_indicator = match pool {
            Some(Pool::P2pool(_))
                if time_donated > 0.0 && time_donated != XVB_TIME_ALGO as f32 / 1000.0 =>
            {
                // algo is mining on p2pool but will switch to XvB after
                // show time remaining on p2pool
                // todo: debug and fix brief 0s
                // We want to show in seconds as we do not refresh the UI fast enough for milis for performance reasons
                pub_api.lock().unwrap().stats_priv.time_switch_pool = (XVB_TIME_ALGO
                    .checked_sub(last_algorithm.lock().unwrap().elapsed().as_millis() as u64)
                    .unwrap_or_default()
                    .checked_sub((time_donated * 1000.0) as u64)
                    .unwrap_or_default()
                    / 1000)
                    as u32;
                "time until switch to mining on XvB".to_string()
            }
            _ => {
                // algo is mining on XvB or complelty mining on p2pool.
                // show remaining time before next decision of algo
                // because time of last algorithm could depass a little bit XVB_TIME_ALGO before next run, check the sub.
                pub_api.lock().unwrap().stats_priv.time_switch_pool = (XVB_TIME_ALGO
                    .checked_sub(last_algorithm.lock().unwrap().elapsed().as_millis() as u64)
                    .unwrap_or_default()
                    / 1000)
                    as u32;
                "time until next decision of algorithm".to_string()
            }
        };
        pub_api.lock().unwrap().stats_priv.msg_indicator = msg_indicator;
    } else {
        // if algo is not running or process not alive
        pub_api.lock().unwrap().stats_priv.time_switch_pool = 0;
        pub_api.lock().unwrap().stats_priv.msg_indicator = "Algorithm is not running".to_string();
    }
}
