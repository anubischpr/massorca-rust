//! `massorca git ...` against a real service and real repositories.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, execute, git_repo, single_document, stdout_text, with_json};
use predicates::str::contains;

/// A git project at `<home>/repo` with a workspace `feature`; returns the workspace directory.
fn workspace(sandbox: &Sandbox) -> PathBuf {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
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
fn status_reports_changes_as_json_and_text() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    std::fs::write(dir.join("README.md"), "hello\nmore\n").expect("write");
    std::fs::write(dir.join("new file.txt"), "x\n").expect("write");

    let status = sandbox.json_ok("default", &["git", "status", "-w", "feature"]);
    assert_eq!(status["branch"]["head"], "feature");
    assert_eq!(status["entries"][0]["path"], "README.md");
    assert_eq!(status["entries"][0]["unstaged"], "modified");
    assert_eq!(status["entries"][0]["unstaged_lines"]["added"], 1);
    assert_eq!(status["entries"][1]["untracked"], true);

    let text = sandbox
        .run("default", &["git", "status", "--workspace", "feature"])
        .success();
    let text = stdout_text(&text);
    assert!(text.contains("on feature"), "{text}");
    assert!(text.contains(" M README.md  +1 -0"), "{text}");
    assert!(text.contains("?? new file.txt"), "{text}");
}

#[test]
fn status_defaults_to_the_workspace_in_the_current_directory() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let outcome = execute(
        sandbox
            .command("default")
            .current_dir(&dir)
            .args(with_json(&["git", "status"])),
    )
    .success();
    let document = single_document(&outcome);
    assert_eq!(document["result"]["branch"]["head"], "feature");
    assert_eq!(
        document["result"]["entries"].as_array().map(Vec::len),
        Some(0)
    );
}

#[test]
fn status_of_a_folder_workspace_is_refused_with_exit_1() {
    let sandbox = Sandbox::new();
    let folder = sandbox.home().join("plain");
    std::fs::create_dir_all(&folder).expect("mkdir");
    let folder_text = folder.to_string_lossy().into_owned();
    let root_text = sandbox.home().join("ws").to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            &folder_text,
            "--workspaces-root",
            &root_text,
        ],
    );
    sandbox.json_ok("default", &["workspace", "create", "plain", "notes"]);
    sandbox
        .run("default", &["git", "status", "-w", "notes"])
        .code(1)
        .stderr(contains("git.not_a_repository"));
}

#[test]
fn diff_lists_staged_files_and_patch_shows_one() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    std::fs::write(dir.join("README.md"), "hello\nstaged\n").expect("write");
    harness::git(&dir, &["add", "README.md"]);

    let unstaged = sandbox.json_ok("default", &["git", "diff", "-w", "feature"]);
    assert_eq!(unstaged["files"].as_array().map(Vec::len), Some(0));
    let staged = sandbox.json_ok("default", &["git", "diff", "-w", "feature", "--staged"]);
    assert_eq!(staged["files"][0]["path"], "README.md");
    assert_eq!(staged["totals"]["added"], 1);

    let text = sandbox
        .run(
            "default",
            &["git", "patch", "README.md", "-w", "feature", "--staged"],
        )
        .success();
    let text = stdout_text(&text);
    assert!(text.contains("+staged"), "{text}");

    sandbox
        .run("default", &["git", "patch", "../x", "-w", "feature"])
        .code(1)
        .stderr(contains("git.path_outside_worktree"));
}

#[test]
fn log_prints_a_page_and_the_cursor_for_the_next() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    std::fs::write(dir.join("a.txt"), "a").expect("write");
    harness::git(&dir, &["add", "a.txt"]);
    harness::git(&dir, &["commit", "-q", "-m", "second"]);

    let page = sandbox.json_ok("default", &["git", "log", "-w", "feature", "-n", "1"]);
    assert_eq!(page["commits"][0]["subject"], "second");
    let cursor = page["next_cursor"].as_str().expect("a cursor").to_owned();
    let next = sandbox.json_ok(
        "default",
        &["git", "log", "-w", "feature", "--cursor", &cursor],
    );
    assert_eq!(next["commits"].as_array().map(Vec::len), Some(1));
    assert!(next.get("next_cursor").is_none());

    let text = sandbox
        .run("default", &["git", "log", "-w", "feature", "-n", "1"])
        .success();
    assert!(stdout_text(&text).contains("(more: --cursor "));
}

#[test]
fn stage_unstage_and_discard_with_its_waivers() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    std::fs::write(dir.join("README.md"), "edited").expect("write");
    std::fs::write(dir.join("new.txt"), "work").expect("write");

    sandbox
        .run("default", &["git", "stage", "README.md", "-w", "feature"])
        .success();
    let status = sandbox.json_ok("default", &["git", "status", "-w", "feature"]);
    assert_eq!(status["entries"][0]["staged"], "modified");
    sandbox
        .run("default", &["git", "unstage", "--all", "-w", "feature"])
        .success();

    sandbox
        .run("default", &["git", "discard", "--all", "-w", "feature"])
        .code(1)
        .stderr(contains("git.discard_untracked"))
        .stderr(contains("new.txt"));
    let planned = sandbox
        .run(
            "default",
            &[
                "git",
                "discard",
                "--all",
                "--delete-untracked",
                "--dry-run",
                "-w",
                "feature",
            ],
        )
        .success();
    assert!(stdout_text(&planned).contains("would delete new.txt"));
    assert!(dir.join("new.txt").exists());

    sandbox
        .run(
            "default",
            &[
                "git",
                "discard",
                "--all",
                "--delete-untracked",
                "-w",
                "feature",
            ],
        )
        .success();
    assert!(!dir.join("new.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("README.md"))
            .expect("read")
            .trim_end(),
        "hello"
    );
}

#[test]
fn commit_then_amend_through_the_cli() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    harness::git(&dir, &["config", "user.name", "t"]);
    harness::git(&dir, &["config", "user.email", "t@example.invalid"]);
    std::fs::write(dir.join("a.txt"), "a").expect("write");
    sandbox
        .run("default", &["git", "stage", "a.txt", "-w", "feature"])
        .success();

    let text = sandbox
        .run(
            "default",
            &["git", "commit", "-m", "add a", "-w", "feature"],
        )
        .success();
    assert!(
        stdout_text(&text).starts_with("[feature "),
        "{}",
        stdout_text(&text)
    );
    let amended = sandbox.json_ok(
        "default",
        &[
            "git",
            "commit",
            "--amend",
            "-m",
            "add a file",
            "-w",
            "feature",
        ],
    );
    assert_eq!(amended["subject"], "add a file");
    assert!(amended["replaced"].is_string());

    sandbox
        .run(
            "default",
            &["git", "commit", "-m", "again", "-w", "feature"],
        )
        .code(1)
        .stderr(contains("git.nothing_to_commit"));
}
