//! The chat arm against a real service in a throwaway profile, with the fake agent playing Claude's stream-json
//! (decision 0101): a chat started through the bridge streams its snapshot and its reply into the transcript model, one
//! approval is answered with the `seq` the person saw, a second prompt is sent, and the running turn is interrupted.
//! No vendor CLI runs: the service may probe only the fake agent, which the CLI crate's source provides as an example.
//!
//! One test only: it sets a process variable the service reads, which no other test in this binary may race with.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::agents::{AgentPromptParams, AgentStartParams, ReceiptKind};
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalScope, ApprovalState, ChatApprovalAnswerParams, ChatItem,
    ChatItemBody, RunType, TurnState,
};
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;
use massorca_server::{RunningService, Service, ServiceConfig};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
/// The variable that names the programs a test service may probe for a version (core-agents' `test-hooks`).
const PROBE_ALLOW: &str = "MASSORCA_TEST_VERSION_PROBE_ALLOW";

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

#[allow(unsafe_code)]
fn allow_probing(program: &Path) {
    // SAFETY: this binary holds one test, and it calls this before it starts a service, a bridge or any thread.
    unsafe { std::env::set_var(PROBE_ALLOW, program) };
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// The kind `fakeclaude`: the fake agent with the Claude adapter's chat block.
fn declare_fake_claude(home: &Path, fake: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("the kinds folder");
    let kind = json!({
        "id": "fakeclaude",
        "name": "Fake Claude",
        "program": [fake],
        "prompt": { "mode": "none" },
        "session_ref": { "assign": ["--session-id", "{ref}"] },
        "resume": ["--resume", "{ref}"],
        "chat": { "adapter": "claude-stream-json", "min_version": "2.1.258", "max_major": 2 }
    });
    std::fs::write(dir.join("fakeclaude.json"), kind.to_string()).expect("the kind file");
}

fn git(dir: &Path, args: &[&str]) {
    let output = massorca_process::command("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn frame(mut value: Value) -> Value {
    value["session_id"] = json!("s-test");
    value
}

/// What the fake CLI plays: it waits for the first prompt, streams a reply, asks to run a command and waits for the
/// answer, ends the turn, then waits for a second prompt and a running turn it can be interrupted in.
fn script() -> Vec<Value> {
    let delta = |text: &str| {
        frame(json!({ "type": "stream_event", "parent_tool_use_id": null,
            "event": { "type": "content_block_delta", "index": 0,
                       "delta": { "type": "text_delta", "text": text } } }))
    };
    vec![
        json!({ "await": "user" }),
        frame(json!({ "type": "system", "subtype": "init", "claude_code_version": "2.1.330" })),
        frame(json!({ "type": "stream_event", "parent_tool_use_id": null,
            "event": { "type": "message_start", "message": { "id": "m1" } } })),
        frame(json!({ "type": "stream_event", "parent_tool_use_id": null,
            "event": { "type": "content_block_start", "index": 0,
                       "content_block": { "type": "text", "text": "" } } })),
        delta("Hello"),
        delta(", world"),
        frame(
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": {
            "id": "m1", "role": "assistant", "content": [{ "type": "text", "text": "Hello, world" }] } }),
        ),
        frame(
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": {
            "id": "m1", "role": "assistant", "content": [{ "type": "tool_use", "id": "toolu_1",
                "name": "Bash", "input": { "command": "touch probe.txt" } }] } }),
        ),
        json!({ "type": "control_request", "request_id": "r-1", "request": {
            "subtype": "can_use_tool", "tool_name": "Bash", "input": { "command": "touch probe.txt" },
            "permission_suggestions": [{ "type": "addRules", "behavior": "allow",
                "destination": "localSettings",
                "rules": [{ "toolName": "Bash", "ruleContent": "touch probe.txt" }] }],
            "tool_use_id": "toolu_1" } }),
        json!({ "await": "control_response" }),
        frame(
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": "toolu_1",
              "content": "(Bash completed with no output)", "is_error": false }] } }),
        ),
        frame(
            json!({ "type": "result", "subtype": "success", "is_error": false, "result": "done" }),
        ),
        json!({ "await": "user" }),
        frame(
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": {
            "id": "m2", "role": "assistant", "content": [{ "type": "text", "text": "Working." }] } }),
        ),
        json!({ "await": "interrupt" }),
        frame(json!({ "type": "result", "subtype": "error_during_execution", "is_error": true })),
    ]
}

/// Applies the bridge's updates to the store state until `found` finds something in the session's transcript.
async fn until<T>(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    session: &str,
    what: &str,
    mut found: impl FnMut(&[ChatItem]) -> Option<T>,
) -> T {
    let outcome = timeout(WAIT, async {
        loop {
            if let Some(transcript) = state.chat.get(session)
                && let Some(value) = found(transcript.items())
            {
                return value;
            }
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    outcome.unwrap_or_else(|_| {
        panic!(
            "{what} in time; transcript: {:#?}",
            state.chat.get(session).map(|held| held.items().to_vec())
        )
    })
}

fn turn_states(items: &[ChatItem]) -> Vec<TurnState> {
    items
        .iter()
        .filter_map(|item| match &item.body {
            ChatItemBody::Turn(turn) => Some(turn.state),
            _ => None,
        })
        .collect()
}

fn text_of<'a>(items: &'a [ChatItem], wanted: &str) -> Option<&'a ChatItem> {
    items.iter().find(
        |item| matches!(&item.body, ChatItemBody::AssistantMessage { text, .. } if text == wanted),
    )
}

#[tokio::test]
async fn a_chat_started_through_the_bridge_streams_answers_an_approval_and_interrupts() {
    let fake = fake_agent();
    allow_probing(&fake);
    let temp = tempfile::tempdir().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    declare_fake_claude(&home, &fake);
    let repo = temp.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository folder");
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("README.md"), "hello\n").expect("a readme");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "initial"]);
    let script_path = temp.path().join("script.jsonl");
    let text: String = script().iter().map(|step| format!("{step}\n")).collect();
    std::fs::write(&script_path, text).expect("the script");

    let service: RunningService = Service::start(ServiceConfig {
        profile: profile(&home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts");
    let mut bridge =
        ServiceBridge::start(profile(&home), connect_options(None)).expect("the bridge starts");
    let handle: ServiceHandle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    let mut state = StoreState::default();

    // The store learns that the service chats and that the fake kind can: the probe found a version in range.
    timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
            if state.agent_chat
                && state
                    .agent_kind_chat
                    .get("fakeclaude")
                    .is_some_and(|chat| chat.supported)
            {
                return;
            }
        }
    })
    .await
    .expect("the service offers chat for the fake kind");

    let project = handle
        .add_project(ProjectAddParams {
            path: repo.display().to_string(),
            name: Some("demo".to_owned()),
            workspaces_root: Some(temp.path().join("demo-worktrees").display().to_string()),
            ..base_project_add_params()
        })
        .await
        .expect("the project is added");
    let workspace = handle
        .create_workspace(WorkspaceCreateParams {
            project: project.id,
            ..base_workspace_create_params()
        })
        .await
        .expect("the workspace is created");

    // Start as the start page does: Chat mode, the first prompt in the call, no grid.
    let started = handle
        .start_session(
            AgentStartParams {
                workspace: Some(workspace.id),
                kind: "fakeclaude".to_owned(),
                prompt: Some("say hello".to_owned()),
                env: BTreeMap::from([(
                    "FAKE_CLAUDE_SCRIPT".to_owned(),
                    script_path.display().to_string(),
                )]),
                cols: Some(120),
                rows: Some(40),
                ..base_agent_start_params()
            },
            true,
        )
        .await
        .expect("the chat starts");
    assert_eq!(started.fell_back, None, "the fake kind can chat");
    assert_eq!(started.session.run, RunType::Chat);
    assert_eq!(started.session.terminal_id, None, "a chat has no terminal");
    let session = started.session.id;

    // The stream: its snapshot, then the reply as it is written, up to the permission request.
    handle.watch_chat(&session);
    let pending = until(
        &mut updates,
        &mut state,
        &session,
        "the permission request",
        |items| {
            items.iter().find_map(|item| match &item.body {
                ChatItemBody::Approval(approval)
                    if approval.request_id == "r-1" && approval.state == ApprovalState::Pending =>
                {
                    Some((item.seq, approval.clone()))
                }
                _ => None,
            })
        },
    )
    .await;
    let (seq, approval) = pending;
    assert!(approval.can_always);
    assert_eq!(approval.tool, "Bash");
    assert_eq!(approval.input_summary, "touch probe.txt");
    assert_eq!(approval.suggestions.len(), 1, "{approval:?}");
    assert_eq!(approval.suggestions[0].rules, ["Bash(touch probe.txt)"]);
    let held = state.chat.get(&session).expect("the transcript is held");
    assert!(held.items().iter().any(|item| matches!(
        &item.body,
        ChatItemBody::UserMessage { text } if text == "say hello"
    )));
    assert!(
        text_of(held.items(), "Hello, world").is_some(),
        "the streamed deltas joined into the reply: {:#?}",
        held.items()
    );

    // One approval answered with the seq the person saw; the first answer wins.
    let answered = handle
        .agent_chat_answer(ChatApprovalAnswerParams {
            session: session.clone(),
            scope: Some(ApprovalScope::Always),
            seq: Some(seq),
            ..base_chat_approval_answer_params()
        })
        .await
        .expect("the answer is accepted");
    let ChatItemBody::Approval(settled) = &answered.body else {
        panic!("an approval item: {answered:?}");
    };
    assert_eq!(settled.state, ApprovalState::Allowed);
    assert_eq!(settled.scope, Some(ApprovalScope::Always));
    let again = handle
        .agent_chat_answer(ChatApprovalAnswerParams {
            session: session.clone(),
            decision: ApprovalDecision::Deny,
            seq: Some(seq),
            ..base_chat_approval_answer_params()
        })
        .await
        .expect_err("a second answer is refused");
    assert_eq!(again.code(), Some("agent.approval_settled"), "{again:?}");
    until(
        &mut updates,
        &mut state,
        &session,
        "the first turn's end",
        |items| (turn_states(items) == [TurnState::Completed]).then_some(()),
    )
    .await;

    // A second prompt goes to the chat, and the turn it opens is interrupted; the session stays.
    let receipt = handle
        .agent_prompt(AgentPromptParams {
            session: session.clone(),
            ..base_agent_prompt_params()
        })
        .await
        .expect("the prompt is accepted");
    assert_eq!(receipt.receipt, ReceiptKind::SentToChat);
    until(
        &mut updates,
        &mut state,
        &session,
        "the second reply",
        |items| text_of(items, "Working.").map(|_| ()),
    )
    .await;
    let interrupted = handle
        .agent_chat_interrupt(&session)
        .await
        .expect("the interrupt");
    assert!(interrupted.interrupted);
    until(
        &mut updates,
        &mut state,
        &session,
        "the second turn interrupted",
        |items| {
            (turn_states(items) == [TurnState::Completed, TurnState::Interrupted]).then_some(())
        },
    )
    .await;

    // Stopping the view's stream drops the transcript from the store.
    handle.unwatch_chat(&session);
    timeout(WAIT, async {
        while state.chat.contains_key(&session) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await
    .expect("the store drops the transcript of a stream nobody follows");

    drop(bridge);
    service.shutdown_handle().shutdown();
    timeout(WAIT, service.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

/// Every `ChatApprovalAnswerParams` field at its usual value; tests override the fields they vary.
fn base_chat_approval_answer_params() -> massorca_protocol::methods::chat::ChatApprovalAnswerParams
{
    massorca_protocol::methods::chat::ChatApprovalAnswerParams {
        session: String::new(),
        request_id: "r-1".to_owned(),
        decision: massorca_protocol::methods::chat::ApprovalDecision::Allow,
        scope: None,
        message: None,
        seq: None,
    }
}

/// Every `AgentPromptParams` field at its usual value; tests override the fields they vary.
fn base_agent_prompt_params() -> massorca_protocol::methods::agents::AgentPromptParams {
    massorca_protocol::methods::agents::AgentPromptParams {
        session: String::new(),
        text: "again".to_owned(),
        force: false,
    }
}

/// Every `AgentStartParams` field at its usual value; tests override the fields they vary.
fn base_agent_start_params() -> massorca_protocol::methods::agents::AgentStartParams {
    massorca_protocol::methods::agents::AgentStartParams {
        workspace: None,
        kind: "fake".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: Default::default(),
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
fn base_workspace_create_params() -> massorca_protocol::methods::workspaces::WorkspaceCreateParams {
    massorca_protocol::methods::workspaces::WorkspaceCreateParams {
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
    }
}

/// Every `ProjectAddParams` field at its usual value; tests override the fields they vary.
fn base_project_add_params() -> massorca_protocol::methods::projects::ProjectAddParams {
    massorca_protocol::methods::projects::ProjectAddParams {
        path: String::new(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    }
}
