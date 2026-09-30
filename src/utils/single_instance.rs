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
//! running instance and exits.
//!
//! An advisory lock on `gupax.lock` decides who is primary, and is released
//! when its owner exits.
//!
//! How the second launch reaches the first differs:
//! - Unix: a socket next to the lock. Connecting is the whole message,
//!   and one byte says whether the window is wanted.
//! - Windows: a `gupax.show` sentinel file the primary watches, while a
//!   `gupax.sock` file says it is watching. A Gupax hidden to the tray has
//!   no window to send a message to.
//!
//! A quitting Gupax stops answering and keeps the lock until it exits, so a
//! launch meanwhile waits for it and becomes primary after it exits.
//!
//! Daemon mode does not call [`init`].

use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use log::warn;

use crate::disk::consts::DIRECTORY;
use crate::tray::TraySender;

/// Returns `false` when another instance is running: it was asked to show
/// its window, and the caller should exit. Blocks only while a running Gupax
/// is quitting; on every failure Gupax runs normally.
///
/// `show_window` is false for a [--tray] launch, which asks for a Gupax in
/// the tray and so must leave the running window hidden.
pub fn init(sender: TraySender, show_window: bool) -> bool {
    // Per-user: `XDG_RUNTIME_DIR` on Linux, else Gupax's cache directory.
    // The data directory must not exist before [crate::app::App::new],
    // whose first-run setup depends on it.
    let Some(dir) =
        dirs::runtime_dir().or_else(|| dirs::cache_dir().map(|dir| dir.join(DIRECTORY)))
    else {
        warn!("Single instance | no runtime or cache directory, disabled");
        return true;
    };
    match claim(&dir.join("gupax.sock"), sender, show_window) {
        // The guard lives as long as the process: store it for [stop_answering].
        Claim::Primary(guard) => {
            *guard_slot().lock().unwrap() = Some(guard);
            true
        }
        Claim::Deferred => false,
        Claim::Unguarded => true,
    }
}

/// Wire protocol between launches: a single byte, so a second `--tray`
/// launch can announce itself and leave the running window hidden.
/// A launch that sends nothing asks for the window.
#[cfg(unix)]
const SHOW_WINDOW: u8 = b'S';
#[cfg(unix)]
const STAY_HIDDEN: u8 = b'T';

/// How a launch waits for a primary that holds the lock but does not answer
/// yet, before it counts it as quitting.
const ANSWER_TRIES: u32 = 5;
const ANSWER_RETRY: std::time::Duration = std::time::Duration::from_millis(100);

/// How often the Windows primary looks for a launch that wants the window.
/// A poll keeps this in safe Rust, for one file check per tick.
#[cfg(windows)]
const SENTINEL_POLL: std::time::Duration = std::time::Duration::from_millis(300);

/// Stop answering second launches, and keep the lock until Gupax exits.
/// Called when a shutdown starts, and before the auto-updater starts the new
/// Gupax.
pub fn stop_answering() {
    if let Some(guard) = guard_slot().lock().unwrap().as_ref() {
        guard.close();
    }
}

struct Guard {
    _lock: std::fs::File,
    /// The socket (Unix), or the file saying the sentinel is watched
    /// (Windows).
    rendezvous: PathBuf,
    /// Stops the Windows watcher with the guard, as a new primary watches the
    /// same sentinel.
    #[cfg(windows)]
    stop: Arc<AtomicBool>,
}

impl Guard {
    fn close(&self) {
        let _ = std::fs::remove_file(&self.rendezvous);
        #[cfg(windows)]
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(windows)]
impl Drop for Guard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn guard_slot() -> &'static std::sync::Mutex<Option<Guard>> {
    static GUARD: std::sync::Mutex<Option<Guard>> = std::sync::Mutex::new(None);
    &GUARD
}

/// Outcome of trying to take the guard.
enum Claim {
    /// This process is primary; the guard must be held while it runs.
    Primary(Guard),
    /// Another running instance owns it and was told what this launch asks for.
    Deferred,
    /// The guard is unusable; run unguarded.
    Unguarded,
}

/// Try to become the primary instance. Returns the guard for [`init`] to
/// store, so the process-wide slot has exactly one writer.
fn claim(socket: &Path, sender: TraySender, show_window: bool) -> Claim {
    use std::fs::{OpenOptions, TryLockError};

    // Gupax's cache directory does not exist on a first run.
    if let Some(dir) = socket.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
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
            // A primary that is starting answers within [ANSWER_TRIES] tries; one
            // that does not answer is quitting, and this launch becomes primary
            // after it exits.
            for _ in 0..ANSWER_TRIES {
                if let Some(claim) = signal_primary(socket, show_window) {
                    return claim;
                }
                std::thread::sleep(ANSWER_RETRY);
            }
            log::info!("Single instance | the running Gupax is quitting, waiting for it to exit");
            if let Err(e) = lock.lock() {
                warn!("Single instance | disabled: {e}");
                return Claim::Unguarded;
            }
        }
        Err(TryLockError::Error(e)) => {
            warn!("Single instance | disabled: {e}");
            return Claim::Unguarded;
        }
    }
    listen(socket, lock, sender)
}

/// Another running Gupax holds the guard: send it what this launch asks for.
/// `None` when it does not answer.
#[cfg(unix)]
fn signal_primary(socket: &Path, show_window: bool) -> Option<Claim> {
    use std::io::Write as _;
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(socket).ok()?;
    let intent = if show_window {
        SHOW_WINDOW
    } else {
        STAY_HIDDEN
    };
    let _ = stream.write_all(&[intent]);
    Some(Claim::Deferred)
}

/// Windows has no socket to connect to, so the request is written to disk
/// for the watcher of the primary.
#[cfg(windows)]
fn signal_primary(socket: &Path, show_window: bool) -> Option<Claim> {
    if !socket.exists() {
        return None;
    }
    // A [--tray] launch asked for a Gupax in the tray, which is already the
    // case, so it leaves the running window hidden.
    if !show_window {
        return Some(Claim::Deferred);
    }
    match std::fs::write(sentinel(socket), b"") {
        Ok(()) => Some(Claim::Deferred),
        Err(e) => {
            warn!("Single instance | could not reach the running Gupax, starting anyway: {e}");
            Some(Claim::Unguarded)
        }
    }
}

/// The file a second Windows launch creates to ask for the window.
#[cfg(windows)]
fn sentinel(socket: &Path) -> PathBuf {
    socket.with_extension("show")
}
/// Start answering second launches. This process holds the lock, so a socket
/// file left here belongs to an exited process, and no other process uses it.
#[cfg(unix)]
fn listen(socket: &Path, lock: std::fs::File, sender: TraySender) -> Claim {
    use std::io::Read as _;
    use std::os::unix::net::UnixListener;
    use std::time::Duration;

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
        rendezvous: socket.to_path_buf(),
    })
}

/// On Windows, watch for the sentinel file a second launch creates.
#[cfg(windows)]
fn listen(socket: &Path, lock: std::fs::File, sender: TraySender) -> Claim {
    let sentinel = sentinel(socket);
    // A sentinel left by a crashed process: the lock accounts for every running one.
    let _ = std::fs::remove_file(&sentinel);
    // Present while this Gupax answers, see [`signal_primary`].
    if let Err(e) = std::fs::write(socket, b"") {
        warn!("Single instance | disabled: {e}");
        return Claim::Unguarded;
    }
    let watched = sentinel.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(SENTINEL_POLL);
            if stopped.load(Ordering::Relaxed) {
                break;
            }
            // Remove it first: if the removal failed, the same request would
            // repeat on every tick.
            if std::fs::remove_file(&watched).is_ok() {
                log::info!("Single instance | another launch asked to show the window");
                sender.send(crate::tray::TrayCmd::Show);
            }
        }
    });
    Claim::Primary(Guard {
        _lock: lock,
        rendezvous: socket.to_path_buf(),
        stop,
    })
}

#[cfg(test)]
mod test {
    use super::{Claim, claim, guard_slot, signal_primary, stop_answering};
    use crate::tray::{TrayChannel, TrayCmd};
    use std::time::Duration;

    fn socket(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("gupax-si-{tag}-{}.sock", std::process::id()))
    }

    fn clean(socket: &std::path::Path) {
        let _ = std::fs::remove_file(socket);
        let _ = std::fs::remove_file(socket.with_extension("lock"));
        let _ = std::fs::remove_file(socket.with_extension("show"));
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

    #[test]
    fn stale_socket_is_reclaimed() {
        let socket = socket("stale");
        clean(&socket);
        // A socket file with no listener, as a killed instance leaves it.
        std::fs::write(&socket, b"").unwrap();
        let app = TrayChannel::new();
        let _guard = primary(&socket, &app);
        clean(&socket);
    }

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

    /// The only test using the process-wide slot, so it can not affect the
    /// others.
    #[test]
    fn stop_answering_keeps_the_lock() {
        let socket = socket("release");
        clean(&socket);
        let app = TrayChannel::new();
        *guard_slot().lock().unwrap() = Some(primary(&socket, &app));
        stop_answering();
        assert!(
            signal_primary(&socket, true).is_none(),
            "a launch must get no answer"
        );
        let lock = std::fs::File::options()
            .write(true)
            .open(socket.with_extension("lock"))
            .unwrap();
        assert!(
            matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
            "the lock must stay held until exit"
        );
        *guard_slot().lock().unwrap() = None;
        clean(&socket);
    }

    #[test]
    fn launch_during_shutdown_waits_then_takes_over() {
        let socket = socket("handover");
        clean(&socket);
        let old = TrayChannel::new();
        let guard = primary(&socket, &old);
        guard.close();
        let (claimed, rx) = std::sync::mpsc::channel();
        let path = socket.clone();
        std::thread::spawn(move || {
            let new = TrayChannel::new();
            let primary = matches!(claim(&path, new.sender(), true), Claim::Primary(_));
            claimed.send(primary).unwrap();
        });
        assert!(
            rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "must wait while the old Gupax runs"
        );
        drop(guard);
        assert!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "must take over once the old Gupax exited"
        );
        clean(&socket);
    }
}
