use crate::app::ErrorState;
use crate::app::Restart;
use crate::app::panels::middle::common::toggle::toggle_ui_compact;
use crate::app::panels::middle::*;
use crate::components::gupax::*;
use crate::components::update::Update;
use crate::components::update::check_binary_path;
use crate::disk::state::*;
use crate::helper::notification::NotificationApi;
use crate::miscs::height_txt_before_button;
use common::state_edit_field::slider_state_field;
use log::debug;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use strum::EnumCount;
use strum::IntoEnumIterator;
impl Gupax {
    #[inline(always)] // called once
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        og: &Arc<Mutex<State>>,
        state_path: &Path,
        update: &Arc<Mutex<Update>>,
        file_window: &Arc<Mutex<FileWindow>>,
        error_state: &mut ErrorState,
        restart: &Arc<Mutex<Restart>>,
        _frame: &mut eframe::Frame,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        must_resize: &mut bool,
        api_notification: &Arc<Mutex<NotificationApi>>,
    ) {
        // Update button + Progress bar
        debug!("Gupax Tab | Rendering [Update] button + progress bar");
        let height_font = ui.text_style_height(&TextStyle::Body);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.style_mut().spacing.item_spacing = [height_font, height_font].into();
            ui.group(|ui| {
                let updating = *update.lock().unwrap().updating.lock().unwrap();
                ui.vertical_centered(|ui| {
                    ui.add_space(height_font);
                    ui.style_mut().spacing.button_padding = ui.style().spacing.button_padding * 3.0;
                    // If [Gupax] is being built for a Linux distro,
                    // disable built-in updating completely.
                    #[cfg(feature = "distro")]
                    ui.disable();
                    #[cfg(feature = "distro")]
                    // ui.add_sized([width, button], Button::new("Updates are disabled"))
                    // .on_disabled_hover_text(DISTRO_NO_UPDATE);
                    ui.button("Updates are disabled")
                        .on_disabled_hover_text(DISTRO_NO_UPDATE);
                    #[cfg(not(feature = "distro"))]
                    ui.add_enabled_ui(!updating && *restart.lock().unwrap() == Restart::No, |ui| {
                        #[cfg(not(feature = "distro"))]
                        // if ui
                        //     .add_sized([width, button], Button::new("Check for updates"))
                        if ui
                            .button("Check for updates")
                            .on_hover_text(GUPAX_UPDATE)
                            .clicked()
                        {
                            Update::spawn_thread(
                                og,
                                self,
                                state_path,
                                update,
                                error_state,
                                restart,
                            );
                        }
                    });
                    ui.add_enabled_ui(updating, |ui| {
                        let prog = *update.lock().unwrap().prog.lock().unwrap();
                        let msg = format!(
                            "{}\n{}{}",
                            *update.lock().unwrap().msg.lock().unwrap(),
                            prog,
                            "%"
                        );
                        ui.label(msg);
                        if updating {
                            ui.spinner();
                        } else {
                            ui.label("...");
                        }
                        ui.add(ProgressBar::new(
                            update.lock().unwrap().prog.lock().unwrap().round() / 100.0,
                        ));
                    });
                });
            });

            // debug!("Gupax Tab | Rendering bool buttons");
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
            // debug!("Gupax Tab | Rendering bool buttons");
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
            if self.simple {
                return;
            }

            debug!("Gupax Tab | Rendering Node/P2Pool/XMRig/XMRig-Proxy path selection");
            // need to clone bool so file_window is not locked across a thread
            let window_busy = file_window.lock().unwrap().thread.to_owned();
            ui.group(|ui| {
                ui.push_id(2, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(Label::new(
                            RichText::new("Node/P2Pool/XMRig/XMRig-Proxy PATHs")
                                .underline()
                                .color(LIGHT_GRAY),
                        ))
                        .on_hover_text("Gupax is online");
                    });
                    ui.separator();
                    ScrollArea::horizontal().show(ui, |ui| {
                        ui.vertical(|ui| {
                            BundledProcess::iter().for_each(|name| {
                                path_binary(
                                    self.path_binary(&name),
                                    name.process_name(),
                                    ui,
                                    window_busy,
                                    file_window,
                                )
                            });
                        });
                    });
                });
                let mut guard = file_window.lock().unwrap();
                if guard.picked_p2pool {
                    self.p2pool_path.clone_from(&guard.p2pool_path);
                    guard.picked_p2pool = false;
                }
                if guard.picked_xmrig {
                    self.xmrig_path.clone_from(&guard.xmrig_path);
                    guard.picked_xmrig = false;
                }
                if guard.picked_xp {
                    self.xmrig_proxy_path.clone_from(&guard.xmrig_proxy_path);
                    guard.picked_xp = false;
                }
                if guard.picked_node {
                    self.node_path.clone_from(&guard.node_path);
                    guard.picked_node = false;
                }
                drop(guard);
            });
            // Saved [Tab]
            debug!("Gupax Tab | Rendering [Tab] selector");
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(Label::new(
                        RichText::new("Default Tab").underline().color(LIGHT_GRAY),
                    ))
                    .on_hover_text(GUPAX_TAB);
                });
                ui.separator();
                ui.push_id(1, |ui| {
                    ScrollArea::horizontal().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let width = ((ui.available_width() / Tab::COUNT as f32)
                                - (ui.spacing().button_padding.y * 2.0
                                    + ui.spacing().item_spacing.x)
                                - SPACE)
                                .max(height_txt_before_button(ui, &TextStyle::Button) * 2.0);
                            Tab::iter().enumerate().for_each(|(count, tab)| {
                                if ui
                                    .add_sized(
                                        [width, height_txt_before_button(ui, &TextStyle::Button)],
                                        Button::selectable(self.tab == tab, tab.to_string()),
                                    )
                                    .on_hover_text(tab.msg_default_tab())
                                    .clicked()
                                {
                                    self.tab = tab;
                                }

                                if count + 1 != Tab::COUNT {
                                    ui.separator();
                                }
                            })
                        });
                    });
                });
            });

            // Gupax App resolution sliders
            debug!("Gupax Tab | Rendering resolution sliders");
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(Label::new(
                        RichText::new("Width/Height/Scaling Adjustment")
                            .underline()
                            .color(LIGHT_GRAY),
                    ))
                    .on_hover_text(GUPAX_ADJUST);
                    ui.separator();
                });
                ui.horizontal(|ui| {
                    ScrollArea::horizontal().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width() / 2.0);
                            match self.ratio {
                                Ratio::None => (),
                                Ratio::Width => {
                                    let width = self.selected_width as f64;
                                    let height = (width / 1.333).round();
                                    self.selected_height = height as u16;
                                }
                                Ratio::Height => {
                                    let height = self.selected_height as f64;
                                    let width = (height * 1.333).round();
                                    self.selected_width = width as u16;
                                }
                            }
                            // let height = height / 3.5;
                            // let size = vec2(width, height);
                            ui.add_enabled_ui(self.ratio != Ratio::Height, |ui| {
                                let description = format!(
                                    " Width   [{}-{}]:",
                                    APP_MIN_WIDTH as u16, APP_MAX_WIDTH as u16
                                );
                                slider_state_field(
                                    ui,
                                    &description,
                                    GUPAX_WIDTH,
                                    &mut self.selected_width,
                                    APP_MIN_WIDTH as u16..=APP_MAX_WIDTH as u16,
                                );
                            });
                            ui.add_enabled_ui(self.ratio != Ratio::Width, |ui| {
                                let description = format!(
                                    " Height  [{}-{}]:",
                                    APP_MIN_HEIGHT as u16, APP_MAX_HEIGHT as u16
                                );
                                slider_state_field(
                                    ui,
                                    &description,
                                    GUPAX_HEIGHT,
                                    &mut self.selected_height,
                                    APP_MIN_HEIGHT as u16..=APP_MAX_HEIGHT as u16,
                                );
                            });
                            ui.horizontal(|ui| {
                                let description =
                                    format!(" Scaling   [{APP_MIN_SCALE}..{APP_MAX_SCALE}]:");
                                ui.add_sized(
                                    [0.0, height_txt_before_button(ui, &TextStyle::Body)],
                                    Label::new(description),
                                );
                                ui.style_mut().spacing.slider_width = (ui.available_width()
                                    - ui.spacing().item_spacing.x * 4.0
                                    - ui.spacing().scroll.bar_width
                                    - SPACE * 1.0
                                    + 2.0)
                                    .max(80.0);
                                ui.add(
                                    Slider::new(
                                        &mut self.selected_scale,
                                        APP_MIN_SCALE..=APP_MAX_SCALE,
                                    )
                                    .step_by(0.1),
                                )
                                .on_hover_text(GUPAX_SCALE);
                            });
                        });
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Button);
                        ui.separator();
                        // Width/Height locks
                        ui.vertical(|ui| {
                            use Ratio::*;
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(self.ratio == Width, "Lock to width")
                                    .on_hover_text(GUPAX_LOCK_WIDTH)
                                    .clicked()
                                {
                                    self.ratio = Width;
                                }
                                ui.separator();
                                if ui
                                    .selectable_label(self.ratio == Height, "Lock to height")
                                    .on_hover_text(GUPAX_LOCK_HEIGHT)
                                    .clicked()
                                {
                                    self.ratio = Height;
                                }
                                ui.separator();
                                if ui
                                    .selectable_label(self.ratio == None, "No lock")
                                    .on_hover_text(GUPAX_NO_LOCK)
                                    .clicked()
                                {
                                    self.ratio = None;
                                }
                                ui.separator();
                                if ui.button("Set").on_hover_text(GUPAX_SET).clicked() {
                                    let size = Vec2::new(
                                        self.selected_width as f32,
                                        self.selected_height as f32,
                                    );
                                    ui.ctx().send_viewport_cmd(
                                        egui::viewport::ViewportCommand::InnerSize(size),
                                    );
                                    *must_resize = true;
                                }
                            });
                        });
                    })
                });
            });
            debug!("Gupax Tab | Rendering Renderer chooser");
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    let label = ui
                        .add(Label::new(
                            RichText::new("[WGPU]/[GLOW] Renderer")
                                .underline()
                                .color(LIGHT_GRAY),
                        ))
                        .on_hover_text(GUPAX_RENDERER);
                    ui.separator();
                    if toggle_ui_compact(&mut self.renderer_use_glow, ui)
                        .labelled_by(label.id)
                        .on_hover_text("Disabled: WGPU\nEnabled: GLOW")
                        .clicked()
                    {
                        *restart.lock().unwrap() = Restart::Yes
                    }
                });
            });
            debug!("Gupax Tab | Rendering Notification checkbox");
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(Label::new(
                        RichText::new("Notifications").underline().color(LIGHT_GRAY),
                    ))
                    .on_hover_text(GUPAX_ADJUST);
                    ui.separator();
                    self.horizontal_flex_notifications(
                        ui,
                        Notification::iter().collect(),
                        api_notification,
                    );
                });
            });
        });
    }
    /// widget: AutoStart variant and selectable label (true) or checkbox (false)
    pub fn horizontal_flex_notifications(
        &mut self,
        ui: &mut Ui,
        notifications: Vec<Notification>,
        api_notification: &Arc<Mutex<NotificationApi>>,
    ) {
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
                                let mut is_checked = self.notifications.contains(notification);
                                let widget =
                                    Checkbox::new(&mut is_checked, notification.to_string());

                                if ui
                                    .add_sized(size, widget)
                                    .on_hover_text(notification.help_msg())
                                    .clicked()
                                {
                                    if is_checked {
                                        self.notifications.push(*notification);
                                        // reorganize in case the order was changed
                                        self.notifications.sort_unstable();
                                    } else {
                                        self.notifications.retain(|n| n != notification);
                                    }
                                    // apply the settings immediately if they change
                                    api_notification.lock().unwrap().notifications =
                                        self.notifications.clone();
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
                                let mut is_checked = self.auto.is_enabled(auto);
                                let widget = Checkbox::new(&mut is_checked, auto.to_string());

                                if ui
                                    .add_sized(size, widget)
                                    .on_hover_text(auto.help_msg())
                                    .clicked()
                                {
                                    self.auto.enable(auto, is_checked);
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
                                    let mut is_checked = self.show_processes.contains(process);
                                    let widget =
                                        Checkbox::new(&mut is_checked, process.to_string());

                                    if ui.add_sized(size, widget).clicked() {
                                        if is_checked {
                                            self.show_processes.push(*process);
                                            // reorganize in case the order was changed
                                            self.show_processes.sort_unstable();
                                        } else {
                                            self.show_processes.retain(|p| p != process);
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
