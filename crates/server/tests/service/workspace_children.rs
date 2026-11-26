//! Removing a workspace whose delegated children run in workspaces of their own (decision 0110): refused with
//! `workspace.has_children` unless `with_children`, which removes the children first, deepest first, through the same
//! checks. No agent is started: the sessions are written to the store directly, as ended runs.

use std::path::Path;
use std::time::Duration;

use massorca_client::{Client, ClientError};
use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, RunMode, Store, WorkspaceKind as StoreWorkspaceKind,
    WorkspaceRow, WorkspaceState as StoreWorkspaceState, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
};
use massorca_protocol::methods::projects::ScriptKind;
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceArchive, WorkspaceRef, WorkspaceRemoveParams, WorkspaceRemoved,
    WorkspaceState,
};
use serde_json::{Value, json};
use tokio::time::timeout;

use super::common::{connect, profile, start, stop, temp_home};
use super::workspace_archive::{ARCHIVED, copy_script, repo_with_archive};
use super::workspace_setup::{add_project, trust};
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
    let mut events = client.subscribe_events().await.expect("subscribe");

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect_err("a dirty child");
    // Only the grandchild's removal is announced; the dirty child and the lead never start one.
    for topic in [TOPIC_WORKSPACE_REMOVING, TOPIC_WORKSPACE_REMOVED] {
        let event = next_event(&mut events).await;
        assert_eq!(
            (event.topic.as_str(), event.id),
            (topic, tree.grandchild.id.clone())
        );
    }
    assert!(
        timeout(Duration::from_millis(500), events.next_as::<ChangeEvent>())
            .await
            .is_err(),
        "no other workspace began a removal"
    );
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

/// A workspace row for the project's main worktree, which removal always refuses.
fn main_worktree_row(store: &Store, project_id: &str, repo: &Path) -> Workspace {
    let row = WorkspaceRow {
        id: new_id(),
        project_id: project_id.to_owned(),
        kind: StoreWorkspaceKind::Worktree,
        name: "main-row".to_owned(),
        path: repo.to_str().expect("utf-8").to_owned(),
        path_key: "main-row-key".to_owned(),
        branch: None,
        base_ref: None,
        created_branch: false,
        state: StoreWorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
        comment: None,
    };
    store
        .insert_workspace(&row)
        .expect("insert the main worktree row");
    Workspace {
        id: row.id,
        project_id: row.project_id,
        kind: workspaces::WorkspaceKind::Worktree,
        name: row.name,
        path: row.path,
        branch: None,
        base_ref: None,
        created_branch: false,
        state: WorkspaceState::Ready,
        missing: false,
        created_at_ms: row.created_at,
        locked: None,
        prunable: None,
        base_refresh: None,
        comment: None,
        setup: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_parent_s_own_refusal_comes_first_and_removes_no_child() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let project = add_repo_project(&client, home.path(), &repo).await;
    let main = main_worktree_row(&store, &project.id, &repo);
    let child = create(&client, &project.name, "main-row-1").await;
    let parent = session(&store, &main, None, false);
    session(&store, &child, Some(&parent), false);

    for params in [
        remove_params("main-row", true),
        with_children("main-row", true),
    ] {
        let refused = client
            .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &params)
            .await
            .expect_err("the main worktree");
        assert_eq!(error_body(&refused).0, codes::WORKSPACE_MAIN_WORKTREE);
    }
    assert!(Path::new(&child.path).is_dir());
    assert_eq!(state_of(&client, &child).await, WorkspaceState::Ready);
    assert_eq!(state_of(&client, &main).await, WorkspaceState::Ready);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dirty_parent_without_force_removes_no_child() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let tree = tree(&client, home.path(), &store).await;
    std::fs::write(Path::new(&tree.lead.path).join("scratch.txt"), "work").expect("write change");

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect_err("a dirty parent");
    let (code, _, data) = error_body(&refused);
    assert_eq!(code, codes::WORKSPACE_DIRTY);
    assert_eq!(data, Value::Null, "the parent's own refusal, not a child's");
    for workspace in [&tree.lead, &tree.child, &tree.grandchild] {
        assert!(Path::new(&workspace.path).is_dir());
        assert_eq!(state_of(&client, workspace).await, WorkspaceState::Ready);
    }
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_child_whose_row_is_gone_is_skipped() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let tree = tree(&client, home.path(), &store).await;
    assert!(
        store
            .delete_workspace(&tree.grandchild.id)
            .expect("forget the grandchild")
    );

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect("removal without the forgotten child");
    assert_eq!(names(&removed.removed_children), ["lead-1"]);
    assert!(!Path::new(&tree.lead.path).exists());
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn with_children_runs_a_trusted_archive_script_in_each_child_before_removing_it() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let lead_repo = home.path().join("lead-repo");
    git_repo(&lead_repo);
    let lead_project = add_repo_project(&client, home.path(), &lead_repo).await;
    let lead = create(&client, &lead_project.name, "lead").await;
    let child_repo = home.path().join("child-repo");
    repo_with_archive(&child_repo, copy_script());
    let child_project = add_project(&client, home.path(), &child_repo).await;
    trust(&client, &child_project.id, ScriptKind::Archive).await;
    let child = create(&client, &child_project.name, "lead-1").await;
    let parent = session(&store, &lead, None, false);
    session(&store, &child, Some(&parent), false);

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &with_children("lead", false))
        .await
        .expect("removal with children");

    let child_removed = removed
        .removed_children
        .first()
        .expect("the child was removed");
    assert!(
        matches!(
            child_removed.archive,
            Some(WorkspaceArchive::Succeeded { .. })
        ),
        "the child's archive did not run: {:?}",
        child_removed.archive
    );
    // Git may check the worktree's copy out with CRLF line ends.
    let archived = std::fs::read_to_string(child_repo.join(ARCHIVED))
        .expect("the child's archive script ran in its worktree");
    assert_eq!(archived.replace("\r\n", "\n"), "work worth keeping\n");
    assert!(!Path::new(&child.path).exists());
    stop(running).await;
}
