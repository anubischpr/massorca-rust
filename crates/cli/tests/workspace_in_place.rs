//! `massorca workspace create --in-place` against a real service and a real repository (decision 0112).

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, git_repo, stdout_text};

/// A git project named `repo` at `<home>/repo`, whose workspaces would go under `<home>/ws`.
fn project(sandbox: &Sandbox) -> PathBuf {
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
    repo
}

#[test]
fn in_place_create_registers_the_checkout_and_remove_only_forgets_it() {
    let sandbox = Sandbox::new();
    let repo = project(&sandbox);

    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "--in-place"]);

    assert_eq!(created["in_place"], true);
    assert_eq!(
        created["name"], "repo",
        "NAME defaults to the project's name"
    );
    assert_eq!(created["created_branch"], false);
    let stored = Path::new(created["path"].as_str().expect("workspace path"));
    assert_eq!(
        stored.canonicalize().expect("the checkout exists"),
        repo.canonicalize().expect("the checkout exists")
    );

    let removed = sandbox.json_ok("default", &["workspace", "rm", "repo"]);

    assert_eq!(removed["directory_kept"], true);
    assert_eq!(removed["branch_deleted"], false);
    assert!(
        repo.join("README.md").is_file(),
        "the checkout's files stay"
    );
}

#[test]
fn in_place_create_refuses_a_branch_with_its_invalid_params_code() {
    let sandbox = Sandbox::new();
    project(&sandbox);

    let refused = sandbox
        .run(
            "default",
            &[
                "--json",
                "workspace",
                "create",
                "repo",
                "--in-place",
                "--branch",
                "topic",
            ],
        )
        .code(1);
    let document: serde_json::Value =
        serde_json::from_str(stdout_text(&refused).trim()).expect("one JSON document");

    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "request.invalid_params");
}
