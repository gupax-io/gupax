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

use std::path::Path;
use std::process::Command;

// Shows the UAC prompt, then runs the helper and waits for it. PowerShell reads the
// values from the environment as plain text, and ProcessStartInfo keeps the
// wildcard characters of the path.
// PowerShell then exits with the code of the helper, which is the code of XMRig, or
// with 1 when the start failed.
const SCRIPT: &str = "$ErrorActionPreference='Stop'; \
    $p=[Diagnostics.Process]::Start([Diagnostics.ProcessStartInfo]@{\
    FileName=$env:GUPAX_HELPER_EXE; Arguments=$env:GUPAX_HELPER_ARGS; \
    Verb='RunAs'; UseShellExecute=$true}); \
    $p.WaitForExit(); exit $p.ExitCode";

// Read by [Cli] in the elevated Gupax.
pub(super) fn helper_args(
    xmrig: &Path,
    args: &[String],
    stdin_name: &str,
    stdout_name: &str,
) -> Vec<String> {
    [
        "--elevated-helper",
        "--name-stdin-pipe",
        stdin_name,
        "--name-stdout-pipe",
        stdout_name,
        "--binary-path",
        &xmrig.display().to_string(),
        &format!("--arguments={}", args.join(",")),
    ]
    .map(String::from)
    .to_vec()
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
            r#"--elevated-helper --name-stdin-pipe stdin --name-stdout-pipe stdout --binary-path "C:\Users\Jo O'Neil [x]\XMRig\xmrig.exe" "--arguments=--rig-id=Jo's rig,--threads,6""#
        );
        assert_eq!(
            command_line(
                r"C:\XMRig\xmrig.exe",
                &[r#"--rig-id="a b""#, r"C:\my logs\"]
            ),
            r#"--elevated-helper --name-stdin-pipe stdin --name-stdout-pipe stdout --binary-path C:\XMRig\xmrig.exe "--arguments=--rig-id=\"a b\",C:\my logs\\""#
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
}
