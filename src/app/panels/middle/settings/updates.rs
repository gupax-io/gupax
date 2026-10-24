use crate::utils::i18n::t;
#[cfg(not(feature = "distro"))]
use egui::{Checkbox, ComboBox, Label, ProgressBar, ScrollArea, Separator, TextStyle};
#[cfg(not(feature = "distro"))]
use egui_commonmark::CommonMarkViewer;
#[cfg(not(feature = "distro"))]
use log::warn;

use crate::app::App;
use egui::{RichText, Ui};

#[cfg(not(feature = "distro"))]
use crate::{
    components::update::BINARIES_NAME,
    miscs::height_txt_before_button,
    utils::{
        constants::SPACE,
        errors::{ErrorButtons, ErrorFerris, WarnUpdateData},
    },
};

impl App {
    pub fn show_settings_updates(&mut self, ui: &mut Ui) {
        #[cfg(feature = "distro")]
        {
            ui.horizontal_wrapped(|ui|{
            ui.label(RichText::new(t!("Gupax has been installed by your distribution package manager.\nYou should update it from it."))
                    .color(crate::ORANGE));
                });
        }
        #[cfg(not(feature = "distro"))]
        ScrollArea::both().show(ui, |ui| {
            self.update_progress(ui);
            ui.horizontal(|ui| {
                self.update_all_widget(ui);
                ui.group(|ui| {
                    self.horizontal_flex_button_update(ui);
                });
            });
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    for name in BINARIES_NAME {
                        self.binary_update_settings_widget(ui, name);
                    }
                });
                self.view_changelog(ui);
            });
        });
    }

    #[cfg(not(feature = "distro"))]
    fn view_changelog(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.vertical_centered(|ui| {
                if let Some(release) = &self.changelog_selected {
                    ui.group(|ui| {
                        ui.heading(t!("Changelog"));
                        ui.label(
                            RichText::new(format!(
                                "{}\n{}\n{}",
                                release.1,
                                release.0.tag_name,
                                release.0.published_at.date_naive(),
                            ))
                            .text_style(TextStyle::Button),
                        );
                    });
                    ScrollArea::vertical().show(ui, |ui| {
                        CommonMarkViewer::new().show(ui, &mut self.markdown_cache, &release.0.body);
                    });
                } else {
                    ui.group(|ui| {
                        ui.heading(t!("Changelog"));
                        ui.label(t!(
                            "Click on a refresh button and then on a version to see the changelog"
                        ));
                    });
                }
            });
        });
    }
    #[cfg(not(feature = "distro"))]
    fn update_progress(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.vertical_centered(|ui| {
                let updating = self.update.lock().unwrap().updating;
                ui.add_enabled_ui(updating, |ui| {
                    let prog = self.update.lock().unwrap().prog;
                    let msg = format!("{}\n{}{}", self.update.lock().unwrap().msg, prog, "%");
                    ui.label(msg);
                    if updating {
                        ui.spinner();
                    } else {
                        ui.label("...");
                    }
                    ui.add(ProgressBar::new(prog.round() / 100.0));
                });
            });
        });
    }
    #[cfg(not(feature = "distro"))]
    fn warn_downgrade(&mut self, name: &str) -> bool {
        let selected = parse_version(self.state.gupax.updates.selected_version_by_name(name));
        let current = parse_version(self.binaries_version.version_by_name(name));
        if selected < current {
            warn!("Trying to downgrade");
            let msg = if name != "gupax" {
                t!(
                    "You are trying to downgrade a binary. This is potentially dangerous as it is unsupported."
                )
            } else {
                t!(
                    "You are trying to downgrade Gupax. This is really dangerous as you could loose the ability to upgrade it and be stuck on the old version. You would need to re-install it manually. You also could loose your saved configuration"
                )
            };

            self.error_state.set(
                msg,
                ErrorFerris::Oops,
                ErrorButtons::WarnUpdate(WarnUpdateData {
                    yes_button: "Yes downgrade".to_string(),
                    no_button: "No Abort the downgrade".to_string(),
                    name: name.to_string(),
                }),
            );
            return false;
        }
        true
    }
    #[cfg(not(feature = "distro"))]
    pub fn warn_switch_beta(&mut self, name: &str) -> bool {
        if !self.binaries_version.version_by_name(name).contains("BETA")
            && self
                .state
                .gupax
                .updates
                .selected_version_by_name(name)
                .contains("BETA")
        {
            warn!("Trying to downgrade");
            let msg = t!(
                "You are trying to update to a BETA. Do this only if you want to help test unstable software."
            );
            self.error_state.set(
                msg,
                ErrorFerris::Panic,
                ErrorButtons::WarnUpdate(WarnUpdateData {
                    yes_button: "Yes, switch to BETA".to_string(),
                    no_button: "No, stay in STABLE releases".to_string(),
                    name: name.to_string(),
                }),
            );
            return false;
        }
        true
    }
    #[cfg(not(feature = "distro"))]
    fn update_all_widget(&mut self, ui: &mut Ui) {
        ui.add_space(SPACE * 0.9);
        ui.group(|ui|{
            ui.vertical(|ui|{
        ui.add_space(SPACE);
        ui.horizontal(|ui|{
            ui.vertical(|ui|{
        ui.add_space(SPACE * 0.5);
                 let updating = self.update.lock().unwrap().updating;
        ui.add_enabled_ui(!updating, |ui|{
        if ui.button(t!("Update Everything")).on_hover_text(t!("Update all binaries to the latest release")).clicked() {
            self.update.update_all(
                BINARIES_NAME.into_iter().map(|s| s.to_string()).collect(),
                self.state.gupax.clone(),
                self.binaries_version.clone(),
                self.restart.clone(),
            );
        }
        });
            });
        ui.add_space(SPACE );
        ui.checkbox(&mut self.state.gupax.updates.beta, t!("Beta")).on_hover_text(t!("Participate in pre-release. Check only if you want to test release to come before they are stabilized. You will experience bugs."));           
        });
        ui.add_space(SPACE * 0.5);
            });
        });
    }

    #[cfg(not(feature = "distro"))]
    fn binary_update_settings_widget(&mut self, ui: &mut Ui, name: &str) {
        ui.group(|ui| {
            ui.set_min_width((ui.available_width() / 2.0) - SPACE);
            ui.heading(name);
            ui.horizontal(|ui| {
                let version = self.binaries_version.version_by_name(name);
                ui.label(t!("Currently installed version:    "));
                ui.label(version);
            });
            ui.horizontal(|ui| {
                self.refresh_versions_button(ui, name);
                self.update_binary_button(ui, name);
            });
            self.list_versions(ui, name);
            ui.spacing_mut().text_edit_width =
                ui.available_width() / 2.0 - (ui.text_style_height(&TextStyle::Body) * 10.0);
            self.source_field(ui, name);
        });
    }
    #[cfg(not(feature = "distro"))]
    fn refresh_versions_button(&mut self, ui: &mut Ui, name: &str) {
        let updating = self.update.lock().unwrap().updating;
        ui.add_enabled_ui(!updating, |ui| {
            if ui
                .button(t!("Refresh"))
                .on_hover_text(t!("Refresh available versions"))
                .clicked()
            {
                self.update.refresh_versions(
                    vec![name.to_string()],
                    self.state.gupax.clone(),
                    self.binaries_version.clone(),
                );
            }
        });
    }
    #[cfg(not(feature = "distro"))]
    fn update_binary_button(&mut self, ui: &mut Ui, name: &str) {
        let enable = !self
            .update
            .lock()
            .unwrap()
            .releases_by_name(name)
            .is_empty()
            && !self.update.lock().unwrap().updating;
        ui.add_enabled_ui(enable, |ui| {
            if ui
                .button(t!("Update"))
                .on_hover_text(t!("Update to the selected version"))
                .clicked()
                && self.warn_downgrade(name)
                && self.warn_switch_beta(name)
            {
                self.update.update_version(
                    vec![name.to_string()],
                    self.state.gupax.clone(),
                    self.binaries_version.clone(),
                    self.restart.clone(),
                );
            }
        });
    }
    #[cfg(not(feature = "distro"))]
    fn list_versions(&mut self, ui: &mut Ui, name: &str) {
        let selected_version = self.state.gupax.updates.selected_version_by_name_mut(name);
        if !self
            .update
            .lock()
            .unwrap()
            .releases_by_name(name)
            .is_empty()
        {
            ComboBox::new(format!("combo_version_{name}"), "")
                .selected_text(selected_version.to_string())
                .wrap_mode(egui::TextWrapMode::Extend)
                .height(
                    12.0 * (ui.text_style_height(&TextStyle::Button)
                        + (ui.spacing().button_padding.y * 2.0)
                        + ui.spacing().item_spacing.y),
                )
                .show_ui(ui, |ui| {
                    for release in self.update.lock().unwrap().releases_by_name(name) {
                        if ui
                            .selectable_value(
                                selected_version,
                                release.to_string(),
                                release.tag_name.clone(),
                            )
                            .clicked()
                        {
                            self.changelog_selected = Some((release.clone(), name.to_owned()));
                        }
                    }
                });
        }
    }
    #[cfg(not(feature = "distro"))]
    fn source_field(&mut self, ui: &mut Ui, name: &str) {
        let repo = self.state.gupax.updates.source_by_name_mut(name);
        ui.horizontal(|ui|{
        ui.add_sized(
            [0.0, height_txt_before_button(ui, &TextStyle::Body)],
            Label::new(t!("repository:     ")),
        );
        ui.text_edit_singleline(repo).on_hover_text(t!("repository from where the binary will be downloaded. Only github compatible API with specific name convention in releases for binaries are supported.\nExample: github.com/gupax-io/gupax"));
        });
    }

    #[cfg(not(feature = "distro"))]
    pub fn horizontal_flex_button_update(&mut self, ui: &mut Ui) {
        let notification_button = (
            Checkbox::new(
                &mut self.state.gupax.updates.notification_update,
                t!("Notification for new updates"),
            ),
            t!(
                "Checks for new updates every 24 hours and at each restart.\nDoes not apply them.\nRestart Gupax to apply changes."
            ),
        );
        let auto_update_button = (
            Checkbox::new(
                &mut self.state.gupax.updates.automatic_update,
                t!("Automatic updates"),
            ),
            t!(
                "Apply for new updates automatically. It is done every 24h and at each restart\nRestart Gupax to apply changes."
            ),
        );
        let auto_restart_button = (
            Checkbox::new(
                &mut self.state.gupax.updates.automatic_restart,
                t!("Automatic restart"),
            ),
            t!("Gupax will be restarted at the end of an update.\nRestart Gupax to apply changes."),
        );
        let widgets = vec![notification_button, auto_update_button, auto_restart_button];
        let text_style = TextStyle::Button;
        ui.style_mut().override_text_style = Some(text_style);
        let spacing = 2.0;
        ScrollArea::horizontal().show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                let width = (((ui.available_width()) / widgets.len() as f32)
                    - ((ui.style().spacing.item_spacing.x * 2.0) + spacing))
                    .max(0.0);
                let size = [width, 0.0];
                let len = widgets.iter().len();
                for (count, (widget, hover)) in widgets.into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.add_sized(size, widget).on_hover_text(hover);
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

fn parse_version(version: &str) -> Vec<u32> {
    let version = version.trim_start_matches('v');
    let (base, prerelease) = match version.split_once("-rc") {
        Some((base, rc)) => (base, Some(rc.parse().unwrap_or(0))),
        None => (version, None),
    };
    let mut version: Vec<u32> = base.split('.').map(|n| n.parse().unwrap_or(0)).collect();
    //   v2.0.0      -> [2,0,0, 1,0]
    //   v2.0.0-rc1  -> [2,0,0, 0,1]
    match prerelease {
        Some(rc) => version.extend([0, rc]),
        None => version.extend([1, 0]),
    }
    version
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    fn is_downgrade(selected: &str, current: &str) -> bool {
        parse_version(selected) < parse_version(current)
    }

    #[test]
    fn parse_strips_v_prefix_and_splits_components() {
        // A "1,0" marker is appended to every final release (no pre-release).
        assert_eq!(parse_version("v4.16"), vec![4, 16, 1, 0]);
        assert_eq!(parse_version("v4.15.1"), vec![4, 15, 1, 1, 0]);
        assert_eq!(parse_version("v6.25.0"), vec![6, 25, 0, 1, 0]);
        assert_eq!(parse_version("v0.18.4.6"), vec![0, 18, 4, 6, 1, 0]);
        assert_eq!(parse_version("v2.0.1"), vec![2, 0, 1, 1, 0]);
    }

    #[test]
    fn parse_handles_missing_v_prefix() {
        assert_eq!(parse_version("4.16"), vec![4, 16, 1, 0]);
        assert_eq!(parse_version("0.18.4.5"), vec![0, 18, 4, 5, 1, 0]);
    }

    #[test]
    fn parse_release_candidate_marker() {
        assert_eq!(parse_version("v2.0.0-rc3"), vec![2, 0, 0, 0, 3]);
        assert_eq!(parse_version("v2.0.0-rc2"), vec![2, 0, 0, 0, 2]);
        assert_eq!(parse_version("v2.0.0"), vec![2, 0, 0, 1, 0]);
        assert!(is_downgrade("v2.0.0-rc2", "v2.0.0-rc3"));
        assert!(!is_downgrade("v2.0.0-rc3", "v2.0.0-rc2"));
        assert!(!is_downgrade("v2.0.0-rc3", "v2.0.0-rc3"));
        assert!(!is_downgrade("v2.0.0", "v2.0.0-rc3"));
        assert!(is_downgrade("v2.0.0-rc3", "v2.0.0"));
        assert!(is_downgrade("v1.3.11", "v2.0.0-rc3"));
    }

    #[test]
    fn p2pool_pre_release() {
        assert_eq!(parse_version("pre-release-v4.16"), vec![0, 16, 1, 0]);
    }

    #[test]
    fn upgrade_is_not_a_downgrade() {
        assert!(!is_downgrade("v4.16", "v4.15.1"));
        assert!(!is_downgrade("v4.15", "v4.14"));
        assert!(!is_downgrade("v4.15.1", "v4.15"));
        assert!(!is_downgrade("v6.26.0", "v6.25.0"));
        assert!(!is_downgrade("v6.22.3", "v6.22.2"));
        assert!(!is_downgrade("v6.23.0", "v6.22.3"));
        assert!(!is_downgrade("v0.18.5.0", "v0.18.4.6"));
        assert!(!is_downgrade("v0.18.4.6", "v0.18.4.5"));
        assert!(is_downgrade("v0.18.4.6", "v0.18.5.0"));
        assert!(is_downgrade("v0.18.4.5", "v0.18.4.6"));
        assert!(is_downgrade("v0.18.4.0", "v0.18.4.1"));
        assert!(!is_downgrade("v2.0.1", "v2.0.0"));
        assert!(!is_downgrade("v2.0.0", "v1.3.11"));
        assert!(!is_downgrade("v1.3.11", "v1.3.10"));
    }

    #[test]
    fn downgrade_is_detected() {
        assert!(is_downgrade("v4.15.1", "v4.16"));
        assert!(is_downgrade("v4.14", "v4.15"));
        assert!(is_downgrade("v4.15", "v4.15.1"));
        assert!(is_downgrade("v6.25.0", "v6.26.0"));
        assert!(is_downgrade("v6.22.2", "v6.22.3"));
        assert!(is_downgrade("v6.22.3", "v6.23.0"));
        assert!(is_downgrade("v2.0.0", "v2.0.1"));
        assert!(is_downgrade("v1.3.11", "v2.0.0"));
        assert!(is_downgrade("v1.3.10", "v1.3.11"));
    }

    #[test]
    fn same_version_is_not_a_downgrade() {
        assert!(!is_downgrade("v4.16", "v4.16"));
        assert!(!is_downgrade("v4.15.1", "v4.15.1"));
        assert!(!is_downgrade("v0.18.5.0", "v0.18.5.0"));
        assert!(!is_downgrade("v2.0.0-rc1", "v2.0.0-rc1"));
    }
}
