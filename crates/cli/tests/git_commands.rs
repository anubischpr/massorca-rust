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

#[test]
fn branch_commands_create_switch_rename_list_and_delete() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);

    let created = sandbox.json_ok(
        "default",
        &["git", "branch-create", "topic", "--switch", "-w", "feature"],
    );
    assert_eq!(created["name"], "topic");
    assert_eq!(created["current"], true);
    sandbox
        .run("default", &["git", "switch", "feature", "-w", "feature"])
        .success();
    sandbox
        .run(
            "default",
            &["git", "branch-rename", "topic", "topic-2", "-w", "feature"],
        )
        .success();

    let listed = sandbox.json_ok("default", &["git", "branches", "-w", "feature"]);
    assert_eq!(listed["branches"][0]["name"], "feature");
    // The project's own branch is `main` or `master`, depending on the developer's `init.defaultBranch`.
    let primary = listed["branches"][1]["name"]
        .as_str()
        .expect("branch name")
        .to_owned();
    assert_eq!(listed["branches"][1]["checked_out_elsewhere"], true);
    assert_eq!(listed["branches"][2]["name"], "topic-2");
    let text = sandbox
        .run("default", &["git", "branches", "-w", "feature"])
        .success();
    let text = stdout_text(&text);
    assert!(text.contains("* feature "), "{text}");
    assert!(
        text.contains(&format!("  {primary} ")) && text.contains("(checked out elsewhere)"),
        "{text}"
    );

    sandbox
        .run(
            "default",
            &["git", "branch-delete", &primary, "-w", "feature"],
        )
        .code(1)
        .stderr(contains("branch.checked_out"));
    let deleted = sandbox.json_ok(
        "default",
        &["git", "branch-delete", "topic-2", "-w", "feature"],
    );
    assert_eq!(deleted["branch"], "topic-2");
    assert!(deleted["commit"].as_str().is_some_and(|c| c.len() >= 40));
    assert!(dir.join("README.md").exists());
}

#[test]
fn push_fetch_and_pull_through_the_cli() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let remote = sandbox.home().join("remote.git");
    let remote_text = remote.to_string_lossy().into_owned();
    harness::git(sandbox.home(), &["init", "-q", "--bare", &remote_text]);
    harness::git(&dir, &["remote", "add", "origin", &remote_text]);

    let pushed = sandbox.json_ok("default", &["git", "push", "-w", "feature"]);
    assert_eq!(pushed["outcome"]["kind"], "created");
    assert_eq!(pushed["upstream_set"], true);
    let text = sandbox
        .run("default", &["git", "fetch", "-w", "feature"])
        .success();
    assert!(stdout_text(&text).contains("already up to date"));
    let pulled = sandbox.json_ok(
        "default",
        &["git", "pull", "-w", "feature", "--mode", "rebase"],
    );
    assert_eq!(pulled["outcome"], "up_to_date");
    sandbox
        .run("default", &["git", "abort", "-w", "feature"])
        .code(1)
        .stderr(contains("git.no_operation"));
}

/// A pull that could not fast-forward and a push the remote refused did not do what was asked: both exit 1 with a
/// typed code, in text and under `--json`, and the JSON error carries the result.
#[test]
fn a_diverged_pull_and_a_rejected_push_exit_1_with_their_codes() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let remote = sandbox.home().join("remote.git");
    let remote_text = remote.to_string_lossy().into_owned();
    harness::git(sandbox.home(), &["init", "-q", "--bare", &remote_text]);
    harness::git(&dir, &["remote", "add", "origin", &remote_text]);
    sandbox.json_ok("default", &["git", "push", "-w", "feature"]);

    let other = sandbox.home().join("other");
    let other_text = other.to_string_lossy().into_owned();
    harness::git(
        sandbox.home(),
        &["clone", "-q", "-b", "feature", &remote_text, &other_text],
    );
    std::fs::write(other.join("theirs.txt"), "theirs").expect("write");
    harness::git(&other, &["add", "theirs.txt"]);
    harness::git(&other, &["commit", "-q", "-m", "theirs"]);
    harness::git(&other, &["push", "-q", "origin", "feature"]);
    std::fs::write(dir.join("ours.txt"), "ours").expect("write");
    harness::git(&dir, &["add", "ours.txt"]);
    harness::git(&dir, &["commit", "-q", "-m", "ours"]);

    sandbox
        .run("default", &["git", "pull", "-w", "feature"])
        .code(1)
        .stderr(contains("diverge"))
        .stderr(contains("git.diverged"));
    let pulled = sandbox
        .run("default", &["--json", "git", "pull", "-w", "feature"])
        .code(1);
    let document: serde_json::Value =
        serde_json::from_str(stdout_text(&pulled).trim()).expect("one JSON document");
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "git.diverged");
    assert_eq!(document["error"]["data"]["outcome"], "diverged");

    sandbox
        .run("default", &["git", "push", "-w", "feature"])
        .code(1)
        .stderr(contains("rejected"))
        .stderr(contains("git.push_rejected"));
    let pushed = sandbox
        .run("default", &["--json", "git", "push", "-w", "feature"])
        .code(1);
    let document: serde_json::Value =
        serde_json::from_str(stdout_text(&pushed).trim()).expect("one JSON document");
    assert_eq!(document["error"]["code"], "git.push_rejected");
    assert_eq!(document["error"]["data"]["outcome"]["kind"], "rejected");
}

#[test]
fn watch_prints_ready_then_the_change_and_stops_after_max_events() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let mut child = sandbox
        .command("default")
        .args([
            "--json",
            "git",
            "watch",
            "-w",
            "feature",
            "--max-events",
            "1",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn watch");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let ready: serde_json::Value =
        serde_json::from_str(&lines.next().expect("a ready line").expect("read")).expect("json");
    assert_eq!(ready["result"]["ready"], true);
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::fs::write(dir.join("new.txt"), "x").expect("write");
    let changed: serde_json::Value =
        serde_json::from_str(&lines.next().expect("a change line").expect("read")).expect("json");
    assert!(
        changed["result"]["kinds"]
            .as_array()
            .is_some_and(|kinds| !kinds.is_empty())
    );
    assert!(child.wait().expect("wait").success());
}

/// An edit made the moment the ready line arrives is reported: the service watches before it says ready.
#[test]
fn watch_reports_an_edit_made_right_after_the_ready_line() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let mut child = sandbox
        .command("default")
        .args(["git", "watch", "-w", "feature", "--max-events", "1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn watch");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let ready = lines.next().expect("a ready line").expect("read");
    assert_eq!(ready, "watching");
    std::fs::write(
        dir.join("README.md"),
        "edited at once
",
    )
    .expect("write");
    let changed = lines.next().expect("a change line").expect("read");
    assert!(changed.starts_with("changed: "), "{changed}");
    assert!(changed.contains("worktree"), "{changed}");
    assert!(child.wait().expect("wait").success());
}

/// A watch whose service stops says its view is unverifiable, comes back when the service does, and keeps watching.
#[test]
fn watch_survives_a_service_restart_and_reports_the_gap() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let mut child = sandbox
        .command("default")
        .args([
            "--json",
            "git",
            "watch",
            "-w",
            "feature",
            "--max-events",
            "1",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn watch");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut next = move || -> serde_json::Value {
        let line = lines.next().expect("a line").expect("read");
        serde_json::from_str(&line).expect("json")
    };
    assert_eq!(next()["result"]["ready"], true);

    sandbox.run("default", &["service", "stop"]).success();
    let lost = next();
    assert_eq!(lost["result"]["link"], "unverifiable", "{lost}");
    assert_eq!(lost["result"]["attempt"], 1, "{lost}");

    // The watch never starts the service itself; this command does.
    sandbox.json_ok("default", &["workspace", "list"]);
    let back = loop {
        let item = next();
        if item["result"]["link"] != "unverifiable" {
            break item;
        }
    };
    assert_eq!(back["result"]["link"], "live", "{back}");
    assert_eq!(next()["result"]["ready"], true, "a new watch says ready");

    std::fs::write(dir.join("after-restart.txt"), "x").expect("write");
    let changed = next();
    assert!(
        changed["result"]["kinds"]
            .as_array()
            .is_some_and(|kinds| !kinds.is_empty()),
        "{changed}"
    );
    assert!(child.wait().expect("wait").success());
}

#[test]
fn patch_of_a_submodule_shows_its_pointer_in_text_and_json() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let library = sandbox.home().join("lib");
    git_repo(&library);
    std::fs::write(library.join("lib.txt"), "two\n").expect("write");
    harness::git(&library, &["add", "lib.txt"]);
    harness::git(&library, &["commit", "-q", "-m", "lib two"]);
    let second = harness::git(&library, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let first = harness::git(&library, &["rev-parse", "HEAD~1"])
        .trim()
        .to_owned();
    let library_text = library.to_string_lossy().into_owned();
    harness::git(
        &dir,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            &library_text,
            "vendor/lib",
        ],
    );
    harness::git(&dir, &["commit", "-q", "-m", "add the library"]);
    harness::git(&dir.join("vendor/lib"), &["checkout", "-q", &first]);
    let summary = format!("Subproject commit {second}..{first}");

    let text = sandbox
        .run("default", &["git", "patch", "vendor/lib", "-w", "feature"])
        .success();
    let text = stdout_text(&text);
    assert!(
        text.contains(&format!("submodule vendor/lib: {summary}")),
        "{text}"
    );
    assert!(text.contains("working tree clean"), "{text}");

    let json = sandbox.json_ok("default", &["git", "patch", "vendor/lib", "-w", "feature"]);
    assert_eq!(json["submodule"]["summary"], summary);
    assert_eq!(json["submodule"]["dirty"], false);
    assert!(json.get("patch").is_none(), "{json}");
}
