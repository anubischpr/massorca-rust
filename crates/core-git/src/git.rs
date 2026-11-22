use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::GitError;
use crate::process::{Bounded, Output, run_bounded};
use crate::version::GitVersion;

/// Time a single read-only git query may run before it is killed. Commands that
/// change the repository have no timeout unless `Git::with_mutation_timeout` sets one.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Environment variable that overrides the git executable.
const BINARY_ENV: &str = "MASSORCA_GIT";

#[derive(Debug, Clone)]
pub struct Git {
    binary: PathBuf,
    timeout: Duration,
    /// `None` lets a mutation run to completion: killing one midway can leave a
    /// half-created worktree or branch behind.
    mutation_timeout: Option<Duration>,
}

impl Git {
    /// Uses the given git executable: a path, or a name resolved through PATH.
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            timeout: DEFAULT_TIMEOUT,
            mutation_timeout: None,
        }
    }

    /// Finds git through `MASSORCA_GIT` or PATH, and checks that it runs.
    pub fn discover() -> Result<Self, GitError> {
        discover_with(std::env::var_os(BINARY_ENV))
    }

    /// Replaces the timeout of read-only queries.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the timeout of commands that change the repository; `None`, the default, waits for them.
    #[must_use]
    pub fn with_mutation_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.mutation_timeout = timeout;
        self
    }

    /// The version of the git binary, from `git --version`.
    pub fn version(&self) -> Result<GitVersion, GitError> {
        let captured = self.run(None, ["--version"])?;
        captured.require_success()?;
        let text = captured.text()?;
        GitVersion::parse_version_output(text).ok_or_else(|| {
            captured.invalid_output(format!("unrecognised version line `{}`", text.trim()))
        })
    }

    /// Runs a read-only query in `cwd` (or the process directory) and returns its
    /// output whatever the exit status.
    pub(crate) fn run<I, S>(&self, cwd: Option<&Path>, args: I) -> Result<Captured, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_limited(cwd, args, Some(self.timeout), true, None, &[])
    }

    /// Like `run`, keeping at most `max_stdout` bytes of output; `Captured::truncated` says whether more existed.
    pub(crate) fn run_capped<I, S>(
        &self,
        cwd: Option<&Path>,
        args: I,
        max_stdout: usize,
    ) -> Result<Captured, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_limited(cwd, args, Some(self.timeout), true, Some(max_stdout), &[])
    }

    /// Like `run`, for a command that changes the repository or belongs to one that does.
    pub(crate) fn run_mutation<I, S>(
        &self,
        cwd: Option<&Path>,
        args: I,
    ) -> Result<Captured, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_limited(cwd, args, self.mutation_timeout, false, None, &[])
    }

    /// Like `run_mutation`, bounded by `timeout` unless the configured mutation timeout is shorter.
    pub(crate) fn run_mutation_within<I, S>(
        &self,
        cwd: Option<&Path>,
        args: I,
        timeout: Duration,
    ) -> Result<Captured, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let timeout = self
            .mutation_timeout
            .map_or(timeout, |own| own.min(timeout));
        self.run_limited(cwd, args, Some(timeout), false, None, &[])
    }

    /// Runs a command that talks to a remote, bounded by `timeout`, with `env` added (decision 0013).
    pub(crate) fn run_network<I, S>(
        &self,
        cwd: Option<&Path>,
        args: I,
        timeout: Duration,
        env: &[(&str, &str)],
    ) -> Result<Captured, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_limited(cwd, args, Some(timeout), false, None, env)
    }

    fn run_limited<I, S>(
        &self,
        cwd: Option<&Path>,
        args: I,
        timeout: Option<Duration>,
        read_only: bool,
        max_stdout: Option<usize>,
        env: &[(&str, &str)],
    ) -> Result<Captured, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<OsString> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect();
        let shown: Vec<String> = args
            .iter()
            .map(|arg| redact_urls(&arg.to_string_lossy()))
            .collect();
        let mut command = massorca_process::command(&self.binary);
        command
            .arg("-c")
            .arg("core.quotepath=off")
            .args(&args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            // Nothing may wait for an answer (decision 0013): an empty askpass also skips `core.askPass`.
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .env("SSH_ASKPASS_REQUIRE", "never")
            .env("GCM_INTERACTIVE", "never")
            .env_remove("GIT_TRACE")
            .env_remove("GIT_TRACE_CURL")
            .env_remove("GIT_TRACE_PACKET")
            .env_remove("GIT_CURL_VERBOSE")
            // A repository location inherited from a git hook would override the cwd.
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE");
        if read_only {
            // A read never refreshes the index, so it cannot wake a watcher (decision 0012).
            command.env("GIT_OPTIONAL_LOCKS", "0");
        }
        command.envs(env.iter().copied());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        match run_bounded(&mut command, timeout, max_stdout) {
            Ok(Bounded::Finished(output)) => Ok(Captured {
                args: shown,
                output,
            }),
            Ok(Bounded::TimedOut) => Err(GitError::Timeout { args: shown }),
            Err(error) => Err(GitError::Spawn(error)),
        }
    }
}

/// Output of one git run, kept with its arguments for error reports.
#[derive(Debug)]
pub(crate) struct Captured {
    args: Vec<String>,
    output: Output,
}

impl Captured {
    pub(crate) fn succeeded(&self) -> bool {
        self.output.status.success()
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.output.status.code()
    }

    pub(crate) fn require_success(&self) -> Result<(), GitError> {
        if self.succeeded() {
            Ok(())
        } else {
            Err(self.failure())
        }
    }

    pub(crate) fn failure(&self) -> GitError {
        GitError::Failed {
            args: self.args.clone(),
            code: self.exit_code(),
            stderr: redact_urls(String::from_utf8_lossy(&self.output.stderr).trim()),
        }
    }

    /// Reads git's message text. Only `Git::toplevel` may use it (ADR 0008).
    pub(crate) fn stderr_contains(&self, needle: &str) -> bool {
        String::from_utf8_lossy(&self.output.stderr).contains(needle)
    }

    /// The output stopped at the byte budget of `Git::run_capped`; the process was killed, so its status is not git's.
    pub(crate) fn truncated(&self) -> bool {
        self.output.truncated
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.output.stdout
    }

    pub(crate) fn text(&self) -> Result<&str, GitError> {
        std::str::from_utf8(&self.output.stdout)
            .map_err(|_| self.invalid_output("output is not valid UTF-8"))
    }

    pub(crate) fn invalid_output(&self, reason: impl Into<String>) -> GitError {
        GitError::InvalidOutput {
            args: self.args.clone(),
            reason: reason.into(),
        }
    }
}

/// Replaces the user information of every URL in `text` with `***`, so a credential in a remote URL
/// never reaches a reply, an error or a log (decision 0013).
pub fn redact_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(scheme_end) = rest.find("://") {
        let (before, after) = rest.split_at(scheme_end + 3);
        out.push_str(before);
        let authority_end = after
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(after.len());
        match after[..authority_end].rfind('@') {
            Some(at) => {
                out.push_str("***");
                out.push_str(&after[at..authority_end]);
            }
            None => out.push_str(&after[..authority_end]),
        }
        rest = &after[authority_end..];
    }
    out.push_str(rest);
    out
}

/// Turns one path printed by git into a `PathBuf`. Git uses forward slashes on
/// Windows, which `PathBuf` accepts; `dunce` strips a verbatim prefix if present.
pub(crate) fn path_from_git(text: &str) -> PathBuf {
    dunce::simplified(Path::new(text)).to_path_buf()
}

/// Resolves the executable: a non-empty `MASSORCA_GIT` value, else `git`.
fn resolve_binary(override_binary: Option<OsString>) -> PathBuf {
    match override_binary {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from("git"),
    }
}

/// Builds a `Git` from the override and checks that it runs; a missing binary is `NotFound`.
fn discover_with(override_binary: Option<OsString>) -> Result<Git, GitError> {
    let git = Git::with_binary(resolve_binary(override_binary));
    match git.version() {
        Ok(_) => Ok(git),
        Err(GitError::Spawn(error)) if error.kind() == io::ErrorKind::NotFound => {
            Err(GitError::NotFound)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{GitError, discover_with, redact_urls, resolve_binary};

    #[test]
    fn credentials_in_urls_are_redacted_and_everything_else_is_kept() {
        assert_eq!(
            redact_urls("fatal: unable to access 'https://user:s3cret@host.example/r.git/': 403"),
            "fatal: unable to access 'https://***@host.example/r.git/': 403"
        );
        assert_eq!(
            redact_urls("ssh://git@host/r and https://tok@x.example"),
            "ssh://***@host/r and https://***@x.example"
        );
        assert_eq!(redact_urls("https://host/a@b"), "https://host/a@b");
        assert_eq!(redact_urls("no url here"), "no url here");
    }

    #[test]
    fn the_binary_is_git_unless_overridden() {
        assert_eq!(resolve_binary(None), PathBuf::from("git"));
        assert_eq!(resolve_binary(Some(OsString::new())), PathBuf::from("git"));
        assert_eq!(
            resolve_binary(Some(OsString::from("/opt/git/bin/git"))),
            PathBuf::from("/opt/git/bin/git")
        );
    }

    #[test]
    fn a_missing_binary_is_reported_as_not_found() {
        let error = discover_with(Some(OsString::from("massorca-no-such-git-binary")))
            .expect_err("no such binary");
        assert!(matches!(error, GitError::NotFound), "{error:?}");
    }
}
