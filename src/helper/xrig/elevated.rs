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

#[cfg(target_os = "windows")]
use elevated_helper::interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode::Bytes,
};
#[cfg(target_os = "windows")]
use std::io;
use std::path::Path;
use std::process::Command;

// Shows the UAC prompt, then runs the helper and waits for it. PowerShell reads the
// values from the environment as plain text, and Escape makes Start-Process read the
// wildcard characters of the path literally. Start-Process and [WildcardPattern] work
// in the Constrained Language Mode of locked-down PCs.
// PowerShell then exits with the code of the helper, which is the code of XMRig, or
// with 1 when the start failed.
const SCRIPT: &str = "$ErrorActionPreference='Stop'; \
    $p=Start-Process -FilePath ([WildcardPattern]::Escape($env:GUPAX_HELPER_EXE)) \
    -ArgumentList $env:GUPAX_HELPER_ARGS -Verb RunAs -PassThru -Wait; exit $p.ExitCode";

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
    // Command looks for a program in the folder of Gupax first.
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut cmd =
        Command::new(Path::new(&windows).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
    // The user's profile could prompt in the hidden window or slow every start.
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT]);
    cmd.env("GUPAX_HELPER_EXE", gupax_exe);
    cmd.env("GUPAX_HELPER_ARGS", command_line);
    cmd
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
    powershell: &mut std::process::Child,
    stopped: impl Fn() -> bool,
) -> io::Result<Option<(Pipe, Pipe)>> {
    let mut accept = |listener: &PipeListener<Bytes, Bytes>| {
        super::wait_ready(
            powershell,
            || {
                let pipe = listener.accept()?;
                // Streams of a nonblocking listener are nonblocking too.
                pipe.set_nonblocking(false)?;
                Ok(pipe)
            },
            &stopped,
            "PowerShell",
            "the elevated helper connected",
        )
    };
    let Some(stdin_pipe) = accept(stdin)? else {
        return Ok(None);
    };
    Ok(accept(stdout)?.map(|stdout_pipe| (stdin_pipe, stdout_pipe)))
}

// XMRig is started by an elevated Gupax, which relays its input and output.
// The child is PowerShell. None on a stop.
#[cfg(target_os = "windows")]
pub(super) fn start(
    xmrig: &Path,
    args: &[String],
    powershell: impl FnOnce(&Path, &str) -> Command,
    stopped: impl Fn() -> bool,
    gupax_replaced: bool,
) -> io::Result<Option<(std::process::Child, Pipe, Pipe)>> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;
    // PowerShell would show its UAC prompt for a stopped XMRig.
    if stopped() {
        return Ok(None);
    }
    // The elevated Gupax would run another version, whose arguments may differ.
    if gupax_replaced {
        return Err(io::Error::other(
            "Gupax.exe was replaced by another version, restart Gupax",
        ));
    }
    // The command line of the helper holds the path as text.
    if xmrig.to_str().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not valid Unicode", xmrig.display()),
        ));
    }
    let id = uuid::Uuid::new_v4();
    let (stdin_name, stdout_name) = (format!("xmrig_stdin_{id}"), format!("xmrig_stdout_{id}"));
    let (stdin, stdout) = (listener(&stdin_name)?, listener(&stdout_name)?);
    let mut cmd = powershell(
        &std::env::current_exe()?,
        &command_line(&helper_args(xmrig, args, &stdin_name, &stdout_name)),
    );
    cmd.creation_flags(CREATE_NO_WINDOW.0);
    let mut child = cmd
        .spawn()
        .map_err(|e| io::Error::new(e.kind(), format!("PowerShell could not start: {e}")))?;
    let connected = connect(&stdin, &stdout, &mut child, stopped);
    // A helper started later from the UAC prompt exits, as its pipes are gone.
    if connected.is_err()
        && let Err(e) = child.kill()
    {
        log::warn!("XMRig | Could not kill PowerShell: {e}");
    }
    Ok(connected?.map(|(stdin, stdout)| (child, stdin, stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
        use std::collections::HashMap;
        let cmd = powershell(Path::new(r"C:\Gupax [x]\Gupax.exe"), "--elevated-helper");
        assert_eq!(
            cmd.get_args().collect::<Vec<_>>(),
            ["-NoProfile", "-NonInteractive", "-Command", SCRIPT]
        );
        assert_eq!(
            cmd.get_envs().collect::<HashMap<_, _>>(),
            HashMap::from([
                (
                    "GUPAX_HELPER_ARGS".as_ref(),
                    Some("--elevated-helper".as_ref())
                ),
                (
                    "GUPAX_HELPER_EXE".as_ref(),
                    Some(r"C:\Gupax [x]\Gupax.exe".as_ref())
                )
            ])
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn powershell_runs_from_the_windows_folder() {
        let cmd = powershell(Path::new(r"C:\Gupax\Gupax.exe"), "--elevated-helper");
        let program = Path::new(cmd.get_program());
        assert!(program.is_absolute() && program.exists(), "{program:?}");
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

    #[cfg(target_os = "windows")]
    mod pipes {
        use super::super::{connect, listener, start};
        use crate::helper::sleeping;
        use elevated_helper::interprocess::os::windows::named_pipe::DuplexPipeStream;
        use elevated_helper::interprocess::os::windows::named_pipe::pipe_mode::Bytes;
        use std::io::{self, Read, Write};
        use std::path::Path;
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;

        fn name(end: &str) -> String {
            format!("gupax_test_late_{}_{end}", std::process::id())
        }

        fn helper(end: &str) -> DuplexPipeStream<Bytes> {
            DuplexPipeStream::<Bytes>::connect_by_path(format!(r"\\.\pipe\{}", name(end))).unwrap()
        }

        #[test]
        fn connect_reads_a_helper_accepted_late() {
            let stdin = listener(&name("stdin")).unwrap();
            let stdout = listener(&name("stdout")).unwrap();
            let (wrote, written) = mpsc::channel();
            let helper = std::thread::spawn(move || {
                let _stdin = helper("stdin");
                let mut stdout = helper("stdout");
                stdout.write_all(b"Error: not found\n").unwrap();
                wrote.send(()).unwrap();
                // Returns once the watchdog read the line.
                stdout.flush().unwrap();
            });
            written.recv_timeout(Duration::from_secs(5)).unwrap();
            let mut powershell = sleeping();
            let (_stdin, mut stdout) = connect(&stdin, &stdout, &mut powershell, || false)
                .unwrap()
                .unwrap();
            let mut line = [0; 17];
            stdout.read_exact(&mut line).unwrap();
            assert_eq!(&line, b"Error: not found\n");
            helper.join().unwrap();
            powershell.kill().unwrap();
        }

        // Tells whether XMRig started with `powershell` as PowerShell.
        fn start_with(
            powershell: impl FnOnce(&Path, &str) -> Command,
            stopped: impl Fn() -> bool,
            gupax_replaced: bool,
        ) -> io::Result<bool> {
            let xmrig = Path::new(r"C:\XMRig\xmrig.exe");
            start(xmrig, &[], powershell, stopped, gupax_replaced).map(|started| started.is_some())
        }

        fn never(_: &Path, _: &str) -> Command {
            panic!("PowerShell started")
        }

        #[test]
        fn a_stop_cancels_a_start_that_waits_for_the_uac_prompt() {
            let asked = std::cell::Cell::new(0);
            // Stopped once PowerShell runs.
            let stopped = || asked.replace(asked.get() + 1) > 0;
            let ping = |_: &Path, _: &str| {
                let mut ping = Command::new(r"C:\Windows\System32\PING.EXE");
                ping.args(["-n", "10", "127.0.0.1"]);
                ping
            };
            let started = start_with(ping, stopped, false);
            assert!(!started.unwrap());
        }

        #[test]
        fn start_after_a_stop_starts_no_powershell() {
            assert!(!start_with(never, || true, false).unwrap());
        }

        #[test]
        fn start_refuses_a_path_that_is_not_unicode() {
            use std::os::windows::ffi::OsStringExt;
            let xmrig = std::ffi::OsString::from_wide(&[0xD800, u16::from(b'x')]);
            let error = start(Path::new(&xmrig), &[], never, || false, false)
                .map(|_| ())
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }

        #[test]
        fn start_reports_a_powershell_that_does_not_start() {
            let missing = |_: &Path, _: &str| Command::new(r"C:\missing\powershell.exe");
            let error = start_with(missing, || false, false).unwrap_err();
            assert!(
                error
                    .to_string()
                    .starts_with("PowerShell could not start: "),
                "{error}"
            );
        }

        #[test]
        fn an_updated_gupax_asks_for_a_restart_before_the_uac_prompt() {
            let error = start_with(never, || false, true).unwrap_err();
            assert_eq!(
                error.to_string(),
                "Gupax.exe was replaced by another version, restart Gupax"
            );
        }
    }
}
