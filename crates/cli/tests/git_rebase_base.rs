//! `massorca git rebase-base -w <WS> [--refresh]`: a worktree's branch rebased onto its base, the outcomes, and the refusals.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, git, git_repo};
use predicates::str::contains;

/// A git project at `<home>/repo` with a workspace `feature`; returns the workspace directory.
fn workspace(sandbox: &Sandbox) -> PathBuf {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    // The service runs `git rebase` without the harness identity, so the repository needs its own.
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = sandbox.home().join("ws").to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            &repo_text,
            "--workspaces-root",
            &root_text,
        ],
    );
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "feature"]);
    PathBuf::from(created["path"].as_str().expect("workspace path"))
}

#[test]
fn rebase_base_moves_the_branch_onto_the_advanced_base_then_is_up_to_date() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let repo = sandbox.home().join("repo");
    std::fs::write(dir.join("feature.txt"), "feature\n").expect("write feature file");
    git(&dir, &["add", "feature.txt"]);
    git(&dir, &["commit", "-q", "-m", "feature work"]);
    std::fs::write(repo.join("main.txt"), "main\n").expect("write main file");
    git(&repo, &["add", "main.txt"]);
    git(&repo, &["commit", "-q", "-m", "main moves on"]);
    let base_tip = git(&repo, &["rev-parse", "HEAD"]).trim().to_owned();

    let rebased = sandbox.json_ok("default", &["git", "rebase-base", "-w", "feature"]);

    let head = git(&dir, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(rebased["outcome"], "rebased", "{rebased}");
    assert_eq!(rebased["head"], head.as_str(), "{rebased}");
    assert_eq!(git(&dir, &["rev-parse", "HEAD~1"]).trim(), base_tip);

    let again = sandbox.json_ok(
        "default",
        &["git", "rebase-base", "-w", "feature", "--refresh"],
    );
    assert_eq!(again["outcome"], "up_to_date", "{again}");
    sandbox
        .run("default", &["git", "rebase-base", "-w", "feature"])
        .success()
        .stdout(contains("already up to date"));
}

#[test]
fn a_dirty_worktree_is_refused_before_git_runs() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    std::fs::write(dir.join("loose.txt"), "not committed\n").expect("write loose file");
    let head_before = git(&dir, &["rev-parse", "HEAD"]).trim().to_owned();

    sandbox
        .run("default", &["git", "rebase-base", "-w", "feature"])
        .code(1)
        .stderr(contains("workspace.dirty"));
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]).trim(), head_before);
}

#[test]
fn a_conflicting_rebase_exits_1_names_the_paths_and_leaves_the_worktree_as_it_was() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let repo = sandbox.home().join("repo");
    std::fs::write(dir.join("README.md"), "from feature\n").expect("write README");
    git(&dir, &["commit", "-q", "-am", "feature change"]);
    std::fs::write(repo.join("README.md"), "from main\n").expect("write README");
    git(&repo, &["commit", "-q", "-am", "main change"]);
    let head_before = git(&dir, &["rev-parse", "HEAD"]).trim().to_owned();

    sandbox
        .run("default", &["git", "rebase-base", "-w", "feature"])
        .code(1)
        .stderr(contains("git.conflicts"))
        .stderr(contains("README.md"));

    assert_eq!(git(&dir, &["rev-parse", "HEAD"]).trim(), head_before);
    // Git may write the file back with CRLF endings (core.autocrlf), so the line endings are not compared.
    assert_eq!(
        std::fs::read_to_string(dir.join("README.md"))
            .expect("read README")
            .replace("\r\n", "\n"),
        "from feature\n"
    );
}
