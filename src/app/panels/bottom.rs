use std::sync::{Arc, OnceLock};

use crate::app::eframe_impl::{ProcessStateGui, ProcessStatesGui};
use crate::app::submenu_enum::{Submenu, SubmenuP2pool, SubmenuStatus};
use crate::app::{Restart, keys::KeyPressed};
use crate::disk::node::Node;
use crate::disk::pool::Pool;
use crate::disk::state::{Gupax, GupaxTheme, State};
use crate::helper::node::{CheckLocalOutsideNode, spawn_local_outside_checker};
use crate::helper::{Helper, ProcessName, ProcessState};
use crate::utils::constants::*;
use crate::utils::errors::{ErrorButtons, ErrorFerris};
use crate::utils::macros::sleep;
use crate::utils::regex::Regexes;
use egui::*;
use log::{debug, error, warn};
use strum::IntoEnumIterator;

use crate::app::Tab;
use crate::helper::ProcessState::*;
impl crate::app::App {
    #[allow(clippy::too_many_arguments)]
    pub fn bottom_panel(
        &mut self,
        ctx: &egui::Context,
        key: &KeyPressed,
        wants_input: bool,
        states: &ProcessStatesGui,
    ) {
        // Bottom: app info + state/process buttons
        debug!("App | Rendering BOTTOM bar");
        TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.style_mut().override_text_style = Some(TextStyle::Button);
            let size_font = ui
                .style()
                .text_styles
                .get(&TextStyle::Monospace)
                .expect("Monospace should be defined at startup")
                .size;
            let extra_separator = size_font * 0.7;
            let bar_height = size_font * 1.7;
            let tiny_width = ui.available_width() < APP_DEFAULT_WIDTH;
            // [(status group)(run)(2 submenus)(save/reset)]
            // [(status group)(3 submenus)(save/reset)]
            // [(status group)(space)(save/reset)]
            ScrollArea::horizontal()
                .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing.x = if !tiny_width {
                        ui.available_width() / 200.0
                    } else {
                        ui.style_mut().spacing.window_margin.left = 0;
                        ui.style_mut().spacing.window_margin.right = 0;
                        ui.style_mut().spacing.window_margin.top = 0;
                        ui.style_mut().spacing.window_margin.bottom = 0;
                        // let a minimum space between widget
                        3.0
                    };
                    // ui.style_mut().spacing.item_spacing.y = 0.0;
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.group(|ui| {
                            self.version(ui, bar_height);
                            ui.add(Separator::default().grow(extra_separator));
                            ui.label(self.os);
                            ui.add(Separator::default().grow(extra_separator));
                            self.theme_show(ui, ctx);
                            // width of each status
                            let width_status = if !tiny_width {
                                ((ui.available_width()
                                    / 3.0
                                    / states
                                        .iter()
                                        .filter(|s| {
                                            self.state.gupax.show_processes.contains(&s.name)
                                        })
                                        .count() as f32)
                                    - spacing(ui))
                                .max(0.0)
                            } else {
                                0.0
                            };
                            states
                                .iter()
                                .filter(|s| self.state.gupax.show_processes.contains(&s.name))
                                .for_each(|p| {
                                    ui.add(Separator::default().grow(extra_separator));
                                    // width must be minimum if less than 16px is available.
                                    Self::status_process(p, ui, width_status);
                                });
                        });

                        if let Some(name) = self.tab.linked_process() {
                            // add space, smaller when run actions
                            let width = ui.available_width() / 16.0;
                            if !tiny_width {
                                ui.add_space(width);
                            }
                            self.run_actions(ui, states.find(name), key, wants_input);
                        } else if self.tab != Tab::About {
                            // bigger space for other tab
                            let width = ui.available_width() / 8.0;
                            if !tiny_width {
                                ui.add_space(width);
                            }
                        } else {
                            // even bigger for about tab
                            let width = ui.available_width() / 2.0;
                            if !tiny_width {
                                ui.add_space(width);
                            }
                        }

                        self.submenu(ui);
                        self.save_reset_ui(ui, key, wants_input);
                    });
                });
        });
    }

    fn version(&self, ui: &mut Ui, height: f32) {
        // ui.add_space(space);
        match *self.restart.lock().unwrap() {
            Restart::Yes => ui
                .add_sized(
                    [0.0, height],
                    Label::new(RichText::new(&self.name_version).color(YELLOW)),
                )
                .on_hover_text(GUPAX_SHOULD_RESTART),
            _ => ui.add_sized([0.0, height], Label::new(&self.name_version)),
        };
    }
    fn theme_show(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let icon = match self.state.gupax.theme {
            GupaxTheme::Dark => "🌙",
            GupaxTheme::Light => "🌞",
            GupaxTheme::System => "🔳",
        };
        if ui
            .add(Button::new(icon))
            .on_hover_text_at_pointer(self.state.gupax.theme.helper())
            .clicked()
        {
            self.toggle_theme(ctx);
        }
    }
    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.state.gupax.theme = match self.state.gupax.theme {
            GupaxTheme::Dark => GupaxTheme::Light,
            GupaxTheme::Light => GupaxTheme::System,
            GupaxTheme::System => GupaxTheme::Dark,
        };

        self.set_theme(ctx);
    }
    fn status_process(process: &ProcessStateGui, ui: &mut Ui, width: f32) {
        let color;
        let hover_text = match process.state {
            Alive => {
                color = GREEN;
                match process.name {
                    ProcessName::Node => NODE_ALIVE,
                    ProcessName::P2pool => P2POOL_ALIVE,
                    ProcessName::Xmrig => XMRIG_ALIVE,
                    ProcessName::XmrigProxy => XMRIG_PROXY_ALIVE,
                    ProcessName::Xvb => XVB_ALIVE,
                }
            }
            Dead => {
                color = GRAY;
                match process.name {
                    ProcessName::Node => NODE_DEAD,
                    ProcessName::P2pool => P2POOL_DEAD,
                    ProcessName::Xmrig => XMRIG_DEAD,
                    ProcessName::XmrigProxy => XMRIG_PROXY_DEAD,
                    ProcessName::Xvb => XVB_DEAD,
                }
            }
            Failed => {
                color = RED;
                match process.name {
                    ProcessName::Node => NODE_FAILED,
                    ProcessName::P2pool => P2POOL_FAILED,
                    ProcessName::Xmrig => XMRIG_FAILED,
                    ProcessName::XmrigProxy => XMRIG_PROXY_FAILED,
                    ProcessName::Xvb => XVB_FAILED,
                }
            }
            Syncing | NotMining | OfflinePoolsAll => {
                color = ORANGE;
                match process.name {
                    ProcessName::Node => NODE_SYNCING,
                    ProcessName::P2pool => P2POOL_SYNCING,
                    ProcessName::Xmrig => XMRIG_NOT_MINING,
                    ProcessName::XmrigProxy => XMRIG_PROXY_NOT_MINING,
                    ProcessName::Xvb => XVB_PUBLIC_ONLY,
                }
            }
            Middle | Waiting => {
                color = YELLOW;
                match process.name {
                    ProcessName::Node => NODE_MIDDLE,
                    ProcessName::P2pool => P2POOL_MIDDLE,
                    ProcessName::Xmrig => XMRIG_MIDDLE,
                    ProcessName::XmrigProxy => XMRIG_PROXY_MIDDLE,
                    ProcessName::Xvb => XVB_MIDDLE,
                }
            }
        };
        let text = format!("{} ⏺", process.name);
        ui.add_sized(
            [width, ui.available_height()],
            Label::new(RichText::new(text).color(color)),
        )
        .on_hover_text(hover_text);
    }
    fn run_actions(
        &mut self,
        ui: &mut Ui,
        process: &ProcessStateGui,
        key: &KeyPressed,
        wants_input: bool,
    ) {
        ui.group(|ui| {
            // width is left available width divided by 5 (5 widgets) and 3 (3 buttons) less spacing between widget.
            let spacing = spacing(ui);
            let size = [
                ((ui.available_width() / 5.0 / 3.0) - spacing).max(0.0),
                ui.available_height(),
            ];
            let name = process.name;
            let stop_msg = format!("Stop {name}");
            let start_msg = format!("Start {name}");
            let restart_msg = format!("Restart {name}");
            if process.waiting {
                ui.add_enabled_ui(false, |ui| {
                    ui.add_sized(size, Button::new("▶"))
                        .on_disabled_hover_text(process.run_middle_msg());
                    ui.add(Separator::default().grow(0.0));
                    ui.add_sized(size, Button::new("⏹"))
                        .on_disabled_hover_text(process.run_middle_msg());
                    ui.add(Separator::default().grow(0.0));
                    ui.add_sized(size, Button::new("⟲"))
                        .on_disabled_hover_text(process.run_middle_msg());
                });
            } else if process.alive {
                ui.add_enabled_ui(false, |ui| {
                    ui.add_sized(size, Button::new("▶"))
                        .on_disabled_hover_text(start_msg)
                });
                ui.add(Separator::default().grow(0.0));
                if key.is_down() && !wants_input
                    || ui
                        .add_sized(size, Button::new("⏹"))
                        .on_hover_text(stop_msg)
                        .clicked()
                {
                    process.stop(&self.helper);
                }
                ui.add(Separator::default().grow(0.0));
                if key.is_up() && !wants_input
                    || ui
                        .add_sized(size, Button::new("⟲"))
                        .on_hover_text(restart_msg)
                        .clicked()
                {
                    let _ = self.og.lock().unwrap().update_absolute_path();
                    let _ = self.state.update_absolute_path();
                    // could improve this code with helper
                    match process.name {
                        ProcessName::Node => {
                            Helper::restart_node(
                                &self.helper,
                                &self.state.node,
                                &self.state.gupax.absolute_node_path,
                            );
                        }
                        ProcessName::P2pool => {
                            Helper::restart_p2pool(
                                &self.helper,
                                &self.state.p2pool,
                                &self.state.node,
                                &self.state.gupax.absolute_p2pool_path,
                                self.backup_hosts.clone(),
                                false,
                                &self.crawler
                            );
                        }
                        ProcessName::Xmrig => {
                                Helper::restart_xmrig(
                                    &self.helper,
                                    &self.state.xmrig,
                                    &self.state.p2pool,
                                    &self.state.xmrig_proxy,
                                    &self.state.gupax.absolute_xmrig_path,
                                );
                        }
                        ProcessName::XmrigProxy => {
                            Helper::restart_xp(
                                &self.helper,
                                &self.state.xmrig_proxy,
                                &self.state.p2pool,
                                &self.state.gupax.absolute_xp_path,
                            );
                        }
                        ProcessName::Xvb => {
                            Helper::restart_xvb(
                                &self.helper,
                                &self.state.xvb,
                                &self.state.p2pool,
                                &self.state.xmrig,
                                &self.state.xmrig_proxy,
                            );
                        }
                    }
                }
            } else {
                let text_err = self.start_ready(process).err().unwrap_or_default();
                let ui_enabled = text_err.is_empty();
                ui.add_enabled_ui(ui_enabled, |ui| {
                    let color = if ui_enabled { GREEN } else { RED };
                    if (ui_enabled && key.is_up() && !wants_input)
                        || ui
                            .add_sized(size, Button::new(RichText::new("▶").color(color)))
                            .on_hover_text(start_msg)
                            .on_disabled_hover_text(text_err)
                            .clicked()
                    {
                        // check if process is running outside of Gupax, warn about it and do not start it.
                        // Except for Node which will be treated differently.
                        if name != ProcessName::Node && name.is_process_running(&mut self.helper.lock().unwrap().sys_info.lock().unwrap())   {
                            error!("Process already running outside: {name}");
                            self.error_state.set(
                                PROCESS_OUTSIDE,
                                ErrorFerris::Error,
                                ErrorButtons::Okay,
                            );
                            return;
                        }
                        let _ = self.og.lock().unwrap().update_absolute_path();
                        let _ = self.state.update_absolute_path();
                        // start process
                        match process.name {
                            ProcessName::Node => {
                            // check if a local node is running outside of gupax. If that's the case, check if it's compatible with p2pool. If that's the case, offer the choice to the user to use it.
                            // The scanning of nodes should take less than 100ms since it's local
                            // the checking can take a bit of time, specially if a node is running with opened ports but not responding as expected.
                            // The check needs to run in another thread.
                            let check_local_outside = Arc::new(OnceLock::new());

                            spawn_local_outside_checker(check_local_outside.clone());
                            let mut count = 0;
                            while check_local_outside.get().is_none() {
                                // a precaution to free the UI from the freeze after 500ms if something wrong happens with the check and it get stuck.
                                if count >= 300 {break}
                                // wait just a little bit with the UI freezing, it should take under 100ms which is too short to be annoying.
                                sleep!(10);
                                count += 1;
                            }
                            if let Some(check) = check_local_outside.get() {
                                match check {
                                    CheckLocalOutsideNode::Valid(rpc_port, zmq_port) => {
                                        // show window prompt to ask the user if they want to use the Node outside Gupax.
                                        // But the prompt will only come with the next refresh of frames.
                                        warn!("A monero Node is already running outside of Gupax and can be used for p2pool");
                                        self.error_state.set(NODE_START_DETECT_VALID,ErrorFerris::Cute,ErrorButtons::UseDetectedLocalNode((*rpc_port, *zmq_port)));
                                        // the start will start from the prompt
                                    }
                                    CheckLocalOutsideNode::NonValid => {
                                        // show window with error explaining a node is running but can be used
                                        error!("A monero Node is already running outside of Gupax and can not be used for p2pool");
                                        self.error_state.set(NODE_START_DETECT_NON_VALID,ErrorFerris::Oops,ErrorButtons::Okay);
                                    }
                                    CheckLocalOutsideNode::None => {
                                        // No running local outside node, nothing special to do. Start normally.
                                        Helper::start_node(
                                        &self.helper,
                                        &self.state.node,
                                        &self.state.gupax.absolute_node_path);                                    }
                                    }
                                } else {
                                    // if for some reason we were stuck for 3s, we go back to process_running check
                                    if name.is_process_running(&mut self.helper.lock().unwrap().sys_info.lock().unwrap())  {
                            error!("Process already running outside: {name}");
                            self.error_state.set(
                                PROCESS_OUTSIDE,
                                ErrorFerris::Error,
                                ErrorButtons::Okay,
                            );
                                } else {
                                        Helper::start_node(
                                        &self.helper,
                                        &self.state.node,
                                        &self.state.gupax.absolute_node_path);                                    }
                                }
                                },
                            ProcessName::P2pool => {
                            // check if button to use local node is checked and if the local node is running
                            // It prevents starting p2pool if the node is not ready
                            // It is much clearer for the user what the issue is than trying to debug the terminal output.
                            // Still allow the user to continue if the issue is that the node is syncing.
                            // Could also be used with a remote node if we make a request
                            if self.state.p2pool.local_node && self.node.lock().unwrap().state == ProcessState::Syncing {
                                warn!("Trying to start {name} with an unsynced Node");
                                self.error_state.set(
                                    "P2Pool needs a fully synced Node.\nThe one selected is not. You can continue to start P2Pool but it won't be ready until the Node is synced".to_string(),
                                    ErrorFerris::Cute,
                                    ErrorButtons::UseNonSyncedNode,
                                );
                                return;
                            }
                            else if self.state.p2pool.local_node && self.node.lock().unwrap().state != ProcessState::Alive {
                                error!(" {name} needs {} to start because the button \"{P2POOL_USE_LOCAL_NODE_BUTTON}\", but it was not ready", ProcessName::Node);
                                self.error_state.set(
                                    format!("P2Pool needs a local Node to be alive and well. Check the Node tab to start one or uncheck the button \"{P2POOL_USE_LOCAL_NODE_BUTTON}\""),
                                    ErrorFerris::Error,
                                    ErrorButtons::Okay,
                                );
                                return;
                            }
                            Helper::start_p2pool(
                                &self.helper,
                                &self.state.p2pool,
                                &self.state.node,
                                &self.state.gupax.absolute_p2pool_path,
                                &self.backup_hosts.clone(),
                                false,
                                &self.crawler
                            )
                            },

                            ProcessName::Xmrig => {
                                    Helper::start_xmrig(
                                        &self.helper,
                                        &self.state.xmrig,
                                        &self.state.p2pool,
                                        &self.state.xmrig_proxy,
                                        &self.state.gupax.absolute_xmrig_path,
                                    );
                            }

                            ProcessName::XmrigProxy => Helper::start_xp(
                                &self.helper,
                                &self.state.xmrig_proxy,
                                &self.state.p2pool,
                                &self.state.gupax.absolute_xp_path,
                            ),
                            ProcessName::Xvb => Helper::start_xvb(
                                &self.helper,
                                &self.state.xvb,
                                &self.state.p2pool,
                                &self.state.xmrig,
                                &self.state.xmrig_proxy,
                            ),
                        }
                    }
                });
                    ui.add(Separator::default().grow(0.0));
                ui.add_enabled_ui(false, |ui| {
                    ui.add_sized(size, Button::new("⏹"))
                        .on_disabled_hover_text(stop_msg);
                    ui.add(Separator::default().grow(0.0));
                    ui.add_sized(size, Button::new("⟲"))
                        .on_disabled_hover_text(restart_msg);
                    ui.add(Separator::default().grow(0.0));
                });
            }
        });
    }
    fn submenu(&mut self, ui: &mut Ui) {
        match self.tab {
            Tab::About => {}
            Tab::Gupax => self.gupax_submenu(ui),
            Tab::Status => Self::status_submenu(&mut self.state.status.submenu, ui),
            Tab::Node => self.node_submenu(ui),
            Tab::P2pool => Self::p2pool_submenu(&mut self.state.p2pool.submenu, ui),
            Tab::Xmrig => self.xmrig_submenu(ui),
            Tab::XmrigProxy => self.xp_submenu(ui),
            Tab::Xvb => self.xvb_submenu(ui),
        }
    }
    fn gupax_submenu(&mut self, ui: &mut Ui) {
        Self::simple_advanced_submenu(
            ui,
            &mut self.state.gupax.simple,
            (GUPAX_SIMPLE, GUPAX_ADVANCED),
        );
    }
    fn node_submenu(&mut self, ui: &mut Ui) {
        Self::simple_advanced_submenu(
            ui,
            &mut self.state.node.simple,
            (NODE_SIMPLE, NODE_ADVANCED),
        );
    }
    fn xmrig_submenu(&mut self, ui: &mut Ui) {
        Self::simple_advanced_submenu(
            ui,
            &mut self.state.xmrig.simple,
            (XMRIG_SIMPLE, XMRIG_ADVANCED),
        );
    }
    fn xp_submenu(&mut self, ui: &mut Ui) {
        Self::simple_advanced_submenu(
            ui,
            &mut self.state.xmrig_proxy.simple,
            (XMRIG_PROXY_SIMPLE, XMRIG_PROXY_ADVANCED),
        );
    }
    fn xvb_submenu(&mut self, ui: &mut Ui) {
        Self::simple_advanced_submenu(ui, &mut self.state.xvb.simple, (XVB_SIMPLE, XVB_ADVANCED));
    }
    fn status_submenu(state_submenu: &mut SubmenuStatus, ui: &mut Ui) {
        ui.group(|ui| {
            let spacing = spacing(ui);
            // should be calculated from the len of variants and their name
            let width = ((ui.available_width() / 1.5 / 3.0) - spacing).max(0.0);
            let variants = SubmenuStatus::iter();
            let nb_variants = variants.len();
            for (nb, variant) in variants.enumerate() {
                if ui
                    .add_sized(
                        [width, ui.available_height()],
                        Button::selectable(*state_submenu == variant, variant.to_string()),
                    )
                    .on_hover_text(variant.hover_text())
                    .clicked()
                {
                    *state_submenu = variant;
                }
                if nb != nb_variants - 1 {
                    ui.separator();
                }
            }
        });
    }
    fn p2pool_submenu(state_submenu: &mut SubmenuP2pool, ui: &mut Ui) {
        ui.group(|ui| {
            let spacing = spacing(ui);
            // should be calculated from the len of variants and their name
            let width = ((ui.available_width() / 1.5 / 3.0) - spacing).max(0.0);
            let variants = SubmenuP2pool::iter();
            let nb_variants = variants.len();
            for (nb, variant) in variants.enumerate() {
                if ui
                    .add_sized(
                        [width, ui.available_height()],
                        Button::selectable(*state_submenu == variant, variant.to_string()),
                    )
                    .on_hover_text(variant.hover_text())
                    .clicked()
                {
                    *state_submenu = variant;
                }
                if nb != nb_variants - 1 {
                    ui.separator();
                }
            }
        });
    }

    fn simple_advanced_submenu(ui: &mut Ui, simple: &mut bool, hover_text: (&str, &str)) {
        let spacing = spacing(ui);
        let width = ((ui.available_width() - spacing) / 4.0).max(0.0);
        ui.group(|ui| {
            if ui
                .add_sized(
                    [width, ui.available_height()],
                    Button::selectable(*simple, "Simple"),
                )
                // .selectable_label(*value, "Simple")
                .on_hover_text(hover_text.0)
                .clicked()
            {
                *simple = true;
            }
            ui.separator();
            if ui
                .add_sized(
                    [width, ui.available_height()],
                    Button::selectable(!*simple, "Advanced"),
                )
                // .selectable_label(*value, "Advanced")
                .on_hover_text(hover_text.1)
                .clicked()
            {
                *simple = false;
            }
        });
    }
    fn save_reset_ui(&mut self, ui: &mut Ui, key: &KeyPressed, wants_input: bool) {
        ui.add_enabled_ui(self.diff, |ui| {
            ui.group(|ui| {
                let spacing = spacing(ui);
                let width = ((ui.available_width() - spacing) / 2.0).max(0.0);
                if key.is_s() && !wants_input && self.diff
                    || ui
                        .add_sized([width, ui.available_height()], Button::new("Save"))
                        .on_hover_text("Save changes")
                        .clicked()
                {
                    match State::save(&mut self.state, &self.state_path) {
                        Ok(_) => {
                            let mut og = self.og.lock().unwrap();
                            og.status = self.state.status.clone();
                            og.gupax = self.state.gupax.clone();
                            og.node = self.state.node.clone();
                            og.p2pool = self.state.p2pool.clone();
                            og.xmrig = self.state.xmrig.clone();
                            og.xmrig_proxy = self.state.xmrig_proxy.clone();
                            og.xvb = self.state.xvb.clone();
                        }
                        Err(e) => {
                            self.error_state.set(
                                format!("State file: {e}"),
                                ErrorFerris::Error,
                                ErrorButtons::Okay,
                            );
                        }
                    };
                    match Node::save(&self.node_vec, &self.node_path) {
                        Ok(_) => self.og_node_vec.clone_from(&self.node_vec),
                        Err(e) => self.error_state.set(
                            format!("Node list: {e}"),
                            ErrorFerris::Error,
                            ErrorButtons::Okay,
                        ),
                    };
                    match Pool::save(&self.pool_vec, &self.pool_path) {
                        Ok(_) => self.og_pool_vec.clone_from(&self.pool_vec),
                        Err(e) => self.error_state.set(
                            format!("Pool list: {e}"),
                            ErrorFerris::Error,
                            ErrorButtons::Okay,
                        ),
                    };
                }
                ui.add(Separator::default().grow(0.0));
                if key.is_r() && !wants_input && self.diff
                    || ui
                        .add_sized([width, ui.available_height()], Button::new("Reset"))
                        .on_hover_text("Reset changes")
                        .clicked()
                {
                    let og = self.og.lock().unwrap().clone();
                    self.state.status = og.status;
                    self.state.gupax = og.gupax;
                    self.state.node = og.node;
                    self.state.p2pool = og.p2pool;
                    self.state.xmrig = og.xmrig;
                    self.state.xmrig_proxy = og.xmrig_proxy;
                    self.state.xvb = og.xvb;
                    self.node_vec.clone_from(&self.og_node_vec);
                    self.pool_vec.clone_from(&self.og_pool_vec);
                }
            })
        });
    }
    pub fn start_ready(&self, state: &ProcessStateGui) -> Result<(), String> {
        // custom check and var
        let name = state.name;
        let path = match name {
            ProcessName::Node => {
                // check path of DB valid, empty valid.
                if !self.state.node.path_db.is_empty()
                    && !Gupax::path_is_dir(&self.state.node.path_db)
                {
                    return Err(format!("Error: {NODE_DB_DIR}"));
                }
                &self.state.gupax.node_path
            }
            ProcessName::P2pool => {
                // check if p2pool address is valid.
                if !Regexes::addr_ok(&self.state.p2pool.address) {
                    return Err(format!("Error: {P2POOL_ADDRESS}"));
                }
                // check if a remote node is selected if button unchecked and not in advanced mode
                if !self.state.p2pool.local_node
                    && self.state.p2pool.selected_remote_node.is_none()
                    && self.state.p2pool.submenu != SubmenuP2pool::Advanced
                {
                    return Err("Error: no Node for p2pool has been found.\nClick on the button to start finding one or check your connection.\nOtherwise you can use a local Node instead".to_string());
                }
                // check if a local node is not running and the button is checked
                // if self.state.p2pool.local_node &&
                &self.state.gupax.p2pool_path
            }
            ProcessName::Xmrig => &self.state.gupax.xmrig_path,
            ProcessName::XmrigProxy => &self.state.gupax.xmrig_proxy_path,
            ProcessName::Xvb => {
                if !Regexes::addr_ok(&self.state.p2pool.address) {
                    return Err(format!("Error: {XVB_NOT_CONFIGURED}"));
                }
                ""
            }
        };
        // check path of binary except for XvB
        if name != ProcessName::Xvb {
            if path.is_empty() {
                return Err(name.msg_binary_path_empty().to_string());
            }
            if !Gupax::path_is_file(path) {
                return Err(name.msg_binary_path_not_file().to_string());
            }
            if !crate::components::update::check_binary_path(path, name) {
                return Err(name.msg_binary_path_invalid().to_string());
            }
        }
        Ok(())
    }
}

fn spacing(ui: &Ui) -> f32 {
    (ui.style().spacing.item_spacing.x + ui.style().spacing.button_padding.x) * 2.0
}
