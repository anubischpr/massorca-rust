//! Shared support of the Codex chat tests (decision 0173): the fake agent's `app-server` mode plays hand-written JSON-RPC
//! messages as kind `fakecodex`, whose `chat` block names the Codex adapter. The fake answers the handshake, the thread
//! and the turn requests itself; a script line may name the open thread as `$THREAD` and the last turn as `$TURN`. No
//! real vendor CLI runs, and the service gets the Claude support's private folders and none of the vendor CLIs' homes.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::path::PathBuf;

use crate::chat_claude_support::{FAKE_AGENT, Logs, VENDOR_HOME_ENV, write_script};
use crate::harness::{Sandbox, with_json};
use serde_json::{Value, json};

/// The thread id the fake app-server opens.
pub const THREAD: &str = "thr-fake-1";

pub fn notify(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params })
}

pub fn await_method(method: &str) -> Value {
    json!({ "await": method })
}

/// A script step that keeps the thread's answer back until the file `name` in the script's folder exists (the fake
/// app-server's `hold_open`): a test sends prompts while the thread still opens, then creates the file with [`open_gate`].
pub fn hold_open(name: &str) -> Value {
    json!({ "hold_open": name })
}

/// The file `hold_open(name)` waits for, in the sandbox's scripts folder.
pub fn open_gate(sandbox: &Sandbox, name: &str) -> PathBuf {
    sandbox.home().join("scripts").join(name)
}

/// `item/started` or `item/completed` of `item` in the running turn.
pub fn item(completed: bool, item: Value) -> Value {
    let method = if completed {
        "item/completed"
    } else {
        "item/started"
    };
    notify(
        method,
        json!({ "threadId": "$THREAD", "turnId": "$TURN", "item": item }),
    )
}

pub fn text_delta(item_id: &str, delta: &str) -> Value {
    notify(
        "item/agentMessage/delta",
        json!({ "threadId": "$THREAD", "turnId": "$TURN", "itemId": item_id, "delta": delta }),
    )
}

pub fn command(id: &str, command: &str, status: &str) -> Value {
    json!({ "type": "commandExecution", "id": id, "command": command, "cwd": ".", "status": status })
}

pub fn approval_request(request_id: u64, item_id: &str, command: &str) -> Value {
    json!({ "id": request_id, "method": "item/commandExecution/requestApproval",
            "params": { "threadId": "$THREAD", "turnId": "$TURN", "itemId": item_id, "command": command } })
}

pub fn turn_completed(status: &str) -> Value {
    notify(
        "turn/completed",
        json!({ "threadId": "$THREAD", "turn": { "id": "$TURN", "status": status } }),
    )
}

pub fn turn_started() -> Value {
    notify(
        "turn/started",
        json!({ "threadId": "$THREAD", "turn": { "id": "$TURN", "status": "inProgress" } }),
    )
}

/// A sandbox whose service knows kind `fakecodex` (the fake agent with the Codex adapter's `chat` block, a model and an
/// effort flag, and a mode that asks for approvals) and has a folder workspace `main`. `script` is what a started thread
/// plays, `resume_script` what a resumed one plays.
pub fn sandbox(script: &[Value], resume_script: &[Value]) -> (Sandbox, Logs) {
    let sandbox = Sandbox::new();
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
        ("FAKE_CLAUDE_VERSION", PathBuf::from("codex-cli 0.172.0")),
        ("FAKE_CODEX_THREAD", PathBuf::from(THREAD)),
        ("FAKE_CODEX_SCRIPT", first),
        ("FAKE_CODEX_RESUME_SCRIPT", resumed),
        ("FAKE_CODEX_STDIN_LOG", logs.stdin.clone()),
        ("FAKE_CODEX_ARGS_LOG", logs.args.clone()),
    ] {
        sandbox.set_env(name, &value.to_string_lossy());
    }

    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fakecodex",
        "name": "Fake Codex",
        "program": [FAKE_AGENT],
        "prompt": { "mode": "none" },
        "session_ref": { "learn": "thread-id" },
        "resume": ["resume", "{ref}"],
        "chat": { "adapter": "codex-app-server", "min_version": "0.172.0", "max_major": 0 },
        "permission_modes": [{
            "id": "auto", "label": "Auto", "description": "Asks when needed.",
            "args": ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"]
        }],
        "default_permission_mode": "auto",
        "model": { "args": ["-m", "{value}"], "choices": [{ "id": "gpt-test", "label": "GPT test" }] },
        "effort": {
            "args": ["-c", "model_reasoning_effort={value}"],
            "levels": [{ "id": "high", "label": "High" }]
        }
    });
    std::fs::write(dir.join("fakecodex.json"), kind.to_string()).expect("write a kind file");

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

/// Starts a `fakecodex` chat with `prompt` and the kind's model and effort; returns the session id.
pub fn start_chat(sandbox: &Sandbox, prompt: &str) -> String {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fakecodex",
            "--workspace",
            "main",
            "--chat",
            "--model",
            "gpt-test",
            "--effort",
            "high",
            "--prompt",
            prompt,
        ],
    );
    assert_eq!(started["run"], "chat", "{started}");
    started["id"].as_str().expect("a session id").to_owned()
}

/// The lines that reached the fake app-server with `method`.
pub fn sent_with(logs: &Logs, method: &str) -> Vec<Value> {
    logs.stdin_lines()
        .into_iter()
        .filter(|line| line["method"] == method)
        .collect()
}
