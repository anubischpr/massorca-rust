//! Permission requests (decision 0101, D6, as amended). A `control_request` `can_use_tool` becomes a pending `approval`;
//! `control_cancel_request` cancels it and nothing answers it. A request the adapter cannot read, or whose input is not
//! an object, is denied with a notice; a control request of another kind is refused with an error answer.
//!
//! An `always` answer writes back only the offered changes the approval item lists: rule additions that allow, written to
//! `session` or `localSettings`. A mode switch, a rule that denies, and any other destination are dropped and never sent.

use massorca_protocol::methods::chat::{
    Approval, ApprovalDecision, ApprovalScope, ApprovalState, ApprovalSuggestion, ChatItemBody,
    NoticeLevel,
};
use serde_json::{Value, json};

use super::items::input_text;
use super::{ClaudeAdapter, Request, bounded, bounded_name, line_of, notice, text_of};
use crate::chat::adapter::{ApprovalAnswer, JournalOp, approval_key};

/// What a deny tells the agent when the person gave no message.
pub const DENIED_MESSAGE: &str = "The person denied this tool call.";
/// What the agent is told for a permission request the adapter could not read.
pub const UNREADABLE_REQUEST_MESSAGE: &str = "MassOrca could not decode this permission request.";

/// Where an `always` answer may write rules.
const ACCEPTED_DESTINATIONS: &[&str] = &["session", "localSettings"];
/// Most suggestions an item lists, and most rules of each.
const MAX_SUGGESTIONS: usize = 16;

/// One rule as text: `Tool` or `Tool(rule)`.
fn rule_text(rule: &Value) -> Option<String> {
    let tool = bounded_name(text_of(&rule["toolName"])?);
    Some(match text_of(&rule["ruleContent"]) {
        Some(content) => format!("{tool}({})", bounded(content).0),
        None => tool,
    })
}

/// The offered change, when an `always` answer may write it: an `addRules` that allows, to an accepted destination,
/// with rules that all read. Returns it as the item lists it.
fn accepted(suggestion: &Value) -> Option<ApprovalSuggestion> {
    let destination = suggestion["destination"].as_str()?;
    if suggestion["type"] != "addRules"
        || suggestion["behavior"] != "allow"
        || !ACCEPTED_DESTINATIONS.contains(&destination)
    {
        return None;
    }
    let rules = suggestion["rules"].as_array()?;
    let texts: Option<Vec<String>> = rules.iter().map(rule_text).collect();
    let texts = texts.filter(|texts| !texts.is_empty() && texts.len() <= MAX_SUGGESTIONS)?;
    Some(ApprovalSuggestion {
        kind: "addRules".to_owned(),
        rules: texts,
        destination: destination.to_owned(),
    })
}

fn deny_line(request_id: &str, message: &str, tool_use_id: Option<&str>) -> Vec<u8> {
    let mut response = json!({ "behavior": "deny", "message": message });
    if let Some(id) = tool_use_id {
        response["toolUseID"] = json!(id);
    }
    line_of(&json!({
        "type": "control_response",
        "response": { "subtype": "success", "request_id": request_id, "response": response }
    }))
}

impl ClaudeAdapter {
    /// `control_request`: a permission prompt, or a request this adapter does not serve.
    pub(super) fn control_request(&mut self, frame: &Value) -> Vec<JournalOp> {
        let Some(request_id) = text_of(&frame["request_id"]) else {
            return vec![notice(
                NoticeLevel::Warning,
                "A request from the agent could not be read and was not answered.",
            )];
        };
        let request = &frame["request"];
        if request["subtype"] != "can_use_tool" {
            let subtype = request["subtype"]
                .as_str()
                .map_or_else(|| "unknown".to_owned(), bounded_name);
            return vec![
                notice(
                    NoticeLevel::Warning,
                    format!(
                        "The agent asked for something MassOrca does not serve ({subtype}); it was refused."
                    ),
                ),
                JournalOp::Send {
                    bytes: line_of(&json!({
                        "type": "control_response",
                        "response": {
                            "subtype": "error",
                            "request_id": request_id,
                            "error": format!("MassOrca does not serve {subtype} requests"),
                        }
                    })),
                },
            ];
        }
        let tool_use_id = text_of(&request["tool_use_id"]).map(str::to_owned);
        let (Some(tool), Value::Object(_)) = (text_of(&request["tool_name"]), &request["input"])
        else {
            // Without a tool or an object input there is nothing the person could approve as asked.
            self.requests.remove(request_id);
            return vec![
                notice(
                    NoticeLevel::Warning,
                    "A permission request from the agent could not be read; it was denied.",
                ),
                JournalOp::Send {
                    bytes: deny_line(
                        request_id,
                        UNREADABLE_REQUEST_MESSAGE,
                        tool_use_id.as_deref(),
                    ),
                },
            ];
        };
        let input = request["input"].clone();
        let (raw, suggestions): (Vec<Value>, Vec<ApprovalSuggestion>) =
            request["permission_suggestions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|offered| accepted(offered).map(|shown| (offered.clone(), shown)))
                .take(MAX_SUGGESTIONS)
                .unzip();
        let (input_summary, truncated) = bounded(&input_text(tool, &input));
        let approval = Approval {
            request_id: request_id.to_owned(),
            tool: bounded_name(tool),
            input_summary,
            truncated,
            tool_use_id: tool_use_id.clone(),
            can_always: !suggestions.is_empty(),
            suggestions,
            state: ApprovalState::Pending,
            scope: None,
            settled_at_ms: None,
        };
        let parent = tool_use_id
            .as_deref()
            .and_then(|id| self.calls.get(id))
            .and_then(|call| call.parent.clone());
        let body = ChatItemBody::Approval(approval.clone());
        // A request sent again under the same id replaces the one before, input and all.
        self.requests.insert(
            request_id.to_owned(),
            Request {
                approval,
                input,
                accepted: raw,
            },
        );
        vec![JournalOp::Add {
            key: Some(approval_key(request_id)),
            parent,
            body,
        }]
    }

    /// `control_cancel_request`: the CLI withdrew a request; it is cancelled and never answered.
    pub(super) fn control_cancel(&mut self, frame: &Value) -> Vec<JournalOp> {
        let Some(request_id) = text_of(&frame["request_id"]) else {
            return Vec::new();
        };
        let Some(mut request) = self.requests.remove(request_id) else {
            return Vec::new();
        };
        request.approval.state = ApprovalState::Cancelled;
        vec![JournalOp::Replace {
            key: approval_key(request_id),
            body: ChatItemBody::Approval(request.approval),
        }]
    }

    /// The answer to a pending request, without forgetting it. An `always` writes back only the accepted changes.
    pub(super) fn answer_line(&self, request_id: &str, answer: &ApprovalAnswer) -> Option<Vec<u8>> {
        let request = self.requests.get(request_id)?;
        let tool_use_id = request.approval.tool_use_id.as_deref();
        if answer.decision == ApprovalDecision::Deny {
            let message = answer.message.as_deref().unwrap_or(DENIED_MESSAGE);
            return Some(deny_line(request_id, message, tool_use_id));
        }
        let mut response = json!({ "behavior": "allow", "updatedInput": request.input });
        if answer.scope == ApprovalScope::Always && !request.accepted.is_empty() {
            response["updatedPermissions"] = Value::Array(request.accepted.clone());
        }
        if let Some(id) = tool_use_id {
            response["toolUseID"] = json!(id);
        }
        Some(line_of(&json!({
            "type": "control_response",
            "response": { "subtype": "success", "request_id": request_id, "response": response }
        })))
    }
}
