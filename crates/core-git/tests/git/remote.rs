//! Fetch, pull, push and abort against real local remotes, and the typed failures of decision 0013.

use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_core_git::{FetchTarget, Git, RemoteError};
use massorca_protocol::codes;
use massorca_protocol::methods::git::remote::{GitPulled, PullMode};
use massorca_protocol::methods::git::status::GitOperation;

use crate::fixture::{Fixture, commit_file, rev, run_git, text};

const TIMEOUT: Duration = Duration::from_secs(60);

fn git() -> Git {
    Git::discover().expect("git is installed")
}

/// `repo` on `main` tracking `origin/main` in a bare remote, and a second clone `other` that pushes to it.
struct Remotes {
    fixture: Fixture,
    remote: PathBuf,
    other: PathBuf,
}

impl Remotes {
    fn new() -> Self {
        let fixture = Fixture::new();
        let repo = fixture.repo();
        identity(&repo);
        let remote = fixture.sibling("remote.git");
        run_git(
            &repo,
            &["clone", "-q", "--bare", text(&repo), text(&remote)],
        );
        run_git(&repo, &["remote", "add", "origin", text(&remote)]);
        run_git(&repo, &["fetch", "-q", "origin"]);
        run_git(&repo, &["branch", "-q", "--set-upstream-to=origin/main"]);
        let other = fixture.sibling("other");
        run_git(
            fixture.sibling("").as_path(),
            &["clone", "-q", text(&remote), text(&other)],
        );
        identity(&other);
        Self {
            fixture,
            remote,
            other,
        }
    }

    fn repo(&self) -> PathBuf {
        self.fixture.repo()
    }

    /// Commits `file` in the other clone and pushes it.
    fn advance_remote(&self, file: &str, contents: &str) {
        commit_file(&self.other, file, contents, &format!("remote {file}"));
        run_git(&self.other, &["push", "-q", "origin", "main"]);
    }
}

fn identity(repo: &Path) {
    run_git(repo, &["config", "user.name", "t"]);
    run_git(repo, &["config", "user.email", "t@example.invalid"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);
    run_git(repo, &["config", "core.autocrlf", "false"]);
    // An empty helper list keeps the machine's credential store out of these tests.
    run_git(repo, &["config", "credential.helper", ""]);
}

fn refused(error: RemoteError) -> &'static str {
    match error {
        RemoteError::Refused { code, .. } => code,
        RemoteError::Git(error) => panic!("expected a refusal, got {error}"),
    }
}

#[test]
fn fetch_reports_new_and_pruned_remote_refs() {
    let remotes = Remotes::new();
    remotes.advance_remote("a.txt", "a\n");
    run_git(&remotes.other, &["push", "-q", "origin", "main:topic"]);
    let fetched = git()
        .fetch(&remotes.repo(), FetchTarget::Default, TIMEOUT)
        .expect("fetch");
    let names: Vec<_> = fetched
        .updated
        .iter()
        .map(|change| change.name.as_str())
        .collect();
    assert_eq!(names, ["origin/main", "origin/topic"]);
    assert!(fetched.updated[1].old.is_none());

    run_git(
        &remotes.other,
        &["push", "-q", "origin", "--delete", "topic"],
    );
    let fetched = git()
        .fetch(&remotes.repo(), FetchTarget::Remote("origin"), TIMEOUT)
        .expect("fetch");
    assert_eq!(fetched.updated.len(), 1);
    assert_eq!(fetched.updated[0].name, "origin/topic");
    assert!(
        fetched.updated[0].new.is_none(),
        "the deleted branch is pruned"
    );

    let error = git()
        .fetch(&remotes.repo(), FetchTarget::Remote("nope"), TIMEOUT)
        .expect_err("no such remote");
    assert_eq!(refused(error), codes::GIT_REMOTE_NOT_FOUND);
}

#[test]
fn pull_fast_forwards_and_ff_only_refuses_to_merge_diverged_branches() {
    let remotes = Remotes::new();
    let repo = remotes.repo();
    assert_eq!(
        git().pull(&repo, PullMode::FfOnly, TIMEOUT).expect("pull"),
        GitPulled::UpToDate
    );
    remotes.advance_remote("a.txt", "a\n");
    let before = rev(&repo, "HEAD");
    match git().pull(&repo, PullMode::FfOnly, TIMEOUT).expect("pull") {
        GitPulled::FastForwarded { from, to } => {
            assert_eq!(from, before);
            assert_eq!(to, rev(&repo, "HEAD"));
        }
        other => panic!("{other:?}"),
    }

    remotes.advance_remote("b.txt", "b\n");
    commit_file(&repo, "local.txt", "l\n", "local");
    let head = rev(&repo, "HEAD");
    assert_eq!(
        git().pull(&repo, PullMode::FfOnly, TIMEOUT).expect("pull"),
        GitPulled::Diverged {
            ahead: 1,
            behind: 1
        }
    );
    assert_eq!(rev(&repo, "HEAD"), head, "ff_only changed the branch");
    assert!(matches!(
        git().pull(&repo, PullMode::Merge, TIMEOUT).expect("pull"),
        GitPulled::Merged { .. }
    ));
    assert_eq!(
        run_git(&repo, &["rev-list", "--count", "--merges", "HEAD"]).trim(),
        "1"
    );
}

#[test]
fn pull_rebase_replays_local_commits() {
    let remotes = Remotes::new();
    let repo = remotes.repo();
    remotes.advance_remote("a.txt", "a\n");
    commit_file(&repo, "local.txt", "l\n", "local");
    let pulled = git().pull(&repo, PullMode::Rebase, TIMEOUT).expect("pull");
    assert!(matches!(pulled, GitPulled::Rebased { .. }), "{pulled:?}");
    assert_eq!(
        run_git(&repo, &["rev-list", "--count", "--merges", "HEAD"]).trim(),
        "0"
    );
    assert_eq!(rev(&repo, "HEAD~1"), rev(&repo, "origin/main"));
}

#[test]
fn a_conflicting_pull_reports_the_files_and_abort_undoes_it() {
    let remotes = Remotes::new();
    let repo = remotes.repo();
    remotes.advance_remote("README.md", "theirs\n");
    commit_file(&repo, "README.md", "ours\n", "ours");
    let head = rev(&repo, "HEAD");
    match git().pull(&repo, PullMode::Merge, TIMEOUT).expect("pull") {
        GitPulled::Conflicts { paths } => assert_eq!(paths, ["README.md"]),
        other => panic!("{other:?}"),
    }
    let aborted = git().abort(&repo).expect("abort");
    assert_eq!(aborted.operation, GitOperation::Merge);
    assert_eq!(rev(&repo, "HEAD"), head);
    let error = git().abort(&repo).expect_err("nothing left");
    assert_eq!(refused(error), codes::GIT_NO_OPERATION);
}
