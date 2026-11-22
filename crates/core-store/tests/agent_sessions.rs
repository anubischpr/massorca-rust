//! Agent session rows: insert, read, list newest first, exit and status recording, the service-stopped sweep and the foreign keys.

mod common;

use common::{assert_conflict, project_row, workspace_row};
use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, Store, StoreError, StoredActivity, new_id,
};

/// A store with one project and one workspace; returns the store and the workspace id.
fn store_with_workspace() -> (Store, String) {
    let store = Store::open_in_memory().unwrap();
    let project = project_row("app", "/repos/app");
    store.insert_project(&project).unwrap();
    let workspace = workspace_row(&project.id, "main", "/repos/app-workspaces/main");
    store.insert_workspace(&workspace).unwrap();
    (store, workspace.id)
}

fn session_row(workspace_id: &str, created_at: i64) -> AgentSessionRow {
    AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace_id.to_owned(),
        parent_id: None,
        title: Some("a title".to_owned()),
        read_only: false,
        agent_ref: Some("ref-1".to_owned()),
        terminal_id: Some(new_id()),
        request_id: None,
        created_at,
        exit: None,
        activity: None,
        result: None,
        brought_back_at: None,
    }
}

#[test]
fn a_session_round_trips_with_every_field() {
    let (store, workspace) = store_with_workspace();
    let mut row = session_row(&workspace, 10);
    row.request_id = Some("req-1".to_owned());
    row.exit = Some(RunExit {
        code: Some(3),
        signal: None,
        reason: RunExitReason::Exited,
        at: 20,
    });
    row.activity = Some(StoredActivity {
        state: "done".to_owned(),
        outcome: Some("failure".to_owned()),
        source: "hook".to_owned(),
        at: 19,
    });
    row.result = Some("result text".to_owned());
    row.brought_back_at = Some(30);
    store.insert_agent_session(&row).unwrap();

    assert_eq!(store.get_agent_session(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(
        store.find_agent_session_by_request_id("req-1").unwrap(),
        Some(row)
    );
    assert_eq!(
        store.find_agent_session_by_request_id("other").unwrap(),
        None
    );
}

#[test]
fn sessions_list_newest_first_and_per_workspace() {
    let (store, workspace) = store_with_workspace();
    let project = store.list_projects().unwrap().remove(0);
    let other = workspace_row(&project.id, "other", "/repos/app-workspaces/other");
    store.insert_workspace(&other).unwrap();
    let older = session_row(&workspace, 10);
    let newer = session_row(&workspace, 20);
    let elsewhere = session_row(&other.id, 15);
    for row in [&older, &newer, &elsewhere] {
        store.insert_agent_session(row).unwrap();
    }

    let ids = |rows: Vec<AgentSessionRow>| rows.into_iter().map(|row| row.id).collect::<Vec<_>>();
    assert_eq!(
        ids(store.list_agent_sessions(None).unwrap()),
        [newer.id.clone(), elsewhere.id.clone(), older.id.clone()]
    );
    assert_eq!(
        ids(store.list_agent_sessions(Some(&workspace)).unwrap()),
        [newer.id, older.id]
    );
}

#[test]
fn an_exit_is_recorded_once() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    let killed = RunExit {
        code: None,
        signal: None,
        reason: RunExitReason::Killed,
        at: 11,
    };
    assert!(store.set_agent_session_exit(&row.id, &killed).unwrap());
    let later = RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 12,
    };
    assert!(!store.set_agent_session_exit(&row.id, &later).unwrap());
    assert_eq!(
        store.get_agent_session(&row.id).unwrap().unwrap().exit,
        Some(killed)
    );
}

#[test]
fn the_startup_sweep_marks_only_runs_not_seen_to_end() {
    let (store, workspace) = store_with_workspace();
    let live = session_row(&workspace, 10);
    let mut ended = session_row(&workspace, 11);
    let ended_exit = RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 12,
    };
    ended.exit = Some(ended_exit.clone());
    store.insert_agent_session(&live).unwrap();
    store.insert_agent_session(&ended).unwrap();

    assert_eq!(store.mark_agent_runs_service_stopped(99).unwrap(), 1);
    let swept = store
        .get_agent_session(&live.id)
        .unwrap()
        .unwrap()
        .exit
        .unwrap();
    assert_eq!(
        (swept.reason, swept.at),
        (RunExitReason::ServiceStopped, 99)
    );
    assert_eq!(
        store.get_agent_session(&ended.id).unwrap().unwrap().exit,
        Some(ended_exit)
    );
}

#[test]
fn prefixes_match_literally() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    let found = store
        .find_agent_sessions_by_prefix(&row.id[..8], 2)
        .unwrap();
    assert_eq!(found.len(), 1);
    assert!(
        store
            .find_agent_sessions_by_prefix("%", 2)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn removing_a_workspace_removes_its_sessions() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    assert!(store.delete_workspace(&workspace).unwrap());
    assert_eq!(store.get_agent_session(&row.id).unwrap(), None);
}

#[test]
fn forgetting_a_parent_keeps_its_children_without_a_parent() {
    let (store, workspace) = store_with_workspace();
    let parent = session_row(&workspace, 10);
    let mut child = session_row(&workspace, 11);
    child.parent_id = Some(parent.id.clone());
    store.insert_agent_session(&parent).unwrap();
    store.insert_agent_session(&child).unwrap();

    assert!(store.delete_agent_session(&parent.id).unwrap());
    assert_eq!(
        store
            .get_agent_session(&child.id)
            .unwrap()
            .unwrap()
            .parent_id,
        None
    );
}

#[test]
fn an_unknown_workspace_or_a_repeated_request_id_is_refused() {
    let (store, workspace) = store_with_workspace();
    let orphan = session_row("no-such-workspace", 10);
    assert!(matches!(
        store.insert_agent_session(&orphan),
        Err(StoreError::Constraint { .. })
    ));
    let mut first = session_row(&workspace, 10);
    first.request_id = Some("req".to_owned());
    let mut second = session_row(&workspace, 11);
    second.request_id = Some("req".to_owned());
    store.insert_agent_session(&first).unwrap();
    assert_conflict(
        store.insert_agent_session(&second),
        "agent_sessions.request_id",
    );
}

#[test]
fn a_status_change_writes_activity_and_result_and_learns_the_ref_once() {
    let (store, workspace) = store_with_workspace();
    let mut row = session_row(&workspace, 10);
    row.agent_ref = None;
    store.insert_agent_session(&row).unwrap();
    let working = StoredActivity {
        state: "working".to_owned(),
        outcome: None,
        source: "hook".to_owned(),
        at: 11,
    };

    assert!(
        store
            .set_agent_session_status(&row.id, Some(&working), None, Some("learned"))
            .unwrap()
    );
    let read = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(read.activity, Some(working));
    assert_eq!(read.result, None);
    assert_eq!(read.agent_ref.as_deref(), Some("learned"));

    let done = StoredActivity {
        state: "done".to_owned(),
        outcome: Some("success".to_owned()),
        source: "hook".to_owned(),
        at: 12,
    };
    store
        .set_agent_session_status(&row.id, Some(&done), Some("the result"), Some("other"))
        .unwrap();
    let read = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(read.activity, Some(done));
    assert_eq!(read.result.as_deref(), Some("the result"));
    assert_eq!(
        read.agent_ref.as_deref(),
        Some("learned"),
        "a ref already set is kept"
    );

    store
        .set_agent_session_status(&row.id, None, None, None)
        .unwrap();
    let read = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(read.activity, None);
    assert_eq!(read.result, None);
    assert_eq!(read.agent_ref.as_deref(), Some("learned"));
    assert!(
        !store
            .set_agent_session_status("no-such-session", None, None, None)
            .unwrap()
    );
}

#[test]
fn a_restart_moves_an_exited_session_to_a_new_terminal_and_clears_its_exit_and_activity() {
    let (store, workspace) = store_with_workspace();
    let mut row = session_row(&workspace, 10);
    row.exit = Some(RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 20,
    });
    row.activity = Some(StoredActivity {
        state: "done".to_owned(),
        outcome: Some("success".to_owned()),
        source: "hook".to_owned(),
        at: 19,
    });
    row.result = Some("kept".to_owned());
    store.insert_agent_session(&row).unwrap();

    let terminal = new_id();
    assert!(store.restart_agent_session(&row.id, &terminal).unwrap());
    let restarted = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(restarted.terminal_id.as_deref(), Some(terminal.as_str()));
    assert_eq!(restarted.exit, None);
    assert_eq!(restarted.activity, None, "absent until the agent reports");
    assert_eq!(restarted.result.as_deref(), Some("kept"));
    assert_eq!(restarted.agent_ref.as_deref(), Some("ref-1"));
}

#[test]
fn a_live_session_is_not_restarted() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();

    assert!(!store.restart_agent_session(&row.id, &new_id()).unwrap());
    assert_eq!(store.get_agent_session(&row.id).unwrap(), Some(row));
}
