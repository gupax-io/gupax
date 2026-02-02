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

use std::process::exit;

use crate::app::eframe_impl::ProcessStateGui;
use crate::app::keys::KeyPressed;
use crate::disk::node::Node;
use crate::disk::state::State;
use crate::helper::Helper;
use crate::utils::constants::*;
use crate::utils::ferris::*;
use crate::utils::macros::arc_mut;
use crate::utils::resets::{reset_nodes, reset_state};
use egui::*;

impl crate::app::App {
    pub(in crate::app) fn quit_error_panel(
        &mut self,
        ctx: &egui::Context,
        processes: &[ProcessStateGui],
        key: &KeyPressed,
    ) {
        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Set width/height/font
                let width = self.size.x;
                let height = self.size.y / 4.0;
                ui.style_mut().override_text_style = Some(TextStyle::Heading);

                // Display ferris
                use crate::utils::errors::ErrorButtons;
                use crate::utils::errors::ErrorButtons::*;
                use crate::utils::errors::ErrorFerris;
                use crate::utils::errors::ErrorFerris::*;
                let ferris = match self.error_state.ferris {
                    Happy => Image::from_bytes("bytes://happy.png", FERRIS_HAPPY),
                    Cute => Image::from_bytes("bytes://cute.png", FERRIS_CUTE),
                    Oops => Image::from_bytes("bytes://oops.png", FERRIS_OOPS),
                    Error => Image::from_bytes("bytes://error.png", FERRIS_ERROR),
                    Panic => Image::from_bytes("bytes://panic.png", FERRIS_PANIC),
                    #[cfg(target_os = "windows")]
                    ErrorFerris::Admin => Image::from_bytes("bytes://panic.png", FERRIS_ADMIN),
                };

                match self.error_state.buttons {
                    ErrorButtons::Debug => ui.add_sized(
                        [width, height / 4.0],
                        Label::new("--- Debug Info ---\n\nPress [ESC] to quit"),
                    ),
                    _ => ui.add_sized(Vec2::new(width, height), ferris),
                };

                // Error/Quit screen
                match self.error_state.buttons {
                    StayQuit => {
                        let mut text = "".to_string();
                        if *self.update.lock().unwrap().updating.lock().unwrap() {
                            text = format!(
                                "{text}\nUpdate is in progress...! Quitting may cause file corruption!"
                            );
                        }
                        for process in processes {
                            if process.alive {
                            text = format!("{}\n{} is online...!", text, process.name);
                            }
                        }
                        ui.add_sized(
                            [width, height],
                            Label::new("--- Are you sure you want to quit? ---"),
                        );
                        ui.add_sized([width, height], Label::new(text))
                    }
                    ResetState => {
                        ui.add_sized(
                            [width, height],
                            Label::new(format!(
                                "--- Gupax has encountered an error! ---\n{}",
                                &self.error_state.msg
                            )),
                        );
                        ui.add_sized(
                            [width, height],
                            Label::new("Reset Gupax state? (Your settings)"),
                        )
                    }
                    ResetNode => {
                        ui.add_sized(
                            [width, height],
                            Label::new(format!(
                                "--- Gupax has encountered an error! ---\n{}",
                                &self.error_state.msg
                            )),
                        );
                        ui.add_sized([width, height], Label::new("Reset the manual node list?"))
                    }
                    ErrorButtons::WindowsAdmin => {
                        let text = format!(
                            "Why does XMRig need admin privilege?\n{XMRIG_ADMIN_REASON}"
                        );
                        let height = height / 4.0;
                        ui.add_sized(
                            [width, height],
                            Label::new(format!(
                                "--- Gupax needs admin privilege for XMRig! ---\n{}",
                                &self.error_state.msg
                            )),
                        );
                        ui.style_mut().override_text_style = Some(TextStyle::Small);
                        ui.add_sized([width / 2.0, height], Label::new(text));
                        ui.add_sized(
                            [width, height],
                            Hyperlink::from_label_and_url(
                                "Click here for more info.",
                                "https://xmrig.com/docs/miner/randomx-optimization-guide",
                            ),
                        )
                    }
                    Debug => {
                        egui::Frame::NONE.fill(DARK_GRAY).show(ui, |ui| {
                            let width = ui.available_width();
                            let height = ui.available_height();
                            egui::ScrollArea::vertical()
                                .max_width(width)
                                .max_height(height)
                                .auto_shrink([false; 2])
                                .show_viewport(ui, |ui, _| {
                                    ui.add_sized(
                                        [width - 20.0, height],
                                        TextEdit::multiline(&mut self.error_state.msg.as_str()),
                                    );
                                });
                        });
                        ui.label("")
                    }
                    _ => {
                        match self.error_state.ferris {
                            Panic => ui.add_sized(
                                [width, height],
                                Label::new("--- Gupax has encountered an unrecoverable error! ---"),
                            ),
                            Happy => ui.add_sized([width, height], Label::new("--- Success! ---")),
                            Cute => ui.add_sized([width, height], Label::new("--- Gupax needs your Attention ! ---")),
                            _ => ui.add_sized(
                                [width, height],
                                Label::new("--- Gupax has encountered an error! ---"),
                            ),
                        };
                        let height = height / 2.0;
                        // Show GitHub rant link for Windows admin problems.
                        if cfg!(windows) && self.error_state.buttons == ErrorButtons::WindowsAdmin {
                            ui.add_sized([width, height], Hyperlink::from_label_and_url(
								"[Why does Gupax need to be Admin? (on Windows)]",
								"https://github.com/gupax-io/gupax/tree/main/ADMIN.md"
							));
                            ui.add_sized([width, height], Label::new(&self.error_state.msg))
                        } else {
                            ui.add_sized([width, height], Label::new(&self.error_state.msg))
                        }
                    }
                };
                let height = ui.available_height();

                match self.error_state.buttons {
                    UseDetectedLocalNode((rpc, zmq)) => {
                        if ui
                            .add_sized([width, height / 2.0], Button::new("Use the detected Node"))
                            .clicked()
                        {
                            *self.helper.lock().unwrap().ports_detected_local_node.lock().unwrap() = Some((rpc, zmq));
                            self.error_state.reset();
                                Helper::start_node(
                                &self.helper,
                                &self.state.node,
                                &self.state.gupax.absolute_node_path);
                        }
                        // If [Esc] was pressed, assume [No]
                        if key.is_esc()
                            || ui
                                .add_sized([width, height / 2.0], Button::new("Cancel"))
                                .clicked()
                        {
                            self.error_state.reset()
                        }
                    }
                    UseNonSyncedNode => {
                        if ui
                            .add_sized([width, height / 2.0], Button::new("Use the unsynced Node"))
                            .clicked()
                        {
                            self.error_state.reset();
                            Helper::start_p2pool(
                                &self.helper,
                                &self.state.p2pool,
                                &self.state.node,
                                &self.state.gupax.absolute_p2pool_path,
                                &self.backup_hosts,
                                false,
                                &self.crawler
                            )
                        }
                        // If [Esc] was pressed, assume [No]
                        if key.is_esc()
                            || ui
                                .add_sized([width, height / 2.0], Button::new("Cancel"))
                                .clicked()
                        {
                            self.error_state.reset()
                        }
                    },
                    // no means to exit without saving the state
                    ErrorButtons::YesQuit => {
                        if ui
                            .add_sized([width, height / 2.0], Button::new("Yes"))
                            .clicked()
                        {
                            self.error_state.reset()
                        }
                        // If [Esc] was pressed, assume [No]
                        if key.is_esc()
                            || ui
                                .add_sized([width, height / 2.0], Button::new("No"))
                                .clicked()
                        {
                            exit(0);
                        }
                    }
                    // Quit means exiting saving the state
                    StayQuit => {
                        // If [Esc] was pressed, assume [Stay]
                        if key.is_esc()
                            || ui
                                .add_sized([width, height / 2.0], Button::new("Stay"))
                                .clicked()
                        {
                            self.error_state.reset();
                        }
                        if ui
                            .add_sized([width, height / 2.0], Button::new("Quit"))
                            .clicked()
                        {
                            if self.state.gupax.auto.save_before_quit {
                                self.save_before_quit();
                            }
                            exit(0);
                        }
                    }
                    // This code handles the [state.toml/node.toml] resetting, [panic!]'ing if it errors once more
                    // Another error after this either means an IO error or permission error, which Gupax can't fix.
                    // [Yes/No] buttons
                    ResetState => {
                        if ui
                            .add_sized([width, height / 2.0], Button::new("Yes"))
                            .clicked()
                        {
                            match reset_state(&self.state_path) {
                                Ok(_) => match State::get(&self.state_path) {
                                    Ok(s) => {
                                        self.state = s;
                                        self.og = arc_mut!(self.state.clone());
                                        self.error_state.set(
                                            "State read OK",
                                            ErrorFerris::Happy,
                                            ErrorButtons::Okay,
                                        );
                                    }
                                    Err(e) => self.error_state.set(
                                        format!("State read fail: {e}"),
                                        ErrorFerris::Panic,
                                        ErrorButtons::Quit,
                                    ),
                                },
                                Err(e) => self.error_state.set(
                                    format!("State reset fail: {e}"),
                                    ErrorFerris::Panic,
                                    ErrorButtons::Quit,
                                ),
                            };
                        }
                        if key.is_esc()
                            || ui
                                .add_sized([width, height / 2.0], Button::new("No"))
                                .clicked()
                        {
                            self.error_state.reset()
                        }
                    }
                    ResetNode => {
                        if ui
                            .add_sized([width, height / 2.0], Button::new("Yes"))
                            .clicked()
                        {
                            match reset_nodes(&self.node_path) {
                                Ok(_) => match Node::get(&self.node_path) {
                                    Ok(s) => {
                                        self.node_vec = s;
                                        self.og_node_vec.clone_from(&self.node_vec);
                                        self.error_state.set(
                                            "Node read OK",
                                            ErrorFerris::Happy,
                                            ErrorButtons::Okay,
                                        );
                                    }
                                    Err(e) => self.error_state.set(
                                        format!("Node read fail: {e}"),
                                        ErrorFerris::Panic,
                                        ErrorButtons::Quit,
                                    ),
                                },
                                Err(e) => self.error_state.set(
                                    format!("Node reset fail: {e}"),
                                    ErrorFerris::Panic,
                                    ErrorButtons::Quit,
                                ),
                            };
                        }
                        if key.is_esc()
                            || ui
                                .add_sized([width, height / 2.0], Button::new("No"))
                                .clicked()
                        {
                            self.error_state.reset()
                        }
                    }
                    crate::app::ErrorButtons::Okay | crate::app::ErrorButtons::WindowsAdmin => {
                        if key.is_esc()
                            || ui.add_sized([width, height], Button::new("Okay")).clicked()
                        {
                            self.error_state.reset();
                        }
                    }
                    Debug => {
                        if key.is_esc() {
                            self.error_state.reset();
                        }
                    }
                    Quit => {
                        if ui.add_sized([width, height], Button::new("Quit")).clicked() {
                            exit(1);
                        }
                    }
                }
            })
        });
    }
}
