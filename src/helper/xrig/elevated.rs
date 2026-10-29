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

use super::xmrig::PubXmrigApi;
use crate::helper::{Process, ProcessName, ProcessSignal, ProcessState};
use crate::miscs::output_console;
use crate::utils::macros::sleep;
#[cfg(target_os = "windows")]
use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::Mutex;

// Shows the UAC prompt, then runs the helper and waits for it. PowerShell reads the
// values from the environment as plain text, and ProcessStartInfo keeps the
// wildcard characters of the path.
// PowerShell then exits with the code of the helper, which is the code of XMRig, or
// with 1 when the start failed.
// The trap writes the reason of a failure in UTF-8.
const SCRIPT: &str = "$ErrorActionPreference='Stop'; \
    trap { $e=[IO.StreamWriter]::new([Console]::OpenStandardError()); \
    $e.WriteLine($_.Exception.GetBaseException().Message); $e.Flush(); exit 1 }; \
    $p=[Diagnostics.Process]::Start([Diagnostics.ProcessStartInfo]@{\
    FileName=$env:GUPAX_HELPER_EXE; Arguments=$env:GUPAX_HELPER_ARGS; \
    Verb='RunAs'; UseShellExecute=$true}); \
    $p.WaitForExit(); exit $p.ExitCode";

// The elevated Gupax parses them with [elevated_helper::cli::Args].
pub(super) fn helper_args(
    xmrig: &Path,
    args: &[String],
    stdin_name: &str,
    stdout_name: &str,
) -> Vec<String> {
    [
        "elevated-helper",
        "--name-pipe-stdin",
        stdin_name,
        "--name-pipe-stdout",
        stdout_name,
        "--program-path",
        &xmrig.display().to_string(),
        "--",
    ]
    .map(String::from)
    .into_iter()
    .chain(args.iter().cloned())
    .collect()
}

pub(super) fn command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| shell_escape::windows::escape(arg.into()))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn powershell(gupax_exe: &Path, command_line: &str) -> Command {
    let mut cmd = Command::new("powershell");
    // The user's profile could prompt in the hidden window or slow every start.
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT]);
    cmd.env("GUPAX_HELPER_EXE", gupax_exe);
    cmd.env("GUPAX_HELPER_ARGS", command_line);
    cmd
}

// PowerShell exits once the elevated helper exits or could not be started.
pub(super) fn wait_connection<S>(
    mut accept: impl FnMut() -> io::Result<S>,
    mut powershell_exited: impl FnMut() -> io::Result<Option<ExitStatus>>,
    stopped: impl Fn() -> bool,
) -> io::Result<Option<S>> {
    loop {
        match accept() {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if stopped() {
                    return Ok(None);
                }
                if let Some(status) = powershell_exited()? {
                    return Err(io::Error::other(format!(
                        "PowerShell exited ({status}) before the elevated helper connected"
                    )));
                }
                sleep!(100);
            }
            accepted => return accepted.map(Some),
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) type Pipe = DuplexPipeStream<Bytes>;

#[cfg(target_os = "windows")]
fn listener(name: &str) -> io::Result<PipeListener<Bytes, Bytes>> {
    let path = format!(r"\\.\pipe\{name}");
    PipeListenerOptions::new()
        .path(Path::new(&path))
        .mode(PipeMode::Bytes)
        .nonblocking(true)
        .create_duplex::<Bytes>()
}

#[cfg(target_os = "windows")]
pub(super) fn connect(
    stdin: &PipeListener<Bytes, Bytes>,
    stdout: &PipeListener<Bytes, Bytes>,
    mut powershell_exited: impl FnMut() -> io::Result<Option<ExitStatus>>,
    stopped: impl Fn() -> bool,
) -> io::Result<Option<(Pipe, Pipe)>> {
    let mut accept = |listener: &PipeListener<Bytes, Bytes>| {
        wait_connection(
            || {
                let pipe = listener.accept()?;
                // Streams of a nonblocking listener are nonblocking too.
                pipe.set_nonblocking(false)?;
                Ok(pipe)
            },
            &mut powershell_exited,
            &stopped,
        )
    };
    let Some(stdin_pipe) = accept(stdin)? else {
        return Ok(None);
    };
    Ok(accept(stdout)?.map(|stdout_pipe| (stdin_pipe, stdout_pipe)))
}

// XMRig is started by an elevated Gupax, which relays its input and output.
// The child is PowerShell, which exits with the code of XMRig. None on a stop.
#[cfg(target_os = "windows")]
pub(super) fn start(
    xmrig: &Path,
    args: &[String],
    powershell: impl FnOnce(&Path, &str) -> Command,
    stopped: impl Fn() -> bool,
) -> io::Result<Option<(std::process::Child, Pipe, Pipe)>> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;
    let id = uuid::Uuid::new_v4();
    let (stdin_name, stdout_name) = (format!("xmrig_stdin_{id}"), format!("xmrig_stdout_{id}"));
    let (stdin, stdout) = (listener(&stdin_name)?, listener(&stdout_name)?);
    let mut cmd = powershell(
        &std::env::current_exe()?,
        &command_line(&helper_args(xmrig, args, &stdin_name, &stdout_name)),
    );
    cmd.creation_flags(CREATE_NO_WINDOW.0);
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| io::Error::new(e.kind(), format!("PowerShell could not start: {e}")))?;
    let reason = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            let mut reason = Vec::new();
            let _ = stderr.read_to_end(&mut reason);
            String::from_utf8_lossy(&reason).trim().to_string()
        })
    });
    let connected = connect(&stdin, &stdout, || child.try_wait(), stopped);
    if !matches!(connected, Ok(Some(_))) {
        // A helper started later from the UAC prompt exits, as its pipes are gone.
        if let Err(e) = child.kill() {
            log::warn!("XMRig | Could not kill PowerShell: {e}");
        }
    }
    // The stderr of PowerShell ends with it.
    let connected = connected.map_err(|e| {
        let reason = reason
            .and_then(|reason| reason.join().ok())
            .unwrap_or_default();
        if reason.is_empty() {
            e
        } else {
            io::Error::new(e.kind(), format!("{e}: {reason}"))
        }
    });
    Ok(connected?.map(|(stdin, stdout)| (child, stdin, stdout)))
}

pub(super) fn not_started(
    process: &Mutex<Process>,
    gui_api: &Mutex<PubXmrigApi>,
    error: Option<io::Error>,
) {
    let mut lock = process.lock().unwrap();
    lock.signal = ProcessSignal::None;
    let Some(e) = error else {
        log::info!("XMRig Watchdog | Stopped before the elevated helper connected");
        lock.state = ProcessState::Dead;
        return;
    };
    log::error!("XMRig Watchdog | Could not start the elevated helper: {e}");
    lock.state = ProcessState::Failed;
    drop(lock);
    output_console(
        &mut gui_api.lock().unwrap().output,
        &format!("XMRig could not start.\nError: {e}"),
        ProcessName::Xmrig,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn command_line_keeps_arguments_whole() {
        let command_line = |xmrig: &str, args: &[&str]| {
            command_line(&helper_args(
                Path::new(xmrig),
                &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
                "stdin",
                "stdout",
            ))
        };
        assert_eq!(
            command_line(
                r"C:\Users\Jo O'Neil [x]\XMRig\xmrig.exe",
                &["--rig-id=Jo's rig", "--threads", "6"]
            ),
            r#"elevated-helper --name-pipe-stdin stdin --name-pipe-stdout stdout --program-path "C:\Users\Jo O'Neil [x]\XMRig\xmrig.exe" -- "--rig-id=Jo's rig" --threads 6"#
        );
        assert_eq!(
            command_line(
                r"C:\XMRig\xmrig.exe",
                &[r#"--rig-id="a b""#, r"C:\my logs\"]
            ),
            r#"elevated-helper --name-pipe-stdin stdin --name-pipe-stdout stdout --program-path C:\XMRig\xmrig.exe -- "--rig-id=\"a b\"" "C:\my logs\\""#
        );
    }

    #[test]
    fn powershell_takes_the_values_from_the_environment() {
        let cmd = powershell(Path::new(r"C:\Gupax [x]\Gupax.exe"), "--elevated-helper");
        assert_eq!(cmd.get_program(), "powershell");
        assert_eq!(
            cmd.get_args().collect::<Vec<_>>(),
            ["-NoProfile", "-NonInteractive", "-Command", SCRIPT]
        );
        assert_eq!(
            cmd.get_envs().collect::<Vec<_>>(),
            [
                (
                    "GUPAX_HELPER_ARGS".as_ref(),
                    Some("--elevated-helper".as_ref())
                ),
                (
                    "GUPAX_HELPER_EXE".as_ref(),
                    Some(r"C:\Gupax [x]\Gupax.exe".as_ref())
                )
            ]
        );
    }

    // RunAs prompts unless Gupax is already elevated, as on the CI runners.
    #[cfg(target_os = "windows")]
    #[test]
    fn powershell_exits_with_the_helper_code() {
        if !is_elevated::is_elevated() {
            return;
        }
        let status = powershell(Path::new(r"C:\windows\system32\cmd.exe"), "/C exit 3")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(3));
    }

    // A missing file fails before any UAC prompt.
    #[cfg(target_os = "windows")]
    #[test]
    fn powershell_writes_why_the_helper_did_not_start() {
        let output = powershell(Path::new(r"C:\missing\Gupax.exe"), "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let reason = String::from_utf8(output.stderr).unwrap();
        let reason = reason.trim();
        assert!(!reason.is_empty());
        // .NET drops the final period of the system message.
        assert!(
            io::Error::from_raw_os_error(2)
                .to_string()
                .starts_with(reason),
            "{reason}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn helper_reads_the_xmrig_arguments() {
        use clap::Parser;
        let parse = |xmrig_args: &[String]| {
            elevated_helper::cli::Args::try_parse_from(helper_args(
                Path::new(r"C:\XMRig\xmrig.exe"),
                xmrig_args,
                "stdin",
                "stdout",
            ))
            .unwrap()
        };
        let xmrig_args =
            ["--cuda-devices=0,1", "--rig-id=a,b", "--daemon", "state"].map(String::from);
        let args = parse(&xmrig_args);
        assert_eq!(args.name_pipe_stdin, "stdin");
        assert_eq!(args.name_pipe_stdout, "stdout");
        assert_eq!(args.program_path, Path::new(r"C:\XMRig\xmrig.exe"));
        assert_eq!(args.program_args, xmrig_args);
        assert!(parse(&[]).program_args.is_empty());
    }

    #[test]
    fn wait_ends_with_powershell() {
        use std::io::{Error, ErrorKind};
        use std::process::ExitStatus;
        let not_connected = || Error::from(ErrorKind::WouldBlock);
        assert_eq!(
            wait_connection(
                || Err::<(), _>(not_connected()),
                || Ok(Some(ExitStatus::default())),
                || false
            )
            .unwrap_err()
            .to_string(),
            format!(
                "PowerShell exited ({}) before the elevated helper connected",
                ExitStatus::default()
            )
        );
        let mut tries = 0;
        let connected_on_second_try = || {
            tries += 1;
            if tries == 1 {
                Err(not_connected())
            } else {
                Ok(tries)
            }
        };
        assert_eq!(
            wait_connection(connected_on_second_try, || Ok(None), || false).unwrap(),
            Some(2)
        );
        assert_eq!(
            wait_connection(
                || Err::<(), _>(Error::other("pipe broken")),
                || Ok(None),
                || false
            )
            .unwrap_err()
            .to_string(),
            "pipe broken"
        );
    }

    #[test]
    fn wait_ends_on_stop() {
        use std::io::{Error, ErrorKind};
        assert_eq!(
            wait_connection(
                || Err::<(), _>(Error::from(ErrorKind::WouldBlock)),
                || Ok(None),
                || true
            )
            .unwrap(),
            None
        );
    }

    fn xmrig() -> (Mutex<Process>, Mutex<PubXmrigApi>) {
        let mut process = Process::new(ProcessName::Xmrig, String::new(), PathBuf::new());
        process.state = ProcessState::Middle;
        process.signal = ProcessSignal::Stop;
        (Mutex::new(process), Mutex::new(PubXmrigApi::new()))
    }

    #[test]
    fn a_stopped_start_leaves_xmrig_dead() {
        let (process, gui_api) = xmrig();
        not_started(&process, &gui_api, None);
        assert_eq!(process.lock().unwrap().state, ProcessState::Dead);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
        assert!(gui_api.lock().unwrap().output.is_empty());
    }

    #[test]
    fn a_failed_start_shows_why() {
        let (process, gui_api) = xmrig();
        not_started(&process, &gui_api, Some(io::Error::other("denied")));
        assert_eq!(process.lock().unwrap().state, ProcessState::Failed);
        assert_eq!(process.lock().unwrap().signal, ProcessSignal::None);
        let output = &gui_api.lock().unwrap().output;
        assert!(
            output.ends_with("XMRig could not start.\nError: denied\n"),
            "{output}"
        );
    }

    #[cfg(target_os = "windows")]
    mod pipes {
        use super::super::{connect, listener};
        use elevated_helper::interprocess::os::windows::named_pipe::{
            DuplexPipeStream, PipeListener, pipe_mode::Bytes,
        };
        use std::io::{Read, Write};
        use std::process::ExitStatus;
        use std::sync::mpsc;
        use std::time::Duration;

        fn listeners(test: &str) -> (PipeListener<Bytes, Bytes>, PipeListener<Bytes, Bytes>) {
            let listener = |end: &str| listener(&name(test, end)).unwrap();
            (listener("stdin"), listener("stdout"))
        }

        fn name(test: &str, end: &str) -> String {
            format!("gupax_test_{test}_{}_{end}", std::process::id())
        }

        fn helper(test: &str, end: &str) -> DuplexPipeStream<Bytes> {
            DuplexPipeStream::<Bytes>::connect_by_path(format!(r"\\.\pipe\{}", name(test, end)))
                .unwrap()
        }

        #[test]
        fn connect_reads_a_helper_accepted_late() {
            let (stdin, stdout) = listeners("late");
            let helper = std::thread::spawn(|| {
                let _stdin = helper("late", "stdin");
                let mut stdout = helper("late", "stdout");
                stdout.write_all(b"Error: not found\n").unwrap();
                // Returns once the watchdog read the line.
                stdout.flush().unwrap();
            });
            std::thread::sleep(Duration::from_millis(200));
            let (_stdin, mut stdout) = connect(&stdin, &stdout, || Ok(None), || false)
                .unwrap()
                .unwrap();
            let mut line = [0; 17];
            stdout.read_exact(&mut line).unwrap();
            assert_eq!(&line, b"Error: not found\n");
            helper.join().unwrap();
        }

        #[test]
        fn connect_ends_with_powershell() {
            let (stdin, stdout) = listeners("exit");
            let error = connect(
                &stdin,
                &stdout,
                || Ok(Some(ExitStatus::default())),
                || false,
            )
            .unwrap_err();
            assert!(
                error.to_string().starts_with("PowerShell exited"),
                "{error}"
            );
        }

        #[test]
        fn start_ends_powershell_on_a_stop() {
            use std::cell::RefCell;
            let served = name("kill", "powershell");
            let stand_in_pipe = listener(&served).unwrap();
            let stand_in = RefCell::new(None);
            let started = super::super::start(
                std::path::Path::new(r"C:\XMRig\xmrig.exe"),
                &[],
                |_, _| {
                    let mut cmd = std::process::Command::new(r"C:\windows\system32\cmd.exe");
                    cmd.args(["/C", &format!(r"set /p x=<\\.\pipe\{served}")]);
                    cmd
                },
                // Stops once the PowerShell stand-in reads its pipe.
                || {
                    if let Ok(pipe) = stand_in_pipe.accept() {
                        *stand_in.borrow_mut() = Some(pipe);
                    }
                    stand_in.borrow().is_some()
                },
            );
            assert!(started.unwrap().is_none());
            let mut pipe = stand_in.into_inner().unwrap();
            pipe.set_nonblocking(false).unwrap();
            let (read, ended) = mpsc::channel();
            std::thread::spawn(move || read.send(pipe.read(&mut [0; 1])));
            // The pipe ends with the stand-in.
            assert!(matches!(
                ended.recv_timeout(Duration::from_secs(5)),
                Ok(Ok(0) | Err(_))
            ));
        }

        #[test]
        fn start_reports_a_powershell_that_does_not_start() {
            let Err(error) = super::super::start(
                std::path::Path::new(r"C:\XMRig\xmrig.exe"),
                &[],
                |_, _| std::process::Command::new(r"C:\missing\powershell.exe"),
                || false,
            ) else {
                panic!("PowerShell started");
            };
            assert!(
                error
                    .to_string()
                    .starts_with("PowerShell could not start: "),
                "{error}"
            );
        }

        #[test]
        fn start_shows_why_powershell_failed() {
            let Err(error) = super::super::start(
                std::path::Path::new(r"C:\XMRig\xmrig.exe"),
                &[],
                |_, _| {
                    let mut cmd = std::process::Command::new(r"C:\windows\system32\cmd.exe");
                    cmd.args([
                        "/C",
                        "echo The operation was canceled by the user.>&2& exit 1",
                    ]);
                    cmd
                },
                || false,
            ) else {
                panic!("the helper connected");
            };
            assert!(
                error
                    .to_string()
                    .ends_with(": The operation was canceled by the user."),
                "{error}"
            );
        }

        #[test]
        fn start_runs_xmrig_through_powershell_and_the_helper() {
            if !is_elevated::is_elevated() {
                return;
            }
            // A plain `cargo test` builds Gupax there for the integration tests.
            let gupax = std::env::current_exe().unwrap();
            let gupax = gupax.parent().unwrap().parent().unwrap().join("gupax.exe");
            let dir = tempfile::Builder::new()
                .prefix("Jo O'Neil [x] ")
                .tempdir()
                .unwrap();
            let helper = dir.path().join("Gupax.exe");
            std::fs::copy(&gupax, &helper).unwrap();
            // Echoes the arguments as XMRig receives them.
            let xmrig = dir.path().join("xmrig.exe");
            std::fs::copy(r"C:\windows\system32\cmd.exe", &xmrig).unwrap();
            let args = [
                "/C",
                "echo",
                "--rig-id=Jo's rig",
                "--cuda-devices=0,1",
                "&",
                "exit",
                "3",
            ];
            let (mut powershell, _stdin, mut stdout) = super::super::start(
                &xmrig,
                &args.map(String::from),
                |_, command_line| super::super::powershell(&helper, command_line),
                || false,
            )
            .unwrap()
            .unwrap();
            let mut output = Vec::new();
            let mut buf = [0; 4096];
            while let Ok(n @ 1..) = stdout.read(&mut buf) {
                output.extend_from_slice(&buf[..n]);
            }
            let output = String::from_utf8_lossy(&output);
            assert!(
                output.contains(r#""--rig-id=Jo's rig" --cuda-devices=0,1"#),
                "{output:?}"
            );
            assert_eq!(powershell.wait().unwrap().code(), Some(3));
        }
    }
}
