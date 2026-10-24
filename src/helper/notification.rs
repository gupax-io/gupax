use crate::utils::i18n::{t, t_f};
use std::sync::{Arc, Mutex};

use crate::{
    disk::state::Notification,
    helper::{
        Helper, Process, ProcessState, p2pool::PubP2poolApi, sleep_end_loop,
        xrig::xmrig_proxy::PubXmrigProxyApi,
    },
};
use notify_rust::Notification as Notif;

pub struct NotificationApi {
    // we store the different notifications here so that we don't
    // have to restart the service when the user change his preference.
    pub notifications: Vec<Notification>,
}

impl Helper {
    #[cold]
    #[inline(never)]
    // The "frontend" function that parses the arguments, and spawns either the [Simple] or [Advanced] Node watchdog thread.
    pub fn start_notifications(helper: &Arc<Mutex<Self>>) {
        #[cfg(target_os = "macos")]
        let _ = notify_rust::set_application("com.github.cyrix126.gupax");

        let process_node = Arc::clone(&helper.lock().unwrap().node);
        let process_p2pool = Arc::clone(&helper.lock().unwrap().p2pool);
        let process_xmrig = Arc::clone(&helper.lock().unwrap().xmrig);
        let process_proxy = Arc::clone(&helper.lock().unwrap().xmrig_proxy);
        let process_xvb = Arc::clone(&helper.lock().unwrap().xvb);
        let api = Arc::clone(&helper.lock().unwrap().notifications_api);
        let api_p2pool = Arc::clone(&helper.lock().unwrap().gui_api_p2pool);
        let api_proxy = Arc::clone(&helper.lock().unwrap().gui_api_xp);
        std::thread::spawn(move || {
            Self::spawn_notifications_service(
                api,
                api_p2pool,
                api_proxy,
                process_node,
                process_p2pool,
                process_xmrig,
                process_proxy,
                process_xvb,
            );
        });
    }

    // This service will live as long as Gupax is open, so there is no need for a restart/stop method
    #[allow(clippy::too_many_arguments)]
    #[tokio::main]
    async fn spawn_notifications_service(
        api: Arc<Mutex<NotificationApi>>,
        api_p2pool: Arc<Mutex<PubP2poolApi>>,
        api_proxy: Arc<Mutex<PubXmrigProxyApi>>,
        process_node: Arc<Mutex<Process>>,
        process_p2pool: Arc<Mutex<Process>>,
        process_xmrig: Arc<Mutex<Process>>,
        process_proxy: Arc<Mutex<Process>>,
        process_xvb: Arc<Mutex<Process>>,
    ) {
        let mut first_share_found = false;
        let mut last_payouts_count = 0;
        let mut last_xmr_amount = 0.0;
        let mut last_connected_miners = 0;
        let mut node_alive = false;
        let mut p2pool_alive = false;
        let mut xmrig_alive = false;
        let mut proxy_alive = false;
        let mut xvb_alive = false;
        loop {
            let start_loop = std::time::Instant::now();
            {
                let notifications = &api.lock().unwrap().notifications;
                for notification in notifications {
                    match notification {
                        Notification::FirstP2poolShare => {
                            // only check if the p2pool node is alive
                            if process_p2pool.lock().unwrap().state == ProcessState::Alive {
                                // Gupax will send a notification if it's instance find a first share, even if the address already have some in the current PPLNS window.
                                // It's wanted because it will inform the user that this instance works well.
                                if !first_share_found
                                    && api_p2pool
                                        .lock()
                                        .unwrap()
                                        .shares_found
                                        .is_some_and(|s| s == 1)
                                {
                                    first_share_found = true;
                                    notif(t!("Gupax just found it's first P2Pool share !"));
                                }
                            }
                        }
                        Notification::Payout => {
                            // only check if the p2pool node is alive
                            if process_p2pool.lock().unwrap().state == ProcessState::Alive {
                                let new_payouts_count = api_p2pool.lock().unwrap().payouts;
                                let new_amount_xmr = api_p2pool.lock().unwrap().xmr;
                                if new_payouts_count > last_payouts_count {
                                    let amount = new_amount_xmr - last_xmr_amount;
                                    last_payouts_count = new_payouts_count;
                                    last_xmr_amount = new_amount_xmr;
                                    let body = t_f!("New payout ! Your reward is {} xmr", amount);
                                    notif(&body);
                                }
                            }
                        }
                        Notification::DisconnectedMiner => {
                            // only check if the proxy is alive
                            if process_proxy.lock().unwrap().state == ProcessState::Alive {
                                let new_miners_count = api_proxy.lock().unwrap().miners;
                                let diff = new_miners_count.abs_diff(last_connected_miners);
                                if diff != 0 {
                                    last_connected_miners = new_miners_count;
                                    let body = if diff > 1 {
                                        t_f!(
                                            "{} miners have been disconnected from the Proxy",
                                            diff
                                        )
                                    } else {
                                        t!("A miner has been disconnected from the Proxy")
                                            .to_string()
                                    };
                                    notif(&body);
                                }
                            }
                        }
                        Notification::FailedService => {
                            // check if service is alive, set to dead if not
                            // It allows to keep track of if a service should be in an alive state or not
                            // to avoid sending notifications while it is starting
                            let process_node_status = process_node.lock().unwrap().state;
                            let process_p2pool_status = process_p2pool.lock().unwrap().state;
                            let process_xmrig_status = process_xmrig.lock().unwrap().state;
                            let process_proxy_status = process_proxy.lock().unwrap().state;
                            let process_xvb_status = process_xvb.lock().unwrap().state;

                            if process_node_status == ProcessState::Alive && !node_alive {
                                node_alive = true;
                            }
                            if process_p2pool_status == ProcessState::Alive && !p2pool_alive {
                                p2pool_alive = true;
                            }
                            if process_xmrig_status == ProcessState::Alive && !xmrig_alive {
                                xmrig_alive = true;
                            }
                            if process_proxy_status == ProcessState::Alive && !proxy_alive {
                                proxy_alive = true;
                            }
                            if process_xvb_status == ProcessState::Alive && !xvb_alive {
                                xvb_alive = true;
                            }

                            if node_alive && process_node_status == ProcessState::Syncing {
                                notif(t!(
                                    "The Monero Node was synced but is now syncing again\nCheck your network"
                                ));
                                node_alive = false;
                            }
                            if node_alive && process_node_status == ProcessState::Failed {
                                notif(t!(
                                    "The Monero Node is now in a failed state\nCheck your network"
                                ));
                                node_alive = false;
                            }
                            if p2pool_alive && process_p2pool_status == ProcessState::Syncing {
                                notif(t!(
                                    "The P2Pool node was synced but is now syncing again\nCheck your network"
                                ));
                                p2pool_alive = false;
                            }
                            if p2pool_alive && process_p2pool_status == ProcessState::Failed {
                                notif(t!(
                                    "The P2Pool node is now in a failed state\nCheck your network"
                                ));
                                p2pool_alive = false;
                            }
                            if proxy_alive && process_proxy_status == ProcessState::NotMining {
                                notif(t!(
                                    "The Proxy was mining correctly but is not anymore\nCheck your network and the connection to the P2Pool node"
                                ));
                                proxy_alive = false;
                            }
                            if proxy_alive && process_proxy_status == ProcessState::Failed {
                                notif(t!("The Proxy is now in a failed state"));
                                proxy_alive = false;
                            }
                            if xmrig_alive && process_xmrig_status == ProcessState::NotMining {
                                notif(t!(
                                    "XMRig was mining correctly but is not anymore\nCheck your network and the connection to the P2Pool node/Proxy"
                                ));
                                xmrig_alive = false;
                            }
                            if xmrig_alive && process_xmrig_status == ProcessState::Failed {
                                notif(t!("The XMRig is now in a failed state"));
                                xmrig_alive = false;
                            }
                            if xvb_alive && process_xvb_status == ProcessState::OfflinePoolsAll {
                                notif(t!(
                                    "XvB process is disconnected from all XvB Pool.It might be an issue with XvB server and not from Gupax"
                                ));
                                xvb_alive = false;
                            }
                            if xvb_alive && process_xvb_status == ProcessState::Failed {
                                notif(t!("XvB process is now in a failed state"));
                                xvb_alive = false;
                            }
                            if xvb_alive && process_xvb_status == ProcessState::Syncing {
                                notif(t!(
                                    "XvB process is stopped while waiting for P2Pool or XMRig or the Proxy"
                                ));
                                xvb_alive = false;
                            }
                        }
                    }
                }
            }
            sleep_end_loop(start_loop, "Notifications Service").await;
        }
    }
}

pub fn notif(body: &str) {
    // we do not unwrap in case the desktop environment doesn't support notifications
    // We don't need a handle to the notification anyway
    let _ = Notif::new()
        .summary(t!("Gupax event"))
        .body(body)
        .icon("gupax")
        .appname("Gupax")
        .show();
}
