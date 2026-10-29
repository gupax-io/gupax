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
    time::Duration,
};

use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};

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
