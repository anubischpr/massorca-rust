//! Messages between delegation bots (decision 0129): the inbox lists unread messages oldest first and marks only the
//! caller's own read, a `worker_done` settles its task once, and a repeated or contradicting report changes nothing.

use massorca_core_store::{
    Store, TaskOrigin, TaskOutcome, TaskRow, TeamMessageRow, TeamRow, WorkerDoneStored, new_id,
    now_ms,
};

fn team(store: &Store) -> String {
    let row = TeamRow {
        id: new_id(),
        name: "Parser".to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: now_ms(),
    };
    store.insert_team(&row).expect("team");
    row.id
}

fn task(store: &Store, team_id: &str, session: Option<&str>) -> String {
    let now = now_ms();
    let row = TaskRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        project_id: None,
        bot_id: None,
        title: "Task".to_owned(),
        body: String::new(),
        origin: TaskOrigin::Cli,
        session_id: session.map(str::to_owned),
        outcome: None,
        error: None,
        error_code: None,
        workspace_id: None,
        branch: None,
        created_at: now,
        updated_at: now,
    };
    store.insert_task(&row).expect("task");
    row.id
}

fn message(from: &str, to: &str, kind: &str, at: i64) -> TeamMessageRow {
    let id = new_id();
    TeamMessageRow {
        thread_id: id.clone(),
        id,
        from_task_id: from.to_owned(),
        to_task_id: to.to_owned(),
        from_bot_id: None,
        to_bot_id: None,
        kind: kind.to_owned(),
        priority: "normal".to_owned(),
        subject: "Subject".to_owned(),
        body: "Body".to_owned(),
        reply_to: None,
        outcome: None,
        settles_task_id: None,
        delivery: "inbox".to_owned(),
        read_at: None,
        created_at: at,
    }
}

fn worker_done(child: &str, parent: &str, outcome: &str) -> TeamMessageRow {
    let mut row = message(child, parent, "worker_done", now_ms());
    row.outcome = Some(outcome.to_owned());
    row.settles_task_id = Some(child.to_owned());
    row
}

#[test]
fn the_inbox_lists_unread_messages_oldest_first_and_marks_only_its_own_read() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let (parent, child) = (task(&store, &team, None), task(&store, &team, None));
    let later = message(&child, &parent, "status", 20);
    let earlier = message(&child, &parent, "question", 10);
    let downward = message(&parent, &child, "status", 15);
    for row in [&later, &earlier, &downward] {
        store.insert_team_message(row).expect("message");
    }

    let unread = store.team_inbox(&parent, true).expect("inbox");
    let ids: Vec<_> = unread.iter().map(|row| row.id.clone()).collect();
    assert_eq!(ids, vec![earlier.id.clone(), later.id.clone()]);

    // The child's message to the parent is not the child's to mark.
    let marked = store
        .mark_team_messages_read(&child, &ids, now_ms())
        .expect("mark");
    assert_eq!(marked, 0);
    let marked = store
        .mark_team_messages_read(&parent, &ids, now_ms())
        .expect("mark");
    assert_eq!(marked, 2);
    assert!(store.team_inbox(&parent, true).expect("inbox").is_empty());
    assert_eq!(store.team_inbox(&parent, false).expect("inbox").len(), 2);
    assert_eq!(store.team_inbox(&child, true).expect("inbox").len(), 1);
}

#[test]
fn a_worker_done_settles_its_task_once_and_a_repeat_is_a_duplicate() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, None);
    let child = task(&store, &team, Some("child-run"));
    let first = worker_done(&child, &parent, "succeeded");
    let stored = store
        .insert_worker_done(&first, "child-run", TaskOutcome::Success)
        .expect("settle");
    assert_eq!(stored, WorkerDoneStored::Settled);
    let row = store.get_task(&child).expect("read").expect("task");
    assert_eq!(row.outcome, Some(TaskOutcome::Success));

    let again = worker_done(&child, &parent, "succeeded");
    match store
        .insert_worker_done(&again, "child-run", TaskOutcome::Success)
        .expect("repeat")
    {
        WorkerDoneStored::Duplicate(found) => assert_eq!(found.id, first.id),
        other => panic!("expected a duplicate, got {other:?}"),
    }
    assert!(store.get_team_message(&again.id).expect("read").is_none());
}

#[test]
fn a_worker_done_for_a_task_already_ended_keeps_its_outcome() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, None);
    let child = task(&store, &team, Some("child-run"));
    assert!(
        store
            .latch_task_outcome(&child, "child-run", TaskOutcome::Success, now_ms())
            .expect("latch")
    );

    let contradicting = worker_done(&child, &parent, "failed");
    let stored = store
        .insert_worker_done(&contradicting, "child-run", TaskOutcome::Failure)
        .expect("conflict");
    assert_eq!(
        stored,
        WorkerDoneStored::Conflict(Some(TaskOutcome::Success))
    );
    assert!(
        store
            .get_team_message(&contradicting.id)
            .expect("read")
            .is_none()
    );

    let matching = worker_done(&child, &parent, "succeeded");
    let stored = store
        .insert_worker_done(&matching, "child-run", TaskOutcome::Success)
        .expect("match");
    assert_eq!(stored, WorkerDoneStored::Matched);
    assert!(
        store
            .get_team_message(&matching.id)
            .expect("read")
            .is_some()
    );
}

#[test]
fn the_latest_status_is_the_newest_of_its_kind_between_the_two_tasks() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let (parent, child) = (task(&store, &team, None), task(&store, &team, None));
    let mut old = message(&child, &parent, "status", 10);
    old.subject = "Started".to_owned();
    let mut new = message(&child, &parent, "status", 30);
    new.subject = "Halfway".to_owned();
    let question = message(&child, &parent, "question", 40);
    for row in [&old, &new, &question] {
        store.insert_team_message(row).expect("message");
    }
    let latest = store
        .latest_team_message(&child, &parent, "status")
        .expect("latest")
        .expect("a status");
    assert_eq!(latest.subject, "Halfway");
    assert!(
        store
            .latest_team_message(&parent, &child, "status")
            .expect("latest")
            .is_none()
    );
}

#[test]
fn removing_a_task_removes_its_messages() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let (parent, child) = (task(&store, &team, None), task(&store, &team, None));
    let row = message(&child, &parent, "status", 10);
    store.insert_team_message(&row).expect("message");
    assert!(store.delete_task(&child).expect("delete"));
    assert!(store.get_team_message(&row.id).expect("read").is_none());
}
