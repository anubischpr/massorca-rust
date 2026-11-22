//! `Git::commit` against real repositories: commits, amends and every refusal of decision 0013.

use std::path::Path;

use massorca_core_git::{CommitError, CommitRequest, Git, GitError, failure_code};
use massorca_protocol::codes;

use crate::fixture::{Fixture, commit_file, rev, run_git, text};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

/// A fixture whose commits through `Git` use a repository-local identity, whatever the global config says.
fn repo_with_identity() -> Fixture {
    let fixture = Fixture::new();
    run_git(&fixture.repo(), &["config", "user.name", "t"]);
    run_git(
        &fixture.repo(),
        &["config", "user.email", "t@example.invalid"],
    );
    run_git(&fixture.repo(), &["config", "commit.gpgsign", "false"]);
    fixture
}

fn request(message: Option<&str>, amend: bool) -> CommitRequest<'_> {
    CommitRequest {
        message,
        amend,
        allow_published: false,
    }
}

fn refusal(error: CommitError) -> &'static str {
    match error {
        CommitError::Refused { code, .. } => code,
        CommitError::Git(error) => panic!("expected a refusal, got {error}"),
    }
}

fn stage(repo: &Path, name: &str, contents: &str) {
    std::fs::write(repo.join(name), contents).expect("write");
    run_git(repo, &["add", "--", name]);
}

#[test]
fn a_commit_records_the_staged_set_and_reports_it() {
    let fixture = repo_with_identity();
    let repo = fixture.repo();
    stage(&repo, "a.txt", "a\n");
    std::fs::write(repo.join("unstaged.txt"), "not staged\n").expect("write");

    let committed = git()
        .commit(&repo, request(Some("add a\n\nbody"), false))
        .expect("commit");
    assert_eq!(committed.id, rev(&repo, "HEAD"));
    assert_eq!(committed.branch.as_deref(), Some("main"));
    assert_eq!(committed.subject, "add a");
    assert_eq!(committed.replaced, None);
    let shown = run_git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(shown.trim(), "a.txt");
    assert!(!repo.join(".git").join("MASSORCA_COMMIT_MSG").exists());
}

#[test]
fn nothing_staged_and_an_empty_message_are_refused() {
    let fixture = repo_with_identity();
    let repo = fixture.repo();
    let error = git()
        .commit(&repo, request(Some("msg"), false))
        .expect_err("nothing staged");
    assert_eq!(refusal(error), codes::GIT_NOTHING_TO_COMMIT);
    stage(&repo, "a.txt", "a\n");
    let error = git()
        .commit(&repo, request(Some("  \n"), false))
        .expect_err("empty");
    assert_eq!(refusal(error), codes::GIT_COMMIT_MESSAGE_EMPTY);
    let error = git()
        .commit(&repo, request(None, false))
        .expect_err("no message");
    assert_eq!(refusal(error), codes::GIT_COMMIT_MESSAGE_EMPTY);
}

#[test]
fn amend_replaces_head_and_keeps_its_message_when_none_is_given() {
    let fixture = repo_with_identity();
    let repo = fixture.repo();
    commit_file(&repo, "a.txt", "a\n", "first try");
    let before = rev(&repo, "HEAD");
    stage(&repo, "b.txt", "b\n");

    let amended = git().commit(&repo, request(None, true)).expect("amend");
    assert_eq!(amended.replaced.as_deref(), Some(before.as_str()));
    assert_eq!(amended.subject, "first try");
    assert_eq!(rev(&repo, "HEAD~1"), rev(&repo, &format!("{before}~1")));

    let renamed = git()
        .commit(&repo, request(Some("better words"), true))
        .expect("amend");
    assert_eq!(renamed.subject, "better words");
}

#[test]
fn amending_a_published_commit_needs_the_waiver_and_an_empty_history_has_nothing_to_amend() {
    let fixture = repo_with_identity();
    let repo = fixture.repo();
    let remote = fixture.sibling("remote.git");
    run_git(
        &repo,
        &["clone", "-q", "--bare", text(&repo), text(&remote)],
    );
    run_git(&repo, &["remote", "add", "origin", text(&remote)]);
    run_git(&repo, &["fetch", "-q", "origin"]);

    let error = git()
        .commit(&repo, request(Some("rewrite"), true))
        .expect_err("published");
    assert_eq!(refusal(error), codes::GIT_COMMIT_PUBLISHED);
    let waived = CommitRequest {
        message: Some("rewrite"),
        amend: true,
        allow_published: true,
    };
    assert_eq!(
        git().commit(&repo, waived).expect("amend").subject,
        "rewrite"
    );

    let empty = tempfile::tempdir().expect("temp dir");
    run_git(empty.path(), &["init", "-q"]);
    let error = git()
        .commit(empty.path(), request(Some("x"), true))
        .expect_err("no commits");
    assert_eq!(refusal(error), codes::GIT_NO_COMMITS);
}

#[test]
fn a_failing_hook_fails_the_commit_with_its_message() {
    let fixture = repo_with_identity();
    let repo = fixture.repo();
    let hooks = repo.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).expect("hooks dir");
    std::fs::write(
        hooks.join("pre-commit"),
        "#!/bin/sh\necho blocked by hook >&2\nexit 1\n",
    )
    .expect("hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            hooks.join("pre-commit"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("chmod");
    }
    run_git(&repo, &["config", "core.hooksPath", text(&hooks)]);
    stage(&repo, "a.txt", "a\n");
    let error = git()
        .commit(&repo, request(Some("msg"), false))
        .expect_err("the hook blocks it");
    match error {
        CommitError::Git(GitError::Failed { stderr, .. }) => {
            assert!(stderr.contains("blocked by hook"), "{stderr}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_identity_is_reported_as_such() {
    let fixture = repo_with_identity();
    let repo = fixture.repo();
    run_git(&repo, &["config", "user.name", ""]);
    stage(&repo, "a.txt", "a\n");
    let error = git()
        .commit(&repo, request(Some("msg"), false))
        .expect_err("no identity");
    match error {
        CommitError::Git(error) => assert_eq!(
            failure_code(&error),
            Some(codes::GIT_IDENTITY_UNKNOWN),
            "{error}"
        ),
        other => panic!("{other:?}"),
    }
}
