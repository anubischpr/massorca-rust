//! The answers to MassOrca's own requests, the turn's lifecycle, and the interrupt.
//!
//! - `initialize` answered: `initialized`, then the thread opens (`thread/start`, or `thread/resume` of the session's
//!   ref). The opened thread's id is the session's agent ref; prompts sent meanwhile go out once it is open. A refused
//!   handshake or thread is an error notice, and the turns waiting for it end failed.
//! - A refused `turn/start` ends its turn failed, with the app-server's reason.
//! - `turn/completed`: `completed` ends the turn with outcome success, `failed` with failure (and the reason, unless an
//!   `error` notification already showed it), `interrupted` ends it interrupted. A turn MassOrca interrupted was already
//!   ended by the service, so its end is skipped.
//! - `error`: a warning while the app-server retries, else an error notice.
//! - `thread/tokenUsage/updated`: the turn's usage, from the thread's running totals.
//! - The interrupt: `cancel` answers to pending requests, then `turn/interrupt` of the running turn, or of the turn the
//!   app-server names next when it has not named it yet. Prompts sent after it wait until that turn ends.

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{NoticeLevel, TokenUsage, TurnState};
use serde_json::{Value, json};

use super::{CodexAdapter, Sent, Thread, launch, notice, text_of};
use crate::chat::adapter::JournalOp;
use crate::chat::claude::bounded;

/// The turn's end with `outcome`.
fn end(state: TurnState, outcome: Option<Outcome>) -> JournalOp {
    JournalOp::EndTurn { state, outcome }
}

/// The message of a JSON-RPC error, bounded.
fn error_message(message: &Value) -> String {
    bounded(text_of(&message["error"]["message"]).unwrap_or("no reason given")).0
}

impl CodexAdapter {
    /// The answer to one of MassOrca's own requests.
    pub(super) fn response(&mut self, message: &Value) -> Vec<JournalOp> {
        let Some(sent) = message["id"].as_u64().and_then(|id| self.sent.remove(&id)) else {
            return Vec::new();
        };
        let refused = !message["error"].is_null();
        let result = &message["result"];
        match (sent, refused) {
            (Sent::Initialize, false) => {
                let mut bytes = launch::notification("initialized");
                let (method, params) = launch::open_thread(&self.plan);
                bytes.extend(self.send(Sent::OpenThread, method, params));
                vec![JournalOp::Send { bytes }]
            }
            (Sent::OpenThread, false) => self.thread_opened(result),
            (Sent::Initialize | Sent::OpenThread, true) => {
                let text = format!(
                    "Codex could not open the conversation: {}",
                    error_message(message)
                );
                self.thread_failed(text)
            }
            (Sent::TurnStart, false) => {
                if let Some(turn) = text_of(&result["turn"]["id"]) {
                    return self.turn_started_by_answer(turn);
                }
                Vec::new()
            }
            (Sent::TurnStart, true) => {
                self.interrupt_wanted = false;
                vec![
                    notice(
                        NoticeLevel::Error,
                        format!("Codex refused the message: {}", error_message(message)),
                    ),
                    end(TurnState::Completed, Some(Outcome::Failure)),
                ]
            }
            // The service ended the turn when it sent the interrupt; the answer changes nothing.
            (Sent::Interrupt, _) => Vec::new(),
        }
    }

    /// The thread is open: its id is the session's ref, and the waiting prompts go out.
    fn thread_opened(&mut self, result: &Value) -> Vec<JournalOp> {
        let opened = text_of(&result["thread"]["id"]).or_else(|| text_of(&result["threadId"]));
        let Some(thread) = opened.map(str::to_owned) else {
            return self.thread_failed("Codex did not name the conversation it opened.".to_owned());
        };
        if let Some(asked) = self.plan.resume.as_deref()
            && asked != thread
        {
            // A resume that lands on another thread would be a fork under a resume's name.
            return self.thread_failed(
                "Codex opened another conversation than the one asked to resume.".to_owned(),
            );
        }
        self.thread = Thread::Open(thread.clone());
        let mut ops = vec![JournalOp::LearnRef { agent_ref: thread }];
        ops.extend(self.release_prompts());
        ops
    }

    /// The thread could not be opened: an error notice, and the waiting prompts' turns end failed.
    fn thread_failed(&mut self, text: String) -> Vec<JournalOp> {
        self.thread = Thread::Failed;
        let mut ops = vec![notice(NoticeLevel::Error, text)];
        if !std::mem::take(&mut self.waiting).is_empty() {
            ops.push(end(TurnState::Completed, Some(Outcome::Failure)));
        }
        ops
    }

    /// `turn/started`: the running turn's id.
    pub(super) fn turn_started(&mut self, params: &Value) -> Vec<JournalOp> {
        match text_of(&params["turn"]["id"]).or_else(|| text_of(&params["turnId"])) {
            Some(turn) => self.turn_running(turn),
            None => Vec::new(),
        }
    }

    /// The turn a `turn/start` of ours opened, as its answer names it. It is the running turn only while none is known,
    /// and not once it ended.
    fn turn_started_by_answer(&mut self, turn: &str) -> Vec<JournalOp> {
        if self.turn.is_some() {
            return Vec::new();
        }
        self.turn_running(turn)
    }

    /// The app-server named `turn` as the running one; an interrupt asked for while it started goes out now.
    fn turn_running(&mut self, turn: &str) -> Vec<JournalOp> {
        if self.ended.contains(turn) {
            return Vec::new();
        }
        self.turn = Some(turn.to_owned());
        if !std::mem::take(&mut self.interrupt_wanted) {
            return Vec::new();
        }
        vec![JournalOp::Send {
            bytes: self.interrupt_turn(turn),
        }]
    }

    /// Whether a `turn/start` of ours awaits its answer: the turn it opens is the one an interrupt asked now is for.
    fn turn_start_in_flight(&self) -> bool {
        self.sent.values().any(|sent| *sent == Sent::TurnStart)
    }

    /// `turn/completed`: the turn's end, then the prompts that waited for it.
    pub(super) fn turn_completed(&mut self, params: &Value) -> Vec<JournalOp> {
        let turn = text_of(&params["turn"]["id"]).or_else(|| text_of(&params["turnId"]));
        let ours = match (&self.thread, text_of(&params["threadId"])) {
            (Thread::Open(thread), Some(named)) => thread == named,
            _ => true,
        };
        if !ours {
            return Vec::new();
        }
        if let Some(turn) = turn {
            self.ended.insert(turn.to_owned());
        }
        let interrupted = turn.is_some_and(|turn| self.interrupted.remove(turn));
        let shown = self.error_shown;
        self.end_of_turn();
        let mut ops = Vec::new();
        if !interrupted {
            match params["turn"]["status"].as_str() {
                Some("failed") => {
                    if !shown {
                        let reason = text_of(&params["turn"]["error"]["message"]).map_or_else(
                            || "The turn ended with an error.".to_owned(),
                            |text| bounded(text).0,
                        );
                        ops.push(notice(NoticeLevel::Error, reason));
                    }
                    ops.push(end(TurnState::Completed, Some(Outcome::Failure)));
                }
                Some("interrupted") => ops.push(end(TurnState::Interrupted, None)),
                Some("completed") => ops.push(end(TurnState::Completed, Some(Outcome::Success))),
                _ => ops.push(end(TurnState::Completed, None)),
            }
        }
        ops.extend(self.release_prompts());
        ops
    }

    /// `error`: a warning while the app-server retries, else an error notice.
    pub(super) fn error(&mut self, params: &Value) -> Vec<JournalOp> {
        let message = text_of(&params["error"]["message"]).unwrap_or("Codex reported an error.");
        let text = bounded(message).0;
        if params["willRetry"].as_bool() == Some(true) {
            return vec![notice(NoticeLevel::Warning, text)];
        }
        self.error_shown = true;
        vec![notice(NoticeLevel::Error, text)]
    }

    /// `configWarning` or `deprecationNotice`: a warning notice with its summary.
    pub(super) fn warning(&mut self, params: &Value) -> Vec<JournalOp> {
        match text_of(&params["summary"]).or_else(|| text_of(&params["message"])) {
            Some(text) => vec![notice(NoticeLevel::Warning, bounded(text).0)],
            None => Vec::new(),
        }
    }

    /// `thread/tokenUsage/updated`: the thread's running totals, on the open turn.
    pub(super) fn usage(&mut self, params: &Value) -> Vec<JournalOp> {
        let total = &params["tokenUsage"]["total"];
        if !total.is_object() {
            return Vec::new();
        }
        let count = |key: &str| total[key].as_u64().unwrap_or(0);
        let usage = TokenUsage {
            input_tokens: count("inputTokens"),
            cached_input_tokens: count("cachedInputTokens"),
            output_tokens: count("outputTokens"),
            reasoning_output_tokens: count("reasoningOutputTokens"),
            total_tokens: count("totalTokens"),
        };
        vec![JournalOp::Usage { usage }]
    }

    /// The `turn/interrupt` of `turn`, which is then skipped as it ends.
    fn interrupt_turn(&mut self, turn: &str) -> Vec<u8> {
        self.interrupted.insert(turn.to_owned());
        let thread = match &self.thread {
            Thread::Open(thread) => thread.clone(),
            _ => String::new(),
        };
        self.send(
            Sent::Interrupt,
            "turn/interrupt",
            json!({ "threadId": thread, "turnId": turn }),
        )
    }

    /// The interrupt: pending requests cancelled, the waiting prompts dropped (their turns end with the running one),
    /// and `turn/interrupt` of the running turn, now or once the app-server names it.
    pub(super) fn interrupt_bytes(&mut self) -> Vec<u8> {
        let mut bytes = self.cancel_requests();
        self.waiting.clear();
        self.seen.clear();
        self.tools.clear();
        match self.turn.clone() {
            Some(turn) if !self.interrupted.contains(&turn) => {
                bytes.extend(self.interrupt_turn(&turn));
            }
            Some(_) => {}
            None if matches!(self.thread, Thread::Open(_)) && self.turn_start_in_flight() => {
                self.interrupt_wanted = true;
            }
            None => {}
        }
        bytes
    }
}
