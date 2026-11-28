//! Automation destinations (decision 0127): the stored destination fields, and refusals that repeat folding into one run
//! row with a count, without giving any run number twice.

use massorca_core_store::{AutomationRow, AutomationRunRow, Store, destination, new_id, run_state};

const NO_PROJECT_CODE: &str = "automation.no_project";
const NO_PROJECT_SENTENCE: &str = "The automation has no project, so the run did not start.";

fn store_with_automation() -> (Store, AutomationRow) {
    let store = Store::open_in_memory().expect("in-memory store opens");
    let row = AutomationRow {
        id: new_id(),
        project_id: None,
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
        next_run_at: None,
        last_run_at: None,
        destination: destination::PROJECT_DEFAULT.to_owned(),
        destination_workspace_id: None,
        destination_path: None,
        base_branch: None,
        created_at: 1,
        updated_at: 1,
    };
    store.insert_automation(&row).unwrap();
    (store, row)
}

fn refusal(automation_id: &str, scheduled_for: i64) -> AutomationRunRow {
    AutomationRunRow {
        id: new_id(),
        automation_id: automation_id.to_owned(),
        trigger: "schedule".to_owned(),
        scheduled_for,
        occurrences: 1,
        state: run_state::SKIPPED_UNAVAILABLE.to_owned(),
        reason_code: Some(NO_PROJECT_CODE.to_owned()),
        reason: Some(NO_PROJECT_SENTENCE.to_owned()),
        ended_at: Some(scheduled_for),
        created_at: scheduled_for,
        ..AutomationRunRow::default()
    }
}

fn dispatched(automation_id: &str, scheduled_for: i64) -> AutomationRunRow {
    AutomationRunRow {
        state: run_state::DISPATCHED.to_owned(),
        ..refusal(automation_id, scheduled_for)
    }
}

#[test]
fn a_destination_is_read_back_with_its_workspace_path_and_base_branch() {
    let (store, mut row) = store_with_automation();
    row.destination = destination::PROJECT_WORKTREE_PER_RUN.to_owned();
    row.destination_workspace_id = None;
    row.base_branch = Some("main".to_owned());
    assert!(store.update_automation(&row).unwrap());
    let read = store.get_automation(&row.id).unwrap().expect("stored");
    assert_eq!(read.destination, destination::PROJECT_WORKTREE_PER_RUN);
    assert_eq!(read.base_branch.as_deref(), Some("main"));

    row.destination = destination::EXISTING_WORKSPACE.to_owned();
    row.destination_workspace_id = Some("ws-1".to_owned());
    row.destination_path = Some("/work/app-feature".to_owned());
    assert!(store.update_automation(&row).unwrap());
    let read = store.get_automation(&row.id).unwrap().expect("stored");
    assert_eq!(read.destination, destination::EXISTING_WORKSPACE);
    assert_eq!(read.destination_workspace_id.as_deref(), Some("ws-1"));
    assert_eq!(read.destination_path.as_deref(), Some("/work/app-feature"));
}

#[test]
fn a_destination_outside_the_three_kinds_is_refused_by_the_table() {
    let (store, mut row) = store_with_automation();
    row.destination = "somewhere_else".to_owned();
    assert!(store.update_automation(&row).is_err());
}

#[test]
fn a_repeated_refusal_folds_into_the_newest_run_and_its_number_is_not_reused() {
    let (store, row) = store_with_automation();
    let mut first = refusal(&row.id, 100);
    store.insert_automation_run(&mut first, None).unwrap();
    let mut second = refusal(&row.id, 200);
    store.insert_automation_run(&mut second, None).unwrap();
    let mut third = refusal(&row.id, 300);
    store.insert_automation_run(&mut third, None).unwrap();

    let runs = store.list_automation_runs(&row.id, 10, None).unwrap();
    assert_eq!(runs.len(), 1, "three repeats are one row");
    assert_eq!(runs[0].number, 1);
    assert_eq!(runs[0].occurrences, 3);
    assert_eq!(
        runs[0].scheduled_for, 300,
        "the row shows the newest occurrence"
    );
    assert_eq!(third.id, first.id, "the caller sees the folded row");

    let mut other = dispatched(&row.id, 400);
    store.insert_automation_run(&mut other, None).unwrap();
    assert_eq!(other.number, 2);
}

#[test]
fn a_refusal_after_another_run_starts_a_new_row() {
    let (store, row) = store_with_automation();
    let mut first = refusal(&row.id, 100);
    store.insert_automation_run(&mut first, None).unwrap();
    let mut between = dispatched(&row.id, 200);
    store.insert_automation_run(&mut between, None).unwrap();
    let mut again = refusal(&row.id, 300);
    store.insert_automation_run(&mut again, None).unwrap();

    let runs = store.list_automation_runs(&row.id, 10, None).unwrap();
    assert_eq!(runs.len(), 3);
    assert_eq!(again.number, 3);
    assert_eq!(again.occurrences, 1);
}

#[test]
fn a_refusal_with_another_code_starts_a_new_row() {
    let (store, row) = store_with_automation();
    let mut first = refusal(&row.id, 100);
    store.insert_automation_run(&mut first, None).unwrap();
    let mut other = refusal(&row.id, 200);
    other.reason_code = Some("automation.destination_removed".to_owned());
    store.insert_automation_run(&mut other, None).unwrap();
    assert_eq!(other.number, 2);
    assert_eq!(other.occurrences, 1);
}

#[test]
fn a_refusal_from_another_trigger_does_not_fold_into_the_newest_run() {
    let (store, row) = store_with_automation();
    let mut scheduled = refusal(&row.id, 100);
    store.insert_automation_run(&mut scheduled, None).unwrap();
    let mut manual = refusal(&row.id, 200);
    manual.trigger = "manual".to_owned();
    store.insert_automation_run(&mut manual, None).unwrap();

    let runs = store.list_automation_runs(&row.id, 10, None).unwrap();
    assert_eq!(runs.len(), 2, "a manual refusal keeps its own row");
    assert_eq!(manual.number, 2);
    assert_eq!(manual.occurrences, 1);
}
