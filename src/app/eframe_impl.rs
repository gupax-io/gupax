use std::sync::{Arc, Mutex};

use super::App;
use crate::app::Tab;
use crate::app::submenu_enum::SubmenuP2pool;
use crate::components::node::RemoteNodes;
#[cfg(target_os = "windows")]
use crate::errors::{ErrorButtons, ErrorFerris};
use crate::helper::{Helper, ProcessName, ProcessState};
use crate::inits::init_text_styles;
use crate::{NODE_MIDDLE, P2POOL_MIDDLE, SECOND, XMRIG_MIDDLE, XMRIG_PROXY_MIDDLE, XVB_MIDDLE};
use derive_more::derive::{Deref, DerefMut};
use log::debug;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // *-------*
        // | DEBUG |
        // *-------*
        debug!("App | ----------- Start of [update()] -----------");
        // If closing
        self.quit(ctx);
        // Handle Keys
        let (key, wants_input) = self.keys_handle(ctx);

        // Refresh AT LEAST once a second
        debug!("App | Refreshing frame once per second");
        ctx.request_repaint_after(SECOND);

        // Get P2Pool/XMRig process state.
        // These values are checked multiple times so
        // might as well check only once here to save
        // on a bunch of [.lock().unwrap()]s.
        let mut process_states = ProcessStatesGui::new(self);
        // resize window and fonts if button "set" has been clicked in Gupax tab
        if self.must_resize {
            init_text_styles(ctx, self.state.gupax.selected_scale);
            self.must_resize = false;
        }
        // check for windows that a local instance of xmrig is not running outside of Gupax. Important because it could lead to crashes on this platform.
        // Warn only once per restart of Gupax.
        #[cfg(target_os = "windows")]
        if !self.xmrig_outside_warning_acknowledge
            && ProcessName::Xmrig
                .is_process_running(&mut self.helper.lock().unwrap().sys_info.lock().unwrap())
            && !process_states.find(ProcessName::Xmrig).alive
        {
            self.error_state.set("An instance of xmrig is running outside of Gupax.\nThis is not supported and could lead to crashes on this platform.\nPlease stop your local instance and start xmrig from Gupax Xmrig tab.", ErrorFerris::Error, ErrorButtons::Okay);
            self.xmrig_outside_warning_acknowledge = true;
        }
        // If there's an error, display [ErrorState] on the whole screen until user responds
        debug!("App | Checking if there is an error in [ErrorState]");
        if self.error_state.error {
            self.quit_error_panel(ctx, &process_states, &key);
            return;
        }
        // Compare [og == state] & [node_vec/pool_vec] and enable diff if found.
        // The struct fields are compared directly because [Version]
        // contains Arc<Mutex>'s that cannot be compared easily.
        // They don't need to be compared anyway.
        debug!("App | Checking diff between [og] & [state]");
        let og = self.og.lock().unwrap();
        self.diff = og.status != self.state.status
            || og.gupax != self.state.gupax
            || og.node != self.state.node
            || og.p2pool != self.state.p2pool
            || og.xmrig != self.state.xmrig
            || og.xmrig_proxy != self.state.xmrig_proxy
            || og.xvb != self.state.xvb
            || self.og_node_vec != self.node_vec
            || self.og_pool_vec != self.pool_vec;
        drop(og);

        // crawl/pinged/selected remote node refresh
        if self.state.gupax.auto.crawl || self.tab == Tab::P2pool {
            let mut crawler_lock = self.crawler.lock().unwrap();
            let mut ping_lock = self.ping.lock().unwrap();
            let crawling = crawler_lock.crawling;
            let ping_nodes = &mut ping_lock.nodes;
            let crawl_nodes = &mut crawler_lock.nodes;

            if *ping_nodes != *crawl_nodes && !crawl_nodes.is_empty() {
                *ping_nodes = crawl_nodes.clone();
                if !crawling {
                    *crawl_nodes = RemoteNodes::default();
                }
            }

            // refresh the selected node with the fastest from the pinged nodes if it was empty
            if self.state.p2pool.selected_remote_node.is_none() {
                self.state.p2pool.selected_remote_node = ping_nodes.first().cloned();
            }
        }
        // replace backup host by custom ones when user is in p2pool advanced sub menu
        // Only if the backup host is different from the custom ones
        if self.state.p2pool.submenu != SubmenuP2pool::Advanced && self.tab == Tab::P2pool {
            let mut backup_hosts = self.backup_hosts.lock().unwrap();
            if self.node_vec.iter().any(|(_, n)| backup_hosts.contains(n)) {
                *backup_hosts = self.node_vec.iter().map(|n| n.1.clone()).collect();
            }
        }

        self.top_panel(ctx);
        self.bottom_panel(ctx, &key, wants_input, &process_states);
        // xvb_is_alive is not the same for bottom and for middle.
        // for status we don't want to enable the column when it is retrying requests.
        // but also we don't want the user to be able to start it in this case.
        let p_xvb = process_states.find_mut(ProcessName::Xvb);
        p_xvb.alive = p_xvb.state != ProcessState::Dead;
        self.middle_panel(ctx, frame, key, &process_states);
    }
}
#[derive(Debug)]
pub struct ProcessStateGui {
    pub name: ProcessName,
    pub state: ProcessState,
    pub alive: bool,
    pub waiting: bool,
}

impl ProcessStateGui {
    pub fn run_middle_msg(&self) -> &str {
        match self.name {
            ProcessName::Node => NODE_MIDDLE,
            ProcessName::P2pool => P2POOL_MIDDLE,
            ProcessName::Xmrig => XMRIG_MIDDLE,
            ProcessName::XmrigProxy => XMRIG_PROXY_MIDDLE,
            ProcessName::Xvb => XVB_MIDDLE,
        }
    }
    pub fn stop(&self, helper: &Arc<Mutex<Helper>>) {
        match self.name {
            ProcessName::Node => Helper::stop_node(helper),
            ProcessName::P2pool => Helper::stop_p2pool(helper),
            ProcessName::Xmrig => Helper::stop_xmrig(helper),
            ProcessName::XmrigProxy => Helper::stop_xp(helper),
            ProcessName::Xvb => Helper::stop_xvb(helper),
        }
    }
}

#[derive(Deref, DerefMut, Debug)]
pub struct ProcessStatesGui(Vec<ProcessStateGui>);

impl ProcessStatesGui {
    // order is important for lock
    pub fn new(app: &App) -> Self {
        let mut process_states = ProcessStatesGui(vec![]);
        for process in [
            &app.node,
            &app.p2pool,
            &app.xmrig,
            &app.xmrig_proxy,
            &app.xvb,
        ] {
            let lock = process.lock().unwrap();
            process_states.push(ProcessStateGui {
                name: lock.name,
                alive: lock.is_alive(),
                waiting: lock.is_waiting(),
                state: lock.state,
            });
        }
        process_states
    }
    pub fn is_alive(&self, name: ProcessName) -> bool {
        self.iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("This vec should always contains all Processes {self:?}"))
            .alive
    }
    pub fn find(&self, name: ProcessName) -> &ProcessStateGui {
        self.iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("This vec should always contains all Processes {self:?}"))
    }
    pub fn find_mut(&mut self, name: ProcessName) -> &mut ProcessStateGui {
        self.iter_mut()
            .find(|p| p.name == name)
            .expect("This vec should always contains all Processes")
    }
}
