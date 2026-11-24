//! Workspaces through the CLI: worktrees and folders, branch outcomes, refusals with their exit codes, and profiles.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, execute, git, git_repo, single_document, stdout_text, with_json};
use predicates::str::contains;

/// Adds `<home>/repo` as a git project whose workspaces go under `<home>/ws`.
fn add_repo_project(sandbox: &Sandbox) -> PathBuf {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let root = sandbox.home().join("ws");
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = root.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo_text.as_str(),
            "--workspaces-root",
            root_text.as_str(),
        ],
    );
    repo
}

#[test]
fn a_worktree_is_created_listed_shown_and_removed_with_its_branch() {
    let sandbox = Sandbox::new();
    let repo = add_repo_project(&sandbox);

    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "feature"]);
    assert_eq!(created["kind"], "worktree");
    assert_eq!(created["branch"], "feature");
    assert_eq!(created["state"], "ready");
    let path = PathBuf::from(created["path"].as_str().expect("path"));
    assert!(
        path.join("README.md").is_file(),
        "the worktree holds the project's files"
    );

    let listed = sandbox.json_ok("default", &["workspace", "list", "--project", "repo"]);
    assert_eq!(listed["workspaces"].as_array().map(Vec::len), Some(1));

    let text = sandbox.run("default", &["ws", "list"]).success();
    assert!(
        stdout_text(&text).contains("feature"),
        "table row: {}",
        stdout_text(&text)
    );

    let shown = sandbox.json_ok("default", &["workspace", "show", "feature"]);
    assert_eq!(shown["name"], "feature");
    sandbox
        .run("default", &["workspace", "show", "feature"])
        .success()
        .stdout(contains("branch: feature"));

    let removed = sandbox.json_ok("default", &["workspace", "rm", "feature"]);
    assert_eq!(removed["branch_deleted"], true);
    assert!(!path.exists(), "the worktree directory is deleted");
    sandbox
        .run("default", &["workspace", "list"])
        .success()
        .stdout(contains("no workspaces"));
    assert!(!git(&repo, &["branch", "--list", "feature"]).contains("feature"));
}

#[test]
fn a_folder_workspace_is_registered_listed_and_removed_and_its_directory_stays() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    std::fs::write(notes.join("notes.txt"), "keep me").expect("write notes");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );

    let created = sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    assert_eq!(created["kind"], "folder");
    assert!(created.get("branch").is_none(), "a folder has no branch");

    let listed = sandbox.json_ok("default", &["workspace", "list"]);
    assert_eq!(listed["workspaces"].as_array().map(Vec::len), Some(1));

    let removed = sandbox.json_ok("default", &["workspace", "rm", "main"]);
    assert_eq!(removed["directory_kept"], true);
    assert!(notes.join("notes.txt").is_file(), "the folder's files stay");
    sandbox
        .run("default", &["workspace", "rm", "main"])
        .code(1)
        .stderr(contains("workspace.not_found"));
}

#[test]
fn a_dirty_worktree_is_refused_with_exit_1_and_removed_with_force() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "dirty"]);
    let path = PathBuf::from(created["path"].as_str().expect("path"));
    std::fs::write(path.join("scratch.txt"), "uncommitted").expect("write change");

    let refused = sandbox
        .run("default", &with_json(&["workspace", "rm", "dirty"]))
        .code(1);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "workspace.dirty"
    );
    assert!(
        path.join("scratch.txt").is_file(),
        "a refusal changes nothing"
    );

    let forced = sandbox.json_ok("default", &["workspace", "rm", "dirty", "--force"]);
    assert_eq!(forced["workspace"]["state"], "removing");
    assert!(!path.exists());
}

#[test]
fn a_branch_with_commits_outside_its_base_is_kept_and_the_result_says_why() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "work"]);
    let path = PathBuf::from(created["path"].as_str().expect("path"));
    std::fs::write(path.join("change.txt"), "a commit").expect("write file");
    git(&path, &["add", "change.txt"]);
    git(&path, &["commit", "-q", "-m", "work"]);

    let removed = sandbox.json_ok("default", &["workspace", "rm", "work"]);

    assert_eq!(removed["branch_deleted"], false);
    assert_eq!(removed["branch_kept_reason"], "not merged");
}

#[test]
fn an_existing_branch_is_checked_out_and_never_deleted() {
    let sandbox = Sandbox::new();
    let repo = add_repo_project(&sandbox);
    git(&repo, &["branch", "keep-me"]);

    let created = sandbox.json_ok(
        "default",
        &[
            "workspace",
            "create",
            "repo",
            "keep-me",
            "--existing-branch",
        ],
    );
    assert_eq!(created["created_branch"], false);

    let removed = sandbox.json_ok("default", &["workspace", "rm", "keep-me"]);
    assert_eq!(removed["branch_deleted"], false);
    assert!(removed.get("branch_kept_reason").is_none());
    assert!(git(&repo, &["branch", "--list", "keep-me"]).contains("keep-me"));
}

#[test]
fn a_project_with_workspaces_cannot_be_removed_until_they_are_gone() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    sandbox.json_ok("default", &["workspace", "create", "repo", "busy"]);

    let refused = sandbox
        .run("default", &with_json(&["project", "rm", "repo"]))
        .code(1);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "project.has_workspaces"
    );

    sandbox.json_ok("default", &["workspace", "rm", "busy"]);
    sandbox.json_ok("default", &["project", "rm", "repo"]);
}

fn path_text(path: &Path) -> String {
    path.to_str().expect("utf-8 path").to_owned()
}

#[test]
fn refresh_base_starts_the_workspace_from_the_remote_branch() {
    let sandbox = Sandbox::new();
    let seed = sandbox.home().join("seed");
    git_repo(&seed);
    let origin = sandbox.home().join("origin.git");
    let (seed_text, origin_text) = (path_text(&seed), path_text(&origin));
    git(
        sandbox.home(),
        &["clone", "-q", "--bare", &seed_text, &origin_text],
    );
    let repo = sandbox.home().join("repo");
    let (repo_text, root_text) = (path_text(&repo), path_text(&sandbox.home().join("ws")));
    git(sandbox.home(), &["clone", "-q", &origin_text, &repo_text]);
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo_text.as_str(),
            "--workspaces-root",
            root_text.as_str(),
        ],
    );
    let local = git(&repo, &["rev-parse", "HEAD"]).trim().to_owned();

    let other = sandbox.home().join("other");
    git(
        sandbox.home(),
        &["clone", "-q", &origin_text, &path_text(&other)],
    );
    std::fs::write(other.join("next.txt"), "next\n").expect("write file");
    git(&other, &["add", "next.txt"]);
    git(&other, &["commit", "-q", "-m", "next"]);
    git(&other, &["push", "-q", "origin", "HEAD"]);
    let remote = git(&origin, &["rev-parse", "HEAD"]).trim().to_owned();

    let plain = sandbox.json_ok("default", &["workspace", "create", "repo", "plain"]);
    let plain_path = plain["path"].as_str().expect("the path is text");
    assert_eq!(
        git(Path::new(plain_path), &["rev-parse", "HEAD"]).trim(),
        local
    );

    let fresh = sandbox.json_ok(
        "default",
        &["workspace", "create", "repo", "fresh", "--refresh-base"],
    );
    let fresh_path = fresh["path"].as_str().expect("the path is text");
    assert_eq!(
        git(Path::new(fresh_path), &["rev-parse", "HEAD"]).trim(),
        remote
    );
}

#[test]
fn a_repeated_request_id_returns_the_first_workspace() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let first = sandbox.json_ok(
        "default",
        &[
            "workspace",
            "create",
            "repo",
            "once",
            "--request-id",
            "req-1",
        ],
    );

    let again = sandbox.json_ok(
        "default",
        &[
            "workspace",
            "create",
            "repo",
            "twice",
            "--request-id",
            "req-1",
        ],
    );

    assert_eq!(again["id"], first["id"]);
    let listed = sandbox.json_ok("default", &["workspace", "list"]);
    assert_eq!(listed["workspaces"].as_array().map(Vec::len), Some(1));
}

#[test]
fn two_profiles_in_one_home_keep_their_projects_apart() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let repo_text = repo.to_string_lossy().into_owned();
    let root = sandbox.home().join("ws").to_string_lossy().into_owned();
    sandbox
        .run(
            "alpha",
            &[
                "project",
                "add",
                repo_text.as_str(),
                "--workspaces-root",
                root.as_str(),
            ],
        )
        .success();

    sandbox
        .run("beta", &["project", "list"])
        .success()
        .stdout(contains("no projects"));
    sandbox
        .run("alpha", &["project", "list"])
        .success()
        .stdout(contains("repo"));
}

#[test]
fn a_relative_workspace_path_is_taken_from_the_working_directory() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let mut command = sandbox.command("default");
    command.current_dir(sandbox.home());

    let outcome = harness::execute(command.args([
        "--json",
        "workspace",
        "create",
        "repo",
        "placed",
        "--path",
        "elsewhere",
    ]))
    .success();

    let created = single_document(&outcome)["result"].clone();
    assert!(
        created["path"]
            .as_str()
            .is_some_and(|path| Path::new(path).ends_with("elsewhere")),
        "absolute path: {created}"
    );
}

#[test]
fn a_dot_selector_names_the_worktree_the_command_runs_in() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "here"]);
    let path = PathBuf::from(created["path"].as_str().expect("path"));

    let mut command = sandbox.command("default");
    command.current_dir(&path);
    let shown = execute(command.args(["--json", "workspace", "show", "."])).success();

    assert_eq!(single_document(&shown)["result"]["name"], "here");
}

#[test]
fn ignored_files_need_discard_ignored_on_the_command_line() {
    let sandbox = Sandbox::new();
    let repo = add_repo_project(&sandbox);
    std::fs::write(
        repo.join(".gitignore"),
        "*.env
",
    )
    .expect("write ignore rules");
    git(&repo, &["add", ".gitignore"]);
    git(&repo, &["commit", "-q", "-m", "ignore env files"]);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "ign"]);
    let path = PathBuf::from(created["path"].as_str().expect("path"));
    std::fs::write(path.join("secrets.env"), "key").expect("write ignored file");

    let refused = sandbox
        .run(
            "default",
            &with_json(&["workspace", "rm", "ign", "--force"]),
        )
        .code(1);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "workspace.ignored_files"
    );
    assert!(
        path.join("secrets.env").is_file(),
        "a refusal deletes nothing"
    );

    sandbox.json_ok(
        "default",
        &["workspace", "rm", "ign", "--force", "--discard-ignored"],
    );
    assert!(!path.exists());
}

#[test]
fn an_orphan_directory_needs_forget_orphan_and_keeps_its_directory() {
    let sandbox = Sandbox::new();
    let repo = add_repo_project(&sandbox);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "lost"]);
    let path = created["path"].as_str().expect("path").to_owned();
    git(&repo, &["worktree", "remove", "--force", path.as_str()]);
    std::fs::create_dir_all(&path).expect("recreate the directory");
    std::fs::write(Path::new(&path).join("notes.txt"), "keep me").expect("write notes");

    let refused = sandbox
        .run(
            "default",
            &with_json(&["workspace", "rm", "lost", "--force"]),
        )
        .code(1);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "workspace.orphan"
    );

    let removed = sandbox.json_ok("default", &["workspace", "rm", "lost", "--forget-orphan"]);
    assert_eq!(removed["directory_kept"], true);
    assert!(Path::new(&path).join("notes.txt").is_file());
}

#[test]
fn a_worktree_left_by_an_interrupted_create_is_listed_until_a_create_adopts_it() {
    let sandbox = Sandbox::new();
    let repo = add_repo_project(&sandbox);
    let root = sandbox.home().join("ws").join("lost");
    let root_text = root.to_string_lossy().into_owned();
    git(
        &repo,
        &["worktree", "add", "-b", "lost", root_text.as_str(), "HEAD"],
    );

    let listed = sandbox.json_ok("default", &["workspace", "list", "--project", "repo"]);
    assert_eq!(listed["workspaces"].as_array().map(Vec::len), Some(0));
    assert_eq!(listed["unregistered"].as_array().map(Vec::len), Some(1));
    assert_eq!(listed["unregistered"][0]["branch"], "lost");
    sandbox
        .run("default", &["workspace", "list", "--project", "repo"])
        .success()
        .stdout(contains("unregistered worktree"));
    assert!(root.is_dir(), "listing never removes the worktree");

    let adopted = sandbox.json_ok("default", &["workspace", "create", "repo", "lost"]);
    assert_eq!(adopted["branch"], "lost");
    let listed = sandbox.json_ok("default", &["workspace", "list", "--project", "repo"]);
    assert!(listed.get("unregistered").is_none());
}

/// Git prints `prunable` in `worktree list --porcelain` from 2.31; older git leaves it off.
fn git_reports_prunable(repo: &Path) -> bool {
    let version = git(repo, &["--version"]);
    let mut parts = version.trim().trim_start_matches("git version ").split('.');
    let major: u32 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (major, minor) >= (2, 31)
}

#[test]
fn a_locked_or_prunable_worktree_is_marked_in_list_and_show() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    let repo = add_repo_project(&sandbox);
    let usb = sandbox.json_ok("default", &["workspace", "create", "repo", "usb"]);
    let usb_path = usb["path"].as_str().expect("path").to_owned();
    git(
        &repo,
        &[
            "worktree",
            "lock",
            "--reason",
            "on a usb disk",
            usb_path.as_str(),
        ],
    );
    let gone = sandbox.json_ok("default", &["workspace", "create", "repo", "gone"]);
    let gone_path = PathBuf::from(gone["path"].as_str().expect("path"));
    std::fs::remove_dir_all(&gone_path).expect("remove the directory");

    let listed = sandbox.json_ok("default", &["workspace", "list", "--project", "repo"]);
    let usb_row = listed["workspaces"]
        .as_array()
        .expect("workspaces")
        .iter()
        .find(|row| row["name"] == "usb")
        .expect("the usb row");
    assert_eq!(usb_row["locked"], "on a usb disk");
    assert!(usb_row.get("prunable").is_none());
    sandbox
        .run("default", &["workspace", "list", "--project", "repo"])
        .success()
        .stdout(contains("locked: on a usb disk"));
    sandbox
        .run("default", &["workspace", "show", "usb"])
        .success()
        .stdout(contains("locked: on a usb disk"));
    let shown = sandbox.json_ok("default", &["workspace", "show", "usb"]);
    assert_eq!(shown["locked"], "on a usb disk");

    if git_reports_prunable(&repo) {
        let gone_row = listed["workspaces"]
            .as_array()
            .expect("workspaces")
            .iter()
            .find(|row| row["name"] == "gone")
            .expect("the gone row");
        assert!(gone_row["prunable"].is_string());
        sandbox
            .run("default", &["workspace", "list", "--project", "repo"])
            .success()
            .stdout(contains("prunable: "));
    }
}

#[test]
fn current_names_the_worktree_a_subdirectory_belongs_to() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "here"]);
    let inside = PathBuf::from(created["path"].as_str().expect("path"))
        .join("src")
        .join("deep");
    std::fs::create_dir_all(&inside).expect("create subdirectory");

    let mut command = sandbox.command("default");
    command.current_dir(&inside);
    let current = execute(command.args(["--json", "workspace", "current"])).success();

    let result = &single_document(&current)["result"];
    assert_eq!(result["id"], created["id"]);
    assert_eq!(result["name"], "here");
    assert_eq!(result["kind"], "worktree");
}

#[test]
fn current_takes_a_path_and_names_the_folder_workspace_that_holds_it() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(notes.join("sub")).expect("create folder");
    let notes_text = path_text(&notes);
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    let sub_text = path_text(&notes.join("sub"));

    let found = sandbox.json_ok(
        "default",
        &["workspace", "current", "--path", sub_text.as_str()],
    );
    assert_eq!(found["name"], "main");
    assert_eq!(found["kind"], "folder");
}

#[test]
fn current_outside_every_workspace_exits_1_with_workspace_not_found() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    sandbox.json_ok("default", &["workspace", "create", "repo", "here"]);
    let elsewhere = sandbox.home().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("create directory");
    let elsewhere_text = path_text(&elsewhere);

    let refused = sandbox
        .run(
            "default",
            &with_json(&["workspace", "current", "--path", elsewhere_text.as_str()]),
        )
        .code(1);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "workspace.not_found"
    );
}

#[test]
fn current_returns_the_same_workspace_as_show_in_json_and_text() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "same"]);
    let path = path_text(Path::new(created["path"].as_str().expect("path")));
    let shown = sandbox.json_ok("default", &["workspace", "show", "same"]);

    let current = sandbox.json_ok(
        "default",
        &["workspace", "current", "--path", path.as_str()],
    );
    assert_eq!(current, shown);
    sandbox
        .run(
            "default",
            &["workspace", "current", "--path", path.as_str()],
        )
        .success()
        .stdout(contains("branch: same"));
}

#[test]
fn workspace_create_without_a_name_generates_one_for_the_branch_and_folder() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);

    let created = sandbox.json_ok("default", &["workspace", "create", "repo"]);
    let name = created["name"]
        .as_str()
        .expect("a generated name")
        .to_owned();
    assert_eq!(created["branch"], name.as_str());
    let path = PathBuf::from(created["path"].as_str().expect("path"));
    assert_eq!(
        path.file_name().and_then(|leaf| leaf.to_str()),
        Some(name.as_str())
    );

    let text = sandbox
        .run("default", &["workspace", "create", "repo"])
        .success();
    let listed = sandbox.json_ok("default", &["workspace", "list", "--project", "repo"]);
    let second = listed["workspaces"]
        .as_array()
        .expect("workspaces")
        .iter()
        .filter_map(|row| row["name"].as_str())
        .find(|other| *other != name)
        .expect("the second workspace")
        .to_owned();
    assert!(
        stdout_text(&text).contains(&format!("created workspace {second} (worktree)")),
        "the text output names the generated workspace: {}",
        stdout_text(&text)
    );
}

#[test]
fn a_custom_branch_prefix_set_through_config_prefixes_the_branch_of_a_named_workspace() {
    let sandbox = Sandbox::new();
    add_repo_project(&sandbox);
    sandbox.json_ok(
        "default",
        &["config", "set", "workspaces.branch_prefix", "custom"],
    );
    sandbox.json_ok(
        "default",
        &["config", "set", "workspaces.branch_prefix_custom", "team"],
    );

    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "feature"]);

    assert_eq!(created["name"], "feature");
    assert_eq!(created["branch"], "team/feature");
}
