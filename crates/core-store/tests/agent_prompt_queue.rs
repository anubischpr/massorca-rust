//! Queued prompts (decision 0084): insert in order, read, edit, remove, pauses, the restart sweep, the request id and the
//! cascade when a session is forgotten.

mod common;

use common::{assert_conflict, project_row, workspace_row};
use massorca_core_store::{
    AgentSessionRow, NewQueuedPrompt, QueuedPromptRow, QueuedState, RunMode, Store, StoreError,
    new_id, now_ms,
};

/// A store with one workspace and one session in it; returns the store and the session id.
fn store_with_session() -> (Store, String) {
    let store = Store::open_in_memory().unwrap();
    let project = project_row("app", "/repos/app");
    store.insert_project(&project).unwrap();
    let workspace = workspace_row(&project.id, "main", "/repos/app-workspaces/main");
    store.insert_workspace(&workspace).unwrap();
    let session = session_row(&workspace.id);
    store.insert_agent_session(&session).unwrap();
    (store, session.id)
}

fn session_row(workspace_id: &str) -> AgentSessionRow {
    AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace_id.to_owned(),
        parent_id: None,
        title: None,
        read_only: false,
        agent_ref: None,
        terminal_id: Some(new_id()),
        request_id: None,
        created_at: now_ms(),
        exit: None,
        activity: None,
        result: None,
        brought_back_at: None,
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
    }
}

fn queue(store: &Store, session: &str, text: &str, request_id: Option<&str>) -> QueuedPromptRow {
    store
        .insert_queued_prompt(&NewQueuedPrompt {
            id: new_id(),
            session_id: session.to_owned(),
            text: text.to_owned(),
            request_id: request_id.map(str::to_owned),
            at: 10,
            host_admin: false,
        })
        .unwrap()
}

fn texts(rows: &[QueuedPromptRow]) -> Vec<&str> {
    rows.iter().map(|row| row.text.as_str()).collect()
}

#[test]
fn a_queued_prompt_is_stored_waiting_at_the_end_of_its_session_queue() {
    let (store, session) = store_with_session();
    let first = queue(&store, &session, "one", None);
    let second = queue(&store, &session, "two", None);

    assert_eq!(first.state, QueuedState::Waiting);
    assert_eq!(first.reason, None);
    assert_eq!((first.position, second.position), (1, 2));
    assert_eq!(first.created_at_ms, 10);
    assert_eq!(first.updated_at_ms, 10);

    let listed = store.list_queued_prompts(&session).unwrap();
    assert_eq!(texts(&listed), ["one", "two"], "strict FIFO by position");
    assert_eq!(
        store.get_queued_prompt(&second.id).unwrap(),
        Some(second.clone())
    );
}

#[test]
fn positions_are_counted_per_session() {
    let (store, first_session) = store_with_session();
    let workspace = store
        .get_agent_session(&first_session)
        .unwrap()
        .unwrap()
        .workspace_id;
    let other = session_row(&workspace);
    store.insert_agent_session(&other).unwrap();

    queue(&store, &first_session, "a", None);
    let theirs = queue(&store, &other.id, "b", None);

    assert_eq!(theirs.position, 1);
    assert_eq!(store.list_queued_prompts(&first_session).unwrap().len(), 1);
    assert_eq!(store.list_queued_prompts(&other.id).unwrap().len(), 1);
}

#[test]
fn a_request_id_is_unique_within_its_session_only() {
    let (store, session) = store_with_session();
    let row = queue(&store, &session, "one", Some("req-1"));

    assert_eq!(
        store
            .find_queued_prompt_by_request_id(&session, "req-1")
            .unwrap(),
        Some(row)
    );
    let again = store.insert_queued_prompt(&NewQueuedPrompt {
        id: new_id(),
        session_id: session.clone(),
        text: "again".to_owned(),
        request_id: Some("req-1".to_owned()),
        at: 11,
        host_admin: false,
    });
    assert_conflict(
        again.map(|_| ()),
        "agent_prompt_queue.session_id, agent_prompt_queue.request_id",
    );

    let workspace = store
        .get_agent_session(&session)
        .unwrap()
        .unwrap()
        .workspace_id;
    let other = session_row(&workspace);
    store.insert_agent_session(&other).unwrap();
    let elsewhere = store.insert_queued_prompt(&NewQueuedPrompt {
        id: new_id(),
        session_id: other.id.clone(),
        text: "same id elsewhere".to_owned(),
        request_id: Some("req-1".to_owned()),
        at: 11,
        host_admin: false,
    });
    assert!(
        elsewhere.is_ok(),
        "another session may reuse the request id"
    );
}

#[test]
fn an_edit_changes_the_text_and_its_update_time_only() {
    let (store, session) = store_with_session();
    let row = queue(&store, &session, "one", None);

    assert!(
        store
            .update_queued_prompt_text(&row.id, "edited", 20)
            .unwrap()
    );
    let shown = store.get_queued_prompt(&row.id).unwrap().unwrap();
    assert_eq!(shown.text, "edited");
    assert_eq!(shown.position, row.position);
    assert_eq!(shown.created_at_ms, 10);
    assert_eq!(shown.updated_at_ms, 20);
    assert!(
        !store
            .update_queued_prompt_text("no-such-id", "x", 20)
            .unwrap()
    );
}

#[test]
fn a_state_change_sets_the_state_and_reason_together() {
    let (store, session) = store_with_session();
    let row = queue(&store, &session, "one", None);

    assert!(
        store
            .set_queued_prompt_state(&row.id, QueuedState::Failed, Some("the run has exited"), 30)
            .unwrap()
    );
    let failed = store.get_queued_prompt(&row.id).unwrap().unwrap();
    assert_eq!(failed.state, QueuedState::Failed);
    assert_eq!(failed.reason.as_deref(), Some("the run has exited"));

    assert!(
        store
            .set_queued_prompt_state(&row.id, QueuedState::Sending, None, 31)
            .unwrap()
    );
    let sending = store.get_queued_prompt(&row.id).unwrap().unwrap();
    assert_eq!(sending.state, QueuedState::Sending);
    assert_eq!(sending.reason, None, "a new state clears the old reason");
}

#[test]
fn a_removed_prompt_leaves_the_others_in_order() {
    let (store, session) = store_with_session();
    let one = queue(&store, &session, "one", None);
    queue(&store, &session, "two", None);
    queue(&store, &session, "three", None);

    assert!(store.delete_queued_prompt(&one.id).unwrap());
    assert!(!store.delete_queued_prompt(&one.id).unwrap());
    let listed = store.list_queued_prompts(&session).unwrap();
    assert_eq!(texts(&listed), ["two", "three"]);
}

#[test]
fn a_pause_holds_only_waiting_prompts_and_an_unpause_lifts_it() {
    let (store, session) = store_with_session();
    let waiting = queue(&store, &session, "waiting", None);
    let failed = queue(&store, &session, "failed", None);
    store
        .set_queued_prompt_state(&failed.id, QueuedState::Failed, Some("gone"), 30)
        .unwrap();

    assert_eq!(
        store
            .pause_queued_prompts(&session, "paused_stopped", 40)
            .unwrap(),
        1
    );
    let paused = store.get_queued_prompt(&waiting.id).unwrap().unwrap();
    assert_eq!(paused.reason.as_deref(), Some("paused_stopped"));
    assert_eq!(
        store.get_queued_prompt(&failed.id).unwrap().unwrap().state,
        QueuedState::Failed,
        "a failed prompt keeps its failure"
    );

    assert_eq!(store.unpause_queued_prompts(&session, 50).unwrap(), 1);
    assert_eq!(
        store
            .get_queued_prompt(&waiting.id)
            .unwrap()
            .unwrap()
            .reason,
        None
    );
}

#[test]
fn a_service_restart_pauses_waiting_prompts_and_fails_the_one_being_sent() {
    let (store, session) = store_with_session();
    let sending = queue(&store, &session, "sending", None);
    let waiting = queue(&store, &session, "waiting", None);
    store
        .set_queued_prompt_state(&sending.id, QueuedState::Sending, None, 30)
        .unwrap();

    store.restart_queued_prompts(60).unwrap();

    let failed = store.get_queued_prompt(&sending.id).unwrap().unwrap();
    assert_eq!(failed.state, QueuedState::Failed);
    assert_eq!(
        failed.reason.as_deref(),
        Some("the service restarted while sending")
    );
    let paused = store.get_queued_prompt(&waiting.id).unwrap().unwrap();
    assert_eq!(paused.state, QueuedState::Waiting);
    assert_eq!(paused.reason.as_deref(), Some("paused_restarted"));
}

#[test]
fn forgetting_a_session_removes_its_queue() {
    let (store, session) = store_with_session();
    queue(&store, &session, "one", None);
    queue(&store, &session, "two", None);

    assert!(store.delete_agent_session(&session).unwrap());
    assert!(store.list_queued_prompts(&session).unwrap().is_empty());
}

#[test]
fn a_prompt_must_belong_to_a_session() {
    let (store, _session) = store_with_session();
    let orphan = store.insert_queued_prompt(&NewQueuedPrompt {
        id: new_id(),
        session_id: "no-such-session".to_owned(),
        text: "orphan".to_owned(),
        request_id: None,
        at: 1,
        host_admin: false,
    });
    assert!(
        matches!(orphan, Err(StoreError::Constraint { .. })),
        "{orphan:?}"
    );
}

#[test]
fn a_queued_prompt_keeps_whether_its_caller_held_host_admin() {
    let (store, session) = store_with_session();
    let admin = store
        .insert_queued_prompt(&NewQueuedPrompt {
            id: new_id(),
            session_id: session.clone(),
            text: "by an admin".to_owned(),
            request_id: None,
            at: 10,
            host_admin: true,
        })
        .unwrap();

    assert!(admin.host_admin);
    assert!(
        store
            .get_queued_prompt(&admin.id)
            .unwrap()
            .unwrap()
            .host_admin
    );
    assert!(!queue(&store, &session, "by anyone", None).host_admin);
}

#[test]
fn delivered_request_ids_are_kept_up_to_the_limit_per_session() {
    let (store, session) = store_with_session();
    store
        .record_delivered_request(&session, "req-1", 1, 2)
        .unwrap();
    store
        .record_delivered_request(&session, "req-2", 2, 2)
        .unwrap();
    store
        .record_delivered_request(&session, "req-2", 3, 2)
        .unwrap();
    assert_eq!(
        store.find_delivered_request(&session, "req-2").unwrap(),
        Some(2),
        "a repeated id keeps its first time"
    );

    store
        .record_delivered_request(&session, "req-3", 4, 2)
        .unwrap();
    assert_eq!(
        store.find_delivered_request(&session, "req-1").unwrap(),
        None
    );
    assert_eq!(
        store.find_delivered_request(&session, "req-2").unwrap(),
        Some(2)
    );
    assert_eq!(
        store.find_delivered_request(&session, "req-3").unwrap(),
        Some(4)
    );
}
