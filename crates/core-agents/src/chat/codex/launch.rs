//! How a Codex chat run starts: the launch arguments, the handshake, the thread it opens or resumes, and the turn a
//! prompt becomes.
//!
//! The kind's launch plan is built as for a terminal run (`resume <ref>`, the permission mode's flags, `-m <model>`,
//! `-c key=value`), and is turned here into `app-server` and its config overrides plus the thread's settings:
//!
//! - `resume <ref>` names the thread to resume (`thread/resume`); without it a thread starts (`thread/start`).
//! - `-m`/`--model` becomes the thread's `model`.
//! - `--sandbox`/`-s`, `--ask-for-approval`/`-a`, `--approve-for-me` and `--dangerously-bypass-approvals-and-sandbox`
//!   become the thread's `sandbox`, `approvalPolicy` and `approvalsReviewer`: the permission mode is the thread's, as
//!   pv96 carries it, and never left to the account's `config.toml`.
//! - `-c key=value` stays a config override of `app-server` (the effort's `model_reasoning_effort`, say), except the
//!   terminal's `notify` hook (a chat reports through its output) and the permission keys the thread carries.
//! - Anything else is dropped: `app-server` takes a narrower set of options than the interactive CLI.

use serde_json::{Map, Value, json};

use super::line_of;

/// The subcommand a chat run starts: the app-server over standard streams (its default transport).
pub const APP_SERVER_ARG: &str = "app-server";

/// Config keys the thread carries, so a `-c` override of them is dropped.
const THREAD_CONFIG_KEYS: &[&str] = &[
    "approval_policy",
    "sandbox_mode",
    "approvals_reviewer",
    "sandbox_workspace_write",
    "notify",
];

/// The thread a run opens, from the kind's launch plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ThreadPlan {
    /// The thread to resume; a new thread starts without one.
    pub resume: Option<String>,
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub approval_policy: Option<String>,
    pub approvals_reviewer: Option<String>,
}

/// The arguments of `app-server`, and the thread to open, from the arguments the kind's launch plan built.
pub(super) fn plan(planned: Vec<String>) -> (Vec<String>, ThreadPlan) {
    let mut args = vec![APP_SERVER_ARG.to_owned()];
    let mut thread = ThreadPlan::default();
    let mut tokens = planned.into_iter();
    while let Some(token) = tokens.next() {
        let (name, inline) = match token.split_once('=') {
            Some((name, value)) if name.starts_with("--") => {
                (name.to_owned(), Some(value.to_owned()))
            }
            _ => (token.clone(), None),
        };
        let mut value = || inline.clone().or_else(|| tokens.next());
        match name.as_str() {
            "resume" if thread.resume.is_none() => thread.resume = value(),
            "-m" | "--model" => thread.model = value(),
            "-s" | "--sandbox" => thread.sandbox = value(),
            "-a" | "--ask-for-approval" => thread.approval_policy = value(),
            "--approve-for-me" => {
                thread.sandbox = Some("workspace-write".to_owned());
                thread.approval_policy = Some("on-request".to_owned());
                thread.approvals_reviewer = Some("auto_review".to_owned());
            }
            "--dangerously-bypass-approvals-and-sandbox" | "--yolo" => {
                thread.sandbox = Some("danger-full-access".to_owned());
                thread.approval_policy = Some("never".to_owned());
            }
            "-c" | "--config" => {
                if let Some(value) = value().filter(|value| !is_thread_config(value)) {
                    args.push("-c".to_owned());
                    args.push(value);
                }
            }
            "--enable" | "--disable" => {
                if let Some(value) = value() {
                    args.push(name.clone());
                    args.push(value);
                }
            }
            _ => {}
        }
    }
    (args, thread)
}

/// Whether a `-c key=value` override sets a key the thread carries.
fn is_thread_config(value: &str) -> bool {
    let key = value.split('=').next().unwrap_or_default().trim();
    let root = key.split('.').next().unwrap_or_default().trim_matches('"');
    THREAD_CONFIG_KEYS.contains(&root)
}

/// A JSON-RPC request line (the app-server leaves out the `jsonrpc` member on the wire).
pub(super) fn request(id: u64, method: &str, params: Value) -> Vec<u8> {
    line_of(&json!({ "id": id, "method": method, "params": params }))
}

/// A JSON-RPC notification line.
pub(super) fn notification(method: &str) -> Vec<u8> {
    line_of(&json!({ "method": method, "params": {} }))
}

/// The `initialize` params: MassOrca's client info, with the experimental API pv96 opts into.
pub(super) fn initialize_params() -> Value {
    json!({
        "clientInfo": { "name": "massorca", "title": "MassOrca", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "experimentalApi": true }
    })
}

/// The sandbox and approval policy of a thread whose plan names none: the kind's default mode, which asks. Said out loud
/// so the thread never inherits a policy from the account's `config.toml` or from the resumed thread (pv96).
const DEFAULT_SANDBOX: &str = "workspace-write";
const DEFAULT_APPROVAL_POLICY: &str = "on-request";

/// The method and params that open the plan's thread.
pub(super) fn open_thread(plan: &ThreadPlan) -> (&'static str, Value) {
    let mut params = Map::new();
    let sandbox = plan
        .sandbox
        .clone()
        .or_else(|| Some(DEFAULT_SANDBOX.to_owned()));
    let approval =
        (plan.approval_policy.clone()).or_else(|| Some(DEFAULT_APPROVAL_POLICY.to_owned()));
    for (key, value) in [
        ("model", &plan.model),
        ("sandbox", &sandbox),
        ("approvalPolicy", &approval),
        ("approvalsReviewer", &plan.approvals_reviewer),
    ] {
        if let Some(value) = value {
            params.insert(key.to_owned(), json!(value));
        }
    }
    match &plan.resume {
        Some(thread) => {
            params.insert("threadId".to_owned(), json!(thread));
            ("thread/resume", Value::Object(params))
        }
        None => ("thread/start", Value::Object(params)),
    }
}

/// The `turn/start` params that send `text` to `thread`.
pub(super) fn turn_params(thread: &str, text: &str) -> Value {
    json!({ "threadId": thread, "input": [{ "type": "text", "text": text }] })
}
