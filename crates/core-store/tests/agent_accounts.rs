//! Agent accounts (decision 0059): accounts per kind, the active account per kind, per-project choices, and the
//! cascades when an account or a project goes.

use massorca_core_store::{
    AgentAccountRow, ProjectKind, ProjectRow, Store, StoreError, new_id, now_ms,
};

/// A git project row with a fresh id.
fn project_row(name: &str, path_key: &str) -> ProjectRow {
    ProjectRow {
        id: new_id(),
        kind: ProjectKind::Git,
        path: format!("/repos/{name}"),
        path_key: path_key.to_owned(),
        name: name.to_owned(),
        workspaces_root: format!("/repos/{name}-workspaces"),
        added_at: now_ms(),
    }
}

fn account(kind: &str, label: &str, created_at: i64) -> AgentAccountRow {
    AgentAccountRow {
        id: new_id(),
        kind: kind.to_owned(),
        label: label.to_owned(),
        created_at,
        identity: None,
        state: "needs_sign_in".to_owned(),
    }
}

#[test]
fn an_account_round_trips_and_lists_per_kind_oldest_first() {
    let store = Store::open_in_memory().unwrap();
    let work = account("fake", "work", 200);
    let home = account("fake", "home", 100);
    let other = account("other", "elsewhere", 50);
    store.insert_agent_account(&work).unwrap();
    store.insert_agent_account(&home).unwrap();
    store.insert_agent_account(&other).unwrap();

    assert_eq!(
        store.get_agent_account(&work.id).unwrap(),
        Some(work.clone())
    );
    let labels: Vec<String> = store
        .list_agent_accounts("fake")
        .unwrap()
        .into_iter()
        .map(|row| row.label)
        .collect();
    assert_eq!(labels, ["home", "work"]);
}

#[test]
fn a_check_records_the_state_and_an_identity_only_when_given() {
    let store = Store::open_in_memory().unwrap();
    let row = account("fake", "work", 1);
    store.insert_agent_account(&row).unwrap();

    assert!(
        store
            .set_agent_account_check(&row.id, "signed_in", Some("fake@example.com"))
            .unwrap()
    );
    assert!(
        store
            .set_agent_account_check(&row.id, "expired", None)
            .unwrap()
    );

    let read = store.get_agent_account(&row.id).unwrap().unwrap();
    assert_eq!(read.state, "expired");
    assert_eq!(read.identity.as_deref(), Some("fake@example.com"));
    assert!(
        !store
            .set_agent_account_check("missing", "expired", None)
            .unwrap()
    );
}

#[test]
fn a_state_outside_the_vocabulary_is_refused() {
    let store = Store::open_in_memory().unwrap();
    let mut row = account("fake", "work", 1);
    row.state = "logged_in".to_owned();
    let error = store.insert_agent_account(&row).unwrap_err();
    assert!(matches!(error, StoreError::Constraint { .. }), "{error:?}");
}

#[test]
fn one_account_is_active_per_kind_and_activating_another_replaces_it() {
    let store = Store::open_in_memory().unwrap();
    let first = account("fake", "work", 1);
    let second = account("fake", "home", 2);
    store.insert_agent_account(&first).unwrap();
    store.insert_agent_account(&second).unwrap();

    assert_eq!(store.get_active_agent_account("fake").unwrap(), None);
    store.set_active_agent_account("fake", &first.id).unwrap();
    assert_eq!(
        store.get_active_agent_account("fake").unwrap().as_deref(),
        Some(first.id.as_str())
    );
    store.set_active_agent_account("fake", &second.id).unwrap();
    assert_eq!(
        store.get_active_agent_account("fake").unwrap().as_deref(),
        Some(second.id.as_str())
    );
}

#[test]
fn a_project_choice_is_per_kind_and_project_and_lists_its_projects() {
    let store = Store::open_in_memory().unwrap();
    let app = project_row("app", "/repos/app");
    let docs = project_row("docs", "/repos/docs");
    store.insert_project(&app).unwrap();
    store.insert_project(&docs).unwrap();
    let work = account("fake", "work", 1);
    store.insert_agent_account(&work).unwrap();

    store
        .set_agent_account_choice("fake", &app.id, &work.id)
        .unwrap();
    assert_eq!(
        store
            .get_agent_account_choice("fake", &app.id)
            .unwrap()
            .as_deref(),
        Some(work.id.as_str())
    );
    assert_eq!(
        store.get_agent_account_choice("fake", &docs.id).unwrap(),
        None
    );
    assert_eq!(
        store.get_agent_account_choice("other", &app.id).unwrap(),
        None
    );
    assert_eq!(
        store.projects_using_agent_account(&work.id).unwrap(),
        vec![app.id.clone()]
    );
}

#[test]
fn a_choice_needs_a_known_project() {
    let store = Store::open_in_memory().unwrap();
    let work = account("fake", "work", 1);
    store.insert_agent_account(&work).unwrap();
    let error = store
        .set_agent_account_choice("fake", "no-such-project", &work.id)
        .unwrap_err();
    assert!(matches!(error, StoreError::Constraint { .. }), "{error:?}");
}

#[test]
fn removing_an_account_clears_its_active_mark_and_its_choices() {
    let store = Store::open_in_memory().unwrap();
    let app = project_row("app", "/repos/app");
    store.insert_project(&app).unwrap();
    let work = account("fake", "work", 1);
    store.insert_agent_account(&work).unwrap();
    store.set_active_agent_account("fake", &work.id).unwrap();
    store
        .set_agent_account_choice("fake", &app.id, &work.id)
        .unwrap();

    assert!(store.delete_agent_account(&work.id).unwrap());
    assert_eq!(store.get_agent_account(&work.id).unwrap(), None);
    assert_eq!(store.get_active_agent_account("fake").unwrap(), None);
    assert_eq!(
        store.get_agent_account_choice("fake", &app.id).unwrap(),
        None
    );
    assert!(!store.delete_agent_account(&work.id).unwrap());
}

#[test]
fn removing_a_project_clears_its_choices() {
    let store = Store::open_in_memory().unwrap();
    let app = project_row("app", "/repos/app");
    store.insert_project(&app).unwrap();
    let work = account("fake", "work", 1);
    store.insert_agent_account(&work).unwrap();
    store
        .set_agent_account_choice("fake", &app.id, &work.id)
        .unwrap();

    assert!(store.delete_project(&app.id).unwrap());
    assert!(
        store
            .projects_using_agent_account(&work.id)
            .unwrap()
            .is_empty()
    );
}
