use crate::app::App;
#[cfg(not(feature = "distro"))]
use crate::components::update::Update;
use crate::components::update::check_binary_path;
use crate::helper::crawler::Crawler;
use crate::helper::{Helper, ProcessName};
use crate::utils::constants::{
    APP_MAX_HEIGHT, APP_MAX_WIDTH, APP_MIN_HEIGHT, APP_MIN_WIDTH, BYTES_ICON,
};
use crate::utils::regex::Regexes;
use std::io::Write;
//---------------------------------------------------------------------------------------------------- Init functions
use crate::disk::state::*;
use crate::{components::node::Ping, miscs::clamp_scale};
use crate::{info, warn};
use eframe::NativeOptions;
use egui::TextStyle::Small;
use egui::TextStyle::{Body, Button, Heading, Monospace};
use egui::*;
use env_logger::fmt::style::Style;
use env_logger::{Builder, WriteStyle};
use flexi_logger::{FileSpec, Logger};
use log::LevelFilter;
use std::sync::Arc;
use std::time::Instant;

#[cold]
#[inline(never)]
// everything is resized from here with the scale.
pub fn init_text_styles(ctx: &egui::Context, pixels_per_point: f32) {
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (Small, FontId::new(14.0, egui::FontFamily::Monospace)),
            (Monospace, FontId::new(15.0, egui::FontFamily::Monospace)),
            (Body, FontId::new(16.0, egui::FontFamily::Monospace)),
            (Button, FontId::new(17.0, egui::FontFamily::Monospace)),
            (Heading, FontId::new(22.0, egui::FontFamily::Monospace)),
        ]
        .into();
        style.spacing.icon_width_inner = 24.0;
        style.spacing.icon_width = 48.0;
        style.spacing.icon_spacing = 16.0;
        style.spacing.button_padding = [8.0, 8.0].into();
        style.spacing.item_spacing = [8.0, 8.0].into();
        style.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 12.0,
            ..egui::style::ScrollStyle::solid()
        };
    });
    // Make sure scale f32 is a regular number.
    let pixels_per_point = clamp_scale(pixels_per_point);
    ctx.set_pixels_per_point(pixels_per_point);
    ctx.request_repaint();
}

#[cold]
#[inline(never)]
pub fn init_logger(now: Instant, logfile: bool) {
    if logfile {
        Logger::try_with_env_or_str("info")
            .unwrap()
            .log_to_file(FileSpec::default())
            .start()
            .unwrap();
    } else {
        let filter_env = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".to_string());
        let filter = match filter_env.as_str() {
            "error" | "Error" | "ERROR" => LevelFilter::Error,
            "warn" | "Warn" | "WARN" => LevelFilter::Warn,
            "debug" | "Debug" | "DEBUG" => LevelFilter::Debug,
            "trace" | "Trace" | "TRACE" => LevelFilter::Trace,
            _ => LevelFilter::Info,
        };

        Builder::new()
        .format(move |buf, record| {
            let level = record.level();
            let level_style = buf.default_level_style(level);
            let dimmed = Style::new().dimmed();
            writeln!(
                buf,
                "{level_style}[{}]{level_style:#} [{dimmed}{:.3}{dimmed:#}] [{dimmed}{}{dimmed:#}:{dimmed}{}{dimmed:#}] {}",
                level,
                now.elapsed().as_secs_f32(),
                record.file().unwrap_or("???"),
                record.line().unwrap_or(0),
                record.args(),
            )
        })
        .filter_level(filter)
        .write_style(WriteStyle::Always)
        .parse_default_env()
        .format_timestamp_millis()
        .init();
        info!("Log level ... {filter}");
    }
    info!("init_logger() ... OK");
}

#[cold]
#[inline(never)]
pub fn init_options(initial_window_size: Option<Vec2>) -> NativeOptions {
    let mut options = eframe::NativeOptions::default();
    options.viewport.min_inner_size = Some(Vec2::new(APP_MIN_WIDTH, APP_MIN_HEIGHT));
    options.viewport.max_inner_size = Some(Vec2::new(APP_MAX_WIDTH, APP_MAX_HEIGHT));
    options.viewport.inner_size = initial_window_size;
    let icon = image::load_from_memory(BYTES_ICON)
        .expect("Failed to read icon bytes")
        .to_rgba8();
    let (icon_width, icon_height) = icon.dimensions();
    options.viewport.icon = Some(Arc::new(egui::viewport::IconData {
        rgba: icon.into_raw(),
        width: icon_width,
        height: icon_height,
    }));
    info!("init_options() ... OK");
    options
}

#[cold]
#[inline(never)]
pub fn init_auto(app: &mut App) {
    // Return early if [--no-startup] was not passed
    if app.no_startup {
        info!("[--no-startup] flag passed, skipping init_auto()...");
        return;
    } else if app.error_state.error {
        info!("App error detected, skipping init_auto()...");
        return;
    } else {
        info!("Starting init_auto()...");
    }
    // update the absolute path, or gupax will crash if it's not valid and p2pool is enabled since it only verify the relative path.
    // it could be the case if gupax was manually installed, the relative path stay the same but absolute path will also still stay on the old path that maybe is deleted. SO the check with the absolute path would be valid but when launched with the other old/wrong path from absolute, it would panic.
    // this change is non breaking and will fix the issue if it was occurring.
    app.state
        .update_absolute_path()
        .expect("could not get the current path");

    // [Auto-Update]
    #[cfg(not(feature = "distro"))]
    if app.state.gupax.auto.is_enabled(&AutoStart::Update) {
        Update::spawn_thread(
            &app.og,
            &app.state.gupax,
            &app.state_path,
            &app.update,
            &mut app.error_state,
            &app.restart,
        );
    } else {
        info!("Skipping auto-update...");
    }

    // [Auto-Crawl]
    // If the crawling is used, we do not use custom backup nodes
    if app.state.gupax.auto.crawl {
        info!("Auto Stating crawler...");
        Crawler::start(
            &app.crawler,
            &app.state.p2pool.crawl_settings,
            Some(app.backup_hosts.clone()),
        );
    }
    // [Auto-Ping]
    // do not ping if there is no discovered nodes to ping, unless we can add the selected remote node.
    if app.state.p2pool.auto_ping {
        if app.ping.lock().unwrap().nodes.is_empty()
            && let Some(node) = &app.state.p2pool.selected_remote_node
        {
            app.ping.lock().unwrap().nodes.push(node.clone());
        }
        Ping::spawn_thread(&app.ping)
    } else {
        info!("Skipping auto-ping...");
    }

    // [Auto-Node]
    if app
        .state
        .gupax
        .auto
        .is_enabled(&AutoStart::Process(ProcessName::Node))
    {
        if !Gupax::path_is_file(&app.state.gupax.node_path) {
            warn!("Gupax | Node path is not a file! Skipping auto-node...");
        } else if !check_binary_path(&app.state.gupax.node_path, ProcessName::Node) {
            warn!("Gupax | Node path is not valid! Skipping auto-node...");
        } else if ProcessName::Node
            .is_process_running(&mut app.helper.lock().unwrap().sys_info.lock().unwrap())
        {
            warn!(
                "Gupax | Node instance is already running outside of Gupax ! Skipping auto-node..."
            );
        } else {
            // enable hugepage on linux
            // sudo sysctl vm.nr_hugepages=3072
            Helper::start_node(
                &app.helper,
                &app.state.node,
                &app.state.gupax.absolute_node_path,
            );
        }
    } else {
        info!("Skipping auto-node...");
    }
    // [Auto-P2Pool]
    // Needs auto Crawl to be done if it wants to use backup nodes.
    // We can not wait for the crawling to be done here because the user would wait the UI to show up.
    // We must spawn p2pool and make it wait and fetch the backup nodes from the thread.
    if app
        .state
        .gupax
        .auto
        .is_enabled(&AutoStart::Process(ProcessName::P2pool))
    {
        if !Regexes::addr_ok(&app.state.p2pool.address) {
            warn!("Gupax | P2Pool address is not valid! Skipping auto-p2pool...");
        } else if !Gupax::path_is_file(&app.state.gupax.p2pool_path) {
            warn!("Gupax | P2Pool path is not a file! Skipping auto-p2pool...");
        } else if !check_binary_path(&app.state.gupax.p2pool_path, ProcessName::P2pool) {
            warn!("Gupax | P2Pool path is not valid! Skipping auto-p2pool...");
        } else if crate::helper::ProcessName::P2pool
            .is_process_running(&mut app.helper.lock().unwrap().sys_info.lock().unwrap())
        {
            warn!(
                "Gupax | P2pool instance is already running outside of Gupax ! Skipping auto-node..."
            );
        } else if app.state.p2pool.selected_remote_node.is_none() {
            warn!(
                "Gupax | P2pool can start because there is no discovered nodes yet ! Skipping auto-node..."
            );
        } else {
            Helper::start_p2pool(
                &app.helper,
                &app.state.p2pool,
                &app.state.node,
                &app.state.gupax.absolute_p2pool_path,
                &app.backup_hosts.clone(),
                false,
                &app.crawler,
            );
        }
    } else {
        info!("Skipping auto-p2pool...");
    }

    // [Auto-XMRig-Proxy]
    if app
        .state
        .gupax
        .auto
        .is_enabled(&AutoStart::Process(ProcessName::XmrigProxy))
    {
        if !Gupax::path_is_file(&app.state.gupax.xmrig_proxy_path) {
            warn!("Gupax | Xmrig-Proxy path is not a file! Skipping auto-xmrig_proxy...");
        } else if !check_binary_path(&app.state.gupax.xmrig_proxy_path, ProcessName::XmrigProxy) {
            warn!("Gupax | Xmrig-Proxy path is not valid! Skipping auto-xmrig_proxy...");
        } else if crate::helper::ProcessName::XmrigProxy
            .is_process_running(&mut app.helper.lock().unwrap().sys_info.lock().unwrap())
        {
            warn!(
                "Gupax | Xmrig-Proxy instance is already running outside of Gupax ! Skipping auto-node..."
            );
        } else {
            Helper::start_xp(
                &app.helper,
                &app.state.xmrig_proxy,
                &app.state.p2pool,
                &app.state.gupax.absolute_xp_path,
            );
        }
    } else {
        info!("Skipping auto-XMRig-Proxy...");
    }
    // [Auto-XMRig]
    if app
        .state
        .gupax
        .auto
        .is_enabled(&AutoStart::Process(ProcessName::Xmrig))
    {
        if !Gupax::path_is_file(&app.state.gupax.xmrig_path) {
            warn!("Gupax | XMRig path is not an executable! Skipping auto-xmrig...");
        } else if !check_binary_path(&app.state.gupax.xmrig_path, ProcessName::Xmrig) {
            warn!("Gupax | XMRig path is not valid! Skipping auto-xmrig...");
        } else if crate::helper::ProcessName::Xmrig
            .is_process_running(&mut app.helper.lock().unwrap().sys_info.lock().unwrap())
        {
            warn!(
                "Gupax | Xmrig instance is already running outside of Gupax ! Skipping auto-node..."
            );
        } else {
            Helper::start_xmrig(
                &app.helper,
                &app.state.xmrig,
                &app.state.p2pool,
                &app.state.xmrig_proxy,
                &app.state.gupax.absolute_xmrig_path,
            );
        }
    } else {
        info!("Skipping auto-xmrig...");
    }
    // [Auto-XvB]
    if app
        .state
        .gupax
        .auto
        .is_enabled(&AutoStart::Process(ProcessName::Xvb))
    {
        Helper::start_xvb(
            &app.helper,
            &app.state.xvb,
            &app.state.p2pool,
            &app.state.xmrig,
            &app.state.xmrig_proxy,
        );
    } else {
        info!("Skipping auto-xvb...");
    }
    // [Notifications Service]
    Helper::start_notifications(&app.helper);
}
