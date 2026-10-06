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

use std::sync::{Arc, Mutex};

use egui::{
    Button, Label, RichText, ScrollArea, Separator, Slider, TextEdit, TextStyle, ViewportCommand,
    text::{CCursor, CCursorRange},
};
use readable::num::Unsigned;
use strum::{EnumCount, IntoEnumIterator};

use crate::{
    components::gupax::{FileType, FileWindow},
    disk::{
        gupax_p2pool_api::GupaxP2poolApi,
        state::{Gupax, Status},
        status::{Hash, PayoutView},
    },
    helper::{p2pool::PubP2poolApi, payout_sync},
    utils::constants::*,
};

impl Status {
    pub fn p2pool(
        &mut self,
        ui: &mut egui::Ui,
        gupax_p2pool_api: &Arc<Mutex<GupaxP2poolApi>>,
        p2pool_alive: bool,
        p2pool_api: &Arc<Mutex<PubP2poolApi>>,
        observer: &mut String,
        file_window: &Arc<Mutex<FileWindow>>,
    ) {
        let mut api = gupax_p2pool_api.lock().unwrap();
        // let height = size.y;
        // let width = size.x;
        // let text = height / 25.0;
        // let log = height / 2.8;
        // Payout Text + PayoutView buttons
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.style_mut().override_text_style = Some(TextStyle::Body);
        let size_text = ui.text_style_height(&TextStyle::Body);
        let height = (ui.style().spacing.button_padding.y * 2.0) + size_text;
        ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let width =
                            ((ui.available_width() / 3.0) - (SPACE * 4.0)).max(size_text * 9.0);
                        let width_button =
                            ((ui.available_width() / 3.0 / 4.0) - SPACE).max(size_text * 4.0);
                        ui.add_sized(
                            [width, height],
                            Label::new(
                                RichText::new(format!("Total Payouts: {}", api.payout)).underline(),
                            ),
                        )
                        .on_hover_text(STATUS_SUBMENU_PAYOUT);
                        ui.add(Separator::default().vertical());
                        ui.add_sized(
                            [width, height],
                            Label::new(
                                RichText::new(format!("Total XMR: {}", api.xmr)).underline(),
                            ),
                        )
                        .on_hover_text(STATUS_SUBMENU_XMR);
                        // });
                        ui.add(Separator::default().vertical());
                        PayoutView::iter().enumerate().for_each(|(count, p)| {
                            if ui
                                .add_sized(
                                    [width_button, height],
                                    Button::selectable(self.payout_view == p, p.to_string()),
                                )
                                .on_hover_text(p.msg_help())
                                .clicked()
                            {
                                self.payout_view = p;
                            }
                            if count + 1 < PayoutView::COUNT {
                                ui.add(Separator::default().vertical());
                            }
                        });
                    });
                });
                // ui.separator();
                // Actual logs
                egui::Frame::new().fill(DARK_GRAY).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(self.payout_view == PayoutView::Oldest)
                        .max_width(ui.available_width())
                        .max_height(ui.available_height() / 2.8)
                        .auto_shrink([false; 2])
                        .show_viewport(ui, |ui, _| {
                            ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
                            ui.style_mut().spacing.text_edit_width = ui.available_width();
                            match self.payout_view {
                                PayoutView::Latest => {
                                    ui.text_edit_multiline(&mut api.log_rev.as_str())
                                }
                                PayoutView::Oldest => ui.text_edit_multiline(&mut api.log.as_str()),
                                PayoutView::Biggest => {
                                    ui.text_edit_multiline(&mut api.payout_high.as_str())
                                }
                                PayoutView::Smallest => {
                                    ui.text_edit_multiline(&mut api.payout_low.as_str())
                                }
                            };
                        });
                });
            });
            // Payouts sync
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Observer");
                    ui.add(
                        TextEdit::singleline(observer)
                            .hint_text("nano.p2pool.observer")
                            .desired_width(ui.available_width() / 3.0),
                    )
                    .on_hover_text(STATUS_SUBMENU_OBSERVER);
                });
                ui.horizontal(|ui| {
                    ui.add_sized([0.0, height], Label::new("Private view key"));
                    let key = ui
                        .add(
                            TextEdit::singleline(&mut api.view_key)
                                .password(true)
                                .desired_width(ui.available_width() / 3.0),
                        )
                        .on_hover_text(STATUS_SUBMENU_VIEW_KEY);
                    if ui
                        .button("Paste")
                        .on_hover_text(STATUS_SUBMENU_PASTE_VIEW_KEY)
                        .clicked()
                    {
                        // The clipboard content arrives as a paste event in the next frame
                        // and replaces the selected key.
                        let mut state = TextEdit::load_state(ui.ctx(), key.id).unwrap_or_default();
                        let end = CCursor::new(api.view_key.chars().count());
                        state
                            .cursor
                            .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
                        state.store(ui.ctx(), key.id);
                        key.request_focus();
                        ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste);
                    }
                    let window_busy = file_window.lock().unwrap().thread;
                    if ui
                        .add_enabled(!window_busy, Button::new("File"))
                        .on_hover_text(STATUS_SUBMENU_FILE_VIEW_KEY)
                        .clicked()
                    {
                        Gupax::spawn_file_window_thread(file_window, FileType::ViewKey);
                    }
                    let mut file_window = file_window.lock().unwrap();
                    // An error stays until the next file is picked.
                    match &file_window.view_key {
                        Some(Ok(key)) => {
                            api.view_key.clone_from(key);
                            file_window.view_key = None;
                        }
                        Some(Err(e)) => {
                            ui.label(e.as_str());
                        }
                        None => (),
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!api.syncing && p2pool_alive, Button::new("Refresh"))
                        .on_hover_text(STATUS_SUBMENU_REFRESH)
                        .clicked()
                    {
                        let node = p2pool_api.lock().unwrap().current_node.clone();
                        if let Some(node) = node {
                            payout_sync::start(&mut api, gupax_p2pool_api, &node);
                        }
                    }
                    if api.syncing {
                        ui.spinner();
                        if ui
                            .add_enabled(!api.stop_sync, Button::new("Stop"))
                            .clicked()
                        {
                            api.stop_sync = true;
                        }
                    }
                    ui.add(Label::new(api.sync.as_str()).wrap());
                });
            });
            // });
            drop(api);
            // Payout/Share Calculator
            // let button = (width / 20.0) - (SPACE * 1.666);
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    // ui.set_min_width(width - SPACE);
                    let width = ui.available_width() / 10.0;
                    if ui
                        .add_sized(
                            [width, height],
                            Button::selectable(!self.manual_hash, "Automatic"),
                        )
                        .on_hover_text(STATUS_SUBMENU_AUTOMATIC)
                        .clicked()
                    {
                        self.manual_hash = false;
                    }
                    ui.separator();
                    if ui
                        .add_sized(
                            [width, height],
                            Button::selectable(self.manual_hash, "Manual"),
                        )
                        .on_hover_text(STATUS_SUBMENU_MANUAL)
                        .clicked()
                    {
                        self.manual_hash = true;
                    }
                    ui.separator();
                    ui.add_enabled_ui(self.manual_hash, |ui| {
                        if ui
                            .selectable_label(self.hash_metric == Hash::Hash, "Hash")
                            .on_hover_text(STATUS_SUBMENU_HASH)
                            .clicked()
                        {
                            self.hash_metric = Hash::Hash;
                        }
                        ui.separator();
                        if ui
                            .selectable_label(self.hash_metric == Hash::Kilo, "Kilo")
                            .on_hover_text(STATUS_SUBMENU_KILO)
                            .clicked()
                        {
                            self.hash_metric = Hash::Kilo;
                        }
                        ui.separator();
                        if ui
                            .selectable_label(self.hash_metric == Hash::Mega, "Mega")
                            .on_hover_text(STATUS_SUBMENU_MEGA)
                            .clicked()
                        {
                            self.hash_metric = Hash::Mega;
                        }
                        ui.separator();
                        if ui
                            .selectable_label(self.hash_metric == Hash::Giga, "Giga")
                            .on_hover_text(STATUS_SUBMENU_GIGA)
                            .clicked()
                        {
                            self.hash_metric = Hash::Giga;
                        }
                        ui.separator();
                        ui.spacing_mut().slider_width = (ui.available_width() / 1.2).max(0.0);
                        ui.add_sized(
                            [0.0, height],
                            Slider::new(&mut self.hashrate, 1.0..=1_000.0)
                                .suffix(format!(" {}", self.hash_metric)),
                        );
                    });
                })
            });
            // Actual stats
            ui.add_space(height / 2.0);
            ui.add_enabled_ui(p2pool_alive, |ui| {
                let min_height = ui.available_height() / 1.5;
                let api = p2pool_api.lock().unwrap();
                ui.horizontal(|ui| {
                    ui.columns_const(|[col1, col2, col3]| {
                        col1.group(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(height * 2.0);
                                ui.set_min_height(min_height);
                                ui.label(RichText::new("Monero Difficulty").underline())
                                    .on_hover_text(STATUS_SUBMENU_MONERO_DIFFICULTY);
                                ui.label(api.monero_difficulty.as_str());
                                ui.label(RichText::new("Monero Hashrate").underline())
                                    .on_hover_text(STATUS_SUBMENU_MONERO_HASHRATE);
                                ui.label(api.monero_hashrate.as_str());
                                ui.label(RichText::new("P2Pool Difficulty").underline())
                                    .on_hover_text(STATUS_SUBMENU_P2POOL_DIFFICULTY);
                                ui.label(api.p2pool_difficulty.as_str());
                                ui.label(RichText::new("P2Pool Hashrate").underline())
                                    .on_hover_text(STATUS_SUBMENU_P2POOL_HASHRATE);
                                ui.label(api.p2pool_hashrate.as_str());
                            })
                        });
                        col2.group(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(height * 2.0);
                                ui.set_min_height(min_height);
                                if self.manual_hash {
                                    let hashrate =
                                        Hash::convert_to_hash(self.hashrate, self.hash_metric)
                                            as u64;
                                    let p2pool_share_mean =
                                        PubP2poolApi::calculate_share_or_block_time(
                                            hashrate,
                                            api.p2pool_difficulty_u64,
                                        );
                                    let solo_block_mean =
                                        PubP2poolApi::calculate_share_or_block_time(
                                            hashrate,
                                            api.monero_difficulty_u64,
                                        );
                                    ui.label(
                                        RichText::new("Manually Inputted Hashrate").underline(),
                                    );
                                    ui.label(format!("{} H/s", Unsigned::from(hashrate)));
                                    ui.label(RichText::new("P2Pool Block Mean").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_BLOCK_MEAN);
                                    ui.label(api.p2pool_block_mean.display(false));
                                    ui.label(RichText::new("Your P2Pool Share Mean").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_SHARE_MEAN);
                                    ui.label(p2pool_share_mean.display(false));
                                    ui.label(RichText::new("Your Solo Block Mean").underline())
                                        .on_hover_text(STATUS_SUBMENU_SOLO_BLOCK_MEAN);
                                    ui.label(solo_block_mean.display(false));
                                } else {
                                    ui.label(RichText::new("Your P2Pool Hashrate").underline())
                                        .on_hover_text(STATUS_SUBMENU_YOUR_P2POOL_HASHRATE);
                                    ui.label(format!("{} H/s", api.hashrate_1h));
                                    ui.label(RichText::new("P2Pool Block Mean").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_BLOCK_MEAN);
                                    ui.label(api.p2pool_block_mean.display(false));
                                    ui.label(RichText::new("Your P2Pool Share Mean").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_SHARE_MEAN);
                                    ui.label(api.p2pool_share_mean.display(false));
                                    ui.label(RichText::new("Your Solo Block Mean").underline())
                                        .on_hover_text(STATUS_SUBMENU_SOLO_BLOCK_MEAN);
                                    ui.label(api.solo_block_mean.display(false));
                                }
                            })
                        });
                        col3.group(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(height * 2.0);
                                ui.set_min_height(min_height);
                                if self.manual_hash {
                                    let hashrate =
                                        Hash::convert_to_hash(self.hashrate, self.hash_metric)
                                            as u64;
                                    let user_p2pool_percent = PubP2poolApi::calculate_dominance(
                                        hashrate,
                                        api.p2pool_hashrate_u64,
                                    );
                                    let user_monero_percent = PubP2poolApi::calculate_dominance(
                                        hashrate,
                                        api.monero_hashrate_u64,
                                    );
                                    ui.label(RichText::new("P2Pool Miners").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_MINERS);
                                    ui.label(api.miners.as_str());
                                    ui.label(RichText::new("P2Pool Dominance").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_DOMINANCE);
                                    ui.label(api.p2pool_percent.as_str());
                                    ui.label(RichText::new("Your P2Pool Dominance").underline())
                                        .on_hover_text(STATUS_SUBMENU_YOUR_P2POOL_DOMINANCE);
                                    ui.label(user_p2pool_percent.as_str());
                                    ui.label(RichText::new("Your Monero Dominance").underline())
                                        .on_hover_text(STATUS_SUBMENU_YOUR_MONERO_DOMINANCE);
                                    ui.label(user_monero_percent.as_str());
                                } else {
                                    ui.label(RichText::new("P2Pool Miners").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_MINERS);
                                    ui.label(api.miners.as_str());
                                    ui.label(RichText::new("P2Pool Dominance").underline())
                                        .on_hover_text(STATUS_SUBMENU_P2POOL_DOMINANCE);
                                    ui.label(api.p2pool_percent.as_str());
                                    ui.label(RichText::new("Your P2Pool Dominance").underline())
                                        .on_hover_text(STATUS_SUBMENU_YOUR_P2POOL_DOMINANCE);
                                    ui.label(api.user_p2pool_percent.as_str());
                                    ui.label(RichText::new("Your Monero Dominance").underline())
                                        .on_hover_text(STATUS_SUBMENU_YOUR_MONERO_DOMINANCE);
                                    ui.label(api.user_monero_percent.as_str());
                                }
                            })
                        });
                    });
                });
                drop(api);
            });
        });
    }
}
