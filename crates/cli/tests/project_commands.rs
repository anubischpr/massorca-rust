//! Projects through the CLI: add and list in both kinds, show, remove, relative paths, and the typed refusals.

mod harness;

use harness::{Sandbox, execute, git, git_repo, single_document, stdout_text, with_json};
use predicates::prelude::PredicateBooleanExt;
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
fn by_default_a_repositorys_workspaces_go_in_its_worktrees_folder_and_show_prints_the_root() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let repo_text = repo.to_string_lossy().into_owned();

    let added = sandbox.json_ok("default", &["project", "add", repo_text.as_str()]);
    let root = std::path::PathBuf::from(added["workspaces_root"].as_str().expect("root"));
    assert_eq!(
        root.file_name().and_then(|name| name.to_str()),
        Some(".worktrees")
    );
    assert_eq!(
        std::fs::canonicalize(root.parent().expect("parent")).expect("repo"),
        std::fs::canonicalize(&repo).expect("repo")
    );
    assert!(
        !root.exists(),
        "nothing is created until the first worktree"
    );

    let shown = sandbox.json_ok("default", &["project", "show", "repo"]);
    assert_eq!(shown["workspaces_root"], added["workspaces_root"]);
    let text = sandbox
        .run("default", &["project", "show", "repo"])
        .success();
    let expected = format!("workspaces root: {}", root.display());
    assert!(
        stdout_text(&text).contains(&expected),
        "{}",
        stdout_text(&text)
    );

    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "feature"]);
    let path = std::path::PathBuf::from(created["path"].as_str().expect("path"));
    assert_eq!(
        std::fs::canonicalize(&path).expect("the workspace exists"),
        std::fs::canonicalize(root.join("feature")).expect("the workspace exists")
    );
    let exclude = std::fs::read_to_string(repo.join(".git").join("info").join("exclude"))
        .expect("the exclude file");
    assert!(
        exclude.lines().any(|line| line == "/.worktrees/"),
        "{exclude}"
    );
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
fn a_project_base_is_set_shown_and_cleared_as_text_and_json() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    git(&repo, &["branch", "develop"]);
    let repo_text = repo.to_string_lossy().into_owned();
    sandbox.json_ok("default", &["project", "add", repo_text.as_str()]);

    sandbox
        .run("default", &["project", "set-base", "repo", "develop"])
        .success()
        .stdout(contains("project repo base is develop"));
    let shown = sandbox
        .run("default", &["project", "show", "repo"])
        .success();
    assert!(stdout_text(&shown).contains("base: develop"));
    let listed = sandbox.run("default", &["project", "list"]).success();
    assert!(stdout_text(&listed).contains("develop"));
    let set = sandbox.json_ok("default", &["project", "set-base", "repo", "develop"]);
    assert_eq!(set["base_ref"], "develop");

    sandbox
        .run("default", &["project", "set-base", "repo", "--clear"])
        .success()
        .stdout(contains("project repo has no base"));
    let cleared = sandbox.json_ok("default", &["project", "set-base", "repo", "--clear"]);
    assert!(cleared.get("base_ref").is_none(), "{cleared}");

    let unresolved = sandbox
        .run(
            "default",
            &with_json(&["project", "set-base", "repo", "no-such-branch"]),
        )
        .code(1);
    assert_eq!(
        single_document(&unresolved)["error"]["code"],
        "base.not_found"
    );
    sandbox
        .run(
            "default",
            &["project", "set-base", "repo", "develop", "--clear"],
        )
        .failure();
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
fn a_linked_worktree_registers_and_names_its_main_repository() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let linked = sandbox.home().join("linked-wt");
    let linked_text = linked.to_string_lossy().into_owned();
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked-wt",
            linked_text.as_str(),
        ],
    );

    sandbox
        .run("default", &["project", "add", linked_text.as_str()])
        .success()
        .stdout(contains("added project repo (git) at "))
        .stdout(contains("linked-wt").not());
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

#[test]
fn inspect_reports_a_repository_and_a_plain_folder_in_text_and_json() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let nested = repo.join("src");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let nested_text = nested.to_string_lossy().into_owned();
    let notes_text = notes.to_string_lossy().into_owned();

    sandbox
        .run("default", &["project", "inspect", nested_text.as_str()])
        .success()
        .stdout(contains(": git repository"));
    let git = sandbox.json_ok("default", &["project", "inspect", nested_text.as_str()]);
    assert_eq!(git["kind"], "git");
    assert!(
        git["path"].as_str().expect("path").ends_with("repo"),
        "the repository's top level: {git}"
    );
    assert!(git.get("registered").is_none(), "nothing registered: {git}");

    sandbox
        .run("default", &["project", "inspect", notes_text.as_str()])
        .success()
        .stdout(contains(
            ": not a git repository; it would be added as a plain folder",
        ));

    let added = sandbox.json_ok("default", &["project", "add", notes_text.as_str()]);
    let folder = sandbox.json_ok("default", &["project", "inspect", notes_text.as_str()]);
    assert_eq!(folder["kind"], "folder");
    assert_eq!(folder["path"], added["path"]);
    assert_eq!(folder["registered"], added["id"]);
    let id = added["id"].as_str().expect("id");
    sandbox
        .run("default", &["project", "inspect", notes_text.as_str()])
        .success()
        .stdout(contains(format!("already registered as {id}")));
}

#[test]
fn adding_a_plain_folder_without_a_kind_warns_on_stderr_in_text_mode_only() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    let other = sandbox.home().join("other");
    let flagged = sandbox.home().join("flagged");
    for dir in [&notes, &other, &flagged] {
        std::fs::create_dir_all(dir).expect("create folder");
    }
    let warning = "is not a git repository; it was added as a plain folder";

    sandbox
        .run(
            "default",
            &["project", "add", notes.to_string_lossy().as_ref()],
        )
        .success()
        .stdout(contains("added project notes (folder)"))
        .stderr(contains(warning));

    let outcome = sandbox
        .run(
            "default",
            &["--json", "project", "add", other.to_string_lossy().as_ref()],
        )
        .success();
    assert!(!String::from_utf8_lossy(&outcome.get_output().stderr).contains(warning));
    assert_eq!(single_document(&outcome)["result"]["kind"], "folder");

    sandbox
        .run(
            "default",
            &[
                "project",
                "add",
                flagged.to_string_lossy().as_ref(),
                "--folder",
            ],
        )
        .success()
        .stderr(contains(warning).not());
}

#[test]
fn init_git_turns_a_folder_project_into_a_git_project_and_refuses_it_again() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    std::fs::write(notes.join("notes.txt"), "keep me").expect("write notes");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );

    let initialized = sandbox.json_ok("default", &["project", "init-git", "notes"]);

    assert_eq!(initialized["kind"], "git");
    assert_eq!(initialized["name"], "notes");
    assert!(notes.join(".git").is_dir());
    assert!(notes.join("notes.txt").is_file(), "the folder's files stay");
    let again = sandbox
        .run("default", &with_json(&["project", "init-git", "notes"]))
        .code(1);
    assert_eq!(
        single_document(&again)["error"]["code"],
        "project.already_git"
    );
}

#[test]
fn init_git_names_the_new_git_project_in_text_output() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox
        .run(
            "default",
            &["project", "add", notes_text.as_str(), "--folder"],
        )
        .success();

    sandbox
        .run("default", &["project", "init-git", "notes"])
        .success()
        .stdout(contains("initialized Git for project notes (git)"));
}

#[test]
fn the_remote_owner_is_shown_by_list_and_show_and_carried_in_json() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:acme/widgets.git"],
    );
    let repo_text = repo.to_string_lossy().into_owned();
    sandbox.json_ok("default", &["project", "add", repo_text.as_str()]);

    let shown = sandbox
        .run("default", &["project", "show", "repo"])
        .success();
    assert!(
        stdout_text(&shown).contains("owner: acme"),
        "show: {}",
        stdout_text(&shown)
    );

    let listed = sandbox.run("default", &["project", "list"]).success();
    let text = stdout_text(&listed);
    assert!(
        text.contains("OWNER") && text.contains("acme"),
        "list: {text}"
    );

    let json = sandbox.json_ok("default", &["project", "show", "repo"]);
    assert_eq!(json["remote_owner"], "acme");
}
