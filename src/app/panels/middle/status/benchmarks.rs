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

use crate::utils::i18n::{t, t_f};
use std::sync::{Arc, Mutex};

use crate::{app::Benchmark, disk::state::Status, helper::xrig::xmrig::PubXmrigApi};
use egui::{ProgressBar, ScrollArea, TextWrapMode};
use egui_extras::{Column, TableBuilder};
use readable::num::{Float, Percent, Unsigned};

use crate::constants::*;
use egui::{Label, RichText};
use log::*;
impl Status {
    pub(super) fn benchmarks(
        &mut self,
        ui: &mut egui::Ui,
        benchmarks: &[Benchmark],
        xmrig_alive: bool,
        xmrig_api: &Arc<Mutex<PubXmrigApi>>,
    ) {
        debug!("Status Tab | Rendering [Benchmarks]");
        let text = ui.text_style_height(&egui::TextStyle::Body);
        let double = text * 2.0;

        let width = ui.available_width();
        // [0], The user's CPU (most likely).
        let cpu = &benchmarks[0];
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.set_max_width(ui.available_width() / 2.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(t!("Your CPU")).underline().color(BONE))
                        .on_hover_text(status_submenu_your_cpu());
                    ui.label(cpu.cpu.as_str());
                    ui.label(
                        RichText::new(t!("Total Benchmarks"))
                            .underline()
                            .color(BONE),
                    )
                    .on_hover_text(status_submenu_your_benchmarks());
                    ui.label(format!("{}", cpu.benchmarks));
                    // ui.add_sized([width, text], Label::new(format!("{}", cpu.benchmarks)));
                    ui.label(RichText::new(t!("Rank")).underline().color(BONE))
                        .on_hover_text(status_submenu_your_rank());
                    ui.label(format!("{}/{}", cpu.rank, benchmarks.len()));
                })
            });
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(t!("High Hashrate")).underline().color(BONE))
                        .on_hover_text(status_submenu_your_high());
                    ui.label(format!("{} H/s", Float::from_0(cpu.high.into())));
                    ui.label(
                        RichText::new(t!("Average Hashrate"))
                            .underline()
                            .color(BONE),
                    )
                    .on_hover_text(status_submenu_your_average());
                    ui.label(format!("{} H/s", Float::from_0(cpu.average.into())));
                    ui.label(RichText::new(t!("Low Hashrate")).underline().color(BONE))
                        .on_hover_text(status_submenu_your_low());
                    ui.label(RichText::new(format!(
                        "{} H/s",
                        Float::from_0(cpu.low.into())
                    )));
                })
            })
        });

        // User's CPU hashrate comparison (if XMRig is alive).
        // User's CPU hashrate comparison (if XMRig is alive).
        ui.vertical_centered(|ui| {
            ui.add_space(SPACE);
            if xmrig_alive {
                let api = xmrig_api.lock().unwrap();
                let percent = (api.hashrate_raw / cpu.high) * 100.0;
                let human = Percent::from(percent);
                if percent > 100.0 {
                    ui.add_sized(
                        [width, double],
                        Label::new(t_f!(
                        "Your CPU's is faster than the highest benchmark! It is [{}] faster @ {}!",
                        human, api.hashrate
                    )),
                    );
                    ui.add(ProgressBar::new(1.0));
                } else if api.hashrate_raw == 0.0 {
                    ui.label(t!("Measuring hashrate..."));
                    ui.spinner();
                    ui.add(ProgressBar::new(0.0));
                } else {
                    ui.add_sized(
                        [width, double],
                        Label::new(t_f!(
                            "Your CPU's hashrate is [{}] of the highest benchmark @ {}",
                            human,
                            api.hashrate
                        )),
                    );
                    ui.add(ProgressBar::new(percent / 100.0));
                }
            } else {
                ui.add_enabled_ui(xmrig_alive, |ui| {
                    ui.add_sized(
                        [width, double],
                        Label::new(t!("XMRig is offline. Hashrate cannot be determined.")),
                    );
                    ui.add(ProgressBar::new(0.0));
                });
            }
            ui.add_space(SPACE);
            // Comparison
            ui.group(|ui| {
                ui.hyperlink_to(t!("Other CPUs"), "https://xmrig.com/benchmark")
                    .on_hover_text(status_submenu_other_cpus());
            });
        });

        // Comparison
        let width_column = width / 20.0;
        let (cpu, bar, high, average, low, rank, bench) = (
            width_column * 6.0,
            width_column * 3.0,
            width_column * 2.0,
            width_column * 2.0,
            width_column * 2.0,
            width_column,
            width_column * 2.0,
        );
        ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
        ScrollArea::horizontal().show(ui, |ui| {
            TableBuilder::new(ui)
                .columns(Column::auto(), 7)
                .header(double, |mut header| {
                    header.col(|ui| {
                        ui.add_sized([bar, text], Label::new("CPU"))
                            .on_hover_text(status_submenu_other_cpu());
                    });
                    header.col(|ui| {
                        ui.add_sized([bar, text], Label::new(t!("Relative")))
                            .on_hover_text(status_submenu_other_relative());
                    });
                    header.col(|ui| {
                        ui.add_sized([high, text], Label::new(t!("High")))
                            .on_hover_text(status_submenu_other_high());
                    });
                    header.col(|ui| {
                        ui.add_sized([average, text], Label::new(t!("Average")))
                            .on_hover_text(status_submenu_other_average());
                    });
                    header.col(|ui| {
                        ui.add_sized([low, text], Label::new("Low"))
                            .on_hover_text(status_submenu_other_low());
                    });
                    header.col(|ui| {
                        ui.add_sized([rank, text], Label::new(t!("Rank")))
                            .on_hover_text(status_submenu_other_rank());
                    });
                    header.col(|ui| {
                        ui.add_sized([bench, text], Label::new(t!("Benchmarks")))
                            .on_hover_text(status_submenu_other_benchmarks());
                    });
                })
                .body(|body| {
                    body.rows(text, benchmarks.len(), |mut row| {
                        let benchmark = &benchmarks[row.index()];
                        row.col(|ui| {
                            ui.add_sized([cpu, text], Label::new(benchmark.cpu.as_str()));
                        });
                        row.col(|ui| {
                            ui.add_sized([bar, text], ProgressBar::new(benchmark.percent / 100.0))
                                .on_hover_text(Percent::from(benchmark.percent).as_str());
                        });
                        row.col(|ui| {
                            ui.add_sized(
                                [high, text],
                                Label::new(
                                    [Float::from_0(benchmark.high.into()).as_str(), " H/s"]
                                        .concat(),
                                ),
                            );
                        });
                        row.col(|ui| {
                            ui.add_sized(
                                [average, text],
                                Label::new(
                                    [Float::from_0(benchmark.average.into()).as_str(), " H/s"]
                                        .concat(),
                                ),
                            );
                        });
                        row.col(|ui| {
                            ui.add_sized(
                                [low, text],
                                Label::new(
                                    [Float::from_0(benchmark.low.into()).as_str(), " H/s"].concat(),
                                ),
                            );
                        });
                        row.col(|ui| {
                            ui.add_sized(
                                [rank, text],
                                Label::new(Unsigned::from(benchmark.rank).as_str()),
                            );
                        });
                        row.col(|ui| {
                            ui.add_sized(
                                [bench, text],
                                Label::new(Unsigned::from(benchmark.benchmarks).as_str()),
                            );
                        });
                    });
                });
        });
    }
}
