//! A Codex chat against a real service in a throwaway profile (decision 0173): the fake agent's `app-server` mode plays
//! Codex's JSON-RPC, the chat starts through the bridge as the start page starts it, and the turn's token counts reach
//! the store with the turn, where the transcript's footer reads them. No vendor CLI runs.
//!
//! One test only: it sets process variables the service and the fake read, which no other test in this binary may race
//! with.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{ServiceBridge, StoreState, connect_options};
use massorca_profile::Profile;
use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::chat::{ChatItemBody, RunType, TokenUsage, TurnState};
use massorca_protocol::methods::projects::{ProjectAddParams, ProjectKind};
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;
use massorca_server::{Service, ServiceConfig};
use serde_json::{Value, json};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
const THREAD: &str = "thr-fake-1";

/// The CLI crate's fake agent, built beside this test as the example `massorca-fake-agent`.
fn fake_agent() -> PathBuf {
    let exe = std::env::current_exe().expect("the test's own path");
    let profile_dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder of the test binary");
    let name = format!("massorca-fake-agent{}", std::env::consts::EXE_SUFFIX);
    let path = profile_dir.join("examples").join(name);
    assert!(
        path.is_file(),
        "build the fake agent first: cargo build -p massorca-desktop --examples ({})",
        path.display()
    );
    path
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// Kind `fakecodex`: the fake agent with the Codex adapter's chat block.
fn declare_fake_codex(home: &Path, fake: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("the kinds folder");
    let kind = json!({
        "id": "fakecodex",
        "name": "Fake Codex",
        "program": [fake],
        "prompt": { "mode": "none" },
        "session_ref": { "learn": "thread-id" },
        "resume": ["resume", "{ref}"],
        "chat": { "adapter": "codex-app-server", "min_version": "0.172.0", "max_major": 0 },
        "permission_modes": [{
            "id": "auto", "label": "Auto", "description": "Asks when needed.",
            "args": ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"]
        }],
        "default_permission_mode": "auto"
    });
    std::fs::write(dir.join("fakecodex.json"), kind.to_string()).expect("the kind file");
}

fn notify(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params })
}

/// One turn: a short reply, the conversation's token counts, then the turn's end.
fn script() -> Vec<Value> {
    vec![
        json!({ "await": "turn/start" }),
        notify(
            "turn/started",
            json!({ "threadId": "$THREAD", "turn": { "id": "$TURN", "status": "inProgress" } }),
        ),
        notify(
            "item/completed",
            json!({ "threadId": "$THREAD", "turnId": "$TURN",
                    "item": { "type": "agentMessage", "id": "m1", "text": "Hello" } }),
        ),
        notify(
            "thread/tokenUsage/updated",
            json!({ "threadId": "$THREAD", "turnId": "$TURN",
                    "tokenUsage": { "total": { "inputTokens": 100, "cachedInputTokens": 20, "outputTokens": 7,
                                               "reasoningOutputTokens": 2, "totalTokens": 107 } } }),
        ),
        notify(
            "turn/completed",
            json!({ "threadId": "$THREAD", "turn": { "id": "$TURN", "status": "completed" } }),
        ),
    ]
}

#[allow(unsafe_code)]
fn set_process_env(vars: &[(&str, &Path)]) {
    for (name, value) in vars {
        // SAFETY: this binary holds one test, and it calls this before it starts a service, a bridge or any thread.
        unsafe { std::env::set_var(name, value) };
    }
}

#[tokio::test]
async fn a_codex_chat_started_through_the_bridge_brings_its_turn_token_counts_to_the_store() {
    let fake = fake_agent();
    let temp = tempfile::tempdir().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    declare_fake_codex(&home, &fake);
    let scripts = temp.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("the scripts folder");
    let (first, resumed) = (scripts.join("first.jsonl"), scripts.join("resumed.jsonl"));
    let text: String = script().iter().map(|step| format!("{step}\n")).collect();
    std::fs::write(&first, text).expect("the script");
    std::fs::write(&resumed, "").expect("the resume script");
    let notes = temp.path().join("notes");
    std::fs::create_dir_all(&notes).expect("the folder project");
    set_process_env(&[
        ("MASSORCA_TEST_VERSION_PROBE_ALLOW", &fake),
        ("FAKE_CLAUDE_VERSION", Path::new("codex-cli 0.172.0")),
        ("FAKE_CODEX_THREAD", Path::new(THREAD)),
        ("FAKE_CODEX_SCRIPT", &first),
        ("FAKE_CODEX_RESUME_SCRIPT", &resumed),
    ]);

    let service = Service::start(ServiceConfig {
        profile: profile(&home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts");
    let mut bridge =
        ServiceBridge::start(profile(&home), connect_options(None)).expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    let mut state = StoreState::default();

    // The kinds advert lists chat for the Codex kind, so the start page offers Chat for it.
    let offered = timeout(WAIT, async {
        while !(state.agent_chat
            && state
                .agent_kind_chat
                .get("fakecodex")
                .is_some_and(|chat| chat.supported))
        {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(
        offered.is_ok(),
        "the service offers chat for the Codex kind: chat {}, kinds {:?}",
        state.agent_chat,
        state.agent_kind_chat
    );

    let project = handle
        .add_project(ProjectAddParams {
            path: notes.display().to_string(),
            name: Some("notes".to_owned()),
            kind: Some(ProjectKind::Folder),
            ..base_project_add_params()
        })
        .await
        .expect("the folder project is added");
    let workspace = handle
        .create_workspace(WorkspaceCreateParams {
            project: project.id,
            ..base_workspace_create_params()
        })
        .await
        .expect("the workspace is created")
        .id;

    let started = handle
        .start_session(
            AgentStartParams {
                workspace: Some(workspace),
                kind: "fakecodex".to_owned(),
                prompt: Some("say hello".to_owned()),
                ..base_agent_start_params()
            },
            true,
        )
        .await
        .expect("the chat starts");
    assert_eq!(started.fell_back, None, "the Codex kind can chat");
    assert_eq!(started.session.run, RunType::Chat);
    let session = started.session.id;

    handle.watch_chat(&session);
    let usage = timeout(WAIT, async {
        loop {
            let found = state.chat.get(&session).and_then(|transcript| {
                transcript.items().iter().find_map(|item| match &item.body {
                    ChatItemBody::Turn(turn) if turn.state == TurnState::Completed => turn.usage,
                    _ => None,
                })
            });
            if let Some(usage) = found {
                return usage;
            }
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the completed turn's usage in time; transcript: {:#?}",
            state.chat.get(&session).map(|held| held.items().to_vec())
        )
    });
    assert_eq!(
        usage,
        TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 20,
            output_tokens: 7,
            reasoning_output_tokens: 2,
            total_tokens: 107,
        }
    );

    handle.unwatch_chat(&session);
    drop(bridge);
    service.shutdown_handle().shutdown();
    timeout(WAIT, service.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

/// Every `AgentStartParams` field at its usual value; tests override the fields they vary.
fn base_agent_start_params() -> AgentStartParams {
    AgentStartParams {
        workspace: None,
        kind: "fakecodex".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: BTreeMap::new(),
        read_only: false,
        title: None,
        cols: None,
        rows: None,
        request_id: None,
        permission_mode: None,
        model: None,
        effort: None,
        fast: None,
    }
}

/// Every `WorkspaceCreateParams` field at its usual value; tests override the fields they vary.
fn base_workspace_create_params() -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: String::new(),
        name: "main".to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
        unique_name: false,
        name_generated: false,
        in_place: false,
    }
}

/// Every `ProjectAddParams` field at its usual value; tests override the fields they vary.
fn base_project_add_params() -> ProjectAddParams {
    ProjectAddParams {
        path: String::new(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    }
}
