use crate::app::App;
use crate::app::panels::middle::*;
use crate::app::submenu_enum::SubmenuGupax;
use crate::components::update::check_binary_path;
use crate::disk::state::*;
use std::sync::Arc;
use std::sync::Mutex;
use strum::IntoEnumIterator;

mod advanced;
mod updates;

impl App {
    pub fn show_settings(&mut self, ui: &mut egui::Ui) {
        match self.state.gupax.submenu {
            SubmenuGupax::Simple => self.show_settings_simple(ui),
            SubmenuGupax::Advanced => self.show_settings_advanced(ui),
            SubmenuGupax::Updates => self.show_settings_updates(ui),
        }
    }
    fn show_settings_simple(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical_centered(|ui| {
                ui.add(Label::new(
                    RichText::new("Visible Processes")
                        .underline()
                        .color(LIGHT_GRAY),
                ))
            });
            ui.separator();
            self.horizontal_flex_show_processes(ui, ProcessName::having_tab());
        })
        .response
        .on_hover_text(
            "Show(checked) elements (Tab/Status column/bottom status) related to a process",
        );

        ui.group(|ui| {
            ui.vertical_centered(|ui| {
                ui.add(Label::new(
                    RichText::new("Default Behaviour")
                        .underline()
                        .color(LIGHT_GRAY),
                ))
            });
            ui.separator();
            self.horizontal_flex_auto_start(ui, AutoStart::ALL);
        });
    }
    /// widget: AutoStart variant and selectable label (true) or checkbox (false)
    pub fn horizontal_flex_notifications(&mut self, ui: &mut Ui, notifications: Vec<Notification>) {
        let text_style = TextStyle::Button;
        ui.style_mut().override_text_style = Some(text_style);
        let spacing = 2.0;
        ScrollArea::horizontal().id_salt("notif").show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let width = (((ui.available_width()) / notifications.len() as f32)
                    - ((ui.style().spacing.item_spacing.x * 2.0) + spacing))
                    .max(0.0);
                // TODO: calculate minimum width needed, if ui.available width is less, show items on two lines, then on 3 etc..
                // checkbox padding + item spacing + text + separator

                let size = [width, 0.0];
                let len = notifications.iter().len();
                for (count, notification) in notifications.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let mut is_checked =
                                    self.state.gupax.notifications.contains(notification);
                                let widget =
                                    Checkbox::new(&mut is_checked, notification.to_string());

                                if ui
                                    .add_sized(size, widget)
                                    .on_hover_text(notification.help_msg())
                                    .clicked()
                                {
                                    if is_checked {
                                        self.state.gupax.notifications.push(notification.clone());
                                        // reorganize in case the order was changed
                                        self.state.gupax.notifications.sort_unstable();
                                    } else {
                                        self.state
                                            .gupax
                                            .notifications
                                            .retain(|n| n != notification);
                                    }
                                    // apply the settings immediately if they change
                                    self.notifications_api.lock().unwrap().notifications =
                                        self.state.gupax.notifications.clone();
                                }
                            });
                            // add a space to prevent selectable button to be at the same line as the end of the top bar. Make it the same spacing as separators.
                            ui.add_space(spacing * 4.0);
                        });
                        if count + 1 != len {
                            ui.add(Separator::default().spacing(spacing).vertical());
                        }
                    });
                }
            });
        });
    }
    /// widget: AutoStart variant and selectable label (true) or checkbox (false)
    pub fn horizontal_flex_auto_start(&mut self, ui: &mut Ui, auto_starts: &[AutoStart]) {
        let text_style = TextStyle::Button;
        ui.style_mut().override_text_style = Some(text_style);
        let spacing = 2.0;
        ScrollArea::horizontal().show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let width = (((ui.available_width()) / auto_starts.len() as f32)
                    - ((ui.style().spacing.item_spacing.x * 2.0) + spacing))
                    .max(0.0);
                // TODO: calculate minimum width needed, if ui.available width is less, show items on two lines, then on 3 etc..
                // checkbox padding + item spacing + text + separator

                let size = [width, 0.0];
                let len = auto_starts.iter().len();
                for (count, auto) in auto_starts.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                let mut is_checked = self.state.gupax.auto.is_enabled(auto);
                                let widget = Checkbox::new(&mut is_checked, auto.to_string());

                                if ui
                                    .add_sized(size, widget)
                                    .on_hover_text(auto.help_msg())
                                    .clicked()
                                {
                                    self.state.gupax.auto.enable(auto, is_checked);
                                    // Answers the one-time close question too.
                                    if *auto == AutoStart::HideToTray {
                                        self.state.gupax.asked_close_to_tray = true;
                                    }
                                }
                            });
                            // add a space to prevent selectable button to be at the same line as the end of the top bar. Make it the same spacing as separators.
                            ui.add_space(spacing * 4.0);
                        });
                        if count + 1 != len {
                            ui.add(Separator::default().spacing(spacing).vertical());
                        }
                    });
                }
            });
        });
    }
    /// widget: AutoStart variant and selectable label (true) or checkbox (false)
    pub fn horizontal_flex_show_processes(&mut self, ui: &mut Ui, processes: Vec<ProcessName>) {
        let text_style = TextStyle::Button;
        ui.style_mut().override_text_style = Some(text_style);
        let spacing = 2.0;
        ScrollArea::horizontal()
            .id_salt("show_processes")
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    let width = (((ui.available_width()) / processes.len() as f32)
                        - ((ui.style().spacing.item_spacing.x * 2.0) + spacing))
                        .max(0.0);
                    // TODO: calculate minimum width needed, if ui.available width is less, show items on two lines, then on 3 etc..
                    // checkbox padding + item spacing + text + separator

                    let size = [width, 0.0];
                    let len = processes.iter().len();
                    for (count, process) in processes.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    let mut is_checked =
                                        self.state.gupax.show_processes.contains(process);
                                    let widget =
                                        Checkbox::new(&mut is_checked, process.to_string());

                                    if ui.add_sized(size, widget).clicked() {
                                        if is_checked {
                                            self.state.gupax.show_processes.push(*process);
                                            // reorganize in case the order was changed
                                            self.state.gupax.show_processes.sort_unstable();
                                        } else {
                                            self.state
                                                .gupax
                                                .show_processes
                                                .retain(|p| p != process);
                                        }
                                    }
                                });
                                // add a space to prevent selectable button to be at the same line as the end of the top bar. Make it the same spacing as separators.
                                ui.add_space(spacing * 4.0);
                            });
                            if count + 1 != len {
                                ui.add(Separator::default().spacing(spacing).vertical());
                            }
                        });
                    }
                });
            });
    }
}
fn path_binary(
    path: &mut String,
    name: ProcessName,
    ui: &mut Ui,
    window_busy: bool,
    file_window: &Arc<Mutex<FileWindow>>,
) {
    // align correctly even with different length of name by adapting the space just after.
    let flex_space = " ".repeat(
        ProcessName::iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.to_string()
                    .len()
                    .partial_cmp(&b.to_string().len())
                    .expect("ProcessName should have values")
            })
            .expect("Iterator cant' be empty")
            .1
            .to_string()
            .len()
            - name.to_string().len()
            + 1,
    );
    let msg = format!(" {name}{flex_space}Binary Path");
    // need to precise the height of text or there will be an misalignment with the button if it's bigger than the text.
    let height =
        (ui.style().spacing.button_padding.y * 2.0) + ui.text_style_height(&TextStyle::Body);
    ui.horizontal(|ui| {
        if path.is_empty() {
            ui.add_sized(
                [0.0, height],
                Label::new(RichText::new(msg + " ➖").color(LIGHT_GRAY)),
            )
            .on_hover_text(name.msg_binary_path_empty());
        } else if !Gupax::path_is_file(path) {
            ui.add_sized(
                [0.0, height],
                Label::new(RichText::new(msg + " ❌").color(RED)),
            )
            .on_hover_text(name.msg_binary_path_not_file());
        } else if !check_binary_path(path, name) {
            ui.add_sized(
                [0.0, height],
                Label::new(RichText::new(msg + " ❌").color(RED)),
            )
            .on_hover_text(name.msg_binary_path_invalid());
        } else {
            ui.add_sized(
                [0.0, height],
                Label::new(RichText::new(msg + " ✔").color(GREEN)),
            )
            .on_hover_text(name.msg_binary_path_ok());
        }
        ui.spacing_mut().text_edit_width = (ui.available_width() - SPACE).max(0.0);
        ui.add_enabled_ui(!window_busy, |ui| {
            if ui.button("Open").on_hover_text(GUPAX_SELECT).clicked() {
                Gupax::spawn_file_window_thread(
                    file_window,
                    name.file_type()
                        .expect("XvB process should not be called in a function related to path"),
                );
            }
            ui.text_edit_singleline(path)
                .on_hover_text(name.msg_path_edit());
        });
    });
}
