//! Seen state of an agent's attention states (inventory STAT-19): `agent.mark_seen` marks the latest `waiting` or `done`
//! as seen in the service's one status store and announces it on the same change topics, and a new attention state is
//! unseen again. No agent is started: the session is written to the store directly, as an ended run.

use massorca_core_store::{
    AgentSessionRow, ProjectKind, ProjectRow, RunExit, RunExitReason, RunMode, Store,
    StoredActivity, WorkspaceKind, WorkspaceRow, WorkspaceState, new_id, now_ms,
};
use massorca_protocol::methods::agents::{
    self, ActivityState, AgentSession, AgentSessionRef, AgentWatchEvent, AgentWatchParams,
};

use super::common::{connect, profile, start, stop, temp_home};

fn done_at(at: i64) -> StoredActivity {
    StoredActivity {
        state: "done".to_owned(),
        outcome: Some("success".to_owned()),
        source: "hook".to_owned(),
        at,
        seen_at: None,
    }
}

/// A project, a workspace and one ended session whose last turn is `done`.
fn seed(store: &Store) -> String {
    let project = ProjectRow {
        id: new_id(),
        kind: ProjectKind::Folder,
        path: "/work/app".to_owned(),
        path_key: "/work/app".to_owned(),
        name: "app".to_owned(),
        workspaces_root: "/work/app-workspaces".to_owned(),
        base_ref: None,
        icon: None,
        added_at: now_ms(),
    };
    store.insert_project(&project).expect("project");
    let workspace = WorkspaceRow {
        id: new_id(),
        project_id: project.id,
        kind: WorkspaceKind::Folder,
        name: "main".to_owned(),
        path: "/work/app".to_owned(),
        path_key: "/work/app".to_owned(),
        branch: None,
        base_ref: None,
        created_branch: false,
        state: WorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
        comment: None,
        name_generated: false,
    };
    store.insert_workspace(&workspace).expect("workspace");
    let row = AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace.id,
        parent_id: None,
        title: None,
        read_only: true,
        agent_ref: None,
        terminal_id: None,
        request_id: None,
        created_at: now_ms(),
        exit: Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: now_ms(),
        }),
        activity: Some(done_at(now_ms())),
        result: None,
        brought_back_at: None,
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
        retained: false,
        released_at: None,
    };
    store.insert_agent_session(&row).expect("session");
    row.id
}

fn seen_at(session: &AgentSession) -> Option<i64> {
    session
        .activity
        .as_ref()
        .and_then(|activity| activity.seen_at_ms)
}

#[tokio::test(flavor = "multi_thread")]
async fn marking_seen_is_stored_and_announced_and_a_new_attention_state_is_unseen() {
    let home = temp_home();
    let running = start(home.path(), "server").await;
    let store = Store::open(profile(home.path(), "server").store_path()).expect("the store");
    let id = seed(&store);
    let watcher = connect(home.path(), "server").await;
    let other = connect(home.path(), "server").await;
    let by_ref = AgentSessionRef {
        session: id.clone(),
    };

    let shown: AgentSession = other.call(agents::SHOW, &by_ref).await.expect("agent.show");
    assert_eq!(
        shown.activity.as_ref().map(|a| a.state),
        Some(ActivityState::Done)
    );
    assert_eq!(seen_at(&shown), None, "a done nobody marked is unseen");

    let mut watch = watcher
        .agent_watch(&AgentWatchParams {
            workspace: None,
            session: Some(id.clone()),
        })
        .await
        .expect("agent.watch opens");
    let first = watch
        .next()
        .await
        .expect("a snapshot")
        .expect("not an error");
    assert!(matches!(
        serde_json::from_value(first).expect("a watch item"),
        AgentWatchEvent::Snapshot { .. }
    ));
    let mut events = watcher.subscribe_events().await.expect("events open");

    let marked = other
        .agent_mark_seen(&id, None)
        .await
        .expect("agent.mark_seen");
    let at = seen_at(&marked).expect("the reply is seen");

    let item = watch.next().await.expect("a change").expect("not an error");
    let AgentWatchEvent::Changed { session } =
        serde_json::from_value(item.clone()).expect("a watch item")
    else {
        panic!("expected a change, got {item}");
    };
    assert_eq!(
        seen_at(&session),
        Some(at),
        "every watcher learns it was seen"
    );
    let event = loop {
        let item = events
            .next()
            .await
            .expect("an event")
            .expect("not an error");
        if item["topic"] == "agent.changed" {
            break item;
        }
    };
    assert_eq!(event["id"], id.as_str());

    let again = other
        .agent_mark_seen(&id, None)
        .await
        .expect("a second mark answers");
    assert_eq!(seen_at(&again), Some(at), "the first mark stays");

    let first_at = marked.activity.as_ref().expect("an activity").at_ms;
    // The store is the one source: a newer done written there is unseen for every reader.
    store
        .set_agent_session_status(&id, Some(&done_at(first_at + 1)), None, None)
        .expect("a new done");
    let shown: AgentSession = other.call(agents::SHOW, &by_ref).await.expect("agent.show");
    assert_eq!(seen_at(&shown), None, "a new attention state is unseen");

    // A late mark for the done a client showed earlier leaves the newer one unseen.
    let late = other
        .agent_mark_seen(&id, Some(first_at))
        .await
        .expect("a late mark answers");
    assert_eq!(seen_at(&late), None, "the newer done stays unseen");
    let current = other
        .agent_mark_seen(&id, Some(first_at + 1))
        .await
        .expect("a mark for the shown done");
    assert!(seen_at(&current).is_some());

    drop((watch, events, watcher, other));
    stop(running).await;
}
