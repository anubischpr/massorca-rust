//! `massorca automations create|edit` destinations end to end (decision 0127): `--workspace` names an existing workspace
//! every run goes to, `--new-worktree` names a worktree for each run, the two flags conflict, a per-run worktree needs a
//! git project, and `show` and `list` name the destination.

mod harness;

use harness::{Sandbox, single_document, stdout_text, with_json};
use serde_json::Value;

/// A folder project named `app`, which the automations run in.
fn project(sandbox: &Sandbox) {
    let folder = sandbox.home().join("app");
    std::fs::create_dir_all(&folder).expect("create the folder");
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            folder.to_str().expect("utf-8 path"),
            "--name",
            "app",
            "--folder",
        ],
    );
}

/// A folder workspace named `feature`, at a folder of its own inside the sandbox home.
fn feature_workspace(sandbox: &Sandbox) -> Value {
    let folder = sandbox.home().join("feature");
    std::fs::create_dir_all(&folder).expect("create the workspace folder");
    sandbox.json_ok(
        "default",
        &[
            "workspace",
            "create",
            "app",
            "feature",
            "--path",
            folder.to_str().expect("utf-8 path"),
        ],
    )
}

/// Runs `--json <args>`, which must fail with exit code 1; returns the error body.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure().code(1);
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

#[test]
fn a_run_goes_to_the_workspace_it_names_and_show_and_list_say_so() {
    let sandbox = Sandbox::new();
    project(&sandbox);
    let workspace = feature_workspace(&sandbox);
    let id = workspace["id"].as_str().expect("the workspace id");

    let created = sandbox.json_ok(
        "default",
        &[
            "automations",
            "create",
            "Feature PRs",
            "--project",
            "app",
            "--prompt",
            "Summarise the open pull requests",
            "--zone",
            "UTC",
            "--workspace",
            id,
        ],
    );
    assert_eq!(created["destination"]["kind"], "existing_workspace");
    assert_eq!(created["destination"]["workspace"], id);
    assert_eq!(created["destination"]["workspace_name"], "feature");

    let shown = sandbox
        .run("default", &["automations", "show", "Feature PRs"])
        .success();
    assert!(
        stdout_text(&shown).contains("destination: workspace feature ("),
        "{}",
        stdout_text(&shown)
    );
    let listed = sandbox.run("default", &["automations", "list"]).success();
    assert!(
        stdout_text(&listed).contains("workspace feature ("),
        "{}",
        stdout_text(&listed)
    );
}

#[test]
fn a_new_worktree_per_run_is_refused_in_a_folder_project_with_its_code() {
    let sandbox = Sandbox::new();
    project(&sandbox);

    let error = error_of(
        &sandbox,
        &[
            "automations",
            "create",
            "Per run",
            "--project",
            "app",
            "--prompt",
            "Summarise",
            "--zone",
            "UTC",
            "--new-worktree",
            "--base-branch",
            "main",
        ],
    );
    assert_eq!(error["code"], "automation.destination_invalid", "{error}");
}

#[test]
fn the_workspace_and_new_worktree_flags_conflict() {
    let sandbox = Sandbox::new();
    project(&sandbox);

    sandbox
        .run(
            "default",
            &[
                "automations",
                "create",
                "Both",
                "--project",
                "app",
                "--prompt",
                "Summarise",
                "--workspace",
                "ws-1",
                "--new-worktree",
            ],
        )
        .failure();
}

#[test]
fn an_edit_changes_the_destination_only_when_one_is_given() {
    let sandbox = Sandbox::new();
    project(&sandbox);
    let workspace = feature_workspace(&sandbox);
    let id = workspace["id"]
        .as_str()
        .expect("the workspace id")
        .to_owned();
    sandbox.json_ok(
        "default",
        &[
            "automations",
            "create",
            "Edited",
            "--project",
            "app",
            "--prompt",
            "Summarise",
            "--zone",
            "UTC",
        ],
    );

    let plain = sandbox.json_ok(
        "default",
        &["automations", "edit", "Edited", "--prompt", "Again"],
    );
    assert_eq!(plain["destination"]["kind"], "project_default", "{plain}");

    let moved = sandbox.json_ok(
        "default",
        &["automations", "edit", "Edited", "--workspace", id.as_str()],
    );
    assert_eq!(
        moved["destination"]["kind"], "existing_workspace",
        "{moved}"
    );
    assert_eq!(moved["destination"]["workspace"], id.as_str());
}
