//! Requests the app-server sends, which each take exactly one answer.
//!
//! - `item/commandExecution/requestApproval` and `item/fileChange/requestApproval` become pending `approval` items keyed
//!   by the request's JSON-RPC id. An allow answers `accept`, an `always` allow `acceptForSession` (the one change an
//!   always writes, listed on the item), a deny `decline`. Codex takes no message with a deny.
//! - `serverRequest/resolved` for a request still pending (the app-server withdrew it) cancels its item; nothing answers it.
//! - An approval request of another thread (a sub-agent's) is declined, and one of an interrupted turn is cancelled, at
//!   once: it never becomes an item.
//! - An approval request without an item id cannot be shown as asked: it is declined with a notice.
//! - Every other request is refused or answered so that nothing is granted, with a notice that names its method: a
//!   question (`item/tool/requestUserInput`) is out of this slice.

use massorca_protocol::methods::chat::{
    Approval, ApprovalDecision, ApprovalScope, ApprovalState, ApprovalSuggestion, ChatItemBody,
    NoticeLevel,
};
use serde_json::{Value, json};

use super::items::{changed_paths, command_text};
use super::{CodexAdapter, Request, id_text, line_of, notice, text_of};
use crate::chat::adapter::{ApprovalAnswer, JournalOp, approval_key};
use crate::chat::claude::{bounded, bounded_name};

const COMMAND_APPROVAL: &str = "item/commandExecution/requestApproval";
const FILE_CHANGE_APPROVAL: &str = "item/fileChange/requestApproval";

/// The answer line of request `id`.
fn result_line(id: &Value, result: Value) -> Vec<u8> {
    line_of(&json!({ "id": id, "result": result }))
}

/// The error answer line of request `id`.
fn error_line(id: &Value, code: i64, message: &str) -> Vec<u8> {
    line_of(&json!({ "id": id, "error": { "code": code, "message": message } }))
}

impl CodexAdapter {
    /// A request from the app-server: an approval, or one MassOrca answers itself.
    pub(super) fn server_request(&mut self, method: &str, message: &Value) -> Vec<JournalOp> {
        let id = &message["id"];
        let params = &message["params"];
        if method == COMMAND_APPROVAL || method == FILE_CHANGE_APPROVAL {
            return self.approval(method, id, params);
        }
        let name = bounded_name(method);
        let answer = match method {
            "mcpServer/elicitation/request" => {
                result_line(id, json!({ "action": "decline", "content": null }))
            }
            "item/permissions/requestApproval" => {
                result_line(id, json!({ "permissions": {}, "scope": "turn" }))
            }
            "item/tool/call" => result_line(id, json!({ "contentItems": [], "success": false })),
            "applyPatchApproval" | "execCommandApproval" => {
                result_line(id, json!({ "decision": "abort" }))
            }
            _ => error_line(
                id,
                -32000,
                &format!("MassOrca does not serve {name} requests"),
            ),
        };
        vec![
            notice(
                NoticeLevel::Warning,
                format!(
                    "The agent asked for something MassOrca does not serve ({name}); it was refused."
                ),
            ),
            JournalOp::Send { bytes: answer },
        ]
    }

    /// A command or file change approval request: a pending item, or an answer at once when it is nobody's to show (another
    /// thread's, or one of an interrupted turn), or a decline when it cannot be read.
    fn approval(&mut self, method: &str, id: &Value, params: &Value) -> Vec<JournalOp> {
        if self.foreign(params) {
            let decision = if self.interrupted_turn(params) {
                "cancel"
            } else {
                "decline"
            };
            return vec![JournalOp::Send {
                bytes: result_line(id, json!({ "decision": decision })),
            }];
        }
        let (Some(request_id), Some(item_id)) = (id_text(id), text_of(&params["itemId"])) else {
            return vec![
                notice(
                    NoticeLevel::Warning,
                    "A permission request from the agent could not be read; it was denied.",
                ),
                JournalOp::Send {
                    bytes: result_line(id, json!({ "decision": "decline" })),
                },
            ];
        };
        let reason = text_of(&params["reason"]);
        let (tool, what) = if method == COMMAND_APPROVAL {
            ("shell", command_text(&params["command"]))
        } else {
            let paths = self
                .tools
                .get(item_id)
                .map(|tool| tool.tool.input_summary.clone())
                .filter(|paths| !paths.is_empty())
                .or_else(|| {
                    let paths = changed_paths(params);
                    (!paths.is_empty()).then(|| paths.join("\n"))
                });
            ("apply_patch", paths)
        };
        let shown = match (what.as_deref(), reason) {
            (Some(what), _) => what.to_owned(),
            (None, Some(reason)) => reason.to_owned(),
            (None, None) => tool.to_owned(),
        };
        let (input_summary, truncated) = bounded(&shown);
        let approval = Approval {
            request_id: request_id.clone(),
            tool: tool.to_owned(),
            input_summary: input_summary.clone(),
            truncated,
            tool_use_id: Some(item_id.to_owned()),
            can_always: true,
            suggestions: vec![ApprovalSuggestion {
                kind: "acceptForSession".to_owned(),
                rules: vec![input_summary],
                destination: "session".to_owned(),
            }],
            state: ApprovalState::Pending,
            scope: None,
            settled_at_ms: None,
        };
        let body = ChatItemBody::Approval(approval.clone());
        self.requests.insert(
            request_id.clone(),
            Request {
                id: id.clone(),
                approval,
            },
        );
        vec![JournalOp::Add {
            key: Some(approval_key(&request_id)),
            parent: None,
            body,
        }]
    }

    /// `serverRequest/resolved`: a request still pending was withdrawn; it is cancelled and never answered.
    pub(super) fn request_resolved(&mut self, params: &Value) -> Vec<JournalOp> {
        let Some(request_id) = id_text(&params["requestId"]) else {
            return Vec::new();
        };
        let Some(mut request) = self.requests.remove(&request_id) else {
            return Vec::new();
        };
        request.approval.state = ApprovalState::Cancelled;
        vec![JournalOp::Replace {
            key: approval_key(&request_id),
            body: ChatItemBody::Approval(request.approval),
        }]
    }

    /// The answer to a pending request, without forgetting it.
    pub(super) fn answer_line(&self, request_id: &str, answer: &ApprovalAnswer) -> Option<Vec<u8>> {
        let request = self.requests.get(request_id)?;
        let decision = match (answer.decision, answer.scope) {
            (ApprovalDecision::Deny, _) => "decline",
            (ApprovalDecision::Allow, ApprovalScope::Always) => "acceptForSession",
            (ApprovalDecision::Allow, _) => "accept",
        };
        Some(result_line(&request.id, json!({ "decision": decision })))
    }

    /// `cancel` answers to every pending request, which are forgotten: an interrupt cancels them, never authorizes.
    pub(super) fn cancel_requests(&mut self) -> Vec<u8> {
        self.requests
            .drain()
            .flat_map(|(_, request)| result_line(&request.id, json!({ "decision": "cancel" })))
            .collect()
    }
}
