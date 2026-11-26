mod common;

use common::{assert_conflict, project_row, workspace_row};
use massorca_core_store::{
    ProjectRow, Store, StoreError, WorkspaceKind, WorkspaceRow, WorkspaceState,
};

fn store_with_project() -> (Store, ProjectRow) {
    let store = Store::open_in_memory().expect("in-memory store opens");
    let project = project_row("alpha", "key-alpha");
    store.insert_project(&project).unwrap();
    (store, project)
}

fn names(rows: Vec<WorkspaceRow>) -> Vec<String> {
    rows.into_iter().map(|row| row.name).collect()
}

#[test]
fn workspace_round_trips_through_each_lookup() {
    let (store, project) = store_with_project();
    let row = workspace_row(&project.id, "feature", "ws-feature");
    store.insert_workspace(&row).unwrap();

    assert_eq!(store.get_workspace(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(
        store
            .find_workspace_by_name(&project.id, "feature")
            .unwrap(),
        Some(row.clone())
    );
    assert_eq!(
        store.find_workspace_by_path_key("ws-feature").unwrap(),
        Some(row)
    );
    assert_eq!(store.get_workspace("missing").unwrap(), None);
    assert_eq!(
        store
            .find_workspace_by_name(&project.id, "missing")
            .unwrap(),
        None
    );
}

#[test]
fn folder_workspace_keeps_absent_branch_and_base_as_none() {
    let (store, project) = store_with_project();
    let mut row = workspace_row(&project.id, "plain", "ws-plain");
    row.kind = WorkspaceKind::Folder;
    row.branch = None;
    row.base_ref = None;
    row.created_branch = false;
    store.insert_workspace(&row).unwrap();

    let found = store.get_workspace(&row.id).unwrap().unwrap();
    assert_eq!(found.kind, WorkspaceKind::Folder);
    assert_eq!(found.branch, None);
    assert_eq!(found.base_ref, None);
    assert!(!found.created_branch);
}

#[test]
fn names_are_unique_per_project_but_not_across_projects() {
    let (store, alpha) = store_with_project();
    let beta = project_row("beta", "key-beta");
    store.insert_project(&beta).unwrap();

    store
        .insert_workspace(&workspace_row(&alpha.id, "feature", "ws-a-feature"))
        .unwrap();
    let duplicate = workspace_row(&alpha.id, "feature", "ws-a-feature-2");
    assert_conflict(store.insert_workspace(&duplicate), "workspaces.name");

    store
        .insert_workspace(&workspace_row(&beta.id, "feature", "ws-b-feature"))
        .unwrap();
}

#[test]
fn duplicate_path_key_or_request_id_is_a_conflict() {
    let (store, project) = store_with_project();
    let mut first = workspace_row(&project.id, "one", "ws-shared");
    first.request_id = Some("req-1".to_owned());
    store.insert_workspace(&first).unwrap();

    let same_path = workspace_row(&project.id, "two", "ws-shared");
    assert_conflict(store.insert_workspace(&same_path), "workspaces.path_key");

    let mut same_request = workspace_row(&project.id, "three", "ws-three");
    same_request.request_id = Some("req-1".to_owned());
    assert_conflict(
        store.insert_workspace(&same_request),
        "workspaces.request_id",
    );
}

#[test]
fn find_by_request_id_returns_only_the_matching_workspace() {
    let (store, project) = store_with_project();
    let mut with_request = workspace_row(&project.id, "one", "ws-one");
    with_request.request_id = Some("req-42".to_owned());
    store.insert_workspace(&with_request).unwrap();
    store
        .insert_workspace(&workspace_row(&project.id, "two", "ws-two"))
        .unwrap();

    assert_eq!(
        store.find_workspace_by_request_id("req-42").unwrap(),
        Some(with_request)
    );
    assert_eq!(
        store.find_workspace_by_request_id("req-missing").unwrap(),
        None
    );
}

#[test]
fn list_orders_by_creation_time_and_filters_by_project() {
    let (store, alpha) = store_with_project();
    let beta = project_row("beta", "key-beta");
    store.insert_project(&beta).unwrap();

    let mut late = workspace_row(&alpha.id, "late", "ws-late");
    late.created_at = 300;
    let mut early = workspace_row(&alpha.id, "early", "ws-early");
    early.created_at = 100;
    let mut middle = workspace_row(&alpha.id, "middle", "ws-middle");
    middle.created_at = 200;
    let mut other = workspace_row(&beta.id, "other", "ws-other");
    other.created_at = 50;
    for row in [&late, &early, &middle, &other] {
        store.insert_workspace(row).unwrap();
    }

    assert_eq!(
        names(store.list_workspaces(Some(&alpha.id)).unwrap()),
        ["early", "middle", "late"]
    );
    assert_eq!(
        names(store.list_workspaces(None).unwrap()),
        ["other", "early", "middle", "late"]
    );
}

#[test]
fn set_state_and_delete_report_whether_a_row_changed() {
    let (store, project) = store_with_project();
    let row = workspace_row(&project.id, "feature", "ws-feature");
    store.insert_workspace(&row).unwrap();

    assert!(
        store
            .set_workspace_state(&row.id, WorkspaceState::Removing)
            .unwrap()
    );
    let found = store.get_workspace(&row.id).unwrap().unwrap();
    assert_eq!(found.state, WorkspaceState::Removing);
    assert!(
        !store
            .set_workspace_state("missing", WorkspaceState::Ready)
            .unwrap()
    );

    assert!(store.delete_workspace(&row.id).unwrap());
    assert_eq!(store.get_workspace(&row.id).unwrap(), None);
    assert!(!store.delete_workspace(&row.id).unwrap());
}

#[test]
fn count_workspaces_counts_one_project_only() {
    let (store, alpha) = store_with_project();
    let beta = project_row("beta", "key-beta");
    store.insert_project(&beta).unwrap();
    assert_eq!(store.count_workspaces(&alpha.id).unwrap(), 0);

    store
        .insert_workspace(&workspace_row(&alpha.id, "one", "ws-one"))
        .unwrap();
    store
        .insert_workspace(&workspace_row(&alpha.id, "two", "ws-two"))
        .unwrap();
    store
        .insert_workspace(&workspace_row(&beta.id, "three", "ws-three"))
        .unwrap();
    assert_eq!(store.count_workspaces(&alpha.id).unwrap(), 2);
    assert_eq!(store.count_workspaces(&beta.id).unwrap(), 1);
}

#[test]
fn workspace_for_a_missing_project_is_a_constraint_error() {
    let store = Store::open_in_memory().unwrap();
    let orphan = workspace_row("no-such-project", "feature", "ws-feature");
    let error = store.insert_workspace(&orphan).unwrap_err();
    assert!(matches!(error, StoreError::Constraint { .. }), "{error:?}");
}

#[test]
fn only_a_folder_workspace_can_share_its_writers() {
    let (store, project) = store_with_project();
    let mut folder = workspace_row(&project.id, "team", "ws-team");
    folder.kind = WorkspaceKind::Folder;
    folder.branch = None;
    folder.base_ref = None;
    folder.created_branch = false;
    store.insert_workspace(&folder).unwrap();
    let worktree = workspace_row(&project.id, "feature", "ws-feature");
    store.insert_workspace(&worktree).unwrap();

    assert!(!store.workspace_shares_writers(&folder.id).unwrap());
    assert!(store.share_workspace_writers(&folder.id).unwrap());
    assert!(store.workspace_shares_writers(&folder.id).unwrap());

    assert!(!store.share_workspace_writers(&worktree.id).unwrap());
    assert!(!store.workspace_shares_writers(&worktree.id).unwrap());
    assert!(!store.share_workspace_writers("missing").unwrap());
    assert!(!store.workspace_shares_writers("missing").unwrap());
}

#[test]
fn update_renames_and_comments_one_workspace_and_keeps_its_directory_and_branch() {
    let (store, project) = store_with_project();
    let row = workspace_row(&project.id, "feature", "ws-feature");
    store.insert_workspace(&row).unwrap();
    let other = workspace_row(&project.id, "other", "ws-other");
    store.insert_workspace(&other).unwrap();

    assert!(
        store
            .update_workspace(&row.id, "renamed", Some("waiting on review"), false)
            .unwrap()
    );
    let found = store.get_workspace(&row.id).unwrap().unwrap();
    assert_eq!(found.name, "renamed");
    assert_eq!(found.comment.as_deref(), Some("waiting on review"));
    assert_eq!(found.path, row.path);
    assert_eq!(found.branch, row.branch);
    assert_eq!(
        store.get_workspace(&other.id).unwrap().unwrap().comment,
        None
    );

    assert!(
        store
            .update_workspace(&row.id, "renamed", None, false)
            .unwrap()
    );
    assert_eq!(store.get_workspace(&row.id).unwrap().unwrap().comment, None);
    assert!(
        !store
            .update_workspace("missing", "renamed", None, false)
            .unwrap()
    );
}

#[test]
fn update_to_a_name_another_workspace_of_the_project_holds_changes_nothing() {
    let (store, project) = store_with_project();
    store
        .insert_workspace(&workspace_row(&project.id, "feature", "ws-feature"))
        .unwrap();
    let other = workspace_row(&project.id, "other", "ws-other");
    store.insert_workspace(&other).unwrap();

    let result = store
        .update_workspace(&other.id, "feature", Some("note"), false)
        .map(|_| ());
    assert_conflict(result, "workspaces.name");
    let kept = store.get_workspace(&other.id).unwrap().unwrap();
    assert_eq!(kept.name, "other");
    assert_eq!(kept.comment, None);
}

#[test]
fn the_generated_name_flag_round_trips_and_an_update_sets_it() {
    let (store, project) = store_with_project();
    let row = WorkspaceRow {
        name_generated: true,
        ..workspace_row(&project.id, "otter", "ws-otter")
    };
    store.insert_workspace(&row).unwrap();
    assert!(
        store
            .get_workspace(&row.id)
            .unwrap()
            .unwrap()
            .name_generated
    );

    assert!(
        store
            .update_workspace(&row.id, "typed", None, false)
            .unwrap()
    );
    assert!(
        !store
            .get_workspace(&row.id)
            .unwrap()
            .unwrap()
            .name_generated
    );
}

#[test]
fn renaming_sets_the_name_and_branch_and_keeps_the_path() {
    let (store, project) = store_with_project();
    let row = WorkspaceRow {
        name_generated: true,
        ..workspace_row(&project.id, "otter", "ws-otter")
    };
    store.insert_workspace(&row).unwrap();

    assert!(
        store
            .rename_workspace(&row.id, "fix-login", Some("fix-login"))
            .unwrap()
    );
    let found = store.get_workspace(&row.id).unwrap().unwrap();
    assert_eq!(found.name, "fix-login");
    assert_eq!(found.branch.as_deref(), Some("fix-login"));
    assert_eq!(found.path, row.path);
    assert!(found.name_generated);
    assert!(!store.rename_workspace("missing", "x", None).unwrap());
}
