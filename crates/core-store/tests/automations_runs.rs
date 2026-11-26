//! Automation runs (decision 0125): the claim that stores an occurrence once, run numbers that are never reused, a run
//! that ends by its session, retention that never evicts a run in progress, and history removed with its automation.

use massorca_core_store::{AutomationRow, AutomationRunRow, Store, new_id, run_state};

fn store_with_automation(next_run_at: Option<i64>) -> (Store, AutomationRow) {
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
        next_run_at,
        last_run_at: None,
        destination: "project_default".to_owned(),
        destination_workspace_id: None,
        destination_path: None,
        base_branch: None,
        created_at: 1,
        updated_at: 1,
    };
    store.insert_automation(&row).unwrap();
    (store, row)
}

fn run(automation_id: &str, state: &str, scheduled_for: i64) -> AutomationRunRow {
    AutomationRunRow {
        id: new_id(),
        automation_id: automation_id.to_owned(),
        trigger: "schedule".to_owned(),
        scheduled_for,
        occurrences: 1,
        state: state.to_owned(),
        created_at: scheduled_for,
        ..AutomationRunRow::default()
    }
}

#[test]
fn a_claim_moves_the_next_run_once_and_a_second_claim_of_the_same_occurrence_writes_nothing() {
    let (store, automation) = store_with_automation(Some(100));
    let mut first = [run(&automation.id, run_state::DISPATCHING, 100)];
    assert!(
        store
            .claim_automation_runs(&automation.id, 100, Some(200), Some(100), &mut first)
            .unwrap()
    );
    let mut second = [run(&automation.id, run_state::DISPATCHING, 100)];
    assert!(
        !store
            .claim_automation_runs(&automation.id, 100, Some(200), Some(100), &mut second)
            .unwrap()
    );
    let stored = store.get_automation(&automation.id).unwrap().unwrap();
    assert_eq!(stored.next_run_at, Some(200));
    assert_eq!(stored.last_run_at, Some(100));
    assert_eq!(
        store
            .list_automation_runs(&automation.id, 10, None)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn run_numbers_count_up_and_are_not_reused_after_pruning() {
    let (store, automation) = store_with_automation(None);
    for at in 1..=5 {
        let mut row = run(&automation.id, run_state::COMPLETED, at);
        store.insert_automation_run(&mut row, None).unwrap();
        assert_eq!(row.number, at);
    }
    assert_eq!(store.prune_automation_runs(&automation.id, 2).unwrap(), 3);
    let mut next = run(&automation.id, run_state::COMPLETED, 6);
    store.insert_automation_run(&mut next, None).unwrap();
    assert_eq!(next.number, 6);
}

#[test]
fn retention_keeps_the_newest_final_runs_and_never_evicts_a_run_in_progress() {
    let (store, automation) = store_with_automation(None);
    let mut in_progress = run(&automation.id, run_state::DISPATCHED, 1);
    store.insert_automation_run(&mut in_progress, None).unwrap();
    for at in 2..=4 {
        let mut row = run(&automation.id, run_state::COMPLETED, at);
        store.insert_automation_run(&mut row, None).unwrap();
    }
    assert_eq!(store.prune_automation_runs(&automation.id, 1).unwrap(), 2);
    let numbers: Vec<i64> = store
        .list_automation_runs(&automation.id, 10, None)
        .unwrap()
        .iter()
        .map(|row| row.number)
        .collect();
    assert_eq!(numbers, [4, 1]);
}

#[test]
fn removing_an_automation_removes_its_run_history() {
    let (store, automation) = store_with_automation(None);
    let mut row = run(&automation.id, run_state::COMPLETED, 1);
    store.insert_automation_run(&mut row, None).unwrap();
    assert!(store.delete_automation(&automation.id).unwrap());
    assert!(store.get_automation_run(&row.id).unwrap().is_none());
}

#[test]
fn a_dispatched_run_ends_once_by_its_session_with_its_result() {
    let (store, automation) = store_with_automation(None);
    let mut row = run(&automation.id, run_state::DISPATCHING, 1);
    store.insert_automation_run(&mut row, Some(1)).unwrap();
    assert!(
        store
            .mark_automation_run_dispatched(&row.id, "s1", "w1", 2)
            .unwrap()
    );
    let ended = store
        .end_automation_run_by_session(
            "s1",
            run_state::COMPLETED,
            Some("success"),
            Some("done"),
            None,
            3,
        )
        .unwrap()
        .expect("the run ends");
    assert_eq!(ended.state, run_state::COMPLETED);
    assert_eq!(ended.result.as_deref(), Some("done"));
    assert_eq!(ended.ended_at, Some(3));
    assert!(
        store
            .end_automation_run_by_session("s1", run_state::DISPATCH_FAILED, None, None, None, 4)
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_runs_page_newest_first_from_a_cursor() {
    let (store, automation) = store_with_automation(None);
    for at in 1..=4 {
        let mut row = run(&automation.id, run_state::COMPLETED, at);
        store.insert_automation_run(&mut row, None).unwrap();
    }
    let page: Vec<i64> = store
        .list_automation_runs(&automation.id, 2, Some(4))
        .unwrap()
        .iter()
        .map(|row| row.number)
        .collect();
    assert_eq!(page, [3, 2]);
}

#[test]
fn a_run_left_dispatching_by_a_stop_is_settled_as_failed_and_a_dispatched_one_is_kept() {
    let (store, automation) = store_with_automation(None);
    let mut dispatching = run(&automation.id, run_state::DISPATCHING, 1);
    store.insert_automation_run(&mut dispatching, None).unwrap();
    let mut dispatched = run(&automation.id, run_state::DISPATCHED, 1);
    store.insert_automation_run(&mut dispatched, None).unwrap();
    assert_eq!(
        store
            .settle_dispatching_automation_runs(10, "automation.run_interrupted", "stopped", 11)
            .unwrap(),
        1
    );
    let settled = store.get_automation_run(&dispatching.id).unwrap().unwrap();
    assert_eq!(settled.state, run_state::DISPATCH_FAILED);
    assert_eq!(
        settled.reason_code.as_deref(),
        Some("automation.run_interrupted")
    );
    let kept = store.get_automation_run(&dispatched.id).unwrap().unwrap();
    assert_eq!(kept.state, run_state::DISPATCHED);
}
