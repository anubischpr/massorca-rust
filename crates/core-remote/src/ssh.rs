//! Runs commands on a remote host with the system OpenSSH client (decision 0044).

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use crate::process::{self, CaptureError};

/// One remote command may run this long before its ssh process is stopped.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);
/// OpenSSH exits with 255 when its own connection or authentication fails.
const SSH_OWN_FAILURE: i32 = 255;

/// Runs one command through the remote login shell. The real implementation spawns `ssh`.
pub trait RemoteShell {
    /// Runs `command` on the host, piping `stdin` to it when given.
    fn run(&self, command: &str, stdin: Option<&[u8]>) -> Result<RemoteOutput, SshError>;
}

/// What a remote command printed and how it exited. `status` is -1 when ssh was stopped by a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A host reached with the system `ssh` in batch mode, so the user's config, agent and host-key policy apply.
#[derive(Debug)]
pub struct SystemSsh {
    destination: String,
    extra_args: Vec<String>,
    program: PathBuf,
}

/// Why a remote command could not complete.
#[derive(Debug, Error)]
pub enum SshError {
    #[error("OpenSSH (ssh) was not found on this computer")]
    NotFound,
    #[error("could not run ssh: {0}")]
    Spawn(#[source] io::Error),
    #[error("ssh failed with exit status {status}: {stderr}")]
    Failed { status: i32, stderr: String },
    #[error("{0}")]
    Refused(String),
    #[error("ssh did not finish within {} s and was stopped", .0.as_secs())]
    TimedOut(Duration),
}

impl SystemSsh {
    /// Targets `destination` (`[user@]host`) with extra `ssh` options, such as `-p 2222`.
    pub fn new(destination: &str, extra_args: Vec<String>) -> Result<Self, SshError> {
        check_destination(destination)?;
        let program = find_ssh().ok_or(SshError::NotFound)?;
        Ok(Self {
            destination: destination.to_owned(),
            extra_args,
            program,
        })
    }

    /// Batch mode, the caller's options, then `--`. It must come before the destination:
    /// POSIX getopt stops at the first operand, so a later `--` would reach the remote shell.
    fn arguments<'a>(&'a self, command: &'a str) -> Vec<&'a str> {
        let mut args = vec!["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"];
        args.extend(self.extra_args.iter().map(String::as_str));
        args.extend(["--", self.destination.as_str(), command]);
        args
    }
}

impl RemoteShell for SystemSsh {
    fn run(&self, command: &str, stdin: Option<&[u8]>) -> Result<RemoteOutput, SshError> {
        let mut ssh_command = massorca_process::command(&self.program);
        ssh_command.args(self.arguments(command));
        let captured = process::run_captured(ssh_command, stdin, COMMAND_TIMEOUT).map_err(
            |err| match err {
                CaptureError::Spawn(err) | CaptureError::Wait(err) => SshError::Spawn(err),
                CaptureError::TimedOut => SshError::TimedOut(COMMAND_TIMEOUT),
            },
        )?;
        let status = captured.status.code().unwrap_or(-1);
        if status == SSH_OWN_FAILURE {
            return Err(SshError::Failed {
                status,
                stderr: process::excerpt(&captured.stderr),
            });
        }
        Ok(RemoteOutput {
            status,
            stdout: captured.stdout,
            stderr: captured.stderr,
        })
    }
}

/// Quotes `value` as one POSIX shell word, so the remote shell reads it literally.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Refuses what ssh could read as an option or split into several arguments.
fn check_destination(destination: &str) -> Result<(), SshError> {
    if destination.is_empty() {
        return Err(SshError::Refused("the destination is empty".to_owned()));
    }
    if destination.starts_with('-') {
        return Err(SshError::Refused(format!(
            "the destination {destination:?} must not start with '-'"
        )));
    }
    if destination
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(SshError::Refused(format!(
            "the destination {destination:?} contains a space or a control character"
        )));
    }
    Ok(())
}

/// `ssh` on `PATH`, then the Windows OpenSSH feature location.
fn find_ssh() -> Option<PathBuf> {
    let name = if cfg!(windows) { "ssh.exe" } else { "ssh" };
    process::find_on_path(name).or_else(|| process::first_executable(FALLBACK_LOCATIONS))
}

#[cfg(windows)]
const FALLBACK_LOCATIONS: &[&str] = &[r"C:\Windows\System32\OpenSSH\ssh.exe"];
#[cfg(not(windows))]
const FALLBACK_LOCATIONS: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_mode_then_extra_options_then_the_destination_and_the_command() {
        let ssh = SystemSsh {
            destination: "deploy@203.0.113.7".to_owned(),
            extra_args: vec!["-p".to_owned(), "2222".to_owned()],
            program: PathBuf::from("ssh"),
        };
        assert_eq!(
            ssh.arguments("uname -sm"),
            [
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=15",
                "-p",
                "2222",
                "--",
                "deploy@203.0.113.7",
                "uname -sm",
            ]
        );
    }

    #[cfg(unix)]
    mod with_fake_ssh {
        use super::*;
        use massorca_process::test_executable::write_executable;
        use std::path::Path;

        /// A stand-in `ssh`: prints its arguments, fails on demand, then echoes stdin.
        fn fake_ssh(dir: &Path) -> PathBuf {
            let path = dir.join("ssh");
            let script = r#"#!/bin/sh
printf 'args:'
for arg in "$@"; do printf ' [%s]' "$arg"; done
printf '\n'
case "$*" in
  *FAIL255*) echo boom >&2; exit 255 ;;
  *EXIT3*) exit 3 ;;
esac
cat
"#;
            write_executable(&path, script);
            path
        }

        fn ssh_with(dir: &Path) -> SystemSsh {
            SystemSsh {
                destination: "vps".to_owned(),
                extra_args: Vec::new(),
                program: fake_ssh(dir),
            }
        }

        #[test]
        fn arguments_arrive_as_given_and_stdin_reaches_the_command() {
            let dir = tempfile::tempdir().expect("temp dir");
            let input: Vec<u8> = (0..2_000_000_u32).map(|i| (i % 251) as u8).collect();
            let output = ssh_with(dir.path())
                .run("cat > file", Some(&input))
                .expect("the fake ssh runs");
            assert_eq!(output.status, 0);
            let header =
                b"args: [-o] [BatchMode=yes] [-o] [ConnectTimeout=15] [--] [vps] [cat > file]\n";
            assert!(output.stdout.starts_with(header), "unexpected arguments");
            assert!(
                output.stdout[header.len()..] == input[..],
                "stdin was not echoed back"
            );
        }

        #[test]
        fn exit_255_is_an_ssh_failure_with_its_error_text() {
            let dir = tempfile::tempdir().expect("temp dir");
            match ssh_with(dir.path()).run("FAIL255 now", None) {
                Err(SshError::Failed { status, stderr }) => {
                    assert_eq!(status, 255);
                    assert_eq!(stderr, "boom");
                }
                other => panic!("expected an ssh failure, got {other:?}"),
            }
        }

        #[test]
        fn other_exit_codes_come_back_as_the_remote_status() {
            let dir = tempfile::tempdir().expect("temp dir");
            let output = ssh_with(dir.path())
                .run("EXIT3", None)
                .expect("the fake ssh runs");
            assert_eq!(output.status, 3);
        }
    }
}
