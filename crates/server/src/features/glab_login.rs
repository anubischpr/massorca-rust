//! The GitLab CLI as a service connection (decision 0097): `glab auth status` for the state and `glab auth login
//! --stdin` for a token, which is written to its stdin and never to an argument, the environment or a log.

use std::io;
use std::time::Duration;

use super::extension_process::HostPrograms;

const GLAB: &str = "glab";
const TIMEOUT: Duration = Duration::from_secs(20);
/// The host `glab` signs in to when no base URL is given.
pub(crate) const DEFAULT_HOST: &str = "gitlab.com";

/// How a `glab` run ended. Its output is never kept: it may repeat the host or the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlabOutcome {
    Ok,
    Refused,
    NotInstalled,
    Failed,
}

/// Whether `glab` holds a sign-in on this host.
pub(crate) fn status(programs: &HostPrograms) -> GlabOutcome {
    outcome(programs.run_with_input(GLAB, &["auth", "status"], &[], b"", TIMEOUT))
}

/// Signs `glab` in to `host` (a host name, with a port for a self-hosted server) with `token` on its stdin.
pub(crate) fn login(programs: &HostPrograms, token: &str, host: &str) -> GlabOutcome {
    let (args, env) = login_argv(host);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    outcome(programs.run_with_input(GLAB, &args, &env, token.as_bytes(), TIMEOUT))
}

/// `glab`'s `--hostname` refuses `host:port`, so a ported host goes in `GITLAB_HOST` instead (as pv96 does). The host
/// is joined to its flag, so it can never be read as a flag of its own.
fn login_argv(host: &str) -> (Vec<String>, Vec<(&str, &str)>) {
    let ported = host
        .rsplit_once(':')
        .is_some_and(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
    if ported {
        (
            words(&["auth", "login", "--stdin"]),
            vec![("GITLAB_HOST", host)],
        )
    } else {
        let hostname = format!("--hostname={host}");
        (words(&["auth", "login", &hostname, "--stdin"]), Vec::new())
    }
}

fn words(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn outcome(run: io::Result<massorca_core_extensions::host::Exit>) -> GlabOutcome {
    match run {
        Ok(exit) if exit.success() => GlabOutcome::Ok,
        Ok(exit) if exit.timed_out => GlabOutcome::Failed,
        Ok(_) => GlabOutcome::Refused,
        Err(error) if error.kind() == io::ErrorKind::NotFound => GlabOutcome::NotInstalled,
        Err(_) => GlabOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use massorca_core_agents::SearchEnv;

    use super::*;

    const TOKEN: &str = "glpat-unit-token-5521";

    /// A fake `glab` that records its args and stdin next to itself and exits 1 when `reject` exists there.
    fn fake_glab(bin: &Path) {
        if cfg!(windows) {
            let script = "@echo off\r\necho %*>\"%~dp0args.txt\"\r\n\"%SystemRoot%\\System32\\findstr.exe\" \"^\" >\"%~dp0stdin.txt\"\r\nif exist \"%~dp0reject\" exit /b 1\r\nexit /b 0\r\n";
            fs::write(bin.join("glab.cmd"), script).expect("the fake glab");
        } else {
            let script = "#!/bin/sh\nd=${0%/*}\nprintf '%s\\n' \"$*\" >\"$d/args.txt\"\nwhile IFS= read -r l || [ -n \"$l\" ]; do printf '%s\\n' \"$l\"; done >\"$d/stdin.txt\"\n[ -e \"$d/reject\" ] && exit 1\nexit 0\n";
            let path = bin.join("glab");
            fs::write(&path, script).expect("the fake glab");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
            }
        }
    }

    fn programs(bin: &Path) -> HostPrograms {
        HostPrograms::searching(SearchEnv {
            path: Some(bin.as_os_str().to_owned()),
            pathext: Some(".COM;.EXE;.BAT;.CMD".to_owned()),
        })
    }

    #[test]
    fn login_writes_the_token_to_stdin_and_never_to_the_args() {
        let bin = tempfile::tempdir().expect("a bin folder");
        fake_glab(bin.path());

        let outcome = login(&programs(bin.path()), TOKEN, "gitlab.example.com");

        assert_eq!(outcome, GlabOutcome::Ok);
        let args = fs::read_to_string(bin.path().join("args.txt")).expect("args");
        let stdin = fs::read_to_string(bin.path().join("stdin.txt")).expect("stdin");
        assert_eq!(
            // A batch file sees `=` arguments quoted; the quotes are cmd's transport, not part of the argument.
            args.trim().replace('"', ""),
            "auth login --hostname=gitlab.example.com --stdin"
        );
        assert_eq!(stdin.trim(), TOKEN);
    }

    #[test]
    fn a_ported_host_goes_in_the_environment_and_a_refusal_is_refused() {
        assert_eq!(
            login_argv("gitlab.example.com:8443"),
            (
                words(&["auth", "login", "--stdin"]),
                vec![("GITLAB_HOST", "gitlab.example.com:8443")]
            )
        );
        let bin = tempfile::tempdir().expect("a bin folder");
        fake_glab(bin.path());
        fs::write(bin.path().join("reject"), "").expect("marker");

        assert_eq!(status(&programs(bin.path())), GlabOutcome::Refused);
        assert_eq!(
            login(&programs(bin.path()), TOKEN, DEFAULT_HOST),
            GlabOutcome::Refused
        );
    }

    #[test]
    fn a_host_without_glab_reports_it_not_installed() {
        let bin = tempfile::tempdir().expect("an empty bin folder");
        assert_eq!(status(&programs(bin.path())), GlabOutcome::NotInstalled);
    }
}
