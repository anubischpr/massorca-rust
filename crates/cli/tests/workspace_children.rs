//! `workspace rm` of a workspace whose delegated children run in workspaces of their own (decision 0102): refused with
//! exit 1 and a message naming the children and `--with-children`, which removes them first. No agent is started: the
//! sessions are written to the store while the service is stopped.

mod harness;

use std::path::Path;

use harness::{Sandbox, git_repo, single_document, with_json};
use massorca_core_store::{AgentSessionRow, RunExit, RunExitReason, RunMode, Store, new_id};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// An ended session in `workspace_id`, delegated from `parent` when given. Returns its id.
fn session(store: &Store, workspace_id: &str, parent: Option<&str>) -> String {
    let row = AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace_id.to_owned(),
        parent_id: parent.map(str::to_owned),
        title: None,
        read_only: false,
        agent_ref: None,
        terminal_id: None,
        request_id: None,
        created_at: 1,
        exit: Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: 2,
        }),
        activity: None,
        result: None,
        brought_back_at: None,
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
        retained: false,
        released_at: None,
    };
    store.insert_agent_session(&row).expect("session");
    row.id
}

#[test]
fn rm_names_the_child_workspaces_and_the_flag_and_with_children_removes_them_first() {
    let sandbox = Sandbox::new();
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let root = sandbox.home().join("ws");
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo.to_string_lossy().as_ref(),
            "--workspaces-root",
            root.to_string_lossy().as_ref(),
        ],
    );
    let lead = sandbox.json_ok("default", &["workspace", "create", "repo", "lead"]);
    let child = sandbox.json_ok("default", &["workspace", "create", "repo", "lead-1"]);
    sandbox.run("default", &["service", "stop"]).success();
    {
        let store = Store::open(sandbox.profile_dir("default").join("store.sqlite3"))
            .expect("the store opens");
        let parent = session(&store, lead["id"].as_str().expect("id"), None);
        session(&store, child["id"].as_str().expect("id"), Some(&parent));
    }

    sandbox
        .run("default", &["workspace", "rm", "lead"])
        .code(1)
        .stderr(contains(
            "error: lead has workspaces of delegated children: lead-1; run `massorca workspace rm lead --with-children` to remove them first (workspace.has_children)",
        ))
        .stderr(contains("with_children").not());
    let refused = sandbox
        .run("default", &with_json(&["workspace", "rm", "lead"]))
        .code(1);
    let refused = single_document(&refused);
    assert_eq!(refused["error"]["code"], "workspace.has_children");
    assert_eq!(refused["error"]["data"]["children"][0], "lead-1");
    assert!(Path::new(lead["path"].as_str().expect("path")).is_dir());

    let removed = sandbox.json_ok("default", &["workspace", "rm", "lead", "--with-children"]);
    assert_eq!(
        removed["removed_children"][0]["workspace"]["name"],
        "lead-1"
    );
    assert!(!Path::new(lead["path"].as_str().expect("path")).exists());
    assert!(!Path::new(child["path"].as_str().expect("path")).exists());
}
