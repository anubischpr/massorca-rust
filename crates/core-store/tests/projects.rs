mod common;

use common::{assert_conflict, project_row, workspace_row};
use massorca_core_store::{ProjectKind, Store, StoreError};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

#[test]
fn project_round_trips_through_each_lookup() {
    let store = store();
    let row = project_row("alpha", "key-alpha");
    store.insert_project(&row).unwrap();

    assert_eq!(store.get_project(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(
        store.find_project_by_name("alpha").unwrap(),
        Some(row.clone())
    );
    assert_eq!(
        store.find_project_by_path_key("key-alpha").unwrap(),
        Some(row)
    );
    assert_eq!(store.get_project("missing").unwrap(), None);
    assert_eq!(store.find_project_by_name("missing").unwrap(), None);
}

#[test]
fn folder_project_keeps_its_kind() {
    let store = store();
    let mut row = project_row("notes", "key-notes");
    row.kind = ProjectKind::Folder;
    store.insert_project(&row).unwrap();

    let found = store.get_project(&row.id).unwrap().unwrap();
    assert_eq!(found.kind, ProjectKind::Folder);
}

#[test]
fn duplicate_id_name_or_path_key_is_a_conflict() {
    let store = store();
    let first = project_row("alpha", "key-alpha");
    store.insert_project(&first).unwrap();

    let mut same_id = project_row("beta", "key-beta");
    same_id.id = first.id.clone();
    assert_conflict(store.insert_project(&same_id), "projects.id");

    let same_name = project_row("alpha", "key-other");
    assert_conflict(store.insert_project(&same_name), "projects.name");

    let same_path_key = project_row("gamma", "key-alpha");
    assert_conflict(store.insert_project(&same_path_key), "projects.path_key");

    assert_eq!(store.list_projects().unwrap().len(), 1);
}

#[test]
fn list_projects_orders_by_name() {
    let store = store();
    for name in ["zeta", "alpha", "mu"] {
        store
            .insert_project(&project_row(name, &format!("key-{name}")))
            .unwrap();
    }
    let names: Vec<String> = store
        .list_projects()
        .unwrap()
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["alpha", "mu", "zeta"]);
}

#[test]
fn a_project_base_is_stored_read_back_and_cleared() {
    let store = store();
    let row = project_row("alpha", "key-alpha");
    store.insert_project(&row).unwrap();
    assert_eq!(store.get_project(&row.id).unwrap().unwrap().base_ref, None);

    assert!(
        store
            .set_project_base(&row.id, Some("origin/develop"))
            .unwrap()
    );
    let found = store.find_project_by_name("alpha").unwrap().unwrap();
    assert_eq!(found.base_ref.as_deref(), Some("origin/develop"));

    assert!(store.set_project_base(&row.id, None).unwrap());
    assert_eq!(store.get_project(&row.id).unwrap().unwrap().base_ref, None);
    assert!(!store.set_project_base("missing", Some("main")).unwrap());
}

#[test]
fn delete_project_reports_whether_a_row_was_removed() {
    let store = store();
    let row = project_row("alpha", "key-alpha");
    store.insert_project(&row).unwrap();

    assert!(store.delete_project(&row.id).unwrap());
    assert_eq!(store.get_project(&row.id).unwrap(), None);
    assert!(!store.delete_project(&row.id).unwrap());
}

#[test]
fn deleting_a_project_with_workspaces_is_restricted() {
    let store = store();
    let project = project_row("alpha", "key-alpha");
    store.insert_project(&project).unwrap();
    let workspace = workspace_row(&project.id, "feature", "ws-feature");
    store.insert_workspace(&workspace).unwrap();

    let error = store.delete_project(&project.id).unwrap_err();
    assert!(matches!(error, StoreError::Constraint { .. }), "{error:?}");
    assert!(store.get_project(&project.id).unwrap().is_some());

    assert!(store.delete_workspace(&workspace.id).unwrap());
    assert!(store.delete_project(&project.id).unwrap());
}

#[test]
fn a_project_icon_round_trips_and_can_be_set_and_cleared() {
    let store = store();
    let mut row = project_row("alpha", "key-alpha");
    row.icon = Some(r#"{"type":"emoji","emoji":"🚀"}"#.to_owned());
    store.insert_project(&row).unwrap();
    assert_eq!(store.get_project(&row.id).unwrap(), Some(row.clone()));

    let lucide = r#"{"type":"lucide","name":"Rocket"}"#;
    assert!(store.set_project_icon(&row.id, Some(lucide)).unwrap());
    let found = store.find_project_by_name("alpha").unwrap().unwrap();
    assert_eq!(found.icon.as_deref(), Some(lucide));

    assert!(store.set_project_icon(&row.id, None).unwrap());
    assert_eq!(store.get_project(&row.id).unwrap().unwrap().icon, None);
    assert!(!store.set_project_icon("missing", None).unwrap());
}
