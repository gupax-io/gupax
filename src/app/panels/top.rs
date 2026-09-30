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

use crate::app::Tab;
use egui::{Button, Panel, TextStyle};
use egui::{ScrollArea, Separator, Ui};
use log::debug;

impl crate::app::App {
    pub fn top_panel(&mut self, ui: &mut egui::Ui) {
        debug!("App | Rendering TOP tabs");
        let tabs = Tab::from_show_processes(&self.state.gupax.show_processes);
        Panel::top("top").show_separator_line(true).show(ui, |ui| {
            // low spacing to shrink and be able to show all tabs on one line on 640x480
            ui.style_mut().spacing.item_spacing.x = 4.0;
            // spacing of separator, will reduce width size of the button. Low value so that tabs can be selected easily.
            let spacing_separator = 2.0;
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.style_mut().override_text_style = Some(TextStyle::Heading);
                let height = ui
                    .style()
                    .text_styles
                    .get(&TextStyle::Heading)
                    .unwrap()
                    .size
                    * 2.75;
                // width = (width - / number of tab) - (space between widget * 2.0 + space of separator / 2.0)
                let width = (((self.size.x) / tabs.len() as f32)
                    - ((ui.style().spacing.item_spacing.x * 2.0) + (spacing_separator / 2.0)))
                    .max(0.0);
                // height of tab menu relative to size of text. coeff 2.75 is arbitrary but good enough to be easily clickable.
                self.tabs(ui, [width, height], spacing_separator, tabs);
            });
        });
    }

    fn tabs(&mut self, ui: &mut Ui, size: [f32; 2], spacing_separator: f32, tabs: Vec<Tab>) {
        ScrollArea::horizontal()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                let nb_tabs = tabs.len();
                for (count, tab) in tabs.into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            // we don't want y item spacing to influence the added space
                            ui.style_mut().spacing.item_spacing.y = 0.0;
                            ui.add_space(spacing_separator);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        size,
                                        Button::selectable(self.tab == tab, tab.to_string()),
                                    )
                                    .clicked()
                                {
                                    self.tab = tab
                                }
                            });
                            // add a space to prevent selectable button to be at the same line as the end of the top bar. Make it the same spacing as separators.
                            ui.add_space(spacing_separator);
                        });
                        if count + 1 != nb_tabs {
                            ui.add(Separator::default().spacing(spacing_separator).vertical());
                        }
                    });
                }
            });
    }
}
