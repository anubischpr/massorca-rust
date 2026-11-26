//! The adapter of Codex's app-server dialect (decision 0173; inventory AG-18). The CLI runs as `codex app-server`: JSON-RPC
//! without the `jsonrpc` member, one message per line on standard input and output. The adapter speaks the client side:
//!
//! - [`launch`]: the launch arguments, the handshake (`initialize`, then `initialized`), the thread it starts or resumes,
//!   and the `turn/start` a prompt becomes.
//! - [`items`]: the thread's items (`item/started`, `item/completed`, their deltas) and the turn's plan.
//! - [`requests`]: the requests the app-server sends (approvals and the ones MassOrca refuses) and their answers.
//! - [`turns`]: the answers to MassOrca's own requests, the turn's end, its errors and usage, and the interrupt.
//!
//! The vocabulary is the one of pv96's Codex adapter and the app-server's public documentation, for Codex 0.172.0. A
//! notification outside it becomes one warning notice per method that names it, never its content.

mod items;
mod launch;
mod requests;
mod turns;

use std::collections::{HashMap, HashSet};

use massorca_protocol::methods::chat::{Approval, ChatItemBody, NoticeLevel, ToolUse};
use serde_json::Value;

pub use self::launch::APP_SERVER_ARG;
use self::launch::ThreadPlan;
use super::adapter::{ApprovalAnswer, ChatAdapter, JournalOp};
use super::claude::bounded_name;

/// The name a kind's `chat.adapter` gives this dialect.
pub const ADAPTER_NAME: &str = "codex-app-server";

/// What one of MassOrca's own requests was, so its answer is read for what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sent {
    Initialize,
    OpenThread,
    TurnStart,
    Interrupt,
}

/// Where the run's thread is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Thread {
    /// The handshake or the thread's opening is under way; prompts wait.
    Opening,
    Open(String),
    /// The thread could not be opened: prompts are still sent, and the app-server's refusal ends their turns.
    Failed,
}

/// A tool item the adapter follows until it completes.
#[derive(Debug, Clone)]
struct Tool {
    tool: ToolUse,
    /// The output streamed so far.
    output: String,
}

/// A permission request waiting for the person.
#[derive(Debug, Clone)]
struct Request {
    /// The request's JSON-RPC id, as the app-server sent it (a number or a string).
    id: Value,
    approval: Approval,
}

/// Codex's app-server adapter. One per run: it follows the run's thread, turn, items and requests.
#[derive(Debug)]
pub struct CodexAdapter {
    plan: ThreadPlan,
    thread: Thread,
    next_id: u64,
    sent: HashMap<u64, Sent>,
    /// Prompts waiting for the thread to open, or for an interrupted turn to end.
    waiting: Vec<String>,
    /// The turn the app-server runs, once it named it.
    turn: Option<String>,
    /// The turns the app-server ended: an answer that names one of them names no running turn.
    ended: HashSet<String>,
    /// Turns MassOrca interrupted: the service already ended them, so what they still send is skipped.
    interrupted: HashSet<String>,
    /// An interrupt asked for before the app-server named the turn.
    interrupt_wanted: bool,
    /// Item keys the journal holds this turn.
    seen: HashSet<String>,
    tools: HashMap<String, Tool>,
    requests: HashMap<String, Request>,
    /// The turn's failure was already shown by an `error` notification.
    error_shown: bool,
    /// Notification methods and item types already noted as unknown.
    warned: HashSet<String>,
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            plan: ThreadPlan::default(),
            thread: Thread::Opening,
            next_id: 0,
            sent: HashMap::new(),
            waiting: Vec::new(),
            turn: None,
            ended: HashSet::new(),
            interrupted: HashSet::new(),
            interrupt_wanted: false,
            seen: HashSet::new(),
            tools: HashMap::new(),
            requests: HashMap::new(),
            error_shown: false,
            warned: HashSet::new(),
        }
    }

    /// A request line of MassOrca's own, remembered so its answer is read for what it is.
    fn send(&mut self, sent: Sent, method: &str, params: Value) -> Vec<u8> {
        self.next_id += 1;
        self.sent.insert(self.next_id, sent);
        launch::request(self.next_id, method, params)
    }

    /// The `turn/start` of `text` on the open thread (or on none, which the app-server refuses).
    fn turn_start(&mut self, text: &str) -> Vec<u8> {
        let thread = match &self.thread {
            Thread::Open(thread) => thread.clone(),
            _ => String::new(),
        };
        self.send(
            Sent::TurnStart,
            "turn/start",
            launch::turn_params(&thread, text),
        )
    }

    /// Whether prompts wait: the thread is still opening, or an interrupted turn has not ended yet.
    fn holds_prompts(&self) -> bool {
        self.thread == Thread::Opening
            || self
                .turn
                .as_ref()
                .is_some_and(|turn| self.interrupted.contains(turn))
    }

    /// The waiting prompts' `turn/start`s, once nothing holds them.
    fn release_prompts(&mut self) -> Vec<JournalOp> {
        if self.holds_prompts() {
            return Vec::new();
        }
        std::mem::take(&mut self.waiting)
            .iter()
            .map(|text| JournalOp::Send {
                bytes: self.turn_start(text),
            })
            .collect()
    }

    /// Forgets what the adapter follows within a turn, and an interrupt still waiting for one: it was asked for the turn
    /// that just ended.
    fn end_of_turn(&mut self) {
        self.turn = None;
        self.interrupt_wanted = false;
        self.seen.clear();
        self.tools.clear();
        self.error_shown = false;
    }

    /// A warning notice for a method or item type MassOrca does not know, once per run.
    fn unknown(&mut self, what: &str, name: &str) -> Vec<JournalOp> {
        let name = bounded_name(name);
        if !self.warned.insert(format!("{what}:{name}")) {
            return Vec::new();
        }
        vec![notice(
            NoticeLevel::Warning,
            format!("A {what} of a kind MassOrca does not know was skipped ({name})."),
        )]
    }

    /// Whether a message belongs to another thread (a sub-agent's) or to a turn MassOrca interrupted.
    fn foreign(&self, params: &Value) -> bool {
        self.other_thread(params) || self.interrupted_turn(params)
    }

    /// Whether a message names a thread other than the run's (a sub-agent's).
    fn other_thread(&self, params: &Value) -> bool {
        match (&self.thread, text_of(&params["threadId"])) {
            (Thread::Open(ours), Some(theirs)) => ours != theirs,
            _ => false,
        }
    }

    /// Whether a message names a turn MassOrca interrupted.
    fn interrupted_turn(&self, params: &Value) -> bool {
        let turn = text_of(&params["turnId"]).or_else(|| text_of(&params["turn"]["id"]));
        turn.is_some_and(|turn| self.interrupted.contains(turn))
    }
}

fn notice(level: NoticeLevel, text: impl Into<String>) -> JournalOp {
    JournalOp::Add {
        key: None,
        parent: None,
        body: ChatItemBody::Notice {
            level,
            text: text.into(),
        },
    }
}

fn text_of(value: &Value) -> Option<&str> {
    value.as_str().filter(|text| !text.is_empty())
}

/// A JSON-RPC id as the text an approval item carries.
fn id_text(id: &Value) -> Option<String> {
    match id {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn line_of(value: &Value) -> Vec<u8> {
    let mut bytes = value.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}

/// Notifications of the vocabulary that carry nothing for the journal.
const SKIPPED_METHODS: &[&str] = &[
    "thread/started",
    "thread/status/changed",
    "thread/name/updated",
    "thread/compacted",
    "thread/archived",
    "thread/unarchived",
    "turn/diff/updated",
    "item/reasoning/textDelta",
    "item/reasoning/summaryPartAdded",
    "item/fileChange/outputDelta",
    "item/mcpToolCall/progress",
    "item/plan/delta",
    "account/updated",
    "account/rateLimits/updated",
    "account/login/completed",
    "mcpServer/startupStatus/updated",
    "mcpServer/oauthLogin/completed",
    "skills/changed",
    "windows/worldWritableWarning",
];

impl ChatAdapter for CodexAdapter {
    fn launch_args(&mut self, planned: Vec<String>) -> Vec<String> {
        let (args, plan) = launch::plan(planned);
        self.plan = plan;
        args
    }

    fn opening(&mut self) -> Vec<u8> {
        self.send(Sent::Initialize, "initialize", launch::initialize_params())
    }

    fn encode_prompt(&mut self, text: &str) -> Vec<u8> {
        if self.holds_prompts() {
            self.waiting.push(text.to_owned());
            return Vec::new();
        }
        self.turn_start(text)
    }

    fn translate(&mut self, line: &str) -> Vec<JournalOp> {
        if line.trim().is_empty() {
            return Vec::new();
        }
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return vec![notice(
                NoticeLevel::Warning,
                "A line from the agent could not be read.",
            )];
        };
        let method = message["method"].as_str();
        let has_id = !message["id"].is_null();
        match (method, has_id) {
            (Some(method), true) => self.server_request(method, &message),
            (Some(method), false) => self.notification(method, &message["params"]),
            (None, true) => self.response(&message),
            (None, false) => vec![notice(
                NoticeLevel::Warning,
                "A line from the agent could not be read.",
            )],
        }
    }

    fn encode_approval(&mut self, request_id: &str, answer: &ApprovalAnswer) -> Option<Vec<u8>> {
        self.answer_line(request_id, answer)
    }

    fn forget_request(&mut self, request_id: &str) {
        self.requests.remove(request_id);
    }

    fn encode_interrupt(&mut self) -> Option<Vec<u8>> {
        Some(self.interrupt_bytes())
    }
}

impl CodexAdapter {
    /// A notification: an item, a delta, the turn's lifecycle, an error or usage.
    fn notification(&mut self, method: &str, params: &Value) -> Vec<JournalOp> {
        if method.starts_with("codex/event/") || SKIPPED_METHODS.contains(&method) {
            return Vec::new();
        }
        if method == "turn/completed" {
            return self.turn_completed(params);
        }
        // A request we hold is settled by its notice, whatever thread or turn the notice names.
        if method == "serverRequest/resolved" {
            return self.request_resolved(params);
        }
        if self.foreign(params) {
            return Vec::new();
        }
        match method {
            "turn/started" => self.turn_started(params),
            "item/started" | "item/completed" => {
                self.item(&params["item"], method == "item/completed")
            }
            "item/agentMessage/delta" => self.text_delta(params, false),
            "item/reasoning/summaryTextDelta" => self.text_delta(params, true),
            "item/commandExecution/outputDelta" => self.output_delta(params),
            "turn/plan/updated" => self.plan_updated(params),
            "thread/tokenUsage/updated" => self.usage(params),
            "error" => self.error(params),
            "configWarning" | "deprecationNotice" => self.warning(params),
            other => self.unknown("message", other),
        }
    }
}

#[cfg(test)]
mod tests;
