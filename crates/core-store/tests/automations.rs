//! Automation definitions (decision 0123): the stored shape, the creation key, the order of the list, and what a
//! removed project leaves behind.

use massorca_core_store::{
    AutomationRow, ProjectKind, ProjectRow, Store, StoreError, new_id, now_ms,
};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

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
        base_ref: None,
        icon: None,
    }
}

/// A daily automation in Madrid time, created at `created_at`, for the project `project_id` when it has one.
fn automation(project_id: Option<&str>, name: &str, created_at: i64) -> AutomationRow {
    AutomationRow {
        id: new_id(),
        project_id: project_id.map(str::to_owned),
        name: name.to_owned(),
        prompt: "Summarise the open pull requests".to_owned(),
        agent_kind: "claude".to_owned(),
        cadence: "daily".to_owned(),
        time: "09:00".to_owned(),
        weekday: None,
        cron: None,
        zone: Some("Europe/Madrid".to_owned()),
        grace_minutes: None,
        enabled: true,
        creation_key: None,
        next_run_at: Some(1_000),
        last_run_at: None,
        created_at,
        updated_at: created_at,
    }
}

#[test]
fn an_automation_is_read_back_with_every_field() {
    let store = store();
    let project = project_row("app", "/repos/app");
    store.insert_project(&project).unwrap();
    let mut row = automation(Some(&project.id), "Morning PRs", 10);
    row.grace_minutes = Some(30);
    row.enabled = false;
    row.next_run_at = None;
    row.creation_key = Some("client-key-1".to_owned());
    row.zone = None;
    store.insert_automation(&row).unwrap();

    assert_eq!(store.get_automation(&row.id).unwrap(), Some(row));
}

#[test]
fn automations_are_listed_in_creation_order() {
    let store = store();
    let late = automation(None, "late", 30);
    let early = automation(None, "early", 10);
    let middle = automation(None, "middle", 20);
    for row in [&late, &early, &middle] {
        store.insert_automation(row).unwrap();
    }

    let names: Vec<String> = store
        .list_automations()
        .unwrap()
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["early", "middle", "late"]);
}

#[test]
fn a_creation_key_is_kept_for_one_automation_only() {
    let store = store();
    let mut first = automation(None, "first", 10);
    first.creation_key = Some("same-key".to_owned());
    store.insert_automation(&first).unwrap();

    let mut repeat = automation(None, "repeat", 20);
    repeat.creation_key = Some("same-key".to_owned());
    let error = store
        .insert_automation(&repeat)
        .expect_err("the key is taken");
    assert!(matches!(error, StoreError::Conflict { .. }), "{error:?}");

    let found = store
        .find_automation_by_creation_key("same-key")
        .unwrap()
        .expect("the first automation holds the key");
    assert_eq!(found.id, first.id);
}

#[test]
fn a_cadence_and_its_fields_must_agree() {
    let store = store();
    let mut unknown = automation(None, "unknown", 10);
    unknown.cadence = "monthly".to_owned();
    assert!(matches!(
        store.insert_automation(&unknown),
        Err(StoreError::Constraint { .. })
    ));

    let mut weekday_on_daily = automation(None, "weekday", 10);
    weekday_on_daily.weekday = Some("monday".to_owned());
    assert!(matches!(
        store.insert_automation(&weekday_on_daily),
        Err(StoreError::Constraint { .. })
    ));

    let mut weekly_without_day = automation(None, "weekly", 10);
    weekly_without_day.cadence = "weekly".to_owned();
    assert!(matches!(
        store.insert_automation(&weekly_without_day),
        Err(StoreError::Constraint { .. })
    ));

    let mut cron_without_text = automation(None, "cron", 10);
    cron_without_text.cadence = "cron".to_owned();
    assert!(matches!(
        store.insert_automation(&cron_without_text),
        Err(StoreError::Constraint { .. })
    ));
}

#[test]
fn a_grace_window_longer_than_one_week_is_refused() {
    let store = store();
    let mut too_long = automation(None, "grace", 10);
    too_long.grace_minutes = Some(10_081);
    assert!(matches!(
        store.insert_automation(&too_long),
        Err(StoreError::Constraint { .. })
    ));

    too_long.grace_minutes = Some(10_080);
    store.insert_automation(&too_long).unwrap();
}

#[test]
fn an_automation_for_an_unknown_project_is_refused() {
    let store = store();
    let row = automation(Some("no-such-project"), "orphan", 10);
    assert!(matches!(
        store.insert_automation(&row),
        Err(StoreError::Constraint { .. })
    ));
}

#[test]
fn removing_a_project_keeps_its_automations_without_a_project() {
    let store = store();
    let project = project_row("app", "/repos/app");
    store.insert_project(&project).unwrap();
    let row = automation(Some(&project.id), "kept", 10);
    store.insert_automation(&row).unwrap();

    assert!(store.delete_project(&project.id).unwrap());

    let kept = store
        .get_automation(&row.id)
        .unwrap()
        .expect("still stored");
    assert_eq!(kept.project_id, None);
}

#[test]
fn an_update_stores_the_new_fields_and_reports_a_missing_row() {
    let store = store();
    let mut row = automation(None, "before", 10);
    store.insert_automation(&row).unwrap();

    row.name = "after".to_owned();
    row.cadence = "hourly".to_owned();
    row.time = "00:30".to_owned();
    row.enabled = false;
    row.next_run_at = None;
    row.updated_at = 50;
    assert!(store.update_automation(&row).unwrap());
    assert_eq!(store.get_automation(&row.id).unwrap(), Some(row.clone()));

    let mut missing = automation(None, "missing", 20);
    missing.id = "no-such-automation".to_owned();
    assert!(!store.update_automation(&missing).unwrap());
}

#[test]
fn removing_an_automation_happens_once() {
    let store = store();
    let row = automation(None, "gone", 10);
    store.insert_automation(&row).unwrap();

    assert!(store.delete_automation(&row.id).unwrap());
    assert!(!store.delete_automation(&row.id).unwrap());
    assert_eq!(store.get_automation(&row.id).unwrap(), None);
}
