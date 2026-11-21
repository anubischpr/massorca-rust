//! Fetch, pull, push and abort against real local remotes, and the typed failures of decision 0013.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_core_git::{FetchTarget, Git, GitError, RemoteError, failure_code};
use massorca_protocol::codes;
use massorca_protocol::methods::git::remote::{GitPulled, PullMode, PushOutcome};
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

/// Whether this git can speak HTTP. A git built with `NO_CURL` (the Git 2.25.5 build in CI) cannot, and the
/// HTTP tests then say so on stderr instead of failing on a transport that does not exist.
#[expect(
    clippy::print_stderr,
    reason = "a skipped test must name why it skipped"
)]
fn has_http_transport() -> bool {
    let exec_path = std::process::Command::new("git")
        .arg("--exec-path")
        .output()
        .expect("git --exec-path");
    let dir = PathBuf::from(String::from_utf8_lossy(&exec_path.stdout).trim());
    let present = dir.join("git-remote-http").exists() || dir.join("git-remote-http.exe").exists();
    if !present {
        eprintln!("skipped: this git has no HTTP transport (built with NO_CURL)");
    }
    present
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

fn git_failure(error: RemoteError) -> GitError {
    match error {
        RemoteError::Git(error) => error,
        RemoteError::Refused { code, message } => {
            panic!("expected a git failure, got {code}: {message}")
        }
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

#[test]
fn push_creates_the_upstream_then_fast_forwards_and_reports_rejections() {
    let remotes = Remotes::new();
    let repo = remotes.repo();
    run_git(&repo, &["checkout", "-q", "-b", "feature"]);
    commit_file(&repo, "f.txt", "f\n", "feature");

    let pushed = git().push(&repo, None, None, TIMEOUT).expect("push");
    assert_eq!(pushed.outcome, PushOutcome::Created);
    assert!(pushed.upstream_set);
    assert_eq!(pushed.remote_branch, "origin/feature");
    commit_file(&repo, "g.txt", "g\n", "more");
    assert_eq!(
        git()
            .push(&repo, None, None, TIMEOUT)
            .expect("push")
            .outcome,
        PushOutcome::FastForward
    );
    assert_eq!(
        git()
            .push(&repo, None, None, TIMEOUT)
            .expect("push")
            .outcome,
        PushOutcome::UpToDate
    );

    // Someone else moves the remote branch; ours no longer fast-forwards it.
    run_git(&remotes.other, &["fetch", "-q", "origin"]);
    run_git(
        &remotes.other,
        &["checkout", "-q", "-b", "feature", "origin/feature"],
    );
    commit_file(&remotes.other, "h.txt", "h\n", "theirs");
    run_git(&remotes.other, &["push", "-q", "origin", "feature"]);
    run_git(&repo, &["commit", "-q", "--amend", "-m", "rewritten"]);
    let rejected = git().push(&repo, None, None, TIMEOUT).expect("push");
    assert!(
        matches!(rejected.outcome, PushOutcome::Rejected { .. }),
        "{rejected:?}"
    );

    let stale = rev(&repo, "origin/feature");
    let lease = git()
        .push(&repo, None, Some(&stale), TIMEOUT)
        .expect("push");
    assert_eq!(
        lease.outcome,
        PushOutcome::Rejected {
            reason: "stale info".to_owned()
        }
    );
    let current = run_git(&remotes.remote, &["rev-parse", "refs/heads/feature"])
        .trim()
        .to_owned();
    let forced = git()
        .push(&repo, None, Some(&current), TIMEOUT)
        .expect("push");
    assert_eq!(forced.outcome, PushOutcome::Forced);
}

#[test]
fn a_detached_head_cannot_push_or_pull_and_a_branch_without_upstream_cannot_pull() {
    let remotes = Remotes::new();
    let repo = remotes.repo();
    run_git(&repo, &["checkout", "-q", "-b", "lonely"]);
    let error = git()
        .pull(&repo, PullMode::FfOnly, TIMEOUT)
        .expect_err("no upstream");
    assert_eq!(refused(error), codes::GIT_NO_UPSTREAM);
    run_git(&repo, &["checkout", "-q", "--detach"]);
    let error = git()
        .push(&repo, None, None, TIMEOUT)
        .expect_err("detached");
    assert_eq!(refused(error), codes::GIT_DETACHED_HEAD);
}

#[test]
fn a_missing_remote_repository_is_reported_as_not_found() {
    let remotes = Remotes::new();
    let repo = remotes.repo();
    let gone = remotes.fixture.sibling("gone.git");
    run_git(&repo, &["remote", "set-url", "origin", text(&gone)]);
    let error = git_failure(
        git()
            .fetch(&repo, FetchTarget::Default, TIMEOUT)
            .expect_err("missing"),
    );
    assert_eq!(
        failure_code(&error),
        Some(codes::GIT_REMOTE_NOT_FOUND),
        "{error}"
    );
}

#[test]
fn a_remote_that_wants_credentials_fails_at_once_without_leaking_them() {
    if !has_http_transport() {
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    let remotes = Remotes::new();
    let repo = remotes.repo();
    let without = format!("http://127.0.0.1:{port}/r.git");
    run_git(&repo, &["remote", "set-url", "origin", &without]);
    let started = std::time::Instant::now();
    let error = git_failure(
        git()
            .fetch(&repo, FetchTarget::Default, TIMEOUT)
            .expect_err("needs auth"),
    );
    assert_eq!(
        failure_code(&error),
        Some(codes::GIT_AUTH_REQUIRED),
        "{error}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the fetch waited for an answer"
    );

    let with = format!("http://user:s3cret-token@127.0.0.1:{port}/r.git");
    run_git(&repo, &["remote", "set-url", "origin", &with]);
    let error = git_failure(
        git()
            .fetch(&repo, FetchTarget::Default, TIMEOUT)
            .expect_err("bad credentials"),
    );
    assert_eq!(
        failure_code(&error),
        Some(codes::GIT_AUTH_REQUIRED),
        "{error}"
    );
    assert!(!error.to_string().contains("s3cret-token"), "{error}");
}

#[test]
fn a_closed_port_is_unreachable() {
    if !has_http_transport() {
        return;
    }
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let remotes = Remotes::new();
    let repo = remotes.repo();
    run_git(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            &format!("http://127.0.0.1:{port}/r.git"),
        ],
    );
    let error = git_failure(
        git()
            .fetch(&repo, FetchTarget::Default, TIMEOUT)
            .expect_err("closed"),
    );
    assert_eq!(
        failure_code(&error),
        Some(codes::GIT_REMOTE_UNREACHABLE),
        "{error}"
    );
}
