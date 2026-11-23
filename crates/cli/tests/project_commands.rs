//! Projects through the CLI: add and list in both kinds, show, remove, relative paths, and the typed refusals.

mod harness;

use harness::{Sandbox, execute, git_repo, single_document, stdout_text, with_json};
use predicates::str::contains;

#[test]
fn a_repository_is_added_as_a_git_project_and_listed_shown_and_removed() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let root = sandbox.home().join("ws");
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = root.to_string_lossy().into_owned();

    let added = sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo_text.as_str(),
            "--workspaces-root",
            root_text.as_str(),
        ],
    );
    assert_eq!(added["kind"], "git");
    assert_eq!(added["name"], "repo");
    assert_eq!(added["workspaces_root"], root_text.as_str());

    let listed = sandbox.run("default", &["project", "list"]).success();
    let text = stdout_text(&listed);
    assert!(text.starts_with("ID"), "table header first: {text}");
    assert!(text.contains("repo") && text.contains("git"), "row: {text}");

    let shown = sandbox
        .run("default", &["project", "show", "repo"])
        .success();
    assert!(stdout_text(&shown).contains("kind: git"));

    sandbox
        .run("default", &["project", "rm", "repo"])
        .success()
        .stdout(contains("removed project repo (git)"));
    assert!(
        repo.join("README.md").is_file(),
        "removing a project never deletes its files"
    );
    sandbox
        .run("default", &["project", "list"])
        .success()
        .stdout(contains("no projects"));
}

#[test]
fn a_directory_is_added_as_a_folder_with_the_folder_flag() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();

    let added = sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );

    assert_eq!(added["kind"], "folder");
    assert_eq!(added["name"], "notes");
}

#[test]
fn a_relative_path_is_taken_from_the_working_directory() {
    let sandbox = Sandbox::new();
    git_repo(&sandbox.home().join("repo"));
    let mut command = sandbox.command("default");
    command.current_dir(sandbox.home());

    let outcome = execute(command.args([
        "--json",
        "project",
        "add",
        "repo",
        "--workspaces-root",
        "ws",
    ]))
    .success();

    let added = single_document(&outcome)["result"].clone();
    assert!(
        added["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("repo")),
        "absolute path: {added}"
    );
    assert!(
        added["workspaces_root"]
            .as_str()
            .is_some_and(|path| path.ends_with("ws")),
        "absolute workspaces root: {added}"
    );
}

#[test]
fn git_on_a_plain_directory_is_refused_with_exit_1() {
    let sandbox = Sandbox::new();
    let plain = sandbox.home().join("plain");
    std::fs::create_dir_all(&plain).expect("create dir");
    let plain_text = plain.to_string_lossy().into_owned();

    let outcome = sandbox
        .run(
            "default",
            &with_json(&["project", "add", plain_text.as_str(), "--git"]),
        )
        .code(1);

    assert_eq!(
        single_document(&outcome)["error"]["code"],
        "project.not_git"
    );
}

#[test]
fn adding_the_same_project_twice_and_showing_an_unknown_one_are_refused() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let repo_text = repo.to_string_lossy().into_owned();
    sandbox.json_ok("default", &["project", "add", repo_text.as_str()]);

    let duplicate = sandbox
        .run(
            "default",
            &with_json(&["project", "add", repo_text.as_str()]),
        )
        .code(1);
    assert_eq!(
        single_document(&duplicate)["error"]["code"],
        "project.exists"
    );

    let unknown = sandbox
        .run("default", &with_json(&["project", "show", "nothing"]))
        .code(1);
    assert_eq!(
        single_document(&unknown)["error"]["code"],
        "project.not_found"
    );
}

#[test]
fn forget_workspaces_drops_the_workspace_records_and_touches_no_file() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    std::fs::write(notes.join("notes.txt"), "keep me").expect("write notes");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);

    sandbox
        .run("default", &["project", "rm", "notes"])
        .code(1)
        .stderr(contains("project.has_workspaces"));

    sandbox
        .run(
            "default",
            &["project", "rm", "notes", "--forget-workspaces"],
        )
        .success()
        .stdout(contains("removed project notes (folder)"))
        .stdout(contains("forgot 1 workspace record"))
        .stdout(contains("no files"));
    assert!(notes.join("notes.txt").is_file(), "the folder's files stay");
    sandbox
        .run("default", &["project", "list"])
        .success()
        .stdout(contains("no projects"));
}
