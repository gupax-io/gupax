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
//! - Unix: an advisory lock decides who is primary, and a socket next to
//!   it carries the signal; connecting to it is the whole message, the
//!   listener answers with [`TrayCmd::Show`].
//! - Windows: a named mutex; the second instance shows the existing
//!   (possibly hidden) window directly by its title.
//!
//! Daemon mode does not take part in this ([`init`] is not called).

use std::path::Path;

use log::warn;

use crate::tray::TraySender;

/// Returns `false` when another instance is already running (it was asked
/// to show its window and the caller should exit). Never blocks startup:
/// every failure path degrades to running normally.
///
/// `data_dir` is Gupax's own per-user data directory, used on Unix when
/// the OS offers no runtime directory. `show_window` is false for a
/// [--tray] launch, which asks for a Gupax in the tray and so must not
/// un-hide the one already running.
pub fn init(sender: TraySender, name_version: &str, data_dir: &Path, show_window: bool) -> bool {
    #[cfg(unix)]
    {
        let _ = name_version;
        init_unix(sender, data_dir, show_window)
    }
    #[cfg(windows)]
    {
        let _ = (sender, data_dir);
        init_windows(name_version, show_window)
    }
}

/// Wire protocol between launches: a single byte, so a second `--tray`
/// launch can announce itself without un-hiding the running window.
/// A launch that says nothing at all means the historical "show me".
#[cfg(unix)]
const SHOW_WINDOW: u8 = b'S';
#[cfg(unix)]
const STAY_HIDDEN: u8 = b'T';

/// Give up the guard so a successor process can take it. Only the
/// auto-updater needs this: it spawns the new Gupax *before* exiting, so
/// without it the child can find the guard still held and quit on the
/// spot, leaving no Gupax running at all.
pub fn release() {
    #[cfg(unix)]
    if let Some(guard) = unix_guard().lock().unwrap().take() {
        // dropping the file releases the lock; the socket is only a
        // rendezvous point, and a stale one is handled on the next start
        let _ = std::fs::remove_file(&guard.socket);
    }
    #[cfg(windows)]
    windows_release();
}

#[cfg(unix)]
struct Guard {
    /// Held for its `Drop`, which releases the advisory lock.
    _lock: std::fs::File,
    socket: std::path::PathBuf,
}

#[cfg(unix)]
fn unix_guard() -> &'static std::sync::Mutex<Option<Guard>> {
    static GUARD: std::sync::Mutex<Option<Guard>> = std::sync::Mutex::new(None);
    &GUARD
}

/// Outcome of trying to take the guard.
#[cfg(unix)]
enum Claim {
    /// This process is primary; the guard must be held while it runs.
    Primary(Guard),
    /// Another live instance owns it and has been told what we wanted.
    Deferred,
    /// The guard is unusable; run normally without one.
    Unguarded,
}

#[cfg(unix)]
fn init_unix(sender: TraySender, data_dir: &Path, show_window: bool) -> bool {
    // `XDG_RUNTIME_DIR` is the right home for a socket on Linux: per-user,
    // on tmpfs, cleared at logout. Everywhere else fall back to Gupax's
    // own data directory rather than a shared temp dir -- `runtime_dir()`
    // is `None` on macOS and on Linux without a session, and a socket in
    // a world-writable `/tmp` lets another user's stale `gupax.sock`
    // block the guard, or their live one capture our launch.
    let dir = dirs::runtime_dir().unwrap_or_else(|| data_dir.to_path_buf());
    match claim(&dir.join("gupax.sock"), sender, show_window) {
        // the guard lives as long as the process, so park it where
        // [release] can find it
        Claim::Primary(guard) => {
            *unix_guard().lock().unwrap() = Some(guard);
            true
        }
        Claim::Deferred => false,
        Claim::Unguarded => true,
    }
}

/// Try to become the one Gupax. Returns the guard rather than storing it,
/// so the process-wide slot has exactly one writer ([`init_unix`]).
#[cfg(unix)]
fn claim(socket: &Path, sender: TraySender, show_window: bool) -> Claim {
    use std::fs::{OpenOptions, TryLockError};
    use std::io::{Read as _, Write as _};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::Duration;

    // The lock, not the socket, decides who is primary: taking it is
    // atomic, and the kernel drops it when the owner dies, so a crash can
    // never leave a guard behind. Deciding by `bind` instead meant a
    // stale socket had to be unlinked and re-bound, and two launches
    // racing through that could both end up believing they were primary.
    let lock_path = socket.with_extension("lock");
    let lock = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
    {
        Ok(lock) => lock,
        Err(e) => {
            warn!("Single instance | disabled: {e}");
            return Claim::Unguarded;
        }
    };
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            // Someone alive owns the guard: tell them what this launch
            // wanted. If they are mid-shutdown and nobody answers, take
            // over instead of exiting into nothing.
            match UnixStream::connect(socket) {
                Ok(mut stream) => {
                    let intent = if show_window {
                        SHOW_WINDOW
                    } else {
                        STAY_HIDDEN
                    };
                    let _ = stream.write_all(&[intent]);
                    return Claim::Deferred;
                }
                Err(_) => {
                    warn!("Single instance | the running Gupax did not answer, starting anyway");
                    return Claim::Unguarded;
                }
            }
        }
        Err(TryLockError::Error(e)) => {
            warn!("Single instance | disabled: {e}");
            return Claim::Unguarded;
        }
    }
    // We hold the lock, so any socket still on disk is stale by
    // definition and nobody else can be binding it right now.
    let _ = std::fs::remove_file(socket);
    let listener = match UnixListener::bind(socket) {
        Ok(listener) => listener,
        Err(e) => {
            warn!("Single instance | disabled: {e}");
            return Claim::Unguarded;
        }
    };
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else { continue };
            // Never let a peer that connects and then says nothing stall
            // the loop; a silent one is treated as the historical "show".
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let mut intent = [0u8; 1];
            if matches!(stream.read(&mut intent), Ok(1)) && intent[0] == STAY_HIDDEN {
                log::info!("Single instance | another launch wanted the tray, leaving the window");
                continue;
            }
            log::info!("Single instance | another launch asked to show the window");
            sender.send(crate::tray::TrayCmd::Show);
        }
    });
    Claim::Primary(Guard {
        _lock: lock,
        socket: socket.to_path_buf(),
    })
}

#[cfg(windows)]
static MUTEX_HANDLE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(windows)]
fn init_windows(name_version: &str, show_window: bool) -> bool {
    use std::sync::atomic::Ordering;
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
            // find the other instance's (possibly hidden) window by title.
            // A [--tray] launch asked for a Gupax in the tray, which is
            // already the case, so it must leave the window alone.
            if show_window && let Ok(hwnd) = FindWindowW(None, &HSTRING::from(name_version)) {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
            return false;
        }
        // hold the mutex for the whole process life, unless [release] is
        // called: closing the handle is what hands it to a successor
        MUTEX_HANDLE.store(mutex.0 as isize, Ordering::Relaxed);
    }
    true
}

#[cfg(windows)]
fn windows_release() {
    use std::sync::atomic::Ordering;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    let handle = MUTEX_HANDLE.swap(0, Ordering::Relaxed);
    if handle != 0 {
        unsafe {
            let _ = CloseHandle(HANDLE(handle as *mut core::ffi::c_void));
        }
    }
}

#[cfg(all(test, unix))]
mod test {
    use super::{Claim, claim, release, unix_guard};
    use crate::tray::{TrayChannel, TrayCmd};
    use std::time::Duration;

    /// Each test gets its own socket so they stay independent: [`claim`]
    /// hands back the guard instead of parking it in the process-wide
    /// slot, so nothing here can release another test's lock.
    fn socket(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("gupax-si-{tag}-{}.sock", std::process::id()))
    }

    fn clean(socket: &std::path::Path) {
        let _ = std::fs::remove_file(socket);
        let _ = std::fs::remove_file(socket.with_extension("lock"));
    }

    fn primary(socket: &std::path::Path, chan: &TrayChannel) -> super::Guard {
        match claim(socket, chan.sender(), true) {
            Claim::Primary(guard) => guard,
            Claim::Deferred => panic!("expected to be primary, deferred instead"),
            Claim::Unguarded => panic!("expected to be primary, guard was unusable"),
        }
    }

    #[test]
    fn second_instance_signals_show() {
        let socket = socket("show");
        clean(&socket);
        let first = TrayChannel::new();
        let _guard = primary(&socket, &first);
        let second = TrayChannel::new();
        assert!(
            matches!(claim(&socket, second.sender(), true), Claim::Deferred),
            "second must defer"
        );
        let cmd = first
            .rx
            .recv_timeout(Duration::from_secs(5))
            .expect("primary must receive the show signal");
        assert!(matches!(cmd, TrayCmd::Show));
        clean(&socket);
    }

    /// `gupax --tray` asks for a Gupax in the tray, which is already the
    /// case, so it must defer without un-hiding the running window.
    #[test]
    fn second_instance_with_tray_does_not_show() {
        let socket = socket("tray");
        clean(&socket);
        let first = TrayChannel::new();
        let _guard = primary(&socket, &first);
        let second = TrayChannel::new();
        assert!(
            matches!(claim(&socket, second.sender(), false), Claim::Deferred),
            "[--tray] must still defer"
        );
        assert!(
            first.rx.recv_timeout(Duration::from_secs(1)).is_err(),
            "[--tray] must not ask the running Gupax to show itself"
        );
        clean(&socket);
    }

    /// A socket left behind by a crash must not lock Gupax out, and the
    /// lock file surviving must not either.
    #[test]
    fn stale_socket_is_reclaimed() {
        let socket = socket("stale");
        clean(&socket);
        // no listener ever bound it, exactly what a killed instance leaves
        std::fs::write(&socket, b"").unwrap();
        let app = TrayChannel::new();
        let _guard = primary(&socket, &app);
        clean(&socket);
    }

    /// Dropping the guard is what lets a successor in; it is also what
    /// [`release`] does for the auto-updater.
    #[test]
    fn dropping_the_guard_lets_a_successor_take_over() {
        let socket = socket("successor");
        clean(&socket);
        let old = TrayChannel::new();
        drop(primary(&socket, &old));
        let new = TrayChannel::new();
        let _guard = primary(&socket, &new);
        clean(&socket);
    }

    /// [`release`] is what the auto-updater calls before spawning the new
    /// Gupax. The only test touching the process-wide slot, so it cannot
    /// disturb the others.
    #[test]
    fn release_frees_the_process_wide_guard() {
        let socket = socket("release");
        clean(&socket);
        let app = TrayChannel::new();
        *unix_guard().lock().unwrap() = Some(primary(&socket, &app));
        release();
        assert!(
            unix_guard().lock().unwrap().is_none(),
            "release must empty the slot"
        );
        assert!(!socket.exists(), "release must unlink the socket");
        let successor = TrayChannel::new();
        let _guard = primary(&socket, &successor);
        clean(&socket);
    }
}
