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
