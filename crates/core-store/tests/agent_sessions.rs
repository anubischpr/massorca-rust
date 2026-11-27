//! Agent session rows: insert, read, list newest first, exit and status recording, the service-stopped sweep and the foreign keys.

mod common;

use common::{assert_conflict, project_row, workspace_row};
use massorca_core_store::{
    AgentAccountRow, AgentSessionArchive, AgentSessionRow, BotRow, RunExit, RunExitReason, RunMode,
    Store, StoreError, StoredActivity, TeamRow, new_id,
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
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
        retained: false,
        released_at: None,
    }
}

/// An exited session, which is the only kind a release can take.
fn exited_row(workspace_id: &str) -> AgentSessionRow {
    let mut row = session_row(workspace_id, 10);
    row.exit = Some(RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 20,
    });
    row
}

#[test]
fn a_release_keeps_the_record_and_its_archive_and_marks_it_released() {
    let (store, workspace) = store_with_workspace();
    let row = exited_row(&workspace);
    store.insert_agent_session(&row).unwrap();

    assert!(
        store
            .release_agent_session(&row.id, "last lines", true, 30)
            .unwrap()
    );

    let released = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(released.released_at, Some(30));
    assert_eq!(
        released.exit, row.exit,
        "the record keeps how its run ended"
    );
    assert_eq!(
        store.get_agent_session_archive(&row.id).unwrap(),
        Some(AgentSessionArchive {
            text: "last lines".to_owned(),
            truncated: true,
            released_at: 30,
        })
    );
    assert!(
        !store
            .release_agent_session(&row.id, "later", false, 40)
            .unwrap(),
        "a second release changes nothing"
    );
    assert_eq!(
        store
            .get_agent_session_archive(&row.id)
            .unwrap()
            .unwrap()
            .text,
        "last lines"
    );
}

#[test]
fn a_live_session_cannot_be_released() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();

    assert!(
        !store
            .release_agent_session(&row.id, "text", false, 30)
            .unwrap()
    );
    assert_eq!(store.get_agent_session_archive(&row.id).unwrap(), None);
    assert_eq!(
        store
            .get_agent_session(&row.id)
            .unwrap()
            .unwrap()
            .released_at,
        None
    );
}

#[test]
fn retain_is_a_flag_that_a_release_clears_and_a_restart_keeps() {
    let (store, workspace) = store_with_workspace();
    let row = exited_row(&workspace);
    store.insert_agent_session(&row).unwrap();

    assert!(store.set_agent_session_retained(&row.id, true).unwrap());
    assert!(store.get_agent_session(&row.id).unwrap().unwrap().retained);

    store
        .release_agent_session(&row.id, "text", false, 30)
        .unwrap();
    let released = store.get_agent_session(&row.id).unwrap().unwrap();
    assert!(!released.retained, "an explicit release clears the retain");
    assert_eq!(released.released_at, Some(30));

    assert!(store.set_agent_session_retained(&row.id, true).unwrap());
    let restarted = store
        .restart_agent_session(&row.id, &new_id(), false, &RunMode::default(), None)
        .unwrap();
    assert!(restarted);
    let resumed = store.get_agent_session(&row.id).unwrap().unwrap();
    assert!(resumed.retained, "a new run keeps the retain");
    assert_eq!(resumed.released_at, None, "a new run is not released");
    assert_eq!(
        store.get_agent_session_archive(&row.id).unwrap(),
        None,
        "a new run drops the archive of the one before"
    );
    assert!(!store.set_agent_session_retained("missing", true).unwrap());
}

#[test]
fn deleting_a_session_deletes_its_archive() {
    let (store, workspace) = store_with_workspace();
    let row = exited_row(&workspace);
    store.insert_agent_session(&row).unwrap();
    store
        .release_agent_session(&row.id, "text", false, 30)
        .unwrap();

    assert!(store.delete_agent_session(&row.id).unwrap());
    assert_eq!(store.get_agent_session_archive(&row.id).unwrap(), None);
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
        seen_at: None,
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
fn an_agent_ref_is_set_only_when_the_session_has_none() {
    let (store, workspace) = store_with_workspace();
    let row = AgentSessionRow {
        agent_ref: None,
        ..session_row(&workspace, 10)
    };
    store.insert_agent_session(&row).unwrap();
    assert!(
        store
            .set_agent_session_ref_if_unset(&row.id, "thread-1")
            .unwrap()
    );
    assert!(
        !store
            .set_agent_session_ref_if_unset(&row.id, "thread-2")
            .unwrap()
    );
    let stored = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(stored.agent_ref.as_deref(), Some("thread-1"));
    assert_eq!(stored.title, row.title, "nothing else changes");
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
    let terminal = row.terminal_id.clone().expect("the row's terminal");
    assert!(
        store
            .set_agent_session_exit(&row.id, &terminal, &killed)
            .unwrap()
    );
    let later = RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 12,
    };
    assert!(
        !store
            .set_agent_session_exit(&row.id, &terminal, &later)
            .unwrap()
    );
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
        seen_at: None,
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
        seen_at: None,
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
        seen_at: None,
    });
    row.result = Some("kept".to_owned());
    store.insert_agent_session(&row).unwrap();

    let terminal = new_id();
    assert!(
        store
            .restart_agent_session(&row.id, &terminal, false, &RunMode::default(), None)
            .unwrap()
    );
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

    assert!(
        !store
            .restart_agent_session(&row.id, &new_id(), false, &RunMode::default(), None)
            .unwrap()
    );
    assert_eq!(store.get_agent_session(&row.id).unwrap(), Some(row));
}

/// An exit records only against the terminal the run ran in: an exit for another terminal changes nothing, so an end of
/// an earlier run cannot land on a session that a resume moved onto a new terminal (decision 0058).
#[test]
fn an_exit_for_another_terminal_changes_nothing() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    let end = RunExit {
        code: Some(1),
        signal: None,
        reason: RunExitReason::Exited,
        at: 20,
    };

    assert!(
        !store
            .set_agent_session_exit(&row.id, &new_id(), &end)
            .unwrap(),
        "an exit for another terminal is not recorded"
    );
    assert_eq!(
        store.get_agent_session(&row.id).unwrap().unwrap().exit,
        None
    );

    let own = row.terminal_id.clone().expect("the row's terminal");
    assert!(store.set_agent_session_exit(&row.id, &own, &end).unwrap());
    assert_eq!(
        store.get_agent_session(&row.id).unwrap().unwrap().exit,
        Some(end)
    );
}

#[test]
fn children_list_oldest_first_and_only_those_of_their_parent() {
    let (store, workspace) = store_with_workspace();
    let parent = session_row(&workspace, 10);
    let other = session_row(&workspace, 11);
    store.insert_agent_session(&parent).unwrap();
    store.insert_agent_session(&other).unwrap();
    let mut later = session_row(&workspace, 30);
    later.parent_id = Some(parent.id.clone());
    let mut earlier = session_row(&workspace, 20);
    earlier.parent_id = Some(parent.id.clone());
    let mut elsewhere = session_row(&workspace, 25);
    elsewhere.parent_id = Some(other.id.clone());
    for row in [&later, &earlier, &elsewhere] {
        store.insert_agent_session(row).unwrap();
    }

    assert!(store.list_agent_children(&later.id).unwrap().is_empty());
    assert_eq!(
        store.list_agent_children(&parent.id).unwrap(),
        vec![earlier, later]
    );
}

#[test]
fn a_bring_back_is_recorded_once() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();

    assert!(store.set_agent_session_brought_back(&row.id, 40).unwrap());
    assert!(!store.set_agent_session_brought_back(&row.id, 50).unwrap());
    assert!(!store.set_agent_session_brought_back("missing", 50).unwrap());
    assert_eq!(
        store
            .get_agent_session(&row.id)
            .unwrap()
            .unwrap()
            .brought_back_at,
        Some(40)
    );
}

#[test]
fn the_secrets_mark_is_sticky_across_restarts() {
    let (store, workspace) = store_with_workspace();
    let exited = || {
        Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: 20,
        })
    };
    let mut row = session_row(&workspace, 10);
    row.exit = exited();
    store.insert_agent_session(&row).unwrap();

    assert!(
        store
            .restart_agent_session(&row.id, "t-1", true, &RunMode::default(), None)
            .unwrap()
    );
    assert!(
        store
            .get_agent_session(&row.id)
            .unwrap()
            .unwrap()
            .holds_secrets
    );
    store
        .set_agent_session_exit(&row.id, "t-1", exited().as_ref().unwrap())
        .unwrap();
    // A later restart without secrets keeps the mark: the scrollback may still hold them.
    assert!(
        store
            .restart_agent_session(&row.id, "t-2", false, &RunMode::default(), None)
            .unwrap()
    );
    let restarted = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(restarted.terminal_id.as_deref(), Some("t-2"));
    assert!(restarted.holds_secrets);
}

#[test]
fn a_session_keeps_its_mode_its_source_and_a_pending_mode_apart() {
    let (store, workspace) = store_with_workspace();
    let mut row = session_row(&workspace, 10);
    row.mode = RunMode {
        permission_mode: Some("careful".to_owned()),
        source: Some("chosen".to_owned()),
        confirmed: true,
    };
    store.insert_agent_session(&row).unwrap();
    assert_eq!(store.get_agent_session(&row.id).unwrap().unwrap(), row);

    assert!(
        store
            .set_agent_session_pending_mode(&row.id, Some("wild"))
            .unwrap()
    );
    let pending = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(pending.mode, row.mode, "the running mode is untouched");
    assert_eq!(pending.pending_permission_mode.as_deref(), Some("wild"));

    let switched = RunMode {
        permission_mode: Some("wild".to_owned()),
        source: Some("chosen".to_owned()),
        confirmed: false,
    };
    assert!(store.set_agent_session_mode(&row.id, &switched).unwrap());
    let read = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(read.mode, switched);
    assert_eq!(read.pending_permission_mode, None, "a switch ends the wait");
    assert!(
        !store
            .set_agent_session_mode("no-such-session", &switched)
            .unwrap()
    );

    let plain = session_row(&workspace, 11);
    store.insert_agent_session(&plain).unwrap();
    let read = store.get_agent_session(&plain.id).unwrap().unwrap();
    assert_eq!(
        read.mode,
        RunMode::default(),
        "a kind with no modes has none"
    );
}

#[test]
fn a_restart_takes_the_new_run_s_mode_and_the_wait_it_keeps() {
    let (store, workspace) = store_with_workspace();
    let mut row = session_row(&workspace, 10);
    row.exit = Some(RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 20,
    });
    store.insert_agent_session(&row).unwrap();
    store
        .set_agent_session_pending_mode(&row.id, Some("wild"))
        .unwrap();

    let vendor = RunMode {
        permission_mode: None,
        source: Some("vendor_default".to_owned()),
        confirmed: false,
    };
    assert!(
        store
            .restart_agent_session(&row.id, "t-2", false, &vendor, Some("wild"))
            .unwrap()
    );
    let restarted = store.get_agent_session(&row.id).unwrap().unwrap();
    assert_eq!(restarted.mode, vendor);
    assert_eq!(restarted.pending_permission_mode.as_deref(), Some("wild"));
}

fn activity(state: &str, outcome: Option<&str>, at: i64) -> StoredActivity {
    StoredActivity {
        state: state.to_owned(),
        outcome: outcome.map(str::to_owned),
        source: "hook".to_owned(),
        at,
        seen_at: None,
    }
}

fn seen_at(store: &Store, id: &str) -> Option<i64> {
    store
        .get_agent_session(id)
        .unwrap()
        .unwrap()
        .activity
        .and_then(|activity| activity.seen_at)
}

#[test]
fn a_seen_mark_holds_until_the_activity_changes() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    let done = activity("done", Some("success"), 11);
    store
        .set_agent_session_status(&row.id, Some(&done), None, None)
        .unwrap();
    assert_eq!(seen_at(&store, &row.id), None, "a new done is unseen");

    assert!(store.mark_agent_session_seen(&row.id, None, 20).unwrap());
    assert_eq!(seen_at(&store, &row.id), Some(20));
    assert!(
        !store.mark_agent_session_seen(&row.id, None, 21).unwrap(),
        "an activity already seen keeps its first mark"
    );
    assert_eq!(seen_at(&store, &row.id), Some(20));

    store
        .set_agent_session_status(&row.id, Some(&done), Some("same turn"), None)
        .unwrap();
    assert_eq!(
        seen_at(&store, &row.id),
        Some(20),
        "rewriting the same activity keeps the mark"
    );

    let waiting = activity("waiting", None, 30);
    store
        .set_agent_session_status(&row.id, Some(&waiting), None, None)
        .unwrap();
    assert_eq!(
        seen_at(&store, &row.id),
        None,
        "a new attention state is unseen"
    );
    let again = activity("done", Some("success"), 31);
    store
        .set_agent_session_status(&row.id, Some(&again), None, None)
        .unwrap();
    assert_eq!(seen_at(&store, &row.id), None);
}

#[test]
fn only_waiting_or_done_can_be_marked_seen() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    assert!(
        !store.mark_agent_session_seen(&row.id, None, 20).unwrap(),
        "no activity, nothing to see"
    );
    for state in ["working", "idle"] {
        store
            .set_agent_session_status(&row.id, Some(&activity(state, None, 11)), None, None)
            .unwrap();
        assert!(
            !store.mark_agent_session_seen(&row.id, None, 20).unwrap(),
            "{state}"
        );
        assert_eq!(seen_at(&store, &row.id), None, "{state}");
    }
    assert!(
        !store
            .mark_agent_session_seen("no-such-session", None, 20)
            .unwrap()
    );
}

#[test]
fn a_restart_clears_the_seen_mark_with_the_activity() {
    let (store, workspace) = store_with_workspace();
    let mut row = session_row(&workspace, 10);
    row.exit = Some(RunExit {
        code: Some(0),
        signal: None,
        reason: RunExitReason::Exited,
        at: 20,
    });
    row.activity = Some(activity("done", Some("success"), 19));
    store.insert_agent_session(&row).unwrap();
    assert!(store.mark_agent_session_seen(&row.id, None, 21).unwrap());

    store
        .restart_agent_session(&row.id, &new_id(), false, &RunMode::default(), None)
        .unwrap();
    store
        .set_agent_session_status(
            &row.id,
            Some(&activity("done", Some("success"), 19)),
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        seen_at(&store, &row.id),
        None,
        "the restart cleared the mark, so the same values come back unseen"
    );
}

#[test]
fn a_seen_mark_survives_reopening_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let id = {
        let store = Store::open(&path).unwrap();
        let project = project_row("app", "/repos/app");
        store.insert_project(&project).unwrap();
        let workspace = workspace_row(&project.id, "main", "/repos/app-workspaces/main");
        store.insert_workspace(&workspace).unwrap();
        let mut row = session_row(&workspace.id, 10);
        row.activity = Some(activity("waiting", None, 11));
        store.insert_agent_session(&row).unwrap();
        assert!(store.mark_agent_session_seen(&row.id, None, 12).unwrap());
        row.id
    };
    let reopened = Store::open(&path).unwrap();
    assert_eq!(seen_at(&reopened, &id), Some(12));
}

fn account_row(kind: &str) -> AgentAccountRow {
    AgentAccountRow {
        id: new_id(),
        kind: kind.to_owned(),
        label: "work".to_owned(),
        created_at: 1,
        identity: None,
        state: "unknown".to_owned(),
        origin: "sign_in".to_owned(),
    }
}

#[test]
fn used_kinds_unite_sessions_accounts_pending_sign_ins_account_history_and_bots() {
    let (store, workspace) = store_with_workspace();
    assert!(store.used_agent_kinds().unwrap().is_empty());

    let mut session = session_row(&workspace, 1);
    session.kind = "from-session".to_owned();
    store.insert_agent_session(&session).unwrap();
    store
        .insert_agent_account(&account_row("from-account"))
        .unwrap();
    store
        .insert_pending_sign_in(&account_row("from-pending"))
        .unwrap();
    // A forgotten session whose kept account is gone: only the account history remembers the kind.
    let mut forgotten = session_row(&workspace, 2);
    forgotten.kind = "from-history".to_owned();
    store.insert_agent_session(&forgotten).unwrap();
    let kept = account_row("from-history");
    store.insert_agent_account(&kept).unwrap();
    store.delete_agent_account(&kept.id).unwrap();
    store.delete_agent_session(&forgotten.id).unwrap();
    let team = TeamRow {
        id: new_id(),
        name: "parsers".to_owned(),
        purpose: "Ships the parser".to_owned(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: 1,
    };
    store.insert_team(&team).unwrap();
    store
        .insert_bot(&BotRow {
            id: new_id(),
            team_id: team.id.clone(),
            name: "builder".to_owned(),
            job: "Writes the parser".to_owned(),
            kind: "from-bot".to_owned(),
            model: None,
            effort: None,
            lead: false,
            created_at: 1,
        })
        .unwrap();

    let kinds: Vec<String> = store.used_agent_kinds().unwrap().into_iter().collect();
    assert_eq!(
        kinds,
        [
            "from-account",
            "from-bot",
            "from-history",
            "from-pending",
            "from-session"
        ]
    );
}

#[test]
fn a_mark_for_an_earlier_activity_does_not_mark_a_newer_one() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    store
        .set_agent_session_status(&row.id, Some(&activity("waiting", None, 11)), None, None)
        .unwrap();
    store
        .set_agent_session_status(
            &row.id,
            Some(&activity("done", Some("success"), 13)),
            None,
            None,
        )
        .unwrap();

    assert!(
        !store
            .mark_agent_session_seen(&row.id, Some(11), 20)
            .unwrap(),
        "the caller saw the waiting at 11, not this done"
    );
    assert_eq!(
        seen_at(&store, &row.id),
        None,
        "the newer done stays unseen"
    );

    assert!(
        store
            .mark_agent_session_seen(&row.id, Some(13), 21)
            .unwrap()
    );
    assert_eq!(
        seen_at(&store, &row.id),
        Some(21),
        "the activity the caller saw is marked"
    );
}

#[test]
fn a_mark_without_the_activity_time_marks_the_current_one() {
    let (store, workspace) = store_with_workspace();
    let row = session_row(&workspace, 10);
    store.insert_agent_session(&row).unwrap();
    store
        .set_agent_session_status(
            &row.id,
            Some(&activity("done", Some("success"), 13)),
            None,
            None,
        )
        .unwrap();
    assert!(store.mark_agent_session_seen(&row.id, None, 20).unwrap());
    assert_eq!(seen_at(&store, &row.id), Some(20));
}

#[test]
fn child_workspaces_are_those_of_delegated_writers_deepest_first() {
    let (store, main) = store_with_workspace();
    let project = store.get_workspace(&main).unwrap().unwrap().project_id;
    let workspace = |name: &str| {
        let row = workspace_row(&project, name, &format!("/repos/app-workspaces/{name}"));
        store.insert_workspace(&row).unwrap();
        row.id
    };
    let (child, grandchild, via_reader, unrelated) = (
        workspace("main-1"),
        workspace("main-1-1"),
        workspace("main-2"),
        workspace("other"),
    );
    let parent = session_row(&main, 10);
    // A read-only child runs in its parent's workspace, so it adds none.
    let mut reader = session_row(&main, 11);
    reader.parent_id = Some(parent.id.clone());
    reader.read_only = true;
    let mut writer = session_row(&child, 12);
    writer.parent_id = Some(parent.id.clone());
    let mut grand = session_row(&grandchild, 13);
    grand.parent_id = Some(writer.id.clone());
    let mut reader_writer = session_row(&via_reader, 14);
    reader_writer.parent_id = Some(reader.id.clone());
    let lone = session_row(&unrelated, 15);
    for row in [&parent, &reader, &writer, &grand, &reader_writer, &lone] {
        store.insert_agent_session(row).unwrap();
    }

    let found: Vec<String> = store
        .list_delegated_child_workspaces(&main)
        .unwrap()
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(found.first().map(String::as_str), Some("main-1-1"));
    let mut rest = found[1..].to_vec();
    rest.sort();
    assert_eq!(rest, vec!["main-1".to_owned(), "main-2".to_owned()]);
    assert!(
        store
            .list_delegated_child_workspaces(&grandchild)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .list_delegated_child_workspaces(&unrelated)
            .unwrap()
            .is_empty()
    );

    // A child workspace already gone takes its sessions, so it is no longer listed.
    assert!(store.delete_workspace(&via_reader).unwrap());
    assert_eq!(
        store
            .list_delegated_child_workspaces(&main)
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
        vec!["main-1-1".to_owned(), "main-1".to_owned()]
    );
}
