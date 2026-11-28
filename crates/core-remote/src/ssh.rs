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

/// The arguments of `ssh -W <forward> <destination>`, OpenSSH's stdio forward, which a client dials a paired host
/// through (decision 0133): batch mode, the route's own options such as `-J bastion`, then the forward. `-W` implies
/// no TTY, no remote command and no other forwardings. Each value stays one argument, so no shell reads it.
pub fn stdio_forward_arguments(
    destination: &str,
    extra_args: &[String],
    forward: &str,
) -> Result<Vec<String>, SshError> {
    check_destination(destination)?;
    check_forward(forward)?;
    check_route_options(extra_args)?;
    let mut args: Vec<String> = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"]
        .map(str::to_owned)
        .to_vec();
    args.extend(extra_args.iter().cloned());
    args.extend([
        "-W".to_owned(),
        forward.to_owned(),
        "--".to_owned(),
        destination.to_owned(),
    ]);
    Ok(args)
}

/// The `ssh` a client dials a host through: the absolute path in `MASSORCA_SSH` when set, as `MASSORCA_GIT` names
/// git, else the system `ssh` as `SystemSsh` finds it.
pub fn find_system_ssh() -> Result<PathBuf, SshError> {
    ssh_program_from(std::env::var_os(SSH_PROGRAM_ENV))
}

/// A relative `MASSORCA_SSH` is refused rather than ignored, so a typo never falls back to another ssh.
fn ssh_program_from(named: Option<std::ffi::OsString>) -> Result<PathBuf, SshError> {
    match named.map(PathBuf::from) {
        Some(program) if program.is_absolute() => Ok(program),
        Some(program) => Err(SshError::Refused(format!(
            "{SSH_PROGRAM_ENV} must be an absolute path, not {}",
            program.display()
        ))),
        None => find_ssh().ok_or(SshError::NotFound),
    }
}

/// ssh options that take a value, so the next argument, or the rest of a cluster, is that value.
const SSH_OPTIONS_WITH_VALUE: &str = "BbcDEeFIiJLlmOoPpQRSWw";

/// Refuses route options that would leave ssh running after the connection ends: `-f`, alone or in a cluster such as
/// `-fN`, and `-o ForkAfterAuthentication`.
pub fn check_route_options(extra_args: &[String]) -> Result<(), SshError> {
    let refused = |what: &str| {
        Err(SshError::Refused(format!(
            "the ssh option {what} would keep ssh running after the connection ends"
        )))
    };
    let mut value_of: Option<char> = None;
    for arg in extra_args {
        if let Some(option) = value_of.take() {
            if option == 'o' && forks(arg) {
                return refused(arg);
            }
            continue;
        }
        let Some(cluster) = arg.strip_prefix('-').filter(|rest| !rest.starts_with('-')) else {
            continue;
        };
        for (at, letter) in cluster.char_indices() {
            if letter == 'f' {
                return refused(arg);
            }
            if SSH_OPTIONS_WITH_VALUE.contains(letter) {
                let value = &cluster[at + letter.len_utf8()..];
                if value.is_empty() {
                    value_of = Some(letter);
                } else if letter == 'o' && forks(value) {
                    return refused(arg);
                }
                break;
            }
        }
    }
    Ok(())
}

/// Whether an `-o` value turns on `ForkAfterAuthentication`; ssh reads option names in any case.
fn forks(value: &str) -> bool {
    value
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("forkafterauthentication")
}

/// Names the ssh program of `find_system_ssh`.
pub const SSH_PROGRAM_ENV: &str = "MASSORCA_SSH";

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

/// Refuses a forward that is not one `host:port` word ssh would read as such.
fn check_forward(forward: &str) -> Result<(), SshError> {
    let well_formed = !forward.starts_with('-')
        && !forward.chars().any(|c| c.is_whitespace() || c.is_control())
        && forward
            .rsplit_once(':')
            .is_some_and(|(host, port)| !host.is_empty() && port.parse::<u16>().is_ok());
    if well_formed {
        Ok(())
    } else {
        Err(SshError::Refused(format!(
            "the forward {forward:?} is not host:port"
        )))
    }
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

    #[test]
    fn a_relative_massorca_ssh_is_refused_and_an_absolute_one_is_used() {
        match ssh_program_from(Some("bin/ssh".into())) {
            Err(SshError::Refused(message)) => {
                assert!(
                    message.contains("MASSORCA_SSH must be an absolute path"),
                    "{message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        let absolute = std::env::temp_dir().join("ssh");
        assert_eq!(
            ssh_program_from(Some(absolute.clone().into_os_string())).expect("used"),
            absolute
        );
    }

    #[test]
    fn a_route_option_that_backgrounds_ssh_is_refused() {
        let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        for args in [
            &["-f"][..],
            &["-fN"],
            &["-Nf"],
            &["-4", "-f"],
            &["-o", "ForkAfterAuthentication=yes"],
            &["-oforkafterauthentication yes"],
        ] {
            assert!(
                matches!(check_route_options(&owned(args)), Err(SshError::Refused(_))),
                "{args:?}"
            );
            assert!(
                stdio_forward_arguments("vps", &owned(args), "127.0.0.1:7347").is_err(),
                "{args:?}"
            );
        }
        for args in [
            &["-J", "f"][..],
            &["-p", "2222"],
            &["-i", "/home/f/key"],
            &["-vv", "-4"],
            &["-o", "User=f"],
        ] {
            assert!(check_route_options(&owned(args)).is_ok(), "{args:?}");
        }
    }

    #[test]
    fn a_stdio_forward_is_batch_mode_then_the_route_options_then_the_forward_and_the_destination() {
        let route_args = ["-J", "bastion", "-o", "User=$(id) x"].map(str::to_owned);
        let args =
            stdio_forward_arguments("deploy@vps.example.ts.net", &route_args, "100.64.0.10:7347")
                .expect("a valid route");
        assert_eq!(
            args,
            [
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=15",
                "-J",
                "bastion",
                "-o",
                "User=$(id) x",
                "-W",
                "100.64.0.10:7347",
                "--",
                "deploy@vps.example.ts.net",
            ],
            "every value stays one argument, unquoted and unexpanded"
        );
        let bracketed = stdio_forward_arguments("vps", &[], "[::1]:7347").expect("an IPv6 forward");
        assert_eq!(bracketed[bracketed.len() - 3], "[::1]:7347");
    }

    #[test]
    fn a_destination_or_forward_ssh_could_misread_is_refused() {
        for destination in ["", "-oProxyCommand=evil", "vps extra", "vps\tx"] {
            assert!(
                matches!(
                    stdio_forward_arguments(destination, &[], "127.0.0.1:7347"),
                    Err(SshError::Refused(_))
                ),
                "{destination:?}"
            );
        }
        for forward in [
            "",
            "-W",
            "127.0.0.1",
            "127.0.0.1:port",
            ":7347",
            "a b:7347",
            "127.0.0.1:99999",
        ] {
            assert!(
                matches!(
                    stdio_forward_arguments("vps", &[], forward),
                    Err(SshError::Refused(_))
                ),
                "{forward:?}"
            );
        }
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
