//! Reads for a caller without `workspace` permission (decision 0078) start no program the repository's own
//! configuration names. Each planted program appends to a marker file, so a run that started one leaves it behind.

use std::path::{Path, PathBuf};

use massorca_core_git::{Git, GitError, LogCursor, PathSet};
use massorca_protocol::methods::git::diff::DiffTarget;
use tempfile::TempDir;

use crate::fixture::{Fixture, commit_file, rev, run_git};

struct Planted {
    fixture: Fixture,
    hooks: TempDir,
    marker: PathBuf,
}

impl Planted {
    /// A repository whose `.git/config` names a program for every read MassOrca makes: a filesystem monitor, a
    /// required clean filter, a textconv and command diff driver, an external diff, a signature program and hooks.
    fn new() -> Self {
        let fixture = Fixture::new();
        let repo = fixture.repo();
        let marker = fixture.sibling("marker");
        let record = |name: &str| format!("echo {name} >> '{}'", slashes(&marker));
        std::fs::write(
            repo.join(".gitattributes"),
            "*.txt filter=planted diff=planted\n",
        )
        .expect("write");
        commit_file(&repo, "file.txt", "one\n", "add file");
        let hooks_dir = repo.join(".git").join("planted-hooks");
        std::fs::create_dir_all(&hooks_dir).expect("mkdir");
        for hook in ["pre-commit", "post-checkout", "reference-transaction"] {
            std::fs::write(
                hooks_dir.join(hook),
                format!("#!/bin/sh\n{}\n", record(hook)),
            )
            .expect("write hook");
        }
        let settings = [
            ("core.fsmonitor", record("fsmonitor")),
            ("filter.planted.clean", format!("{}; cat", record("clean"))),
            (
                "filter.planted.smudge",
                format!("{}; cat", record("smudge")),
            ),
            ("filter.planted.required", "true".to_owned()),
            (
                "diff.planted.textconv",
                format!("{}; cat", record("textconv")),
            ),
            ("diff.planted.command", record("diff-command")),
            ("diff.external", record("external")),
            ("log.showSignature", "true".to_owned()),
            ("gpg.program", record("gpg")),
            ("core.hooksPath", slashes(&hooks_dir)),
        ];
        for (key, value) in settings {
            run_git(&repo, &["config", key, &value]);
        }
        std::fs::write(repo.join("file.txt"), "one\ntwo\n").expect("write");
        Self {
            fixture,
            hooks: tempfile::tempdir().expect("hooks dir"),
            marker,
        }
    }

    fn repo(&self) -> PathBuf {
        self.fixture.repo()
    }

    fn untrusted(&self) -> Git {
        Git::discover()
            .expect("git is installed")
            .for_untrusted_caller(self.hooks.path())
    }

    fn ran(&self) -> String {
        std::fs::read_to_string(&self.marker).unwrap_or_default()
    }
}

/// Git's shell takes forward slashes on every platform.
fn slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn reads_for_an_untrusted_caller_start_no_program_the_repository_names() {
    let planted = Planted::new();
    let repo = planted.repo();
    let git = planted.untrusted();
    let status = git.status(&repo, 100).expect("status");
    assert!(
        status.entries.iter().any(|entry| entry.path == "file.txt"),
        "{status:?}"
    );
    git.diff(&repo, &DiffTarget::Worktree, 100).expect("diff");
    let patch = git
        .diff_file(&repo, &DiffTarget::Worktree, "file.txt", None, 4096)
        .expect("diff file");
    assert!(
        patch
            .patch
            .as_deref()
            .is_some_and(|text| text.contains("+two")),
        "{patch:?}"
    );
    let cursor = LogCursor {
        start: rev(&repo, "HEAD"),
        skip: 0,
    };
    git.log(&repo, &cursor, 10).expect("log");
    git.branches(&repo).expect("branches");
    assert_eq!(planted.ran(), "", "a planted program ran");
}

#[test]
fn the_same_reads_for_the_owner_still_run_the_repository_filter() {
    let planted = Planted::new();
    Git::discover()
        .expect("git is installed")
        .diff_file(
            &planted.repo(),
            &DiffTarget::Worktree,
            "file.txt",
            None,
            4096,
        )
        .expect("diff file");
    assert!(
        planted.ran().contains("clean"),
        "the owner's run skipped the filter: {:?}",
        planted.ran()
    );
}

#[test]
fn an_untrusted_caller_cannot_change_the_repository() {
    let planted = Planted::new();
    let error = planted
        .untrusted()
        .stage(&planted.repo(), PathSet::All)
        .expect_err("refused");
    assert!(
        matches!(error, GitError::UnsafeForCaller { .. }),
        "{error:?}"
    );
    assert_eq!(planted.ran(), "");
}

#[test]
fn a_hooks_folder_that_is_missing_or_not_empty_refuses_every_untrusted_run() {
    let planted = Planted::new();
    let repo = planted.repo();
    std::fs::write(planted.hooks.path().join("post-checkout"), "#!/bin/sh\n").expect("write");
    let error = planted.untrusted().status(&repo, 100).expect_err("refused");
    assert!(
        matches!(error, GitError::UnsafeForCaller { .. }),
        "{error:?}"
    );
    let missing = Git::discover()
        .expect("git is installed")
        .for_untrusted_caller(planted.fixture.sibling("no-such-folder"));
    let error = missing.status(&repo, 100).expect_err("refused");
    assert!(
        matches!(error, GitError::UnsafeForCaller { .. }),
        "{error:?}"
    );
    assert_eq!(planted.ran(), "");
}

#[test]
fn a_driver_name_that_cannot_be_overridden_refuses_the_run() {
    let planted = Planted::new();
    let repo = planted.repo();
    run_git(&repo, &["config", "filter.a=b.clean", "cat"]);
    let error = planted.untrusted().status(&repo, 100).expect_err("refused");
    assert!(
        matches!(error, GitError::UnsafeForCaller { .. }),
        "{error:?}"
    );
}

#[test]
fn a_linked_worktree_with_its_own_configuration_is_covered() {
    let planted = Planted::new();
    let repo = planted.repo();
    let linked = planted.fixture.sibling("linked");
    run_git(&repo, &["config", "--unset", "filter.planted.clean"]);
    run_git(
        &repo,
        &["worktree", "add", "-q", "--detach", &slashes(&linked)],
    );
    run_git(&repo, &["config", "extensions.worktreeConfig", "true"]);
    let record = format!("echo worktree-clean >> '{}'; cat", slashes(&planted.marker));
    run_git(
        &linked,
        &["config", "--worktree", "filter.planted.clean", &record],
    );
    std::fs::write(linked.join("file.txt"), "one\nthree\n").expect("write");
    // Setting up the worktree ran the planted smudge filter and hook as the owner; only the read below counts.
    let _ = std::fs::remove_file(&planted.marker);
    planted
        .untrusted()
        .diff_file(&linked, &DiffTarget::Worktree, "file.txt", None, 4096)
        .expect("diff file");
    assert_eq!(planted.ran(), "", "a planted program ran");
}

#[test]
fn a_partial_clone_is_refused_so_a_read_cannot_fetch_through_a_repository_transport() {
    let planted = Planted::new();
    let repo = planted.repo();
    let fetch = format!(
        "ext::sh -c \"echo fetch >> '{}'\"",
        slashes(&planted.marker)
    );
    run_git(&repo, &["config", "core.repositoryformatversion", "1"]);
    run_git(&repo, &["config", "extensions.partialClone", "origin"]);
    run_git(&repo, &["config", "remote.origin.url", &fetch]);
    run_git(&repo, &["config", "protocol.ext.allow", "always"]);
    let error = planted.untrusted().status(&repo, 100).expect_err("refused");
    assert!(
        matches!(error, GitError::UnsafeForCaller { .. }),
        "{error:?}"
    );
    assert_eq!(planted.ran(), "");
}
