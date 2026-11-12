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
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, sleep},
    time::{Duration, Instant},
};

use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};
use windows::Win32::System::Threading::DETACHED_PROCESS;

use std::io::Write;

type Pipe = DuplexPipeStream<Bytes>;

const LIMIT: Duration = Duration::from_secs(10);

// As after UAC, the helper has no console and reports through the pipes.
fn gupax() -> Command {
    let mut gupax = Command::new(env!("CARGO_BIN_EXE_gupax"));
    gupax
        .creation_flags(DETACHED_PROCESS.0)
        .stderr(Stdio::null());
    gupax
}

// Ends the helper and the XMRig it started, then fails the test.
fn timed_out(helper: &mut Child) -> ! {
    let pid = sysinfo::Pid::from_u32(helper.id());
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for process in system.processes().values() {
        if process.parent() == Some(pid) {
            process.kill();
        }
    }
    let _ = helper.kill();
    panic!("the helper still ran after {LIMIT:?}");
}

// Serves the pipes and starts the helper as the XMRig watchdog does.
fn start_helper(program: &str, args: &[&str]) -> (Child, Option<(Pipe, Pipe)>) {
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
    let mut helper = gupax()
        .args(["elevated-helper", "--name-pipe-stdin", &name("stdin")])
        .args(["--name-pipe-stdout", &name("stdout")])
        .args(["--program-path", program, "--"])
        .args(args)
        .spawn()
        .unwrap();
    // A helper with nothing to write may exit before the watchdog accepts.
    let pipes = accept(&stdin, &mut helper).zip(accept(&stdout, &mut helper));
    (helper, pipes)
}

// The pipe ends with the helper.
fn read_output(helper: &mut Child, mut stdout: Pipe) -> String {
    let (sender, output) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let _ = stdout.read_to_end(&mut output);
        let _ = sender.send(String::from_utf8_lossy(&output).into_owned());
    });
    output
        .recv_timeout(LIMIT)
        .unwrap_or_else(|_| timed_out(helper))
}

fn wait(helper: &mut Child) -> Option<i32> {
    let waiting = Instant::now();
    loop {
        if let Some(status) = helper.try_wait().unwrap() {
            return status.code();
        }
        if waiting.elapsed() > LIMIT {
            timed_out(helper);
        }
        sleep(Duration::from_millis(10));
    }
}

// Returns the output pipe content and the exit code of the helper.
fn run_helper(program: &str, args: &[&str]) -> (String, Option<i32>) {
    let (mut helper, pipes) = start_helper(program, args);
    let output = pipes
        .map(|(_stdin, stdout)| read_output(&mut helper, stdout))
        .unwrap_or_default();
    (output, wait(&mut helper))
}

fn accept(listener: &PipeListener<Bytes, Bytes>, helper: &mut Child) -> Option<Pipe> {
    let accepting = Instant::now();
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
                if accepting.elapsed() > LIMIT {
                    timed_out(helper);
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

#[test]
fn helper_shows_why_xmrig_did_not_start() {
    let (output, code) = run_helper(r"C:\missing\xmrig.exe", &[]);
    assert!(
        output.starts_with(r"Error: C:\missing\xmrig.exe: "),
        "{output:?}"
    );
    assert_eq!(code, Some(1));
}

#[test]
fn helper_relays_the_last_lines_of_xmrig() {
    let (output, code) = run_helper(r"C:\windows\system32\cmd.exe", &["/C", "echo last line"]);
    assert!(output.contains("last line"), "{output:?}");
    assert_eq!(code, Some(0));
}

// The PING the helper started as XMRig.
fn ping_of(helper: &Child) -> sysinfo::Pid {
    let helper = sysinfo::Pid::from_u32(helper.id());
    let mut system = sysinfo::System::new();
    let started = Instant::now();
    loop {
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        if let Some(ping) = system.processes().values().find(|process| {
            process.parent() == Some(helper) && process.name().eq_ignore_ascii_case("PING.EXE")
        }) {
            return ping.pid();
        }
        assert!(started.elapsed() < LIMIT, "XMRig did not start");
        sleep(Duration::from_millis(10));
    }
}

fn running(pid: sysinfo::Pid) -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}

// Returns once PING wrote its first line. A Ctrl+C sent while PING starts can be lost.
fn ping_wrote(helper: &mut Child, mut stdout: Pipe) -> Pipe {
    let (sender, read) = mpsc::channel();
    thread::spawn(move || {
        let mut output = String::new();
        let mut buf = [0; 1024];
        while !output.contains("127.0.0.1")
            && let Ok(n @ 1..) = stdout.read(&mut buf)
        {
            output.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
        let _ = sender.send((stdout, output));
    });
    let (stdout, output) = read
        .recv_timeout(LIMIT)
        .unwrap_or_else(|_| timed_out(helper));
    assert!(output.contains("127.0.0.1"), "{output:?}");
    stdout
}

#[test]
fn helper_stops_xmrig_once_gupax_is_gone() {
    let (mut helper, pipes) =
        start_helper(r"C:\windows\system32\PING.EXE", &["-n", "30", "127.0.0.1"]);
    let (stdin, stdout) = pipes.unwrap();
    let ping = ping_of(&helper);
    let stdout = ping_wrote(&mut helper, stdout);
    drop((stdin, stdout));
    wait(&mut helper);
    assert!(!running(ping), "XMRig kept running");
}

#[test]
fn helper_stops_xmrig_on_ctrl_c() {
    let (mut helper, pipes) =
        start_helper(r"C:\windows\system32\PING.EXE", &["-n", "30", "127.0.0.1"]);
    let (mut stdin, stdout) = pipes.unwrap();
    let ping = ping_of(&helper);
    let stdout = ping_wrote(&mut helper, stdout);
    // Gupax stops XMRig with it.
    stdin.write_all(&[0x03]).unwrap();
    read_output(&mut helper, stdout);
    wait(&mut helper);
    assert!(!running(ping), "XMRig kept running");
}

#[test]
fn helper_passes_input_to_xmrig() {
    let (mut helper, pipes) = start_helper(
        r"C:\windows\system32\cmd.exe",
        &["/V:ON", "/C", "set /p x=&echo got !x!"],
    );
    let (mut stdin, stdout) = pipes.unwrap();
    // As Gupax writes a command typed in the XMRig console.
    stdin.write_all(b"h\r\n").unwrap();
    let output = read_output(&mut helper, stdout);
    assert!(output.contains("got h"), "{output:?}");
}

#[test]
fn helper_runs_xmrig_below_normal_priority() {
    let (helper, _pipes) =
        start_helper(r"C:\windows\system32\PING.EXE", &["-n", "30", "127.0.0.1"]);
    let ping = ping_of(&helper);
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
    let mut helper = gupax()
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
        .spawn()
        .unwrap();
    assert_eq!(wait(&mut helper), Some(1));
    assert!(!started.exists());
}
