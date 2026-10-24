use crate::utils::i18n::{t, t_f};
use std::sync::{Arc, Mutex};

use egui::{
    Align, Button, Checkbox, ComboBox, Label, ProgressBar, RichText, ScrollArea, Slider, TextStyle,
    TextWrapMode, Ui, vec2,
};
use log::debug;

use crate::{
    app::BackupNodes,
    components::node::{Ping, RemoteNode, format_ms},
    disk::state::P2pool,
    helper::{crawler::Crawler, p2pool::PubP2poolApi},
    miscs::height_txt_before_button,
    utils::constants::{
        EXPECT_BUTTON_DISABLED, ORANGE, SPACE, button_disabled_by_empty_list_nodes,
        crawler_parameters_help, p2pool_auto_node, p2pool_autoswitch_local_node,
        p2pool_backup_host_simple, p2pool_community_node_warning, p2pool_ping,
        p2pool_select_fastest, p2pool_select_last, p2pool_select_next, p2pool_select_random,
    },
};

impl P2pool {
    pub(super) fn crawler(
        &mut self,
        ui: &mut Ui,
        crawler: &Arc<Mutex<Crawler>>,
        ping: &Arc<Mutex<Ping>>,
        api: &Arc<Mutex<PubP2poolApi>>,
        backup_nodes: BackupNodes,
    ) {
        self.crawl_button(crawler, backup_nodes, ui);
        self.crawl_parameters(ui);
        self.remote_nodes_menu(ui, api, crawler, ping);
    }

    fn crawl_parameters(&mut self, ui: &mut Ui) {
        let text_height = height_txt_before_button(ui, &TextStyle::Button);
        ScrollArea::horizontal()
            .id_salt("crawling parameters")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(ui.spacing().item_spacing.x);
                    ui.add_sized([0.0, text_height], Label::new(t!("Crawl Parameters")))
                        .on_hover_text(crawler_parameters_help());
                });
                ui.group(|ui| {
                    ui.set_max_width(0.0);
                    self.slider_latency(ui);
                    self.slider_nb_fast_nodes(ui);
                    self.slider_nb_medium_nodes(ui);
                    self.slider_timeout(ui);
                    ui.add_space(ui.spacing().item_spacing.x);
                });
            });
    }

    fn remote_nodes_menu(
        &mut self,
        ui: &mut Ui,
        api: &Arc<Mutex<PubP2poolApi>>,
        crawler: &Arc<Mutex<Crawler>>,
        ping: &Arc<Mutex<Ping>>,
    ) {
        ui.vertical(|ui| {
            let crawling = crawler.lock().unwrap().crawling;
            self.list_nodes(ping, crawling, ui);
            ui.add_space(SPACE);
            self.list_nodes_buttons(ping, ui);
            ui.vertical_centered(|ui| {
                ping_progress(ping, ui);
            });
            self.auto_buttons(api, ui);

            warning_should_run_local_node(ui);
        });
    }

    fn list_nodes(&mut self, ping: &Arc<Mutex<Ping>>, crawling: bool, ui: &mut Ui) {
        // don't show anything if there is no nodes nor selected_saved node
        if let Some(selected_saved_node) = self.selected_remote_node.as_mut() {
            // Take the first (fastest) node to replace the selected saved node if it is not present in the found nodes or if the crawler is running.
            // If the crawler is running, we consider the user doesn't care about the last selected node, so we update it to be the fastest.
            // If the crawler is not running, the selected node can be changed for the fastest if it is not in the list.
            let ping_nodes = &mut ping.lock().unwrap().nodes;

            // refresh the selected node with the fastest found
            if crawling && let Some(node) = ping_nodes.first() {
                *selected_saved_node = node.clone();
            }

            // if the selected does not exist anymore, take the fastest
            if !ping_nodes.iter().any(|n| n == selected_saved_node)
                && let Some(node) = ping_nodes.first()
            {
                *selected_saved_node = node.clone();
            }

            // if there is no nodes found, add it to list so it can get pinged
            if ping_nodes.is_empty() {
                ping_nodes.push(selected_saved_node.clone());
            }

            // the selected node saved in the state file will not include the latency
            // The real data must be taken from the ping nodes
            // If the selected node is not present in ping, takes the data from state
            let selected_node = if let Some(node) = ping_nodes.find_selected(selected_saved_node) {
                node
            } else {
                &selected_saved_node.clone()
            };

            // If some node are found, make the selectable list visible
            ui.horizontal(|ui| {
                debug!("P2Pool Tab | Rendering [Ping List]");

                debug!("P2Pool Tab | Rendering [ComboBox] of Remote Nodes");
                let ping_msg = if selected_node.ms == 0 {
                    t!("Latency not measured").to_string()
                } else {
                    format_ms(selected_node.ms).to_string()
                };
                let ip = selected_node.ip.to_string();
                let text = RichText::new(format!(" ⏺ {ping_msg} | {ip}"))
                    .color(selected_node.ping_color());
                ui.style_mut().override_text_valign = Some(Align::Center);
                ui.vertical_centered(|ui| {
                    let screen_size = ui.ctx().content_rect().size();
                    // ui.set_max_size(screen_size);
                    ui.set_max_height(screen_size.y);
                    let width = ui.available_width();
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.ctx().request_repaint();
                        ComboBox::from_id_salt("remote_nodes")
                            .selected_text(text)
                            .width(ui.available_width())
                            .height(ui.available_height())
                            .show_ui(ui, |ui| {
                                // space width fix
                                // doesn't work with height, fix is under
                                ui.set_max_width(width - SPACE);
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                for data in ping_nodes.iter() {
                                    let ip = data.ip.to_string();
                                    let ping_msg = if data.ms == 0 {
                                        t!("Latency not measured").to_string()
                                    } else {
                                        format_ms(data.ms).to_string()
                                    };

                                    let text = RichText::new(format!(" ⏺ {ping_msg} | {ip}"))
                                        .color(data.ping_color());
                                    ui.selectable_value(selected_saved_node, data.clone(), text);
                                }
                                // space height fix
                                // the combo box will keep the height of the first time the menu is clicked.
                                // So if the user click on the menu with only one item, it will keep the same small height when
                                // the menu will have more items, which make it ugly because it forces the user to
                                // use the scrollbar when there was enough space to show more of them.
                                // https://github.com/emilk/egui/issues/5225
                                //
                                // The fix is to preallocate the space

                                if ping_nodes.len() < 4 {
                                    for _ in 1..(5 - ping_nodes.len()) {
                                        //

                                        ui.label(String::new());
                                    }
                                }
                                // });
                            });
                    });
                });
            });
        }
    }
    fn list_nodes_buttons(&mut self, ping: &Arc<Mutex<Ping>>, ui: &mut Ui) {
        if let Some(selected_node) = self.selected_remote_node.as_mut() {
            debug!("P2Pool Tab | Rendering [Select fastest ... Ping] buttons");
            ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .id_salt("horizontal")
                .show(ui, |ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Button);
                    ui.horizontal(|ui| {
                        // style of buttons
                        ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                        let width = ((ui.available_width() / 5.0)
                            - (ui.spacing().item_spacing.x * (4.0 / 5.0)))
                            .max(20.0);
                        let height = height_txt_before_button(ui, &TextStyle::Button) * 2.0;
                        ui.style_mut().override_text_valign = Some(egui::Align::Center);

                        // [Select random node]
                        select_random_node_button(ping, selected_node, width, height, ui);
                        // [Select fastest node]
                        select_fastest_node_button(ping, selected_node, width, height, ui);
                        // [Ping Button]
                        ping_button(ping, width, height, ui);
                        // [Last <-]
                        last_node_button(width, height, selected_node, ping, ui);
                        // [Next ->]
                        next_node_button(width, height, selected_node, ping, ui);
                    });
                });
        }
    }
    fn auto_buttons(&mut self, api: &Arc<Mutex<PubP2poolApi>>, ui: &mut Ui) {
        debug!("P2Pool Tab | Rendering [Auto-*] buttons");
        ui.group(|ui| {
            ui.horizontal(|ui| {
                let width = (((ui.available_width() - ui.spacing().item_spacing.x) / 3.0)
                    - SPACE * 1.5)
                    .max(ui.text_style_height(&TextStyle::Button) * 7.0);
                let size = vec2(
                    width,
                    height_txt_before_button(ui, &TextStyle::Button) * 2.0,
                );
                // Auto-Select doesn't make sense since we don't store the list of remote nodes across restart, only the selected node.
                // ui.add_sized(size, Checkbox::new(&mut self.auto_select, "Auto-select"))
                //     .on_hover_text(p2pool_auto_select());
                // ui.separator();
                ui.add_sized(size, Checkbox::new(&mut self.auto_ping, t!("Auto-ping")))
                    .on_hover_text(p2pool_auto_node());
                ui.separator();
                ui.add_sized(
                    size,
                    Checkbox::new(&mut self.backup_host, t!("Backup host")),
                )
                .on_hover_text(p2pool_backup_host_simple());
                ui.separator();
                // set preferred local node immediately if we are on simple mode.
                if ui
                    .add_sized(
                        size,
                        Checkbox::new(&mut self.prefer_local_node, t!("Auto-Switch to Local Node")),
                    )
                    .on_hover_text(p2pool_autoswitch_local_node())
                    .clicked()
                {
                    api.lock().unwrap().prefer_local_node = self.prefer_local_node;
                }
            })
        });
    }
    pub fn crawl_button(
        &mut self,
        crawler: &Arc<Mutex<Crawler>>,
        backup_hosts: BackupNodes,
        ui: &mut Ui,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(SPACE);
            let crawling = crawler.lock().unwrap().crawling;
            let button_crawl_text = if crawling {
                t!("Stop finding P2Pool compatible Nodes")
            } else {
                t!("Find P2Pool compatible Nodes")
            };
            // prevent user clicking on button if it's currently stopping
            let stopping = crawler.lock().unwrap().stopping;

            ui.add_enabled_ui(!stopping, |ui| {
                if ui
                    .button(button_crawl_text)
                    .on_hover_text(t!("It will reset the current found nodes"))
                    .clicked()
                {
                    if !crawling {
                        self.selected_remote_node = None;
                        Crawler::start(crawler, &self.crawl_settings, Some(backup_hosts));
                    } else {
                        crawler.lock().unwrap().stopping = true;
                        Crawler::stop(crawler);
                    }
                }
            })
            .response
            .on_disabled_hover_text(t!("Stopping the crawling..."));
            crawl_progress(crawler, ui);
        });
    }
    fn slider_latency(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui|{
        let text_height = height_txt_before_button(ui, &TextStyle::Button);
        ui.add_sized([0.0, text_height], Label::new(t!("Max latency (ms) of fast nodes")));
        ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 10.0;
        ui.add_sized(
            [ui.available_width(), text_height],
            Slider::new(&mut self.crawl_settings.max_ping_fast, 0..=100)
                .text("ms")
        )
        .on_hover_text(t!("Set the maximum latency in millisecond where a found capable node is considered fast."));
        });
        // TODO make a state builder
        // slider_state_field(
        //     ui,
        //     "Max latency (ms) of fast nodes",
        //     "Set the maximum latency in millisecond where a found capable node is considered fast.",
        //     &mut self.crawl_settings.max_ping_fast,
        //     0..=100,
        //     "ms"
        // );
    }
    fn slider_nb_fast_nodes(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // let text_height = height_txt_before_button(ui, &TextStyle::Heading) * 1.4;
            let text_height = height_txt_before_button(ui, &TextStyle::Button);
            ui.add_sized(
                [0.0, text_height],
                Label::new(t!("Number of                     ")),
            );
            // ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 18.0;
            ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 10.0;
            ui.add_sized(
                [ui.available_width(), text_height],
                Slider::new(&mut self.crawl_settings.nb_nodes_fast, 1..=10).text(t!("Fast Nodes")),
            )
            .on_hover_text(t!(
                "How many fast nodes the crawler must find before stopping"
            ));
        });
    }
    fn slider_nb_medium_nodes(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui|{
        let text_height = height_txt_before_button(ui, &TextStyle::Button);
        ui.add_sized([0.0, text_height], Label::new(t!("Maximum number of             ")));
        ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 10.0;
        ui.add_sized(
            [ui.available_width(), text_height],
            Slider::new(&mut self.crawl_settings.nb_nodes_medium, 1..=10)
                .text(t!("Medium Nodes"))
        )
        .on_hover_text(t!("How many mediumly fast nodes the crawler must find before replacing the slowest ones.\nUsefull in case no fast nodes are found or to be used as backup nodes"));
        });
    }
    fn slider_timeout(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui|{
        let text_height = height_txt_before_button(ui, &TextStyle::Button);
        ui.add_sized([0.0, text_height], Label::new(t!("Timeout                       ")));
        ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 10.0;
        ui.add_sized(
            [ui.available_width(), text_height],
            Slider::new(&mut self.crawl_settings.timeout, 1..=600)
                .text(t!("seconds"))
        )
        .on_hover_text(t!("Maximum duration for the crawler before terminationg even if it didn't reach the requirements.\nConsider raising this value if you don't find enough fast nodes"));
        });
    }
}

pub fn warning_should_run_local_node(ui: &mut Ui) {
    debug!("P2Pool Tab | Rendering warning text");
    ui.add_space(SPACE);
    ui.vertical_centered(|ui| {
        ui.add(Label::new(
            RichText::new(t!(
                "WARNING: It is recommended to run/use your own Monero Node (hover for details)"
            ))
            .color(ORANGE),
        ))
        .on_hover_text(p2pool_community_node_warning());
    });
}
fn crawl_progress(crawler: &Arc<Mutex<Crawler>>, ui: &mut Ui) {
    // let height = height / 2.0;
    let crawling = crawler.lock().unwrap().crawling;
    ui.add_enabled_ui(crawling, |ui| {
        let prog = crawler.lock().unwrap().prog.round();
        let msg = if prog < 100.0 {
            RichText::new(format!("{} ... {:?}%", crawler.lock().unwrap().msg, prog))
        } else {
            RichText::new(t_f!("Crawling completed\n{}", crawler.lock().unwrap().msg))
        };
        ui.add_space(SPACE);
        ui.label(msg);
        ui.add_space(SPACE);
        if crawling {
            ui.spinner();
        }
        ui.add(ProgressBar::new(prog.round() / 100.0));
        ui.add_space(SPACE);
    });
}
fn ping_progress(ping: &Arc<Mutex<Ping>>, ui: &mut Ui) {
    let pinging = ping.lock().unwrap().pinging;
    ui.add_enabled_ui(pinging, |ui| {
        let prog = ping.lock().unwrap().prog.round();
        let msg = RichText::new(format!("{} ... {}%", ping.lock().unwrap().msg, prog));
        ui.add_space(SPACE);
        ui.label(msg);
        ui.add_space(SPACE);
        if pinging {
            ui.spinner();
        } else {
            ui.label("...");
        }
        ui.add(ProgressBar::new(prog.round() / 100.0));
        ui.add_space(SPACE);
    });
}
fn last_node_button(
    width: f32,
    height: f32,
    selected_node: &mut RemoteNode,
    ping: &Arc<Mutex<Ping>>,
    ui: &mut Ui,
) {
    // make sense to use only if there is more than one found node.
    let ping_lock = ping.lock().unwrap();
    let found_multiple_nodes = ping_lock.nodes.len() > 1;
    ui.add_enabled_ui(found_multiple_nodes, |ui| {
        if ui
            .add_sized([width, height], Button::new("⬅ Last"))
            .on_hover_text(p2pool_select_last())
            .clicked()
        {
            match ping_lock.pinged {
                true => *selected_node = ping_lock.get_last_from_ping(selected_node),
                false => *selected_node = ping_lock.nodes.get_last(selected_node),
            }
        }
    })
    .response
    .on_disabled_hover_text(button_disabled_by_empty_list_nodes());
}
fn next_node_button(
    width: f32,
    height: f32,
    selected_node: &mut RemoteNode,
    ping: &Arc<Mutex<Ping>>,
    ui: &mut Ui,
) {
    // make sense to use only if there is more than one found node.
    let ping_lock = ping.lock().unwrap();
    let found_multiple_nodes = ping_lock.nodes.len() > 1;
    ui.add_enabled_ui(found_multiple_nodes, |ui| {
        if ui
            .add_sized([width, height], Button::new(t!("Next ➡")))
            .on_hover_text(p2pool_select_next())
            .clicked()
        {
            match ping_lock.pinged {
                true => *selected_node = ping_lock.get_next_from_ping(selected_node),
                false => *selected_node = ping_lock.nodes.get_next(selected_node),
            }
        }
    });
}
fn ping_button(ping: &Arc<Mutex<Ping>>, width: f32, height: f32, ui: &mut Ui) {
    // disable if no nodes were found or if we already are pinging
    let ping_lock = ping.lock().unwrap();
    let node_found = !ping_lock.nodes.is_empty();
    let pinging = ping_lock.pinging;
    ui.add_enabled_ui(node_found && !pinging, |ui| {
        if ui
            .add_sized([width, height], Button::new(t!("Ping remote nodes")))
            .on_hover_text(p2pool_ping())
            .clicked()
        {
            // drop(ping_lock);
            Ping::spawn_thread(ping);
        }
    })
    .response
    .on_disabled_hover_text(if node_found {
        button_disabled_by_empty_list_nodes()
    } else {
        t!("Already pinging remote nodes")
    });
}

fn select_fastest_node_button(
    ping: &Arc<Mutex<Ping>>,
    selected_node: &mut RemoteNode,
    width: f32,
    height: f32,
    ui: &mut Ui,
) {
    // make sense to use only if there is more than one found node.
    let ping_lock = ping.lock().unwrap();
    let found_multiple_nodes = ping_lock.nodes.len() > 1;
    ui.add_enabled_ui(found_multiple_nodes, |ui| {
        if ui
            .add_sized([width, height], Button::new(t!("Select fastest node")))
            .on_hover_text(p2pool_select_fastest())
            .clicked()
        {
            *selected_node = ping_lock
                .nodes
                .first()
                .expect(EXPECT_BUTTON_DISABLED)
                .clone();
        }
    })
    .response
    .on_disabled_hover_text(button_disabled_by_empty_list_nodes());
}

fn select_random_node_button(
    ping: &Arc<Mutex<Ping>>,
    selected_node: &mut RemoteNode,
    width: f32,
    height: f32,
    ui: &mut Ui,
) {
    // make sense to use only if there is more than one found node.
    let ping_lock = ping.lock().unwrap();
    let found_multiple_nodes = ping_lock.nodes.len() > 1;
    ui.add_enabled_ui(found_multiple_nodes, |ui| {
        if ui
            .add_sized([width, height], Button::new(t!("Select random node")))
            .on_hover_text(p2pool_select_random())
            .clicked()
        {
            let node = ping_lock
                .nodes
                .get_random_same_ok()
                .expect(EXPECT_BUTTON_DISABLED)
                .clone();
            *selected_node = node;
        }
    })
    .response
    .on_disabled_hover_text(button_disabled_by_empty_list_nodes());
}
