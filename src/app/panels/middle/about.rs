use egui::{Image, Label, ScrollArea, TextStyle, Ui, Vec2};
use log::debug;

use crate::{
    BYTES_BANNER, COMMIT, GUPAX_VERSION, KEYBOARD_SHORTCUTS, OS_NAME, SPACE,
    app::keys::KeyPressed,
    components::update::Update,
    errors::{ErrorButtons, ErrorFerris},
};

impl crate::app::App {
    #[allow(clippy::too_many_arguments)]
    pub fn about_show(&mut self, key: KeyPressed, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui|{
         debug!("App | Entering [About] Tab");
        // If [D], show some debug info with [ErrorState]
        if key.is_d() {
            debug!("App | Entering [Debug Info]");
            #[cfg(feature = "distro")]
            let distro = true;
            #[cfg(not(feature = "distro"))]
            let distro = false;
            let node_gui_len = self.node_api.lock().unwrap().output.len();
            let p2pool_gui_len = self.p2pool_api.lock().unwrap().output.len();
            let xmrig_gui_len = self.xmrig_api.lock().unwrap().output.len();
            let xmrig_proxy_gui_len = self.xmrig_proxy_api.lock().unwrap().output.len();
            let gupax_p2pool_api = self.gupax_p2pool_api.lock().unwrap();
            let debug_info = format!(
"Gupax version: {}\n
Bundled Node version: {}\n
Bundled P2Pool version: {}\n
Bundled XMRig version: {}\n
Bundled XMRig-Proxy version: {}\n
Gupax uptime: {} seconds\n
Selected resolution: {}x{}\n
Internal resolution: {}x{}\n
Operating system: {}\n
Max detected threads: {}\n
Gupax PID: {}\n
State diff: {}\n
Node list length: {}\n
Pool list length: {}\n
Admin privilege: {}\n
Release build: {}\n
Debug build: {}\n
Distro build: {}\n
Build commit: {}\n
OS Data PATH: {}\n
Gupax PATH: {}\n
P2Pool PATH: {}\n
XMRig PATH: {}\n
XMRig-Proxy PATH: {}\n
P2Pool console byte length: {}\n
XMRig console byte length: {}\n
XMRig-Proxy console byte length: {}\n
------------------------------------------ P2POOL IMAGE ------------------------------------------
{:#?}\n
------------------------------------------ XMRIG IMAGE ------------------------------------------
{:#?}\n
------------------------------------------ GUPAX-P2POOL API ------------------------------------------
payout: {:#?}
payout_u64: {:#?}
xmr: {:#?}
path_log: {:#?}\n
------------------------------------------ WORKING STATE ------------------------------------------
{:#?}\n
------------------------------------------ ORIGINAL STATE ------------------------------------------
{:#?}",
							GUPAX_VERSION,
							Update::get_version_binary(&self.state.gupax.absolute_p2pool_path).unwrap_or(String::from("No P2Pool binary found")),
							Update::get_version_binary(&self.state.gupax.absolute_xmrig_path).unwrap_or(String::from("No XMRig binary found")),
							Update::get_version_binary(&self.state.gupax.absolute_xp_path).unwrap_or(String::from("No Xmrig-Proxy binary found")),
							self.now.elapsed().as_secs_f32(),
							self.state.gupax.selected_width,
							self.state.gupax.selected_height,
							self.size.x,
							self.size.y,
							OS_NAME,
							self.max_threads,
							self.pid,
							self.diff,
							self.node_vec.len(),
							self.pool_vec.len(),
							self.admin,
							!cfg!(debug_assertions),
							cfg!(debug_assertions),
							distro,
							COMMIT,
							self.os_data_path.display(),
							self.exe,
							self.state.gupax.absolute_p2pool_path.display(),
							self.state.gupax.absolute_xmrig_path.display(),
							self.state.gupax.absolute_xp_path.display(),
							node_gui_len,
							p2pool_gui_len,
							xmrig_gui_len,
							xmrig_proxy_gui_len,
							self.p2pool_img.lock().unwrap(),
							self.xmrig_img.lock().unwrap(),
							gupax_p2pool_api.payout,
							gupax_p2pool_api.payout_u64,
							gupax_p2pool_api.xmr,
							gupax_p2pool_api.path_log,
							self.state,
							self.og.lock().unwrap(),
						);
            self.error_state
                .set(debug_info, ErrorFerris::Cute, ErrorButtons::Debug);
        }
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            let space = SPACE * 2.0;
            ui.style_mut().override_text_style = Some(TextStyle::Heading);
            // ui.set_max_height(max_height);
            // Display [Gupax] banner
            // let link_width = width / 14.0;
            ui.add_sized(
                Vec2::new(ui.text_style_height(&TextStyle::Heading) * 20.0, 226.0),
                Image::from_bytes("bytes://banner.png", BYTES_BANNER),
            );
            ui.label("is a GUI for mining");
            ui.add_space(space);
            ui.hyperlink_to("[Monero]", "https://www.getmonero.org");
            ui.label("on");
            ui.hyperlink_to("[P2Pool]", "https://www.github.com/SChernykh/p2pool");
            ui.add_space(space);
            ui.label("using");
            ui.add_space(space);
            ui.hyperlink_to("[Monerod]", "https://github.com/monero-project/monero");
            ui.hyperlink_to("[Xmrig]", "https://www.github.com/xmrig/xmrig");
            ui.hyperlink_to("[Xmrig-Proxy]", "https://www.github.com/xmrig/xmrig-proxy");
            ui.add_space(space);
            ui.label(" and participating in");
            ui.hyperlink_to("[XvB Bonus Hashrate Raffle]", "https://xmrvsbeast.com");
            ui.add_space(space);
            ui.add_sized([ui.available_width(), 0.0], Label::new(KEYBOARD_SHORTCUTS));
            ui.add_space(space);

            if cfg!(debug_assertions) {
                ui.label(format!(
                    "Gupax is running in debug mode - {}",
                    self.now.elapsed().as_secs_f64()
                ));
            }
            ui.label(format!(
                "Gupax has been running for\n{}",
                self.pub_sys.lock().unwrap().gupax_uptime
            ));
        });
        });
    }
}
