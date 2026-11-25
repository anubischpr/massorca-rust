//! The chat methods (decision 0101), served under `agent-chat/1`, which the service offers. Each test writes a chat
//! session and its journal to the store directly (no agent is started), and reads it back through the methods.

use massorca_core_store::{
    AgentSessionRow, NewChatItem, ProjectKind, ProjectRow, RunExit, RunExitReason, RunMode,
    SessionOrigin, Store, WorkspaceKind, WorkspaceRow, WorkspaceState, new_id, now_ms,
};
use massorca_protocol::capabilities::AGENT_CHAT;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{self, AgentSession, AgentSessionRef};
use massorca_protocol::methods::chat::{
    self, ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatItemBody,
    ChatSubscribeParams, EndReason, ResetReason, RunType,
};
use tokio::time::timeout;

use super::common::{REPLY_WAIT, connect, profile, remote_code, start, stop, temp_home};

/// A project, a workspace, an ended terminal session and an ended chat session with `notes` notices in its journal.
/// Returns the terminal session's id, the chat session's id and the journal's epoch.
fn seed(store: &Store, notes: usize) -> (String, String, String) {
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
    };
    store.insert_workspace(&workspace).expect("workspace");
    let row = |terminal: Option<String>| AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace.id.clone(),
        parent_id: None,
        title: None,
        read_only: true,
        agent_ref: None,
        terminal_id: terminal,
        request_id: None,
        created_at: now_ms(),
        exit: Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: now_ms(),
        }),
        activity: None,
        result: None,
        brought_back_at: None,
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
        retained: false,
        released_at: None,
    };
    let terminal = row(Some(new_id()));
    store
        .insert_agent_session(&terminal)
        .expect("terminal session");
    let chat = row(None);
    store
        .insert_agent_session_with_origin(&chat, &SessionOrigin::default(), Some("run-1"))
        .expect("chat session");
    let epoch = store
        .open_chat_journal(&chat.id, "epoch-1")
        .expect("journal")
        .epoch;
    for index in 0..notes {
        let text = format!("note {index}");
        let body =
            serde_json::json!({ "type": "notice", "level": "info", "text": text }).to_string();
        store
            .add_chat_item(
                &chat.id,
                &NewChatItem {
                    id: &format!("n{index}"),
                    turn: None,
                    parent: None,
                    at_ms: now_ms(),
                    item_type: "notice",
                    body: &body,
                    text: None,
                },
                1_000,
            )
            .expect("an item");
    }
    (terminal.id, chat.id, epoch)
}

fn notice_text(body: &ChatItemBody) -> &str {
    match body {
        ChatItemBody::Notice { text, .. } => text,
        other => panic!("expected a notice, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_service_offers_the_chat_capability_and_serves_every_chat_method() {
    let home = temp_home();
    let running = start(home.path(), "server").await;
    let client = connect(home.path(), "server").await;
    assert!(client.has_capability(AGENT_CHAT), "the service offers chat");
    // A terminal-only argument is refused before any kind is looked up.
    for params in [
        serde_json::json!({ "kind": "claude", "cols": 80 }),
        serde_json::json!({ "kind": "claude", "rows": 24 }),
        serde_json::json!({ "kind": "claude", "args": ["--debug"] }),
    ] {
        let refused = client
            .call::<_, AgentSession>(chat::START, &params)
            .await
            .expect_err("a terminal argument");
        assert_eq!(
            remote_code(&refused),
            codes::REQUEST_INVALID_PARAMS,
            "{params}"
        );
    }
    let refused = client
        .call::<_, AgentSession>(chat::START, &serde_json::json!({ "kind": "no-such-kind" }))
        .await
        .expect_err("an unknown kind");
    assert_eq!(remote_code(&refused), codes::AGENT_KIND_NOT_FOUND);
    for (method, params) in [
        (
            chat::APPROVAL_ANSWER,
            serde_json::json!({ "session": "nope-nope", "request_id": "r", "decision": "allow" }),
        ),
        (
            chat::INTERRUPT,
            serde_json::json!({ "session": "nope-nope" }),
        ),
    ] {
        let refused = client
            .call::<_, serde_json::Value>(method, &params)
            .await
            .expect_err("an unknown session");
        assert_eq!(remote_code(&refused), codes::AGENT_NOT_FOUND, "{method}");
    }
    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn answers_and_interrupts_of_a_terminal_session_are_refused_as_not_a_chat() {
    let home = temp_home();
    let running = start(home.path(), "server").await;
    let store = Store::open(profile(home.path(), "server").store_path()).expect("the store");
    let (terminal, chat_session, _) = seed(&store, 1);
    let client = connect(home.path(), "server").await;
    let refused = client
        .call::<_, serde_json::Value>(
            chat::INTERRUPT,
            &serde_json::json!({ "session": terminal.clone() }),
        )
        .await
        .expect_err("a terminal session");
    assert_eq!(remote_code(&refused), codes::AGENT_NOT_CHAT);
    let refused = client
        .call::<_, serde_json::Value>(
            chat::APPROVAL_ANSWER,
            &serde_json::json!({ "session": terminal, "request_id": "r", "decision": "allow" }),
        )
        .await
        .expect_err("a terminal session");
    assert_eq!(remote_code(&refused), codes::AGENT_NOT_CHAT);
    // An ended chat run has nothing to interrupt, and no request of its run can be answered.
    let interrupted: serde_json::Value = client
        .call(
            chat::INTERRUPT,
            &serde_json::json!({ "session": chat_session.clone() }),
        )
        .await
        .expect("an interrupt of an ended chat");
    assert_eq!(interrupted, serde_json::json!({ "interrupted": false }));
    let refused = client
        .call::<_, serde_json::Value>(
            chat::APPROVAL_ANSWER,
            &serde_json::json!({ "session": chat_session, "request_id": "r", "decision": "allow", "scope": "always" }),
        )
        .await
        .expect_err("no such request");
    assert_eq!(remote_code(&refused), codes::AGENT_APPROVAL_SETTLED);
    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_session_streams_its_journal_pages_its_history_and_ends_when_forgotten() {
    let home = temp_home();
    let running = start(home.path(), "server").await;
    let store = Store::open(profile(home.path(), "server").store_path()).expect("the store");
    let (terminal, session, epoch) = seed(&store, 5);
    let client = connect(home.path(), "server").await;

    let shown: AgentSession = client
        .call(
            agents::SHOW,
            &AgentSessionRef {
                session: session.clone(),
            },
        )
        .await
        .expect("agent.show");
    assert_eq!(shown.run, RunType::Chat);
    assert_eq!(shown.terminal_id, None);

    let refused = client
        .subscribe(
            chat::SUBSCRIBE,
            &ChatSubscribeParams {
                session: terminal.clone(),
                after: None,
                limit: None,
            },
        )
        .await
        .expect_err("a terminal session has no chat stream");
    assert_eq!(remote_code(&refused), "agent.not_chat");

    let mut stream = client
        .subscribe(
            chat::SUBSCRIBE,
            &ChatSubscribeParams {
                session: session.clone(),
                after: None,
                limit: Some(2),
            },
        )
        .await
        .expect("agent.chat.subscribe");
    let ChatEvent::Snapshot(snapshot) = next_event(&mut stream).await else {
        panic!("the stream opens with a snapshot");
    };
    assert_eq!(
        snapshot.cursor,
        ChatCursor {
            epoch: epoch.clone(),
            seq: 5
        }
    );
    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| notice_text(&item.body))
            .collect::<Vec<_>>(),
        ["note 3", "note 4"]
    );
    assert!(snapshot.has_older);
    assert_eq!(snapshot.latest_turn, None);

    let page: ChatHistoryPage = client
        .call(
            chat::HISTORY,
            &ChatHistoryParams {
                session: session.clone(),
                before: Some(ChatCursor {
                    epoch: epoch.clone(),
                    seq: snapshot.items[0].position,
                }),
                limit: Some(2),
            },
        )
        .await
        .expect("agent.chat.history");
    assert_eq!(page.epoch, epoch);
    assert_eq!(
        page.items
            .iter()
            .map(|item| notice_text(&item.body))
            .collect::<Vec<_>>(),
        ["note 1", "note 2"]
    );
    assert!(page.has_older);
    let stale = client
        .call::<_, ChatHistoryPage>(
            chat::HISTORY,
            &ChatHistoryParams {
                session: session.clone(),
                before: Some(ChatCursor {
                    epoch: "another".to_owned(),
                    seq: 3,
                }),
                limit: None,
            },
        )
        .await
        .expect_err("another epoch's cursor is stale");
    assert_eq!(remote_code(&stale), "agent.cursor_stale");
    let refused = client
        .call::<_, ChatHistoryPage>(
            chat::HISTORY,
            &ChatHistoryParams {
                session: terminal.clone(),
                before: None,
                limit: None,
            },
        )
        .await
        .expect_err("a terminal session has no chat history");
    assert_eq!(remote_code(&refused), "agent.not_chat");

    // A current cursor opens with the batch of what changed after it; a cursor the journal never reached is reset.
    let mut replay = client
        .subscribe(
            chat::SUBSCRIBE,
            &ChatSubscribeParams {
                session: session.clone(),
                after: Some(ChatCursor {
                    epoch: epoch.clone(),
                    seq: 4,
                }),
                limit: None,
            },
        )
        .await
        .expect("a replay");
    let ChatEvent::Batch(batch) = next_event(&mut replay).await else {
        panic!("a current cursor opens with a batch");
    };
    assert_eq!(batch.updates.len(), 1);
    let mut ahead = client
        .subscribe(
            chat::SUBSCRIBE,
            &ChatSubscribeParams {
                session: session.clone(),
                after: Some(ChatCursor {
                    epoch: epoch.clone(),
                    seq: 99,
                }),
                limit: Some(1),
            },
        )
        .await
        .expect("a subscription after an unknown cursor");
    assert!(matches!(
        next_event(&mut ahead).await,
        ChatEvent::Reset {
            reason: ResetReason::EpochChanged
        }
    ));
    assert!(matches!(
        next_event(&mut ahead).await,
        ChatEvent::Snapshot(_)
    ));

    let _: AgentSession = client
        .call(
            agents::FORGET,
            &AgentSessionRef {
                session: session.clone(),
            },
        )
        .await
        .expect("agent.forget");
    for open in [&mut stream, &mut replay, &mut ahead] {
        assert!(matches!(
            next_event(open).await,
            ChatEvent::End {
                reason: EndReason::Forgotten
            }
        ));
    }
    assert_eq!(
        store.chat_journal_head(&session).expect("the store"),
        None,
        "the journal goes with its session"
    );
    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_stream_ends_when_the_service_shuts_down() {
    let home = temp_home();
    let running = start(home.path(), "server").await;
    let store = Store::open(profile(home.path(), "server").store_path()).expect("the store");
    let (_, session, _) = seed(&store, 1);
    let client = connect(home.path(), "server").await;
    let mut stream = client
        .subscribe(
            chat::SUBSCRIBE,
            &ChatSubscribeParams {
                session,
                after: None,
                limit: None,
            },
        )
        .await
        .expect("agent.chat.subscribe");
    assert!(matches!(
        next_event(&mut stream).await,
        ChatEvent::Snapshot(_)
    ));
    running.shutdown_handle().shutdown();
    // The connection ends every open stream with `service.shutting_down` first (decision 0101, as amended).
    let ended = timeout(REPLY_WAIT, stream.next_as::<ChatEvent>())
        .await
        .expect("the stream ends in time")
        .expect("the stream says why it ends")
        .expect_err("the end is an error");
    assert_eq!(remote_code(&ended), codes::SERVICE_SHUTTING_DOWN);
    drop(client);
    stop(running).await;
}

async fn next_event(stream: &mut massorca_client::Subscription) -> ChatEvent {
    timeout(REPLY_WAIT, stream.next_as::<ChatEvent>())
        .await
        .expect("a chat event in time")
        .expect("the stream is open")
        .expect("a chat event")
}
