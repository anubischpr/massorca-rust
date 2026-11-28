//! The automation timer driven with an injected clock: one claim per occurrence, the grace window, missed occurrences
//! recorded and never replayed, and the no-project refusal.

use massorca_core_store::{
    AutomationRow, AutomationRunRow, ProjectKind, ProjectRow, Store, new_id, run_state,
};
use massorca_protocol::codes;

use super::tick;

const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;
/// 2026-11-25 09:00 UTC, an occurrence of a daily 09:00 automation in UTC.
const DUE: i64 = 1_795_597_200_000;

/// A store with one daily 09:00 UTC automation due at `next_run_at`, in a project when `with_project`.
fn store_with(next_run_at: i64, with_project: bool) -> (Store, String) {
    let store = Store::open_in_memory().expect("in-memory store opens");
    let project_id = with_project.then(|| {
        let project = ProjectRow {
            id: new_id(),
            kind: ProjectKind::Folder,
            path: "/work/app".to_owned(),
            path_key: "/work/app".to_owned(),
            name: "app".to_owned(),
            workspaces_root: "/work/app-workspaces".to_owned(),
            added_at: 1,
            base_ref: None,
            icon: None,
        };
        store.insert_project(&project).unwrap();
        project.id
    });
    let row = AutomationRow {
        id: new_id(),
        project_id,
        name: "Morning PRs".to_owned(),
        prompt: "Summarise the open pull requests".to_owned(),
        agent_kind: "claude".to_owned(),
        cadence: "daily".to_owned(),
        time: "09:00".to_owned(),
        weekday: None,
        cron: None,
        zone: Some("UTC".to_owned()),
        grace_minutes: None,
        enabled: true,
        creation_key: None,
        next_run_at: Some(next_run_at),
        last_run_at: None,
        created_at: 1,
        updated_at: 1,
    };
    store.insert_automation(&row).unwrap();
    (store, row.id)
}

/// Runs one check whose launch only notes the runs it was given.
fn check(store: &Store, now: i64) -> Vec<AutomationRunRow> {
    let mut launched = Vec::new();
    tick(store, now, &mut |_automation, run| {
        launched.push(run.clone());
        None
    })
    .unwrap();
    launched
}

fn runs(store: &Store, id: &str) -> Vec<AutomationRunRow> {
    store.list_automation_runs(id, 10, None).unwrap()
}

#[test]
fn a_due_occurrence_is_claimed_once_and_the_schedule_moves_on() {
    let (store, id) = store_with(DUE, true);

    let launched = check(&store, DUE + 30_000);

    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].scheduled_for, DUE);
    assert_eq!(launched[0].state, run_state::DISPATCHING);
    assert!(check(&store, DUE + 30_000).is_empty());
    let automation = store.get_automation(&id).unwrap().unwrap();
    assert_eq!(automation.next_run_at, Some(DUE + DAY));
    assert_eq!(automation.last_run_at, Some(DUE + 30_000));
    assert_eq!(runs(&store, &id).len(), 1);
}

#[test]
fn an_occurrence_late_within_its_grace_still_runs() {
    let (store, _) = store_with(DUE, true);

    let launched = check(&store, DUE + 700 * MINUTE);

    assert_eq!(launched.len(), 1);
}

#[test]
fn an_occurrence_past_its_grace_is_recorded_missed_and_does_not_run() {
    let (store, id) = store_with(DUE, true);

    assert!(check(&store, DUE + 800 * MINUTE).is_empty());

    let recorded = runs(&store, &id);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].state, run_state::SKIPPED_MISSED);
    assert_eq!(
        recorded[0].reason_code.as_deref(),
        Some(codes::AUTOMATION_RUN_MISSED)
    );
    assert!(recorded[0].ended_at.is_some());
    let automation = store.get_automation(&id).unwrap().unwrap();
    assert_eq!(automation.next_run_at, Some(DUE + DAY));
    assert_eq!(automation.last_run_at, None);
}

#[test]
fn earlier_missed_occurrences_are_folded_into_one_row_and_never_replayed() {
    let (store, id) = store_with(DUE - 2 * DAY, true);

    let launched = check(&store, DUE + 10 * MINUTE);

    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].scheduled_for, DUE);
    let recorded = runs(&store, &id);
    assert_eq!(recorded.len(), 2);
    let missed = &recorded[1];
    assert_eq!(missed.state, run_state::SKIPPED_MISSED);
    assert_eq!(missed.scheduled_for, DUE - 2 * DAY);
    assert_eq!(missed.occurrences, 2);
    assert!(check(&store, DUE + 20 * MINUTE).is_empty());
    assert_eq!(runs(&store, &id).len(), 2);
}

#[test]
fn an_automation_without_a_project_records_why_and_starts_nothing() {
    let (store, id) = store_with(DUE, false);

    assert!(check(&store, DUE).is_empty());

    let recorded = runs(&store, &id);
    assert_eq!(recorded[0].state, run_state::SKIPPED_UNAVAILABLE);
    assert_eq!(
        recorded[0].reason_code.as_deref(),
        Some(codes::AUTOMATION_NO_PROJECT)
    );
    assert_eq!(
        store.get_automation(&id).unwrap().unwrap().next_run_at,
        Some(DUE + DAY)
    );
}

#[test]
fn a_disabled_automation_is_never_claimed() {
    let (store, id) = store_with(DUE, true);
    let mut row = store.get_automation(&id).unwrap().unwrap();
    row.enabled = false;
    store.update_automation(&row).unwrap();

    assert!(check(&store, DUE).is_empty());
    assert!(runs(&store, &id).is_empty());
}
