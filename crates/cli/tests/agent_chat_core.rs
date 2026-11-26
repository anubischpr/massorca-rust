//! Chat runs and their journal (decision 0101), end to end: the fake agent in its structured mode (`chat`) declared as
//! kind `fakechat`, which the test service starts as a chat run with the test dialect (`MASSORCA_TEST_CHAT_KINDS`, a
//! test-only hook; the Claude dialect has its own tests in `agent_chat_claude.rs`). The session's journal is read
//! through `agent.chat.subscribe` and `agent.chat.history`; its run is started, stopped and ended through the ordinary
//! agent commands.
//!
//! The service gets a temporary home, application-data and XDG folders, and none of the vendor CLIs' own variables.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, eventually, single_document, with_json};
use massorca_client::{Client, ConnectOptions, Subscription};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::chat::{
    self, ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatItem, ChatItemBody,
    ChatSubscribeParams, ChatUpdate, EndReason, MessageState, TurnState,
};
use serde_json::{Value, json};

const TEST_CHAT_KINDS_ENV: &str = "MASSORCA_TEST_CHAT_KINDS";
const GO_FILE_ENV: &str = "FAKE_CHAT_GO_FILE";
/// Variables a vendor CLI reads its home from; the service never gets them from the developer's shell.
const VENDOR_HOME_ENV: [&str; 3] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"];

/// A sandbox whose service starts kind `fakechat` as a chat run, with a folder workspace `main`. Returns the sandbox
/// and the file the fake agent waits for before it finishes a reply.
fn sandbox() -> (Sandbox, PathBuf) {
    sandbox_with(&[])
}

/// [`sandbox`], with `env` set on the service too.
fn sandbox_with(env: &[(&str, &str)]) -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
    for (name, value) in env {
        sandbox.set_env(name, value);
    }
    // Windows resolves the service's own known folders below `USERPROFILE`, and refuses them when they are absent.
    let user = sandbox.home().join("user");
    for (name, folder) in [
        ("HOME", ""),
        ("USERPROFILE", ""),
        ("APPDATA", "AppData/Roaming"),
        ("LOCALAPPDATA", "AppData/Local"),
        ("XDG_CONFIG_HOME", ".config"),
        ("XDG_DATA_HOME", ".local/share"),
        ("XDG_STATE_HOME", ".local/state"),
        ("XDG_CACHE_HOME", ".cache"),
    ] {
        let path = user.join(folder);
        std::fs::create_dir_all(&path).expect("a private folder");
        sandbox.set_env(name, &path.to_string_lossy());
    }
    sandbox.set_env(TEST_CHAT_KINDS_ENV, "fakechat");
    let go = sandbox.home().join("chat-go");
    sandbox.set_env(GO_FILE_ENV, &go.to_string_lossy());

    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fakechat",
        "name": "Fake chat",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "none" }
    });
    std::fs::write(dir.join("fakechat.json"), kind.to_string()).expect("write a kind file");

    // The first command starts the service, so it is the one whose environment the service keeps.
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    let args = with_json(&["project", "add", notes_text.as_str(), "--folder"]);
    let mut command = sandbox.command("default");
    command.args(&args);
    for name in VENDOR_HOME_ENV {
        command.env_remove(name);
    }
    sandbox
        .run_prepared("default", &args, &mut command)
        .success();
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    (sandbox, go)
}

/// Starts a `fakechat` session with `prompt`; returns its record.
fn start(sandbox: &Sandbox, prompt: &str) -> Value {
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fakechat",
            "--workspace",
            "main",
            "--prompt",
            prompt,
        ],
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
}

async fn connect(home: &Path) -> Client {
    let profile =
        Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
            None
        })
        .expect("the profile resolves");
    let client = Client::connect(
        &profile,
        ConnectOptions::new(PeerInfo {
            name: "chat-core-test".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("the client connects");
    assert!(
        client.has_capability(massorca_protocol::capabilities::AGENT_CHAT),
        "the test service offers the chat methods"
    );
    client
}

async fn next_event(stream: &mut Subscription) -> ChatEvent {
    tokio::time::timeout(harness::WAIT, stream.next_as::<ChatEvent>())
        .await
        .expect("a chat event in time")
        .expect("the stream is open")
        .expect("a chat event")
}

fn session_of(record: &Value) -> String {
    record["id"].as_str().expect("a session id").to_owned()
}

fn show(sandbox: &Sandbox, session: &str) -> Value {
    sandbox.json_ok("default", &["agent", "status", session])
}

/// Waits until the session's run has ended; returns its record.
fn wait_for_exit(sandbox: &Sandbox, session: &str) -> Value {
    eventually("the chat run's end", || {
        let shown = show(sandbox, session);
        (shown["execution"]["state"] == "exited").then_some(shown)
    })
}

fn turn_state(item: &ChatItem) -> Option<TurnState> {
    match &item.body {
        ChatItemBody::Turn(turn) => Some(turn.state),
        _ => None,
    }
}

/// Every item of the session's journal, oldest first, paged back two at a time.
async fn all_items(client: &Client, session: &str) -> (String, Vec<ChatItem>) {
    let mut items: Vec<ChatItem> = Vec::new();
    let mut before = None;
    loop {
        let page: ChatHistoryPage = client
            .call(
                chat::HISTORY,
                &ChatHistoryParams {
                    session: session.to_owned(),
                    before: before.clone(),
                    limit: Some(2),
                },
            )
            .await
            .expect("agent.chat.history");
        assert!(page.items.len() <= 2);
        let mut older = page.items;
        older.append(&mut items);
        items = older;
        if !page.has_older {
            return (page.epoch, items);
        }
        before = Some(ChatCursor {
            epoch: page.epoch,
            seq: items[0].position,
        });
    }
}

#[test]
fn a_chat_session_streams_a_snapshot_then_batches_with_an_append_and_pages_its_history() {
    let (sandbox, go) = sandbox();
    let started = start(&sandbox, "hello chat");
    let session = session_of(&started);
    assert_eq!(started["run"], "chat", "{started}");
    assert!(started.get("terminal_id").is_none(), "{started}");
    assert_eq!(started["execution"]["state"], "live");

    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut stream = client
            .subscribe(
                chat::SUBSCRIBE,
                &ChatSubscribeParams {
                    session: session.clone(),
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("agent.chat.subscribe");
        let ChatEvent::Snapshot(snapshot) = next_event(&mut stream).await else {
            panic!("the stream opens with a snapshot");
        };
        let opened_at = snapshot.cursor.clone();
        let turn = snapshot.latest_turn.clone().expect("the prompt opened a turn");
        assert_eq!(turn_state(&turn), Some(TurnState::Running));
        assert!(
            snapshot.items.iter().any(|item| item.body
                == ChatItemBody::UserMessage {
                    text: "hello chat".to_owned()
                }
                && item.turn.as_deref() == Some(turn.id.as_str())),
            "the prompt is in the transcript: {snapshot:?}"
        );

        // The reply's second part waits for this file, so it arrives after the snapshot.
        std::fs::write(&go, b"go").expect("the go file");
        let mut appended = None;
        let mut ended = false;
        let mut cursor = opened_at.clone();
        while appended.is_none() || !ended {
            let ChatEvent::Batch(batch) = next_event(&mut stream).await else {
                panic!("batches follow the snapshot");
            };
            assert_eq!(batch.cursor.epoch, opened_at.epoch);
            assert!(batch.cursor.seq > cursor.seq, "batches move the cursor on");
            cursor = batch.cursor.clone();
            for update in batch.updates {
                match update {
                    ChatUpdate::Append {
                        offset, text, seq, ..
                    } => {
                        assert!(seq <= cursor.seq);
                        appended = Some((offset, text));
                    }
                    ChatUpdate::Item { item } if item.id == turn.id => {
                        ended = turn_state(&item) == Some(TurnState::Completed);
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            appended,
            Some((6, "hello chat".to_owned())),
            "the reply's second part is an append after the 6 bytes of `echo: `"
        );

        // History pages the whole journal back; a cursor of another epoch is stale.
        let (epoch, items) = all_items(&client, &session).await;
        assert_eq!(epoch, opened_at.epoch);
        let positions: Vec<u64> = items.iter().map(|item| item.position).collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{positions:?}");
        let reply = items
            .iter()
            .find(|item| matches!(item.body, ChatItemBody::AssistantMessage { .. }))
            .expect("the reply");
        assert_eq!(
            reply.body,
            ChatItemBody::AssistantMessage {
                text: "echo: hello chat".to_owned(),
                state: MessageState::Completed,
            }
        );
        assert_eq!(reply.turn.as_deref(), Some(turn.id.as_str()));
        let stale = client
            .call::<_, ChatHistoryPage>(
                chat::HISTORY,
                &ChatHistoryParams {
                    session: session.clone(),
                    before: Some(ChatCursor {
                        epoch: "another".to_owned(),
                        seq: 1,
                    }),
                    limit: None,
                },
            )
            .await
            .expect_err("another epoch's cursor is refused");
        assert!(stale.to_string().contains("agent.cursor_stale"), "{stale}");

        // A resubscription after the snapshot's cursor replays whole items, the finished reply among them.
        let mut again = client
            .subscribe(
                chat::SUBSCRIBE,
                &ChatSubscribeParams {
                    session: session.clone(),
                    after: Some(opened_at.clone()),
                    limit: None,
                },
            )
            .await
            .expect("agent.chat.subscribe after a cursor");
        let ChatEvent::Batch(replayed) = next_event(&mut again).await else {
            panic!("a current cursor opens with a batch");
        };
        assert_eq!(replayed.cursor, cursor);
        assert!(replayed.updates.iter().all(|update| matches!(update, ChatUpdate::Item { .. })));
        assert!(replayed.updates.iter().any(|update| matches!(update, ChatUpdate::Item { item } if item.id == reply.id && item.body == reply.body)));

        // Another epoch's cursor is reset, then served a snapshot.
        let mut reset = client
            .subscribe(
                chat::SUBSCRIBE,
                &ChatSubscribeParams {
                    session: session.clone(),
                    after: Some(ChatCursor {
                        epoch: "another".to_owned(),
                        seq: 1,
                    }),
                    limit: Some(1),
                },
            )
            .await
            .expect("agent.chat.subscribe after a stale cursor");
        assert!(matches!(
            next_event(&mut reset).await,
            ChatEvent::Reset {
                reason: chat::ResetReason::EpochChanged
            }
        ));
        let ChatEvent::Snapshot(fresh) = next_event(&mut reset).await else {
            panic!("a snapshot follows a reset");
        };
        assert_eq!(fresh.items.len(), 1);
        assert!(fresh.has_older);

        // A stop ends the run through the session's exit path; a forget ends the streams.
        let stopped = sandbox.json_ok("default", &["agent", "stop", &session]);
        assert_eq!(stopped["execution"]["state"], "exited", "{stopped}");
        assert_eq!(stopped["execution"]["exit"]["reason"], "killed", "{stopped}");
        sandbox.json_ok("default", &["agent", "forget", &session]);
        loop {
            match next_event(&mut again).await {
                ChatEvent::End { reason } => {
                    assert_eq!(reason, EndReason::Forgotten);
                    break;
                }
                ChatEvent::Batch(_) => {}
                other => panic!("unexpected {other:?}"),
            }
        }
    });
}

#[test]
fn a_chat_run_that_exits_records_its_code_and_leaves_its_turn_unverifiable() {
    let (sandbox, _go) = sandbox();
    let session = session_of(&start(&sandbox, "crash"));
    let ended = wait_for_exit(&sandbox, &session);
    assert_eq!(ended["execution"]["exit"]["reason"], "exited", "{ended}");
    assert_eq!(ended["execution"]["exit"]["code"], 3, "{ended}");
    // A chat resumes as a chat, with its kind's resume form; this kind declares none.
    let outcome = sandbox
        .run("default", &with_json(&["agent", "resume", &session]))
        .failure();
    let refused = single_document(&outcome);
    assert_eq!(
        refused["error"]["code"], "agent.resume_unavailable",
        "{refused}"
    );
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let (_, items) = all_items(&client, &session).await;
        let turns: Vec<_> = items.iter().filter_map(turn_state).collect();
        assert_eq!(turns, [TurnState::Unverifiable]);
    });
}

#[test]
fn a_chat_run_whose_child_holds_its_output_is_ended_with_its_whole_tree() {
    let (sandbox, _go) = sandbox();
    let session = session_of(&start(&sandbox, "hang"));
    // The run's child keeps the output open: its end is seen only once the whole tree is gone.
    let stopped = sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    let ended = if stopped["execution"]["state"] == "exited" {
        stopped
    } else {
        wait_for_exit(&sandbox, &session)
    };
    assert_eq!(ended["execution"]["exit"]["reason"], "killed", "{ended}");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let (_, items) = all_items(&client, &session).await;
        let turns: Vec<_> = items.iter().filter_map(turn_state).collect();
        assert_eq!(turns, [TurnState::Interrupted]);
    });
}

#[test]
fn a_terminal_session_is_not_a_chat() {
    let (sandbox, _go) = sandbox();
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" }
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    assert!(
        started.get("run").is_none(),
        "a terminal session leaves `run` out: {started}"
    );
    let session = session_of(&started);
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let refused = client
            .subscribe(
                chat::SUBSCRIBE,
                &ChatSubscribeParams {
                    session: session.clone(),
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect_err("a terminal session has no chat stream");
        assert!(refused.to_string().contains("agent.not_chat"), "{refused}");
        let refused = client
            .call::<_, ChatHistoryPage>(
                chat::HISTORY,
                &ChatHistoryParams {
                    session: session.clone(),
                    before: None,
                    limit: None,
                },
            )
            .await
            .expect_err("a terminal session has no chat history");
        assert!(refused.to_string().contains("agent.not_chat"), "{refused}");
    });
    let outcome = sandbox
        .run(
            "default",
            &with_json(&["agent", "stop", &session, "--force"]),
        )
        .success();
    assert_eq!(single_document(&outcome)["ok"], true);
}

#[test]
fn a_chat_run_that_exits_while_its_child_holds_the_output_is_recorded_exited() {
    let (sandbox, _go) = sandbox();
    // `leave` ends the turn and exits 0, leaving a child that holds the output open.
    let session = session_of(&start(&sandbox, "leave"));
    let ended = wait_for_exit(&sandbox, &session);
    assert_eq!(ended["execution"]["exit"]["reason"], "exited", "{ended}");
    assert_eq!(ended["execution"]["exit"]["code"], 0, "{ended}");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let (_, items) = all_items(&client, &session).await;
        let turns: Vec<_> = items.iter().filter_map(turn_state).collect();
        assert_eq!(
            turns,
            [TurnState::Completed],
            "the turn's end was read first"
        );
    });
}

#[test]
fn a_chat_run_that_reads_nothing_blocks_neither_other_starts_nor_its_stop() {
    let (sandbox, _go) = sandbox_with(&[("FAKE_CHAT_DEAF", "1")]);
    // The largest prompt there is, to a run that never reads it.
    let prompt = "x".repeat(massorca_protocol::methods::agents::MAX_PROMPT_BYTES);
    let start_read_only = || {
        sandbox.json_ok(
            "default",
            &[
                "agent",
                "start",
                "fakechat",
                "--workspace",
                "main",
                "--read-only",
                "--prompt",
                &prompt,
            ],
        )
    };
    let first = session_of(&start_read_only());
    let second = session_of(&start_read_only());
    for session in [&first, &second] {
        let stopped = sandbox.json_ok("default", &["agent", "stop", session]);
        let ended = if stopped["execution"]["state"] == "exited" {
            stopped
        } else {
            wait_for_exit(&sandbox, session)
        };
        assert_eq!(ended["execution"]["exit"]["reason"], "killed", "{ended}");
    }
}

#[test]
fn a_chat_start_that_fails_after_its_run_started_ends_the_run_and_names_the_session() {
    let (sandbox, _go) = sandbox_with(&[("MASSORCA_TEST_CHAT_FAIL_AT", "begin")]);
    let outcome = sandbox
        .run(
            "default",
            &with_json(&[
                "agent",
                "start",
                "fakechat",
                "--workspace",
                "main",
                "--prompt",
                "hi",
            ]),
        )
        .failure();
    let refused = single_document(&outcome);
    let session = refused["error"]["data"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("the error names the session: {refused}"))
        .to_owned();
    let ended = wait_for_exit(&sandbox, &session);
    assert_eq!(ended["execution"]["exit"]["reason"], "killed", "{ended}");
}
