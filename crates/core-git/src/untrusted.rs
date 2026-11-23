//! Git runs for a caller without `workspace` permission (decision 0078): no program the repository's own
//! configuration names may start. Programs named in the system or global configuration are the owner's choice and
//! keep running, so a Git LFS filter set up there still applies.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::GitError;

/// Settings that start a program on a read, overridden on every untrusted run. All exist in Git 2.25; an empty
/// `core.fsmonitor` turns the monitor off both there (a hook path) and in 2.36+ (a boolean or a path). The
/// transports are closed too: a read that needs a missing object fetches it, and a transport (`ext::`, an
/// `uploadpack`, an `sshCommand`) can name a program.
const FIXED_OVERRIDES: [&str; 11] = [
    "core.fsmonitor=",
    "diff.external=",
    "credential.helper=",
    "log.showSignature=false",
    "protocol.allow=never",
    "protocol.ext.allow=never",
    "protocol.file.allow=never",
    "protocol.ssh.allow=never",
    "protocol.git.allow=never",
    "protocol.http.allow=never",
    "protocol.https.allow=never",
];

/// Repository settings each run must see: drivers that name a program (overridden with an empty value), per-protocol
/// permissions (overridden with `never`, since one beats `protocol.allow`) and the marks of a partial clone
/// (refused). `hook.*.command` is the configured hook of newer git; overriding it is harmless where git lacks it.
const REPOSITORY_KEYS: &str = r"^((filter|diff)\..+\.(clean|smudge|process|textconv|command)|hook\..+\.command|protocol\..+\.allow|extensions\.partialclone|remote\..+\.promisor)$";

/// Inherited variables that would name a program or turn the monitor back on.
pub(crate) const REMOVED_ENV: [&str; 3] = ["GIT_TEST_FSMONITOR", "GIT_EXTERNAL_DIFF", "GIT_PAGER"];

/// Turns lazy fetching off where git knows the variable (2.44+); older git relies on the refusal of partial clones.
pub(crate) const ADDED_ENV: [(&str, &str); 1] = [("GIT_NO_LAZY_FETCH", "1")];

/// Subcommands that look inside submodules, where only `--ignore-submodules=dirty` keeps git from running a
/// `git status` under the submodule's own configuration.
const SUBMODULE_READERS: [&str; 3] = ["status", "diff-files", "diff-index"];

/// The folder `core.hooksPath` points at: it must exist and be empty, so no hook can run.
#[derive(Debug, Clone)]
pub(crate) struct EmptyHooks(PathBuf);

impl EmptyHooks {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn check(&self) -> Result<&Path, GitError> {
        let empty = std::fs::read_dir(&self.0)
            .and_then(|mut entries| entries.next().transpose())
            .map(|first| first.is_none());
        match empty {
            Ok(true) => Ok(&self.0),
            Ok(false) => Err(refused(format!(
                "the empty hooks folder {} is not empty",
                self.0.display()
            ))),
            Err(error) => Err(refused(format!(
                "the empty hooks folder {} cannot be read: {error}",
                self.0.display()
            ))),
        }
    }
}

/// The global arguments that go before every untrusted run: no pager, the fixed overrides, the hooks folder and
/// one override for each setting `repository_keys` lists. A partial clone is refused, because its reads can fetch.
pub(crate) fn global_args(
    hooks: &EmptyHooks,
    repository_keys: &str,
) -> Result<Vec<OsString>, GitError> {
    let mut args: Vec<OsString> = vec!["--no-pager".into()];
    for setting in FIXED_OVERRIDES {
        args.extend(["-c".into(), setting.into()]);
    }
    let mut hooks_path = OsString::from("core.hooksPath=");
    hooks_path.push(hooks.check()?);
    args.extend(["-c".into(), hooks_path]);
    for (key, value) in parse_keys(repository_keys)? {
        if key == "extensions.partialclone" || (key.ends_with(".promisor") && !is_false(value)) {
            return Err(refused(format!(
                "the repository is a partial clone (`{key}`), whose reads can fetch from a remote"
            )));
        }
        if key.ends_with(".promisor") {
            continue;
        }
        if key.starts_with("protocol.") {
            args.extend(["-c".into(), format!("{key}=never").into()]);
            continue;
        }
        args.extend(["-c".into(), format!("{key}=").into()]);
        // A required filter whose command is empty fails the read, so the filter is made optional.
        if let Some((driver, _)) = key
            .strip_prefix("filter.")
            .and_then(|rest| rest.rsplit_once('.'))
        {
            args.extend([
                "-c".into(),
                format!("filter.{driver}.required=false").into(),
            ]);
        }
    }
    Ok(args)
}

/// Adds `--ignore-submodules=dirty` after the subcommand of a command that would look inside submodules.
pub(crate) fn command_args(args: &mut Vec<OsString>) {
    let Some(at) = args
        .iter()
        .position(|arg| !arg.to_string_lossy().starts_with('-'))
    else {
        return;
    };
    if SUBMODULE_READERS.iter().any(|name| args[at] == *name) {
        args.insert(at + 1, "--ignore-submodules=dirty".into());
    }
}

/// The arguments of the `git config` query that lists the repository's settings above, in one file scope.
pub(crate) fn config_query(scope: &'static str) -> [&'static str; 6] {
    [
        "config",
        scope,
        "--includes",
        "--null",
        "--get-regexp",
        REPOSITORY_KEYS,
    ]
}

/// Whether the repository reads a per-worktree configuration file as well (`extensions.worktreeConfig`).
pub(crate) const WORKTREE_CONFIG_QUERY: [&str; 4] =
    ["config", "--local", "--bool", "extensions.worktreeConfig"];

/// Reads `git config --null --get-regexp` output: `key\nvalue\0` records, or `key\0` for a bare key. A key that
/// cannot be passed back through `-c key=` unchanged refuses the run.
fn parse_keys(output: &str) -> Result<Vec<(&str, &str)>, GitError> {
    output
        .split('\0')
        .filter(|record| !record.is_empty())
        .map(|record| {
            let (key, value) = record.split_once('\n').unwrap_or((record, "true"));
            if key.contains('=') {
                Err(refused(format!(
                    "the repository configuration names a driver `{key}` that cannot be overridden"
                )))
            } else {
                Ok((key, value))
            }
        })
        .collect()
}

/// Git's spellings of false. Anything else counts as true, so an unclear value refuses rather than runs.
fn is_false(value: &str) -> bool {
    ["false", "no", "off", "0"].contains(&value.trim().to_ascii_lowercase().as_str())
}

fn refused(reason: String) -> GitError {
    GitError::UnsafeForCaller { reason }
}

/// Maps a failed config query: exit 1 means nothing matched; anything else refuses the run.
pub(crate) fn config_failure(code: Option<i32>, stderr: &[u8]) -> Option<GitError> {
    (code != Some(1)).then(|| {
        refused(format!(
            "the repository configuration could not be read: {}",
            String::from_utf8_lossy(stderr).trim()
        ))
    })
}

/// A spawn error of the config query, reported as the refusal it causes.
pub(crate) fn spawn_failure(error: &io::Error) -> GitError {
    refused(format!(
        "the repository configuration could not be read: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{EmptyHooks, command_args, global_args, parse_keys};
    use crate::error::GitError;

    #[test]
    fn keys_are_read_from_null_records_and_a_key_with_an_equals_sign_is_refused() {
        let keys =
            parse_keys("filter.lfs.process\ngit-lfs\0diff.Evil.textconv\nx\0remote.a.promisor\0")
                .expect("keys");
        assert_eq!(
            keys,
            [
                ("filter.lfs.process", "git-lfs"),
                ("diff.Evil.textconv", "x"),
                ("remote.a.promisor", "true")
            ]
        );
        assert!(parse_keys("").expect("none").is_empty());
        let refused = parse_keys("filter.a=b.clean\nx\0").expect_err("refused");
        assert!(
            matches!(refused, GitError::UnsafeForCaller { .. }),
            "{refused:?}"
        );
    }

    #[test]
    fn protocols_are_closed_and_partial_clones_are_refused() {
        let folder = tempfile::tempdir().expect("hooks");
        let hooks = EmptyHooks::new(folder.path().to_path_buf());
        let args = global_args(
            &hooks,
            "protocol.ext.allow\nalways\0remote.origin.promisor\nfalse\0",
        )
        .expect("args");
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        for expected in ["protocol.allow=never", "protocol.ext.allow=never"] {
            assert!(args.iter().any(|arg| arg == expected), "{args:?}");
        }
        for keys in [
            "extensions.partialclone\norigin\0",
            "remote.origin.promisor\ntrue\0",
            "remote.origin.promisor\0",
        ] {
            let error = global_args(&hooks, keys).expect_err("refused");
            assert!(
                matches!(error, GitError::UnsafeForCaller { .. }),
                "{keys}: {error:?}"
            );
        }
    }

    #[test]
    fn submodule_readers_get_the_flag_after_the_subcommand_and_others_do_not() {
        let mut status: Vec<OsString> = vec!["status".into(), "-z".into()];
        command_args(&mut status);
        assert_eq!(status, ["status", "--ignore-submodules=dirty", "-z"]);
        let mut diff: Vec<OsString> = vec!["--literal-pathspecs".into(), "diff-files".into()];
        command_args(&mut diff);
        assert_eq!(
            diff,
            [
                "--literal-pathspecs",
                "diff-files",
                "--ignore-submodules=dirty"
            ]
        );
        let mut log: Vec<OsString> = vec!["log".into(), "-z".into()];
        command_args(&mut log);
        assert_eq!(log, ["log", "-z"]);
    }
}
