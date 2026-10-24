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

use crate::utils::i18n::t;
use std::sync::{Arc, Mutex};

use egui::{Button, ScrollArea, TextEdit, TextStyle, TextWrapMode, Ui};

use crate::{
    DARK_GRAY,
    helper::{Process, ProcessName},
    regex::num_lines,
};

pub fn console(ui: &mut Ui, text: &str, console_height: &mut u32, process_name: ProcessName) {
    let nb_lines = num_lines(text);
    *console_height = egui::Resize::default()
        .id_salt(process_name.to_string())
        .default_height(*console_height as f32)
        .min_width(ui.available_width())
        .max_width(ui.available_width())
        .show(ui, |ui| {
            egui::Frame::new().fill(DARK_GRAY).show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(TextWrapMode::Wrap);
                ui.style_mut().override_text_style = Some(TextStyle::Small);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_width(ui.available_width())
                    .max_height(ui.available_height())
                    .auto_shrink([false; 2])
                    // .show_viewport(ui, |ui, _| {
                    .show_rows(
                        ui,
                        ui.text_style_height(&TextStyle::Small),
                        nb_lines,
                        |ui, row_range| {
                            for i in row_range {
                                if let Some(line) = text.lines().nth(i) {
                                    ui.label(line);
                                }
                            }
                        },
                    );
            })
        })
        .response
        .rect
        .height() as u32;
}

// input args
pub fn input_args_field(
    ui: &mut Ui,
    buffer: &mut String,
    process: &Arc<Mutex<Process>>,
    hint: &str,
    hover: &str,
) {
    ui.style_mut().spacing.text_edit_width = ui.available_width();
    let response = ui
        .add(TextEdit::hint_text(TextEdit::singleline(buffer), hint))
        .on_hover_text(hover);
    // If the user pressed enter, dump buffer contents into the process STDIN
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        response.request_focus(); // Get focus back
        let buffer = std::mem::take(buffer); // Take buffer
        let mut process = process.lock().unwrap();
        if process.is_alive() {
            process.input.push(buffer);
        } // Push only if alive
    }
}

// Command arguments
pub fn start_options_field(
    ui: &mut Ui,
    arguments: &mut String,
    default_args_simple: &str,
    default_args_advanced: &str,
    hint: &str,
    hover: &str,
) {
    ui.group(|ui| {
        ui.label(t!("Start options:"));
        ui.style_mut().wrap_mode = Some(TextWrapMode::Wrap);
        ui.style_mut().spacing.text_edit_width = ui.available_width();
        ui.add(
            TextEdit::multiline(arguments)
                .hint_text(hint)
                .desired_rows(1)
                .desired_width(ui.available_width()),
        )
        .on_hover_text(hover);
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            ScrollArea::horizontal().show(ui, |ui| {
                if ui
                    .add_enabled(
                        default_args_simple != arguments,
                        Button::new(" Reset to Simple options "),
                    )
                    .on_hover_text(t!(
                        "Reset the start options to arguments used for simple mode"
                    ))
                    .clicked()
                {
                    *arguments = default_args_simple.to_string();
                }

                if ui
                    .add_enabled(
                        default_args_advanced != arguments,
                        Button::new(t!("Reset to Advanced options")),
                    )
                    .on_hover_text(t!(
                        "Reset the start options to arguments used for advanced mode"
                    ))
                    .clicked()
                {
                    *arguments = default_args_advanced.to_string();
                }
                if ui
                    .add_enabled(!arguments.is_empty(), Button::new(t!("Clear")))
                    .on_hover_text(t!(
                        "Clear custom start options to use the advanced settings"
                    ))
                    .clicked()
                {
                    *arguments = String::new();
                }
            });
        });
    });
}
