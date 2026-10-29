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

// Starts Gupax in elevated helper mode directly, as PowerShell does after UAC.
#![cfg(target_os = "windows")]

use std::{
    io::{ErrorKind, Read},
    path::PathBuf,
    process::{Child, Command},
    sync::atomic::{AtomicUsize, Ordering},
    thread::sleep,
    time::{Duration, Instant},
};

use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};

use std::io::Write;

type Pipe = DuplexPipeStream<Bytes>;

// Serves the pipes as the XMRig watchdog does, then starts Gupax with `cli` given
// their names.
fn start_gupax(cli: impl FnOnce(&str, &str) -> Vec<String>) -> (Child, Option<(Pipe, Pipe)>) {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let name = |end: &str| format!("gupax_test_{}_{run}_{end}", std::process::id());
    let listener = |name: &str| {
        PipeListenerOptions::new()
            .path(PathBuf::from(format!(r"\\.\pipe\{name}")))
            .mode(PipeMode::Bytes)
            .nonblocking(true)
            .create_duplex::<Bytes>()
            .unwrap()
    };
    let (stdin, stdout) = (listener(&name("stdin")), listener(&name("stdout")));
    let mut helper = Command::new(env!("CARGO_BIN_EXE_gupax"))
        .args(cli(&name("stdin"), &name("stdout")))
        // As after UAC, the helper has no console and reports through the pipes.
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    // The watchdog polls for the connection.
    sleep(Duration::from_millis(200));
    // A helper with nothing to write may exit before the watchdog accepts.
    let pipes = match (accept(&stdin, &mut helper), accept(&stdout, &mut helper)) {
        (Some(stdin), Some(stdout)) => Some((stdin, stdout)),
        _ => None,
    };
    (helper, pipes)
}

// Starts the helper as the XMRig watchdog does.
fn start_helper(program: &str, args: &[&str]) -> (Child, Option<(Pipe, Pipe)>) {
    start_gupax(|stdin, stdout| {
        [
            "elevated-helper",
            "--name-pipe-stdin",
            stdin,
            "--name-pipe-stdout",
            stdout,
            "--program-path",
            program,
            "--",
        ]
        .iter()
        .chain(args)
        .map(|arg| arg.to_string())
        .collect()
    })
}

// The pipe ends with the helper.
fn read_output(mut stdout: Pipe) -> String {
    let mut output = Vec::new();
    let mut buf = [0; 4096];
    while let Ok(n @ 1..) = stdout.read(&mut buf) {
        output.extend_from_slice(&buf[..n]);
    }
    String::from_utf8_lossy(&output).into_owned()
}

// Returns the output pipe content and the exit code of the helper.
fn run_to_exit((mut helper, pipes): (Child, Option<(Pipe, Pipe)>)) -> (String, Option<i32>) {
    let output = pipes
        .map(|(_stdin, stdout)| read_output(stdout))
        .unwrap_or_default();
    (output, helper.wait().unwrap().code())
}

fn run_helper(program: &str, args: &[&str]) -> (String, Option<i32>) {
    run_to_exit(start_helper(program, args))
}

fn accept(listener: &PipeListener<Bytes, Bytes>, helper: &mut Child) -> Option<Pipe> {
    loop {
        match listener.accept() {
            Ok(pipe) => {
                pipe.set_nonblocking(false).unwrap();
                return Some(pipe);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if helper.try_wait().unwrap().is_some() {
                    return None;
                }
                sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn helper_exits_with_the_code_of_xmrig() {
    let (_, code) = run_helper(r"C:\windows\system32\cmd.exe", &["/C", "exit 3"]);
    assert_eq!(code, Some(3));
}

// Sent by a Gupax v2.0.2 or older still running after a self-update.
#[test]
fn helper_flags_of_older_gupax_still_run_xmrig() {
    let (_, code) = run_to_exit(start_gupax(|stdin, stdout| {
        vec![
            "--elevated-helper".to_string(),
            format!("--name-stdin-pipe={stdin}"),
            format!("--name-stdout-pipe={stdout}"),
            r"--binary-path=C:\windows\system32\cmd.exe".to_string(),
            "--arguments=/C,exit 3".to_string(),
        ]
    }));
    assert_eq!(code, Some(3));
}

#[test]
fn helper_shows_why_xmrig_did_not_start() {
    let (output, code) = run_helper(r"C:\missing\xmrig.exe", &[]);
    assert!(
        output.starts_with(r"Error: C:\missing\xmrig.exe: "),
        "{output:?}"
    );
    assert_eq!(code, Some(1));
}

// Needs a Windows pseudo console: wine relays none of its output.
#[test]
fn helper_relays_the_last_lines_of_xmrig() {
    let (output, code) = run_helper(r"C:\windows\system32\cmd.exe", &["/C", "echo last line"]);
    assert!(output.contains("last line"), "{output:?}");
    assert_eq!(code, Some(0));
}

// Needs a Windows pseudo console: wine relays no Ctrl+C through it.
#[test]
fn helper_stops_xmrig_once_gupax_is_gone() {
    let (mut helper, pipes) =
        start_helper(r"C:\windows\system32\PING.EXE", &["-n", "30", "127.0.0.1"]);
    assert!(pipes.is_some());
    drop(pipes);
    let gone = Instant::now();
    while helper.try_wait().unwrap().is_none() {
        if gone.elapsed() > Duration::from_secs(10) {
            helper.kill().unwrap();
            panic!("XMRig kept running");
        }
        sleep(Duration::from_millis(100));
    }
}

// Reads the output until the helper exits, killing it after `limit`.
fn read_output_within(mut helper: Child, stdout: Pipe, limit: Duration) -> String {
    let (sender, output) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(read_output(stdout));
    });
    output.recv_timeout(limit).unwrap_or_else(|_| {
        helper.kill().unwrap();
        panic!("the helper still ran after {limit:?}");
    })
}

// Needs a Windows pseudo console: wine relays no Ctrl+C through it.
#[test]
fn helper_stops_xmrig_on_ctrl_c() {
    let (helper, pipes) = start_helper(r"C:\windows\system32\PING.EXE", &["-n", "30", "127.0.0.1"]);
    let (mut stdin, stdout) = pipes.unwrap();
    // Gupax stops XMRig with it.
    stdin.write_all(&[0x03]).unwrap();
    let output = read_output_within(helper, stdout, Duration::from_secs(10));
    assert!(!output.starts_with("Error:"), "{output:?}");
}

// Needs a Windows pseudo console: wine relays none of its output.
#[test]
fn helper_passes_input_to_xmrig() {
    let (helper, pipes) = start_helper(
        r"C:\windows\system32\cmd.exe",
        &["/V:ON", "/C", "set /p x=&echo got !x!"],
    );
    let (mut stdin, stdout) = pipes.unwrap();
    // As Gupax writes a command typed in the XMRig console.
    stdin.write_all(b"h\r\n").unwrap();
    let output = read_output_within(helper, stdout, Duration::from_secs(10));
    assert!(output.contains("got h"), "{output:?}");
}

// Needs PowerShell, which wine lacks.
#[test]
fn helper_runs_xmrig_below_normal_priority() {
    let (helper, _pipes) =
        start_helper(r"C:\windows\system32\PING.EXE", &["-n", "30", "127.0.0.1"]);
    let helper = sysinfo::Pid::from_u32(helper.id());
    let mut system = sysinfo::System::new();
    let started = Instant::now();
    let ping = loop {
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        if let Some(ping) = system.processes().values().find(|process| {
            process.parent() == Some(helper) && process.name().eq_ignore_ascii_case("PING.EXE")
        }) {
            break ping.pid();
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "XMRig did not start"
        );
        sleep(Duration::from_millis(10));
    };
    let priority = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!("(Get-Process -Id {ping}).PriorityClass"))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&priority.stdout).trim(),
        "BelowNormal"
    );
}

// As a helper started from a UAC prompt left open after Gupax quit.
#[test]
fn helper_without_its_pipes_starts_no_xmrig() {
    let dir = tempfile::tempdir().unwrap();
    let started = dir.path().join("started");
    let pipe = format!("gupax_test_gone_{}", std::process::id());
    let status = Command::new(env!("CARGO_BIN_EXE_gupax"))
        .args(["elevated-helper", "--name-pipe-stdin", &pipe])
        .args(["--name-pipe-stdout", &pipe, "--program-path"])
        .args([
            r"C:\windows\system32\cmd.exe",
            "--",
            "/C",
            "type",
            "nul",
            ">",
        ])
        .arg(&started)
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(1));
    assert!(!started.exists());
}
