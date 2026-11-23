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
// Locks the process, which the helper locks before the APIs.
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
        ui.label("Start options:");
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
                    .on_hover_text("Reset the start options to arguments used for simple mode")
                    .clicked()
                {
                    *arguments = default_args_simple.to_string();
                }

                if ui
                    .add_enabled(
                        default_args_advanced != arguments,
                        Button::new("Reset to Advanced options"),
                    )
                    .on_hover_text("Reset the start options to arguments used for advanced mode")
                    .clicked()
                {
                    *arguments = default_args_advanced.to_string();
                }
                if ui
                    .add_enabled(!arguments.is_empty(), Button::new("Clear"))
                    .on_hover_text("Clear custom start options to use the advanced settings")
                    .clicked()
                {
                    *arguments = String::new();
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use crate::app::submenu_enum::SubmenuP2pool;
    use crate::components::gupax::FileWindow;
    use crate::components::node::{Ping, RemoteNodes};
    use crate::disk::gupax_p2pool_api::GupaxP2poolApi;
    use crate::disk::pool::Pool;
    use crate::disk::state::{Node, P2pool, StartOptionsMode, State, Xmrig, XmrigProxy};
    use crate::helper::crawler::Crawler;
    use crate::helper::node::{ImgNode, PubNodeApi};
    use crate::helper::notification::NotificationApi;
    use crate::helper::p2pool::{ImgP2pool, PubP2poolApi};
    use crate::helper::sys_info::Sys;
    use crate::helper::xrig::xmrig::{ImgXmrig, PubXmrigApi};
    use crate::helper::xrig::xmrig_proxy::{ImgProxy, PubXmrigProxyApi};
    use crate::helper::xvb::PubXvbApi;
    use crate::helper::{Helper, Process, ProcessName};
    use egui::accesskit::{Role, TreeUpdate};
    use egui::{Context, Event, Key, Modifiers, PointerButton, RawInput, pos2};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    fn not_started(name: ProcessName) -> Arc<Mutex<Process>> {
        Arc::new(Mutex::new(Process::new(
            name,
            String::new(),
            PathBuf::new(),
        )))
    }

    fn frame(
        ctx: &Context,
        events: Vec<Event>,
        draw: &mut impl FnMut(&mut egui::Ui),
    ) -> Option<TreeUpdate> {
        let mut output = ctx.run_ui(
            RawInput {
                events,
                ..Default::default()
            },
            |ui| draw(ui),
        );
        let update = output.platform_output.accesskit_update.take();
        output.drop_without_applying_deltas();
        update
    }

    // Enters a command in the console that `draw` shows, while a panic poisoned `process`. The
    // panic of the command poisons `api` if the console kept its lock.
    fn console_locked_by_command<T>(
        api: &Mutex<T>,
        process: &Arc<Mutex<Process>>,
        mut draw: impl FnMut(&mut egui::Ui),
    ) -> bool {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let update = frame(&ctx, vec![], &mut draw).unwrap();
        // The command field comes first below the console.
        let field = update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::TextInput)
            .filter_map(|(_, node)| node.bounds())
            .min_by(|a, b| a.y0.total_cmp(&b.y0))
            .unwrap();
        let at = pos2(
            ((field.x0 + field.x1) / 2.0) as f32,
            ((field.y0 + field.y1) / 2.0) as f32,
        );
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, vec![Event::PointerMoved(at), button(true)], &mut draw);
        frame(&ctx, vec![button(false)], &mut draw);
        frame(&ctx, vec![Event::Text("h".to_string())], &mut draw);
        let poisoned = Arc::clone(process);
        let _ = std::thread::spawn(move || {
            let _lock = poisoned.lock().unwrap();
            panic!("poisons the process");
        })
        .join();
        let enter = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let command = catch_unwind(AssertUnwindSafe(|| frame(&ctx, vec![enter], &mut draw)));
        assert!(command.is_err(), "the command reached no process");
        api.is_poisoned()
    }

    #[test]
    fn a_command_in_the_node_console_leaves_the_console_unlocked() {
        let (api, process) = (
            Arc::new(Mutex::new(PubNodeApi::new())),
            not_started(ProcessName::Node),
        );
        let mut node = Node {
            simple: false,
            ..Default::default()
        };
        let (mut buffer, file_window) = (String::new(), FileWindow::new());
        assert!(!console_locked_by_command(&api, &process, |ui| {
            node.show(&process, &api, &mut buffer, &file_window, ui)
        }));
    }

    #[test]
    fn a_command_in_the_p2pool_console_leaves_the_console_unlocked() {
        let (api, process) = (
            Arc::new(Mutex::new(PubP2poolApi::new())),
            not_started(ProcessName::P2pool),
        );
        let mut p2pool = P2pool {
            submenu: SubmenuP2pool::Advanced,
            ..Default::default()
        };
        let (mut nodes, mut buffer) = (crate::disk::node::Node::new_vec(), String::new());
        let (state, ping) = (
            Arc::new(Mutex::new(State::new())),
            Arc::new(Mutex::new(Ping::new(RemoteNodes::default()))),
        );
        let (backup_nodes, crawler) = (Arc::new(Mutex::new(Vec::new())), Crawler::new());
        assert!(!console_locked_by_command(&api, &process, |ui| {
            p2pool.show(
                &mut nodes,
                &state,
                &ping,
                &process,
                &api,
                &mut buffer,
                ui,
                Arc::clone(&backup_nodes),
                Path::new(""),
                18083,
                18081,
                &crawler,
            )
        }));
    }

    #[test]
    fn a_command_in_the_xmrig_console_leaves_the_console_unlocked() {
        let (api, process) = (
            Arc::new(Mutex::new(PubXmrigApi::new())),
            not_started(ProcessName::Xmrig),
        );
        let mut xmrig = Xmrig {
            simple: false,
            ..Default::default()
        };
        let (mut pools, mut buffer) = (Pool::new_vec(), String::new());
        assert!(!console_locked_by_command(&api, &process, |ui| {
            xmrig.show(&mut pools, &process, &api, &mut buffer, ui, 3333)
        }));
    }

    #[test]
    fn a_command_in_the_xmrig_proxy_console_leaves_the_console_unlocked() {
        let (api, process) = (
            Arc::new(Mutex::new(PubXmrigProxyApi::new())),
            not_started(ProcessName::XmrigProxy),
        );
        let (local_ip, public_ip, reachable) = (
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(false)),
        );
        let helper = Arc::new(Mutex::new(Helper::new(
            std::time::Instant::now(),
            Arc::new(Mutex::new(Sys::new())),
            not_started(ProcessName::P2pool),
            not_started(ProcessName::Xmrig),
            Arc::clone(&process),
            not_started(ProcessName::Xvb),
            not_started(ProcessName::Node),
            Arc::new(Mutex::new(PubP2poolApi::new())),
            Arc::new(Mutex::new(PubXmrigApi::new())),
            Arc::new(Mutex::new(PubXvbApi::new())),
            Arc::clone(&api),
            Arc::new(Mutex::new(PubNodeApi::new())),
            Arc::new(Mutex::new(ImgNode::new(
                &Node::default(),
                &StartOptionsMode::Simple,
            ))),
            Arc::new(Mutex::new(ImgP2pool::new())),
            Arc::new(Mutex::new(ImgXmrig::new())),
            Arc::new(Mutex::new(ImgProxy::new())),
            Arc::new(Mutex::new(GupaxP2poolApi::new())),
            Arc::clone(&local_ip),
            Arc::clone(&public_ip),
            Arc::clone(&reachable),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(sysinfo::System::new())),
            Arc::new(Mutex::new(NotificationApi {
                notifications: vec![],
            })),
        )));
        let mut xmrig_proxy = XmrigProxy {
            simple: false,
            ..Default::default()
        };
        let (mut pools, mut buffer) = (Pool::new_vec(), String::new());
        assert!(!console_locked_by_command(&api, &process, |ui| {
            xmrig_proxy.show(
                &process,
                &mut pools,
                &api,
                &mut buffer,
                ui,
                3333,
                &local_ip,
                &public_ip,
                &reachable,
                &helper,
            )
        }));
    }
}
