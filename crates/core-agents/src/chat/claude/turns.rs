//! The end of a turn: the `result` frame, and the interrupt.
//!
//! - `result` ends the turn: `completed`, its outcome from `is_error` and the subtype. The result that answers an
//!   interrupt is skipped: the service already ended that turn `interrupted`.
//! - The interrupt is a control request that also cancels the messages the CLI holds (`cancel_queued`), so the turns
//!   the service opened for them end with it. Every pending request is forgotten: nothing answers it afterwards.
//!
//! Either way the adapter forgets the turn's partial messages and unfinished calls.

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{NoticeLevel, TurnState};
use serde_json::{Value, json};

use super::{ClaudeAdapter, bounded, line_of, notice, text_of};
use crate::chat::adapter::JournalOp;

impl ClaudeAdapter {
    /// `result`: the turn's end, unless it answers an interrupt.
    pub(super) fn result(&mut self, frame: &Value) -> Vec<JournalOp> {
        self.end_of_turn();
        if self.interrupts_pending > 0 {
            self.interrupts_pending -= 1;
            return Vec::new();
        }
        let subtype = frame["subtype"].as_str().unwrap_or_default();
        let failed = frame["is_error"].as_bool().unwrap_or(false) || subtype != "success";
        let mut ops = Vec::new();
        if failed {
            let text = match subtype {
                "error_max_turns" => "The turn stopped at its maximum number of turns.".to_owned(),
                "error_max_budget_usd" => "The turn stopped at its spending limit.".to_owned(),
                "error_during_execution" => "The turn ended with an error.".to_owned(),
                _ => match text_of(&frame["result"]) {
                    Some(text) => bounded(text).0,
                    None => "The turn ended with an error.".to_owned(),
                },
            };
            ops.push(notice(NoticeLevel::Error, text));
        }
        ops.push(JournalOp::EndTurn {
            state: TurnState::Completed,
            outcome: Some(if failed {
                Outcome::Failure
            } else {
                Outcome::Success
            }),
        });
        ops
    }

    /// The interrupt control request; the `result` it brings is skipped.
    pub(super) fn interrupt_line(&mut self) -> Vec<u8> {
        self.requests.clear();
        self.end_of_turn();
        self.interrupts_pending += 1;
        self.next_request += 1;
        line_of(&json!({
            "type": "control_request",
            "request_id": format!("massorca-interrupt-{}", self.next_request),
            "request": { "subtype": "interrupt", "cancel_queued": true }
        }))
    }
}
