//! Real repositories for the tests. A fixture owns one temporary directory,
//! removed on drop, that holds the main worktree and room for linked ones.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub struct Fixture {
    root: TempDir,
}

impl Fixture {
    /// A repository on `main` with one commit containing `README.md`.
    pub fn new() -> Self {
        let fixture = Self {
            root: tempfile::tempdir().expect("create temp dir"),
        };
        let repo = fixture.repo();
        std::fs::create_dir_all(&repo).expect("create repo dir");
        run_git(&repo, &["init", "-q"]);
        run_git(&repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        run_git(&repo, &["config", "core.autocrlf", "false"]);
        commit_file(&repo, "README.md", "hello\n", "initial commit");
        fixture
    }

    /// The main worktree.
    pub fn repo(&self) -> PathBuf {
        self.root.path().join("repo")
    }

    /// A directory beside the main worktree, for linked worktrees.
    pub fn sibling(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
}

/// Runs git in `dir` with a fixed identity and returns its stdout; panics on failure.
pub fn run_git(dir: &Path, args: &[&str]) -> String {
    let output = massorca_process::command("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git output is UTF-8")
}

/// Writes `name` in `repo` and commits it on the current branch.
pub fn commit_file(repo: &Path, name: &str, contents: &str, message: &str) {
    std::fs::write(repo.join(name), contents).expect("write file");
    run_git(repo, &["add", "--", name]);
    // A global hooks directory must not run during test commits.
    let hooks = format!(
        "core.hooksPath={}",
        repo.join(".git").join("hooks").display()
    );
    run_git(repo, &["-c", hooks.as_str(), "commit", "-q", "-m", message]);
}

/// Creates `branch` with one commit adding `file`, then returns to `main`.
pub fn commit_on_branch(repo: &Path, branch: &str, file: &str) {
    run_git(repo, &["checkout", "-q", "-b", branch]);
    commit_file(repo, file, "work\n", &format!("work on {branch}"));
    run_git(repo, &["checkout", "-q", "main"]);
}

/// The full commit id a revision resolves to.
pub fn rev(dir: &Path, revision: &str) -> String {
    run_git(dir, &["rev-parse", revision]).trim().to_owned()
}

/// The path with symlinks, short Windows names and verbatim prefixes resolved.
pub fn canonical(path: &Path) -> PathBuf {
    dunce::canonicalize(path).expect("canonicalize test path")
}

/// `path` as UTF-8 text for a git argument.
pub fn text(path: &Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

/// Writes an executable a test then runs; see `massorca_process::test_executable`.
pub use massorca_process::test_executable::write_executable;

/// Installs a `post-checkout` hook running `body` under `sh`, overriding any global hooks path.
pub fn install_post_checkout_hook(repo: &Path, body: &str) {
    let hooks = repo.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).expect("create hooks dir");
    let hook = hooks.join("post-checkout");
    write_executable(&hook, &format!("#!/bin/sh\n{body}\n"));
    run_git(repo, &["config", "core.hooksPath", text(&hooks)]);
}

/// Removes the hook installed by `install_post_checkout_hook`.
pub fn remove_post_checkout_hook(repo: &Path) {
    std::fs::remove_file(repo.join(".git").join("hooks").join("post-checkout"))
        .expect("remove hook");
}
