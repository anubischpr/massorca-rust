//! Shared support of the Claude chat tests (decision 0101): the fake agent plays hand-written stream-json frames as kind
//! `fakeclaude`, whose `chat` block names the Claude adapter. No real vendor CLI runs: the service may probe only the
//! fake agent (`MASSORCA_TEST_VERSION_PROBE_ALLOW`). The service gets a temporary home, application-data and XDG
//! folders, and none of the vendor CLIs' own variables.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::harness::{Sandbox, with_json};
use massorca_client::{Client, ClientError, ConnectOptions, Subscription};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    self, Approval, ApprovalDecision, ChatApprovalAnswerParams, ChatEvent, ChatItem, ChatItemBody,
    ChatSubscribeParams, ChatUpdate, ToolStatus, TurnState,
};
use serde_json::{Value, json};

/// Variables a vendor CLI reads its home from; the service never gets them from the developer's shell.
pub const VENDOR_HOME_ENV: [&str; 3] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"];
pub const FAKE_AGENT: &str = env!("CARGO_BIN_EXE_massorca-fake-agent");

/// The files a test reads back: what reached the fake CLI's standard input, and its arguments per run.
pub struct Logs {
    pub stdin: PathBuf,
    pub args: PathBuf,
}

impl Logs {
    pub fn stdin_lines(&self) -> Vec<Value> {
        read_json_lines(&self.stdin)
    }

    pub fn runs(&self) -> Vec<Vec<String>> {
        read_json_lines(&self.args)
            .into_iter()
            .map(|run| serde_json::from_value(run).expect("an argument list"))
            .collect()
    }
}

pub fn read_json_lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// A frame of the scripts, with the session id the fake CLI reports.
pub fn frame(mut value: Value) -> Value {
    value["session_id"] = json!("s-test");
    value
}

pub fn text_delta(index: u64, text: &str) -> Value {
    frame(json!({ "type": "stream_event", "parent_tool_use_id": null,
                  "event": { "type": "content_block_delta", "index": index,
                             "delta": { "type": "text_delta", "text": text } } }))
}

pub fn message_start(id: &str) -> Value {
    frame(json!({ "type": "stream_event", "parent_tool_use_id": null,
                  "event": { "type": "message_start", "message": { "id": id } } }))
}

pub fn text_start(index: u64) -> Value {
    frame(json!({ "type": "stream_event", "parent_tool_use_id": null,
                  "event": { "type": "content_block_start", "index": index,
                             "content_block": { "type": "text", "text": "" } } }))
}

pub fn assistant(id: &str, content: Value) -> Value {
    frame(json!({ "type": "assistant", "parent_tool_use_id": null,
                  "message": { "id": id, "role": "assistant", "content": content } }))
}

pub fn bash_call(id: &str, command: &str) -> Value {
    json!([{ "type": "tool_use", "id": id, "name": "Bash", "input": { "command": command } }])
}

pub fn permission(request_id: &str, tool_use_id: &str, command: &str) -> Value {
    json!({ "type": "control_request", "request_id": request_id, "request": {
        "subtype": "can_use_tool", "tool_name": "Bash", "input": { "command": command },
        "permission_suggestions": [{ "type": "addRules", "behavior": "allow", "destination": "localSettings",
                                     "rules": [{ "toolName": "Bash", "ruleContent": command }] }],
        "tool_use_id": tool_use_id
    }})
}

pub fn tool_done(id: &str) -> Value {
    frame(
        json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": id, "content": "(Bash completed with no output)", "is_error": false }
        ]}}),
    )
}

pub fn result(success: bool) -> Value {
    if success {
        frame(
            json!({ "type": "result", "subtype": "success", "is_error": false, "result": "done" }),
        )
    } else {
        frame(json!({ "type": "result", "subtype": "error_during_execution", "is_error": true }))
    }
}

pub fn await_input(what: &str) -> Value {
    json!({ "await": what })
}

pub fn write_script(path: &Path, steps: &[Value]) {
    let text: String = steps.iter().map(|step| format!("{step}\n")).collect();
    std::fs::write(path, text).expect("write a script");
}

/// A sandbox whose service knows kind `fakeclaude` (the fake agent with the Claude adapter's `chat` block) and has a
/// folder workspace `main`. `script` is what a first run plays, `resume_script` what a resumed run plays, and `env` more
/// variables for the service.
pub fn sandbox(script: &[Value], resume_script: &[Value], env: &[(&str, &str)]) -> (Sandbox, Logs) {
    let sandbox = Sandbox::new();
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
    let scripts = sandbox.home().join("scripts");
    std::fs::create_dir_all(&scripts).expect("a scripts folder");
    let (first, resumed) = (scripts.join("first.jsonl"), scripts.join("resumed.jsonl"));
    write_script(&first, script);
    write_script(&resumed, resume_script);
    let logs = Logs {
        stdin: scripts.join("stdin.log"),
        args: scripts.join("args.log"),
    };
    for (name, value) in [
        (
            "MASSORCA_TEST_VERSION_PROBE_ALLOW",
            PathBuf::from(FAKE_AGENT),
        ),
        ("FAKE_CLAUDE_SCRIPT", first),
        ("FAKE_CLAUDE_RESUME_SCRIPT", resumed),
        ("FAKE_CLAUDE_STDIN_LOG", logs.stdin.clone()),
        ("FAKE_CLAUDE_ARGS_LOG", logs.args.clone()),
    ] {
        sandbox.set_env(name, &value.to_string_lossy());
    }
    for (name, value) in env {
        sandbox.set_env(name, value);
    }

    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fakeclaude",
        "name": "Fake Claude",
        "program": [FAKE_AGENT],
        "prompt": { "mode": "none" },
        "session_ref": { "assign": ["--session-id", "{ref}"] },
        "resume": ["--resume", "{ref}"],
        "chat": { "adapter": "claude-stream-json", "min_version": "2.1.258", "max_major": 2 }
    });
    std::fs::write(dir.join("fakeclaude.json"), kind.to_string()).expect("write a kind file");

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
    (sandbox, logs)
}

pub fn start_chat(sandbox: &Sandbox, prompt: &str) -> String {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fakeclaude",
            "--workspace",
            "main",
            "--chat",
            "--prompt",
            prompt,
        ],
    );
    assert_eq!(started["run"], "chat", "{started}");
    started["id"].as_str().expect("a session id").to_owned()
}

pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
}

pub async fn connect(home: &Path) -> Client {
    let profile =
        Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
            None
        })
        .expect("the profile resolves");
    let client = Client::connect(
        &profile,
        ConnectOptions::new(PeerInfo {
            name: "chat-claude-test".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("the client connects");
    assert!(
        client.has_capability(massorca_protocol::capabilities::AGENT_CHAT),
        "the service offers the chat methods"
    );
    client
}

pub fn remote_code(error: &ClientError) -> String {
    match error {
        ClientError::Remote(body) => body.code.clone(),
        other => panic!("expected a remote error, got {other:?}"),
    }
}

/// A client's copy of a transcript, kept from a chat stream as a client keeps it.
pub struct Transcript {
    pub stream: Subscription,
    pub items: BTreeMap<String, ChatItem>,
}

impl Transcript {
    pub async fn open(client: &Client, session: &str) -> Self {
        let stream = client
            .subscribe(
                chat::SUBSCRIBE,
                &ChatSubscribeParams {
                    session: session.to_owned(),
                    after: None,
                    limit: Some(200),
                },
            )
            .await
            .expect("agent.chat.subscribe");
        Self {
            stream,
            items: BTreeMap::new(),
        }
    }

    pub fn apply(&mut self, event: ChatEvent) {
        match event {
            ChatEvent::Snapshot(snapshot) => {
                self.items = snapshot
                    .items
                    .into_iter()
                    .map(|item| (item.id.clone(), item))
                    .collect();
            }
            ChatEvent::Batch(batch) => {
                for update in batch.updates {
                    match update {
                        ChatUpdate::Item { item } => {
                            self.items.insert(item.id.clone(), item);
                        }
                        ChatUpdate::Append {
                            id, offset, text, ..
                        } => {
                            let held = self.items.get_mut(&id).expect("an append to a held item");
                            let (ChatItemBody::AssistantMessage { text: held, .. }
                            | ChatItemBody::Thinking { text: held, .. }) = &mut held.body
                            else {
                                panic!("an append to a message");
                            };
                            assert_eq!(held.len() as u64, offset, "appends arrive in order");
                            held.push_str(&text);
                        }
                        ChatUpdate::Unknown => panic!("an unknown update"),
                    }
                }
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    /// Applies events until `found` finds something in the items.
    pub async fn until<T>(
        &mut self,
        what: &str,
        mut found: impl FnMut(&[ChatItem]) -> Option<T>,
    ) -> T {
        loop {
            let items: Vec<ChatItem> = self.ordered();
            if let Some(value) = found(&items) {
                return value;
            }
            let event =
                tokio::time::timeout(crate::harness::WAIT, self.stream.next_as::<ChatEvent>())
                    .await
                    .unwrap_or_else(|_| panic!("{what} in time; items: {items:#?}"))
                    .expect("the stream is open")
                    .expect("a chat event");
            self.apply(event);
        }
    }

    pub fn ordered(&self) -> Vec<ChatItem> {
        let mut items: Vec<ChatItem> = self.items.values().cloned().collect();
        items.sort_by_key(|item| item.position);
        items
    }
}

pub fn approval_of<'a>(items: &'a [ChatItem], request_id: &str) -> Option<&'a Approval> {
    items.iter().find_map(|item| match &item.body {
        ChatItemBody::Approval(approval) if approval.request_id == request_id => Some(approval),
        _ => None,
    })
}

pub fn turns(items: &[ChatItem]) -> Vec<(TurnState, Option<Outcome>)> {
    items
        .iter()
        .filter_map(|item| match &item.body {
            ChatItemBody::Turn(turn) => Some((turn.state, turn.outcome)),
            _ => None,
        })
        .collect()
}

pub fn tool_status(items: &[ChatItem], id: &str) -> Option<ToolStatus> {
    items.iter().find_map(|item| match &item.body {
        ChatItemBody::ToolUse(tool) if tool.tool_use_id == id => Some(tool.status),
        _ => None,
    })
}

pub fn answer(
    session: &str,
    request_id: &str,
    decision: ApprovalDecision,
) -> ChatApprovalAnswerParams {
    ChatApprovalAnswerParams {
        session: session.to_owned(),
        request_id: request_id.to_owned(),
        decision,
        scope: None,
        message: None,
        seq: None,
    }
}

/// Applies events until the transcript shows an assistant message with `text`.
pub async fn until_text(transcript: &mut Transcript, text: &str) {
    transcript
        .until("the assistant's text", |items| {
            items
                .iter()
                .any(|item| matches!(&item.body, ChatItemBody::AssistantMessage { text: held, .. } if held == text))
                .then_some(())
        })
        .await;
}

/// Sends `text` to the session with `agent prompt --force`; returns the receipt.
pub fn force_prompt(sandbox: &Sandbox, session: &str, text: &str) -> Value {
    sandbox.json_ok("default", &["agent", "prompt", session, text, "--force"])
}
