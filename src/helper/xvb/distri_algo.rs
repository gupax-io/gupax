use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use log::info;
use tokio::time::sleep;

use crate::{
    helper::{
        ProcessName,
        xrig::{HashrateProvider, WorkerError},
        xvb::{PubXvbApi, nodes::Pool},
    },
    miscs::output_console,
};

/// An algorithm that allows to know how much percent you
/// need to send your HR on a pool to get to a targeted average HR
/// This distributor is agnostic to the pools you want to distribute your hashrate
/// It has historically been created for participating in the [XvB Raffle](https://xmrvsbeast.com)
/// The Algorithm can now distribute your hashrate with as much pool as you want
/// which can be useful if you want to mine on multiples addresses.
#[derive(Debug)]
pub struct Distributor {
    /// The pool on which we do not care about the target. It will gain any surplus HR.
    pub fallback_pool: Pool,
    /// The different pools and the average HR to reach for each of them.
    pub pools_with_target: Vec<Target>,
    /// The worker to use to provide hashrate
    /// If you need to send hashrate from multiples sources, use a proxy
    pub hashrate_provider: HashrateProvider,
    /// Timeframe to take a new decision and to mine between the pools for this duration.
    /// If you need to attain an average per x (hour/day), timeframe should not be above x.
    pub timeframe: Duration,
}
#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    /// Information needed about the pool we want to send hashrate to.
    pub pool: Pool,
    pub target_hr: f32,
}

impl Distributor {
    /// Run a single decision
    /// In the case the provided HR is not enough to meet every target, it will follow
    /// a priority depending on the order of the targets provided.
    /// If the provided HR is above the total of all targets, the distributor will update the
    /// hashrate provider to mine the fallback pool before switching to pools with target
    /// Tip: Targets could be ordered by their target value to prioritize small target or large target.
    pub async fn run_decision(
        &self,
        gui_api_xvb: &Arc<Mutex<PubXvbApi>>,
    ) -> Result<(), WorkerError> {
        let fallback = &self.fallback_pool;
        let pools_with_target = &self.pools_with_target;
        let hp = &self.hashrate_provider;
        let current_controllable_hr = hp.current_controllable_hr();
        let hr_fallback = current_controllable_hr
            - pools_with_target
                .iter()
                .map(|pwt: &Target| pwt.target_hr)
                .sum::<f32>();

        let duration_fallback = self.timeframe.as_secs_f32()
            * (hr_fallback / self.hashrate_provider.current_controllable_hr()).min(1.0);
        gui_api_xvb.lock().unwrap().stats_priv.time_donated =
            self.timeframe.as_secs_f32() - duration_fallback;
        info!(
            "duration fallback: {}s = timeframe {} * (hr_fallback {} / current controllable hr {}).max(1.0)",
            duration_fallback,
            self.timeframe.as_secs_f32(),
            hr_fallback,
            self.hashrate_provider.current_controllable_hr(),
        );

        if duration_fallback > 0.0 {
            output_console(
                &mut gui_api_xvb.lock().unwrap().output,
                &format!("Sending {duration_fallback}s to {}", fallback),
                ProcessName::Xvb,
            );
            if hp.current_pool().as_ref() != Some(fallback) {
                hp.update_config(fallback).await?;
            }
            info!("sleeping for fallback");
            sleep(Duration::from_secs_f32(duration_fallback)).await;
            info!("end of sleep");
        }

        for target in self.pools_with_target.iter() {
            let duration_target = self.timeframe.as_secs_f32()
                * (target.target_hr / current_controllable_hr).min(1.0);
            info!(
                "duration target {}: {}s = timeframe {} * (target_hr {} / current controllable hr {}).max(1.0)",
                target.pool,
                duration_target,
                self.timeframe.as_secs_f32(),
                target.target_hr,
                self.hashrate_provider.current_controllable_hr(),
            );
            if duration_target > 0.0 {
                output_console(
                    &mut gui_api_xvb.lock().unwrap().output,
                    &format!("Sending {duration_target}s to {}", target.pool),
                    ProcessName::Xvb,
                );
                if hp.current_pool().as_ref() != Some(&target.pool) {
                    hp.update_config(&target.pool).await?;
                }
                info!("sleeping for target");
                sleep(Duration::from_secs_f32(duration_target)).await;
                info!("end of sleep");
            }
        }
        Ok(())
    }
}
