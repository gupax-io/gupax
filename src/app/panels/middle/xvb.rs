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

use crate::helper::xvb::algorithm::{ManualDonationLevel, XvbModeChoice};
use crate::utils::i18n::t;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Align, Image, Label, RichText, ScrollArea, TextStyle, Ui};
use log::debug;
use readable::num::Float;
use readable::up::Uptime;
use strum::EnumCount;

use crate::app::panels::middle::common::console::console;
use crate::app::panels::middle::common::header_tab::header_tab;
use crate::app::panels::middle::common::toggle::toggle_ui_compact;
use crate::helper::ProcessName;
use crate::helper::xvb::PubXvbApi;
use crate::helper::xvb::algorithm::{ManualDonationMetric, XvbMode};
use crate::miscs::height_txt_before_button;
use crate::utils::constants::{
    ORANGE, XVB_URL_RULES, xvb_donated_1h_field, xvb_donated_24h_field,
    xvb_donation_level_donor_help, xvb_donation_level_mega_donor_help,
    xvb_donation_level_vip_donor_help, xvb_donation_level_whale_donor_help, xvb_failure_field,
    xvb_hero_select, xvb_manual_pool, xvb_manual_slider_manual_p2pool_help,
    xvb_manual_slider_manual_xvb_help, xvb_mode_manual_donation_level_help,
    xvb_mode_manual_p2pool_help, xvb_mode_manual_xvb_help, xvb_round_type_field, xvb_timeframe,
    xvb_winner_field,
};
use crate::utils::regex::Regexes;
use crate::{
    constants::{BYTES_XVB, SPACE},
    utils::constants::XVB_URL,
};
use crate::{xvb_mining_on_field, xvb_p2pool_buffer, xvb_sidechain};

impl crate::disk::state::Xvb {
    #[inline(always)] // called once
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        address: &str,
        ui: &mut egui::Ui,
        api: &Arc<Mutex<PubXvbApi>>,
        is_alive: bool,
    ) {
        // logo and website link
        let logo = Some(Image::from_bytes("bytes:/xvb.png", BYTES_XVB));
        header_tab(
            ui,
            logo,
            &[
                ("XMRvsBEAST", XVB_URL, ""),
                (
                    t!("Rules"),
                    XVB_URL_RULES,
                    t!("Click here to read the rules and understand how the raffle works."),
                ),
                ("FAQ", "https://xmrvsbeast.com/p2pool/faq.html", ""),
            ],
            None,
            true,
        );
        egui::ScrollArea::vertical().show(ui, |ui| {

            // console output for log
            debug!("XvB Tab | Rendering [Console]");
            ui.group(|ui| {
                let text = &api.lock().unwrap().output;
                console(ui, text, &mut self.console_height, ProcessName::Xvb);
            });
            ui.add_space(SPACE);
            ui.horizontal(|ui| {

        // --------------------------- XVB Simple -------------------------------------------
        if self.simple {
            ui.add_space(SPACE);
            if ui.checkbox(&mut self.simple_hero_mode, t!("Hero Mode")).on_hover_text(xvb_hero_select()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
            }
        }
    });
        ui.add_space(SPACE);
         // --------------------------- XVB Advanced -----------------------------------------
                        let text_height = height_txt_before_button(ui, &TextStyle::Heading) * 1.4;
                ScrollArea::horizontal().id_salt("horizontal").show(ui, |ui| {
         if !self.simple {

            ui.group(|ui| {
                ui.set_width(0.0);
                ui.vertical(|ui| {
                        ui.style_mut().override_text_valign = Some(Align::Center);
                        ui.set_height(0.0);
                        ui.set_height(0.0);
                        egui::ComboBox::from_label("").height(XvbMode::COUNT as f32 * (ui.text_style_height(&TextStyle::Button) + (ui.spacing().button_padding.y * 2.0) + ui.spacing().item_spacing.y))
                        .selected_text(self.mode.to_string())
                        .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut self.mode, XvbModeChoice::Auto,
                                     XvbModeChoice::Auto.to_string()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                 }
                                if ui.selectable_value(&mut self.mode, XvbModeChoice::Hero,
                                     XvbModeChoice::Hero.to_string()).on_hover_text(xvb_hero_select()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                 }
                                if ui.selectable_value(&mut self.mode, XvbModeChoice::ManualXvb,
                                     XvbModeChoice::ManualXvb.to_string())
                                .on_hover_text(xvb_mode_manual_xvb_help()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                            }
                                if ui.selectable_value(&mut self.mode, XvbModeChoice::ManualP2pool,
                                     XvbModeChoice::ManualP2pool.to_string())
                                .on_hover_text(xvb_mode_manual_p2pool_help()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                            }
                                if ui.selectable_value(&mut self.mode, XvbModeChoice::ManualDonationLevel,
                                     XvbModeChoice::ManualDonationLevel.to_string())
                                .on_hover_text(xvb_mode_manual_donation_level_help()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                            }
                        });
                        if self.mode == XvbModeChoice::ManualXvb {
                            ui.add_space(SPACE);
                            let slider_help_text = xvb_manual_slider_manual_xvb_help();
                            ui.horizontal(|ui| {
                                if ui.add_sized([0.0, text_height],egui::Button::selectable(self.manual_xvb_donation_metric == ManualDonationMetric::Hash, "Hash")).clicked() {
                                    self.manual_xvb_donation_metric = ManualDonationMetric::Hash;
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                }
                                if ui.add_sized([0.0, text_height],egui::Button::selectable(self.manual_xvb_donation_metric == ManualDonationMetric::Kilo, "Kilo")).clicked() {
                                    self.manual_xvb_donation_metric = ManualDonationMetric::Kilo;
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                };
                                if ui.add_sized([0.0, text_height],egui::Button::selectable(self.manual_xvb_donation_metric == ManualDonationMetric::Mega, "Mega")).clicked() {
                                    self.manual_xvb_donation_metric = ManualDonationMetric::Mega;
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                };
                                ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 18.0;
                                if ui.add_sized(
                                    [ui.available_width(), text_height],
                                    egui::Slider::new(&mut self.manual_xvb_slider_amount, 0.0..=1000.0)
                                    .text(self.manual_xvb_donation_metric.to_string())
                                    .max_decimals(3)
                                ).on_hover_text(slider_help_text).changed() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                }

                            });
            ui.add_space(SPACE);
                        }
                        if self.mode == XvbModeChoice::ManualP2pool {
                            ui.add_space(SPACE);
                            let slider_help_text = xvb_manual_slider_manual_p2pool_help();
                            ui.horizontal(|ui| {

                                if ui.add_sized([0.0, text_height],egui::Button::selectable(self.manual_p2pool_donation_metric == ManualDonationMetric::Hash, "Hash")).clicked() {
                                    self.manual_p2pool_donation_metric = ManualDonationMetric::Hash;
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                }
                                if ui.add_sized([0.0, text_height],egui::Button::selectable(self.manual_p2pool_donation_metric == ManualDonationMetric::Kilo, "Kilo")).clicked() {
                                    self.manual_p2pool_donation_metric = ManualDonationMetric::Kilo;
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                };
                                if ui.add_sized([0.0, text_height],egui::Button::selectable(self.manual_p2pool_donation_metric == ManualDonationMetric::Mega, "Mega")).clicked() {
                                    self.manual_p2pool_donation_metric = ManualDonationMetric::Mega;
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                };
                                ui.spacing_mut().slider_width = ui.text_style_height(&TextStyle::Button) * 18.0;
                                if ui.add_sized(
                                    [ui.available_width(), text_height],
                                    egui::Slider::new(&mut self.manual_p2pool_slider_amount, 0.0..=1000.0)
                                    .text(self.manual_p2pool_donation_metric.to_string())
                                    .max_decimals(3)
                                ).on_hover_text(slider_help_text).changed() {
                                    api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                }
                            });
            ui.add_space(SPACE);
                        }
                        if matches!(self.mode, XvbModeChoice::ManualDonationLevel) {
                            ui.add_space(SPACE);
                            ui.horizontal(|ui| {
                            if ui.radio_value(&mut self.manual_donation_level, ManualDonationLevel::Donor,
                                ManualDonationLevel::Donor.to_string())
                            .on_hover_text(xvb_donation_level_donor_help()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                        }
                            if ui.radio_value(&mut self.manual_donation_level, ManualDonationLevel::DonorVIP,
                                ManualDonationLevel::DonorVIP.to_string())
                            .on_hover_text(xvb_donation_level_vip_donor_help())
                                .clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                                }
                            if ui.radio_value(&mut self.manual_donation_level, ManualDonationLevel::DonorWhale,
                                ManualDonationLevel::DonorWhale.to_string())
                            .on_hover_text(xvb_donation_level_whale_donor_help()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                        }
                            if ui.radio_value(&mut self.manual_donation_level, ManualDonationLevel::DonorMega,
                                ManualDonationLevel::DonorMega.to_string())
                            .on_hover_text(xvb_donation_level_mega_donor_help()).clicked() {
                api.lock().unwrap().runtime_mode = XvbMode::from(&*self);
                        }

                            });
            ui.add_space(SPACE);
                        }
                    });
                });


         ui.add_space(SPACE);
        let p2pool_buffer_enabled = matches!(self.mode, XvbModeChoice::Auto | XvbModeChoice::Hero);


         ui.horizontal(|ui|{
            // allow user to modify the buffer for p2pool
            // button
            ui.add_enabled_ui(p2pool_buffer_enabled, |ui|{
 if ui.add_sized(
                [0.0 , text_height],
                egui::Slider::new(&mut self.algo_config.p2pool_buffer, -100..=100)
                .text("% P2Pool Buffer" )
            ).on_hover_text(xvb_p2pool_buffer()).changed() {
                api.lock().unwrap().algo_config.p2pool_buffer = self.algo_config.p2pool_buffer;
            }

            }).response.on_disabled_hover_text(xvb_p2pool_buffer());

         ui.add_space(SPACE);
         // p2pool sidechain HR or stratum data
            if ui.add_sized(
                [0.0, text_height],
                egui::Checkbox::new(&mut self.algo_config.p2pool_watch_sidechain, t!("Watch P2Pool Sidechain HR"))).on_hover_text(xvb_sidechain()).clicked() {
                api.lock().unwrap().algo_config.p2pool_watch_sidechain = self.algo_config.p2pool_watch_sidechain;
            }
         });
         ui.horizontal(|ui|{
            // allow user to modify the buffer for p2pool
            // button
            if ui.add_sized(
                [0.0 , text_height],
                egui::Slider::new(&mut self.manual_timeframe_minutes, 1..=30).suffix("mn")
                .text(t!("Decision timeframe"))
            ).on_hover_text(xvb_timeframe()).changed() {
                self.algo_config.timeframe = Duration::from_mins(self.manual_timeframe_minutes.into());
                api.lock().unwrap().algo_config.timeframe= self.algo_config.timeframe;
                api.lock().unwrap().update_samples_timeframe();
            }

            });
        // Allow user to choose XvB pool manually
        // checkbox to enable
        ui.checkbox(&mut self.manual_pool_enabled, t!("Manual selection of the XvB pool")).on_hover_text(xvb_manual_pool());
        // slider for EU or NA
            ui.horizontal(|ui|{
        ui.add_enabled_ui(self.manual_pool_enabled, |ui|{
                ui.style_mut().override_text_style = Some(TextStyle::Heading);
                ui.add_sized([0.0, text_height], Label::new(" [ NA"));
                toggle_ui_compact(&mut self.manual_pool_eu, ui);
                ui.add_sized([0.0, text_height], Label::new("EU ]"));
            });
        });

        }

        // need to warn the user if no address is set in p2pool tab
        if !Regexes::addr_ok(address) {
            debug!("XvB Tab | Rendering warning text");
                ui.horizontal_wrapped(|ui|{
            ui.label(RichText::new(t!("You don't have any payout address set in the P2pool Tab ! XvB process needs one to function properly."))
                    .color(ORANGE));
                });
        }
            // private stats
            ui.add_space(SPACE);
            ui.add_enabled_ui(is_alive, |ui| {
                let api = &api.lock().unwrap();
                let priv_stats = &api.stats_priv;
                let current_node = &api.current_pool;
                let style_height = ui.text_style_height(&TextStyle::Body);

        let width_column = ui.text_style_height(&TextStyle::Body) * 12.0;
        let height_column = 0.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.horizontal(|ui| {
                    // Failures
                    stat_box(ui, xvb_failure_field(), &priv_stats.fails.to_string(), height_column);
                    stat_box(ui, xvb_donated_1h_field(),
                                        &[
                                            Float::from_3((priv_stats.donor_1hr_avg / 1000.0) as f64).to_string(),
                                            " kH/s".to_string(),
                                        ]
                                        .concat()
                        ,  height_column);
                    stat_box(ui, xvb_donated_24h_field(),
                                        &[
                                            Float::from_3((priv_stats.donor_24hr_avg / 1000.0) as f64).to_string(),
                                            " kH/s".to_string(),
                                        ]
                                        .concat()
                        ,  height_column);
                            ui.add_enabled_ui(priv_stats.round_participate.is_some(), |ui| {
                                 let round = match &priv_stats.round_participate {
                        Some(r) => r.to_string(),
                        None => t!("None").to_string(),
                    };
                    stat_box(ui, xvb_round_type_field(), &round, height_column);
                    }).response
                                .on_disabled_hover_text(
                                    t!("You do not yet have a share in the PPLNS Window."),
                                );
                    stat_box(ui, xvb_winner_field(),
if priv_stats.win_current {
                                        t!("You are Winning the round !")
                                    } else {
                                        t!("You are not the winner")
                                    }
                        , height_column);
                });
                    ui.vertical(|ui| {
                        ui.group(|ui| {
                            ui.set_width(width_column);
                            ui.set_height(height_column);
                            ui.vertical_centered(|ui| {
                                ui.spacing_mut().item_spacing = [style_height / 2.0, style_height / 2.0].into();
                                ui.add_space(SPACE);
                                    ui.label(xvb_mining_on_field())
                                        .on_hover_text_at_pointer(&priv_stats.msg_indicator);
                                    ui.label(
                                        current_node
                                            .as_ref()
                                            .map_or(t!("No where").to_string(), |n| n.to_string()),
                                    )
                                    .on_hover_text_at_pointer(&priv_stats.msg_indicator);
                                    ui.label(Uptime::from(priv_stats.time_switch_pool).to_string())
                                        .on_hover_text_at_pointer(&priv_stats.msg_indicator)
                                })
                            });
                    })
                        .response
                        .on_disabled_hover_text(t!("Algorithm is not running."));
                // indicators
                    })
                });
                    // currently mining on
                });
    }
}
fn stat_box(ui: &mut Ui, title: &str, value: &str, column_height: f32) {
    ui.vertical(|ui| {
        ui.group(|ui| {
            let width_txt = (title.len().max(value.len()) as f32
                * ui.text_style_height(&TextStyle::Button)
                / 2.0)
                + ui.spacing().item_spacing.x * 2.0;
            ui.set_width(width_txt);
            ui.set_height(column_height);
            ui.vertical_centered(|ui| {
                ui.add_space(SPACE * 3.0);
                ui.label(title);
                ui.label(value);
                ui.add_space(SPACE);
            });
        });
    });
}
