//! Removing a workspace whose delegated children run in workspaces of their own (decision 0098): refused with
//! `workspace.has_children` unless `with_children`, which removes the children first, deepest first, through the same
//! checks. No agent is started: the sessions are written to the store directly, as ended runs.

use std::path::Path;

use massorca_client::{Client, ClientError};
use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, RunMode, Store, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::events::{TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceRef, WorkspaceRemoveParams, WorkspaceRemoved, WorkspaceState,
};
use serde_json::{Value, json};

use super::common::{connect, profile, start, stop, temp_home};
use super::workspaces::{add_repo_project, create_params, git_repo, next_event, remove_params};

/// An ended session in `workspace`, delegated from `parent` when given. Returns its id.
fn session(store: &Store, workspace: &Workspace, parent: Option<&str>, read_only: bool) -> String {
    let row = AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace.id.clone(),
        parent_id: parent.map(str::to_owned),
        title: None,
        read_only,
        agent_ref: None,
        terminal_id: None,
        request_id: None,
        created_at: now_ms(),
        exit: Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: now_ms(),
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

fn with_children(workspace: &str, force: bool) -> WorkspaceRemoveParams {
    WorkspaceRemoveParams {
        with_children: true,
        ..remove_params(workspace, force)
    }
}

/// The code, message and data of a failed call.
fn error_body(error: &ClientError) -> (String, String, Value) {
    match error {
        ClientError::Remote(body) => (
            body.code.clone(),
            body.message.clone(),
            body.data.clone().unwrap_or(Value::Null),
        ),
        other => panic!("expected a remote error, got {other:?}"),
    }
}

fn names(removed: &[WorkspaceRemoved]) -> Vec<String> {
    removed
        .iter()
        .map(|child| child.workspace.name.clone())
        .collect()
}

async fn state_of(client: &Client, workspace: &Workspace) -> WorkspaceState {
    let shown: Workspace = client
        .call(
            workspaces::SHOW,
            &WorkspaceRef {
                workspace: workspace.id.clone(),
            },
        )
        .await
        .expect("show");
    shown.state
}

async fn create(client: &Client, project: &str, name: &str) -> Workspace {
    client
        .call(workspaces::CREATE, &create_params(project, name))
        .await
        .expect("create")
}

/// A lead workspace with a read-only child in it, a writer child in `lead-1`, and that child's writer in `lead-1-1`.
struct Tree {
    lead: Workspace,
    child: Workspace,
    grandchild: Workspace,
}

async fn tree(client: &Client, home: &Path, store: &Store) -> Tree {
    let repo = home.join("repo");
    git_repo(&repo);
    let project = add_repo_project(client, home, &repo).await;
    let lead = create(client, &project.name, "lead").await;
    let child = create(client, &project.name, "lead-1").await;
    let grandchild = create(client, &project.name, "lead-1-1").await;
    let parent = session(store, &lead, None, false);
    session(store, &lead, Some(&parent), true);
    let writer = session(store, &child, Some(&parent), false);
    session(store, &grandchild, Some(&writer), false);
    Tree {
        lead,
        child,
        grandchild,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_with_child_workspaces_is_refused_and_the_refusal_names_them() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let tree = tree(&client, home.path(), &store).await;

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &remove_params("lead", false))
        .await
        .expect_err("a workspace with children");
    let (code, message, data) = error_body(&refused);
    assert_eq!(code, codes::WORKSPACE_HAS_CHILDREN);
    assert!(
        message.contains("lead-1-1, lead-1") && message.contains("with_children"),
        "{message}"
    );
    assert_eq!(data, json!({ "children": ["lead-1-1", "lead-1"] }));
    for workspace in [&tree.lead, &tree.child, &tree.grandchild] {
        assert_eq!(state_of(&client, workspace).await, WorkspaceState::Ready);
        assert!(Path::new(&workspace.path).is_dir());
    }
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn with_children_removes_the_children_deepest_first_and_then_the_workspace() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let tree = tree(&client, home.path(), &store).await;
    let mut events = client.subscribe_events().await.expect("subscribe");

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect("removal with children");

    assert_eq!(removed.workspace.id, tree.lead.id);
    assert_eq!(names(&removed.removed_children), ["lead-1-1", "lead-1"]);
    for workspace in [&tree.lead, &tree.child, &tree.grandchild] {
        assert!(!Path::new(&workspace.path).exists());
    }
    let mut order = Vec::new();
    for _ in 0..6 {
        let event = next_event(&mut events).await;
        order.push((event.topic, event.id));
    }
    let expected: Vec<_> = [&tree.grandchild, &tree.child, &tree.lead]
        .into_iter()
        .flat_map(|workspace| {
            [
                (TOPIC_WORKSPACE_REMOVING.to_owned(), workspace.id.clone()),
                (TOPIC_WORKSPACE_REMOVED.to_owned(), workspace.id.clone()),
            ]
        })
        .collect();
    assert_eq!(order, expected);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_child_keeps_the_workspace_and_reports_the_children_already_removed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let tree = tree(&client, home.path(), &store).await;
    std::fs::write(Path::new(&tree.child.path).join("scratch.txt"), "work").expect("write change");

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect_err("a dirty child");
    let (code, message, data) = error_body(&refused);
    assert_eq!(
        code,
        codes::WORKSPACE_DIRTY,
        "the child's refusal keeps its code"
    );
    assert!(message.contains("lead-1"), "{message}");
    assert_eq!(data["child"], "lead-1");
    assert_eq!(data["removed_children"], json!(["lead-1-1"]));
    assert!(!Path::new(&tree.grandchild.path).exists());
    assert_eq!(state_of(&client, &tree.child).await, WorkspaceState::Ready);
    assert_eq!(state_of(&client, &tree.lead).await, WorkspaceState::Ready);
    assert!(Path::new(&tree.lead.path).is_dir());

    // The caller's waivers reach every child.
    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &with_children("lead", true))
        .await
        .expect("forced removal with children");
    assert_eq!(names(&removed.removed_children), ["lead-1"]);
    assert!(!Path::new(&tree.lead.path).exists());
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_child_whose_directory_is_gone_is_removed_without_an_error() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let tree = tree(&client, home.path(), &store).await;
    std::fs::remove_dir_all(&tree.grandchild.path).expect("remove the grandchild's directory");

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect("removal with a missing child");
    assert_eq!(names(&removed.removed_children), ["lead-1-1", "lead-1"]);
    assert!(!Path::new(&tree.lead.path).exists());
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_with_only_a_read_only_child_is_removed_as_before() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let project = add_repo_project(&client, home.path(), &repo).await;
    let solo = create(&client, &project.name, "solo").await;
    let parent = session(&store, &solo, None, false);
    session(&store, &solo, Some(&parent), true);

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &remove_params("solo", false))
        .await
        .expect("plain removal");
    assert!(removed.removed_children.is_empty());
    assert!(removed.branch_deleted);
    assert!(!Path::new(&solo.path).exists());
    stop(running).await;
}
