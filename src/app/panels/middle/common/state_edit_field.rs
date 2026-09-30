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

use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

use egui::{Color32, Label, RichText, Slider, TextEdit};
use egui::{TextStyle, Ui};

use crate::components::gupax::{FileType, FileWindow};
use crate::disk::state::Gupax;
use crate::miscs::height_txt_before_button;
use crate::regex::Regexes;
use crate::{
    GREEN, GUPAX_SELECT, LIGHT_GRAY, NODE_DB_DIR, NODE_DB_PATH_EMPTY, NODE_PATH_OK, RED, SPACE,
};

pub fn slider_state_field(
    ui: &mut Ui,
    description: &str,
    hover_msg: &str,
    field: &mut u16,
    range: RangeInclusive<u16>,
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [0.0, height_txt_before_button(ui, &TextStyle::Body)],
            Label::new(description),
        );
        // not sure what's the right calculation to make
        ui.style_mut().spacing.slider_width = (ui.available_width()
            - ui.spacing().item_spacing.x * 4.0
            - ui.spacing().scroll.bar_width
            - SPACE * 1.0
            + 2.0)
            .max(80.0);
        ui.add_sized(
            [0.0, height_txt_before_button(ui, &TextStyle::Body)],
            Slider::new(field, range),
        )
        .on_hover_text(hover_msg);
    });
}

pub struct StateTextEdit<'a> {
    description: &'a str,
    max_ch: u8,
    help_msg: &'a str,
    validations: &'a [fn(&str) -> bool],
    text_edit_width: f32,
    text_style: TextStyle,
}
#[allow(unused)]
impl<'a> StateTextEdit<'a> {
    pub fn new(ui: &Ui) -> Self {
        StateTextEdit {
            description: "",
            max_ch: 30,
            help_msg: "",
            validations: &[],
            text_edit_width: ui.text_style_height(&TextStyle::Body) * 18.0,
            text_style: TextStyle::Body,
        }
    }
    pub fn build(self, ui: &mut Ui, state_field: &mut String) -> bool {
        let mut valid = false;
        ui.horizontal_centered(|ui| {
            let color;
            let symbol;
            let mut input_validated = true;
            for v in self.validations {
                if !v(state_field) {
                    input_validated = false;
                }
            }
            if state_field.is_empty() {
                symbol = "➖";
                color = Color32::LIGHT_GRAY;
            } else if input_validated {
                symbol = "✔";
                color = Color32::from_rgb(100, 230, 100);
                valid = true;
            } else {
                symbol = "❌";
                color = Color32::from_rgb(230, 50, 50);
            }
            let (len, inside_space) = match self.max_ch {
                x if x >= 100 => (format!("{:03}", state_field.len()), ""),
                10..99 => (format!("{:02}", state_field.len()), " "),
                _ => (format!("{}", state_field.len()), "  "),
            };
            let text = format!(
                "{}[{}{}/{}{}]{}",
                self.description, inside_space, len, self.max_ch, inside_space, symbol
            );
            ui.add_sized(
                [0.0, height_txt_before_button(ui, &self.text_style)],
                Label::new(RichText::new(text).color(color)),
            );
            // allocate the size to leave half of the total width free.
            ui.spacing_mut().text_edit_width = self.text_edit_width;
            ui.text_edit_singleline(state_field)
                .on_hover_text(self.help_msg);
            state_field.truncate(self.max_ch.into());
        });
        valid
    }
    pub fn description(mut self, description: &'a str) -> Self {
        self.description = description;
        self
    }
    pub fn max_ch(mut self, max_ch: u8) -> Self {
        self.max_ch = max_ch;
        self
    }
    pub fn help_msg(mut self, help_msg: &'a str) -> Self {
        self.help_msg = help_msg;
        self
    }
    pub fn validations(mut self, validations: &'a [fn(&str) -> bool]) -> Self {
        self.validations = validations;
        self
    }
    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = text_style;
        self
    }
    pub fn text_edit_width(mut self, text_edit_width: f32) -> Self {
        self.text_edit_width = text_edit_width;
        self
    }
    pub fn text_edit_width_half_left(mut self, ui: &Ui) -> Self {
        self.text_edit_width = ui.available_width() / 2.0;
        self
    }
    pub fn text_edit_width_same_as_max_ch(mut self, ui: &Ui) -> Self {
        self.text_edit_width = ui.text_style_height(&self.text_style) * self.max_ch as f32;
        self
    }
}

// path to choose
pub fn path_db_field(ui: &mut Ui, path: &mut String, file_window: &Arc<Mutex<FileWindow>>) {
    ui.horizontal(|ui| {
        let (symbol, color, hover) = if path.is_empty() {
            ("➖", LIGHT_GRAY, NODE_DB_PATH_EMPTY)
        } else if !Gupax::path_is_dir(path) {
            ("❌", RED, NODE_DB_DIR)
        } else {
            ("✔", GREEN, NODE_PATH_OK)
        };
        let text = ["Node Database Directory ", symbol].concat();
        ui.add_sized(
            [0.0, height_txt_before_button(ui, &TextStyle::Body)],
            Label::new(RichText::new(text).color(color)),
        );
        let window_busy = file_window.lock().unwrap().thread;
        ui.add_enabled_ui(!window_busy, |ui| {
            if ui.button("Open").on_hover_text(GUPAX_SELECT).clicked() {
                Gupax::spawn_file_window_thread(file_window, FileType::NodeDB);
            }
            ui.spacing_mut().text_edit_width = ui.available_width();
            ui.text_edit_singleline(path).on_hover_text(hover);
        });
    });
}
pub fn monero_address_field(address: &mut String, ui: &mut Ui, hover: &str) {
    ui.group(|ui| {
        let len = format!("{:02}", address.len());
        let (text, color) = if address.is_empty() {
            (format!("Monero Address [{len}/95] ➖"), Color32::LIGHT_GRAY)
        } else if Regexes::addr_ok(address) {
            (
                format!("Monero Address [{len}/95] ✔"),
                Color32::from_rgb(100, 230, 100),
            )
        } else {
            (
                format!("Monero Address [{len}/95] ❌"),
                Color32::from_rgb(230, 50, 50),
            )
        };
        ui.style_mut().spacing.text_edit_width = ui.available_width();
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(text).color(color));
            // ui.set_max_width(95.0 * 3.0);
            ui.add_space(SPACE);
            ui.add(
                TextEdit::hint_text(TextEdit::singleline(address), "4...")
                    .horizontal_align(egui::Align::Center),
            )
            .on_hover_text(hover);
            address.truncate(95);
        });
    });
}
