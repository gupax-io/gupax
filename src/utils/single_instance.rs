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

//! One GUI Gupax at a time: a second launch shows the window of the
//! running instance instead of starting a duplicate.
//!
//! - Unix: a socket in the runtime directory; connecting to it is the
//!   whole signal, the listener answers with [`TrayCmd::Show`].
//! - Windows: a named mutex; the second instance shows the existing
//!   (possibly hidden) window directly by its title.
//!
//! Daemon mode does not take part in this ([`init`] is not called).

use log::warn;

use crate::tray::TraySender;

/// Returns `false` when another instance is already running (it was asked
/// to show its window and the caller should exit). Never blocks startup:
/// every failure path degrades to running normally.
pub fn init(sender: TraySender, name_version: &str) -> bool {
    #[cfg(unix)]
    {
        let _ = name_version;
        init_unix(sender)
    }
    #[cfg(windows)]
    {
        let _ = sender;
        init_windows(name_version)
    }
}

#[cfg(unix)]
fn init_unix(sender: TraySender) -> bool {
    let path = dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("gupax.sock");
    init_at(&path, sender)
}

#[cfg(unix)]
fn init_at(path: &std::path::Path, sender: TraySender) -> bool {
    use std::os::unix::net::{UnixListener, UnixStream};
    let listener = match UnixListener::bind(path) {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if UnixStream::connect(path).is_ok() {
                // the running instance got the signal by accepting
                return false;
            }
            // a stale socket from a previous crash or kill
            let _ = std::fs::remove_file(path);
            match UnixListener::bind(path) {
                Ok(listener) => listener,
                Err(e) => {
                    warn!("Single instance | disabled: {e}");
                    return true;
                }
            }
        }
        Err(e) => {
            warn!("Single instance | disabled: {e}");
            return true;
        }
    };
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            if connection.is_ok() {
                log::info!("Single instance | another launch asked to show the window");
                sender.send(crate::tray::TrayCmd::Show);
            }
        }
    });
    true
}

#[cfg(windows)]
fn init_windows(name_version: &str) -> bool {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SW_SHOW, SetForegroundWindow, ShowWindow,
    };
    use windows::core::HSTRING;
    unsafe {
        let Ok(mutex) = CreateMutexW(None, false, &HSTRING::from("Local\\GupaxSingleInstance"))
        else {
            return true;
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // find the other instance's (possibly hidden) window by title
            if let Ok(hwnd) = FindWindowW(None, &HSTRING::from(name_version)) {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
            return false;
        }
        // hold the mutex for the whole process life
        std::mem::forget(mutex);
    }
    true
}

#[cfg(all(test, unix))]
mod test {
    use super::init_at;
    use crate::tray::{TrayChannel, TrayCmd};
    use std::time::Duration;

    #[test]
    fn second_instance_signals_show() {
        let path = std::env::temp_dir().join(format!("gupax-si-test-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let first = TrayChannel::new();
        assert!(init_at(&path, first.sender()), "first instance is primary");
        let second = TrayChannel::new();
        assert!(!init_at(&path, second.sender()), "second must defer");
        let cmd = first
            .rx
            .recv_timeout(Duration::from_secs(5))
            .expect("primary must receive the show signal");
        assert!(matches!(cmd, TrayCmd::Show));
        let _ = std::fs::remove_file(&path);
    }
}
