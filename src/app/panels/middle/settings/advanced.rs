use crate::utils::i18n::t;
use egui::{Button, Label, RichText, ScrollArea, Slider, TextStyle, Vec2};
use log::debug;
use strum::{EnumCount, IntoEnumIterator};

use crate::{
    app::{
        App, Tab,
        panels::middle::{
            common::{state_edit_field::slider_state_field, toggle::toggle_ui_compact},
            settings::path_binary,
        },
    },
    components::gupax::Ratio,
    disk::state::{BundledProcess, Notification},
    miscs::height_txt_before_button,
    utils::constants::{
        APP_MAX_HEIGHT, APP_MAX_SCALE, APP_MAX_WIDTH, APP_MIN_HEIGHT, APP_MIN_SCALE, APP_MIN_WIDTH,
        LIGHT_GRAY, SPACE, gupax_adjust, gupax_height, gupax_lock_height, gupax_lock_width,
        gupax_no_lock, gupax_renderer, gupax_scale, gupax_set, gupax_tab, gupax_width,
    },
};

impl App {
    pub fn show_settings_advanced(&mut self, ui: &mut egui::Ui) {
        ScrollArea::both().show(ui, |ui| {
            self.show_settings_simple(ui);
            debug!("Gupax Tab | Rendering Node/P2Pool/XMRig/XMRig-Proxy path selection");
            // need to clone bool so file_window is not locked across a thread
            let window_busy = self.file_window.lock().unwrap().thread.to_owned();
            ui.group(|ui| {
                ui.push_id(2, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(Label::new(
                            RichText::new(t!("Node/P2Pool/XMRig/XMRig-Proxy PATHs"))
                                .underline()
                                .color(LIGHT_GRAY),
                        ))
                        .on_hover_text(t!("Gupax is online"));
                    });
                    ui.separator();
                    ScrollArea::horizontal().show(ui, |ui| {
                        ui.vertical(|ui| {
                            BundledProcess::iter().for_each(|name| {
                                path_binary(
                                    self.state.gupax.path_binary(&name),
                                    name.process_name(),
                                    ui,
                                    window_busy,
                                    &self.file_window,
                                )
                            });
                        });
                    });
                });
                let mut guard = self.file_window.lock().unwrap();
                if guard.picked_p2pool {
                    self.state.gupax.p2pool_path.clone_from(&guard.p2pool_path);
                    guard.picked_p2pool = false;
                }
                if guard.picked_xmrig {
                    self.state.gupax.xmrig_path.clone_from(&guard.xmrig_path);
                    guard.picked_xmrig = false;
                }
                if guard.picked_xp {
                    self.state
                        .gupax
                        .xmrig_proxy_path
                        .clone_from(&guard.xmrig_proxy_path);
                    guard.picked_xp = false;
                }
                if guard.picked_node {
                    self.state.gupax.node_path.clone_from(&guard.node_path);
                    guard.picked_node = false;
                }
                drop(guard);
            });
            // Saved [Tab]
            debug!("Gupax Tab | Rendering [Tab] selector");
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(Label::new(
                        RichText::new(t!("Default Tab"))
                            .underline()
                            .color(LIGHT_GRAY),
                    ))
                    .on_hover_text(gupax_tab());
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
                                        Button::selectable(
                                            self.state.gupax.tab == tab,
                                            tab.to_string(),
                                        ),
                                    )
                                    .on_hover_text(tab.msg_default_tab())
                                    .clicked()
                                {
                                    self.state.gupax.tab = tab;
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
                        RichText::new(t!("Width/Height/Scaling Adjustment"))
                            .underline()
                            .color(LIGHT_GRAY),
                    ))
                    .on_hover_text(gupax_adjust());
                    ui.separator();
                });
                ui.horizontal(|ui| {
                    ScrollArea::horizontal().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width() / 2.0);
                            match self.state.gupax.ratio {
                                Ratio::None => (),
                                Ratio::Width => {
                                    let width = self.state.gupax.selected_width as f64;
                                    let height = (width / 1.333).round();
                                    self.state.gupax.selected_height = height as u16;
                                }
                                Ratio::Height => {
                                    let height = self.state.gupax.selected_height as f64;
                                    let width = (height * 1.333).round();
                                    self.state.gupax.selected_width = width as u16;
                                }
                            }
                            // let height = height / 3.5;
                            // let size = vec2(width, height);
                            ui.add_enabled_ui(self.state.gupax.ratio != Ratio::Height, |ui| {
                                let description = format!(
                                    " Width   [{}-{}]:",
                                    APP_MIN_WIDTH as u16, APP_MAX_WIDTH as u16
                                );
                                slider_state_field(
                                    ui,
                                    &description,
                                    gupax_width(),
                                    &mut self.state.gupax.selected_width,
                                    APP_MIN_WIDTH as u16..=APP_MAX_WIDTH as u16,
                                );
                            });
                            ui.add_enabled_ui(self.state.gupax.ratio != Ratio::Width, |ui| {
                                let description = format!(
                                    " Height  [{}-{}]:",
                                    APP_MIN_HEIGHT as u16, APP_MAX_HEIGHT as u16
                                );
                                slider_state_field(
                                    ui,
                                    &description,
                                    gupax_height(),
                                    &mut self.state.gupax.selected_height,
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
                                        &mut self.state.gupax.selected_scale,
                                        APP_MIN_SCALE..=APP_MAX_SCALE,
                                    )
                                    .step_by(0.1),
                                )
                                .on_hover_text(gupax_scale());
                            });
                        });
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Button);
                        ui.separator();
                        // Width/Height locks
                        ui.vertical(|ui| {
                            use Ratio::*;
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        self.state.gupax.ratio == Width,
                                        t!("Lock to width"),
                                    )
                                    .on_hover_text(gupax_lock_width())
                                    .clicked()
                                {
                                    self.state.gupax.ratio = Width;
                                }
                                ui.separator();
                                if ui
                                    .selectable_label(
                                        self.state.gupax.ratio == Height,
                                        t!("Lock to height"),
                                    )
                                    .on_hover_text(gupax_lock_height())
                                    .clicked()
                                {
                                    self.state.gupax.ratio = Height;
                                }
                                ui.separator();
                                if ui
                                    .selectable_label(self.state.gupax.ratio == None, t!("No lock"))
                                    .on_hover_text(gupax_no_lock())
                                    .clicked()
                                {
                                    self.state.gupax.ratio = None;
                                }
                                ui.separator();
                                if ui.button("Set").on_hover_text(gupax_set()).clicked() {
                                    let size = Vec2::new(
                                        self.state.gupax.selected_width as f32,
                                        self.state.gupax.selected_height as f32,
                                    );
                                    ui.ctx().send_viewport_cmd(
                                        egui::viewport::ViewportCommand::InnerSize(size),
                                    );
                                    self.must_resize = true;
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
                        .on_hover_text(gupax_renderer());
                    ui.separator();
                    if toggle_ui_compact(&mut self.state.gupax.renderer_use_glow, ui)
                        .labelled_by(label.id)
                        .on_hover_text(t!("Disabled: WGPU\nEnabled: GLOW"))
                        .clicked()
                    {
                        *self.restart.lock().unwrap() = true
                    }
                });
            });
            debug!("Gupax Tab | Rendering Notification checkbox");
            ui.group(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(Label::new(
                        RichText::new(t!("Notifications"))
                            .underline()
                            .color(LIGHT_GRAY),
                    ))
                    .on_hover_text(gupax_adjust());
                    ui.separator();
                    self.horizontal_flex_notifications(ui, Notification::iter().collect());
                });
            });
        });
    }
}
