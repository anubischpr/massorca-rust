//! The rows a chat session's transcript draws, mapped from the items the desktop holds (decision 0101, D3-D6).

use std::rc::Rc;

use gpui::SharedString;
use massorca_ui::components::diff_view::{DiffModel, FileStatus};
use massorca_ui::components::plan_view::{Plan, PlanStep, StepStatus};
use massorca_ui::components::transcript::{
    ApprovalRow, ApprovalState, NoticeLevel, StepRow, StepState, TranscriptRow, TurnState,
};

use crate::model::chat_items as wire;
use crate::model::chat_transcript::ChatTranscript;
use crate::model::source_control_diff::parse_patch;

/// The rows of a transcript in timeline order. The session's newest turn is added last when the page does not hold it
/// and it still runs, so a long turn keeps its footer.
pub(super) fn transcript_rows(transcript: &ChatTranscript) -> Vec<TranscriptRow> {
    let mut rows: Vec<TranscriptRow> = transcript.items().iter().filter_map(item_row).collect();
    if let Some(turn) = transcript.latest_turn()
        && !transcript.items().iter().any(|item| item.id == turn.id)
    {
        rows.extend(item_row(turn));
    }
    rows
}

fn item_row(item: &wire::ChatItem) -> Option<TranscriptRow> {
    let id = SharedString::from(item.id.clone());
    Some(match &item.body {
        wire::ChatBody::UserMessage { text } => TranscriptRow::UserMessage {
            id,
            text: text.clone().into(),
        },
        wire::ChatBody::AssistantMessage { text, .. } => TranscriptRow::Prose {
            id,
            text: text.clone().into(),
        },
        wire::ChatBody::Thinking { text, state } => TranscriptRow::Thinking {
            id,
            text: text.clone().into(),
            streaming: *state == wire::MessageState::Running,
        },
        wire::ChatBody::ToolUse(tool) => TranscriptRow::Step {
            id,
            step: StepRow {
                title: tool.name.clone().into(),
                detail: tool.input_summary.clone().into(),
                state: tool_state(tool.status),
                output: tool.output_summary.clone().map(Into::into),
                truncated: tool.truncated,
            },
        },
        wire::ChatBody::Subagent {
            description,
            agent_type,
            status,
            ..
        } => TranscriptRow::Step {
            id,
            step: StepRow {
                title: agent_type
                    .clone()
                    .unwrap_or_else(|| "Subagent".to_owned())
                    .into(),
                detail: description.clone().into(),
                state: subagent_state(*status),
                output: None,
                truncated: false,
            },
        },
        wire::ChatBody::Plan { entries } => TranscriptRow::Plan {
            id,
            plan: Rc::new(Plan {
                steps: entries
                    .iter()
                    .map(|entry| PlanStep::new(entry.text.clone(), plan_status(entry.status)))
                    .collect(),
                document: None,
            }),
        },
        wire::ChatBody::Diff {
            path,
            patch,
            truncated,
        } => TranscriptRow::Diff {
            id,
            diff: Rc::new(DiffModel::new(parse_patch(
                path,
                FileStatus::Modified,
                patch,
            ))),
            truncated: *truncated,
        },
        wire::ChatBody::Approval(approval) => TranscriptRow::Approval {
            id,
            approval: ApprovalRow {
                request_id: approval.request_id.clone().into(),
                title: approval_title(&approval.tool).into(),
                input: approval.input_summary.clone().into(),
                truncated: approval.truncated,
                state: approval_state(approval.state),
                can_always: approval.can_always,
            },
        },
        wire::ChatBody::Notice { level, text } => TranscriptRow::Notice {
            id,
            level: notice_level(*level),
            text: text.clone().into(),
        },
        wire::ChatBody::Turn(turn) => TranscriptRow::Turn {
            id,
            state: turn_state(turn.state)?,
        },
        wire::ChatBody::Unknown => return None,
    })
}

fn tool_state(status: wire::ToolStatus) -> StepState {
    match status {
        wire::ToolStatus::Started | wire::ToolStatus::Updated => StepState::Running,
        wire::ToolStatus::Completed => StepState::Done,
        wire::ToolStatus::Failed => StepState::Failed,
        wire::ToolStatus::Interrupted => StepState::Interrupted,
    }
}

fn subagent_state(status: wire::SubagentStatus) -> StepState {
    match status {
        wire::SubagentStatus::Running => StepState::Running,
        wire::SubagentStatus::Completed => StepState::Done,
        wire::SubagentStatus::Failed => StepState::Failed,
        wire::SubagentStatus::Interrupted => StepState::Interrupted,
    }
}

fn plan_status(status: wire::PlanStatus) -> StepStatus {
    match status {
        wire::PlanStatus::Pending => StepStatus::Pending,
        wire::PlanStatus::InProgress => StepStatus::InProgress,
        wire::PlanStatus::Completed => StepStatus::Done,
    }
}

/// A value this build does not know is drawn as cancelled: it never reads as pending, and never as allowed.
fn approval_state(state: wire::ApprovalState) -> ApprovalState {
    match state {
        wire::ApprovalState::Pending => ApprovalState::Pending,
        wire::ApprovalState::Allowed => ApprovalState::Allowed,
        wire::ApprovalState::Denied => ApprovalState::Denied,
        wire::ApprovalState::Cancelled | wire::ApprovalState::Unknown => ApprovalState::Cancelled,
    }
}

fn notice_level(level: wire::NoticeLevel) -> NoticeLevel {
    match level {
        wire::NoticeLevel::Info => NoticeLevel::Info,
        wire::NoticeLevel::Warning => NoticeLevel::Warning,
        wire::NoticeLevel::Error => NoticeLevel::Error,
    }
}

/// A completed turn has no footer; the others say how they stand.
fn turn_state(state: wire::TurnState) -> Option<TurnState> {
    match state {
        wire::TurnState::Running => Some(TurnState::Running),
        wire::TurnState::Completed => None,
        wire::TurnState::Interrupted => Some(TurnState::Interrupted),
        wire::TurnState::Unverifiable => Some(TurnState::Unverifiable),
    }
}

/// "Run command?" for a shell command, otherwise the tool's own name.
fn approval_title(tool: &str) -> String {
    if tool == "Bash" {
        "Run command?".to_owned()
    } else {
        format!("Allow {tool}?")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chat_items::{ChatCursor, ChatEvent, ChatItem, ChatSnapshot};

    fn item(id: &str, position: u64, body: wire::ChatBody) -> ChatItem {
        ChatItem {
            id: id.to_owned(),
            position,
            seq: position,
            turn: None,
            parent: None,
            at_ms: 0,
            body,
        }
    }

    fn transcript(items: Vec<ChatItem>, latest_turn: Option<ChatItem>) -> ChatTranscript {
        let mut transcript = ChatTranscript::default();
        transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: ChatCursor {
                epoch: "epoch-1".to_owned(),
                seq: 1,
            },
            items,
            has_older: false,
            latest_turn,
            activity: None,
        })));
        transcript
    }

    fn turn(id: &str, position: u64, state: wire::TurnState) -> ChatItem {
        item(
            id,
            position,
            wire::ChatBody::Turn(wire::Turn {
                state,
                user_item: None,
                completed_at_ms: None,
            }),
        )
    }

    fn approval(tool: &str, state: wire::ApprovalState) -> wire::Approval {
        wire::Approval {
            request_id: "req-1".to_owned(),
            tool: tool.to_owned(),
            input_summary: "cargo test".to_owned(),
            truncated: false,
            tool_use_id: None,
            can_always: true,
            state,
            scope: None,
            settled_at_ms: None,
        }
    }

    fn ids(rows: &[TranscriptRow]) -> Vec<&str> {
        rows.iter().map(|row| row.id().as_str()).collect()
    }

    #[test]
    fn items_map_to_rows_in_timeline_order() {
        let transcript = transcript(
            vec![
                item(
                    "ask",
                    1,
                    wire::ChatBody::UserMessage {
                        text: "Fix it.".to_owned(),
                    },
                ),
                item(
                    "answer",
                    2,
                    wire::ChatBody::AssistantMessage {
                        text: "Done.".to_owned(),
                        state: wire::MessageState::Completed,
                    },
                ),
                item("gone", 3, wire::ChatBody::Unknown),
            ],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert_eq!(ids(&rows), ["ask", "answer"]);
        assert!(matches!(rows[0], TranscriptRow::UserMessage { .. }));
        assert!(matches!(rows[1], TranscriptRow::Prose { .. }));
    }

    #[test]
    fn a_completed_turn_has_no_footer_and_a_running_one_has_its_own_row() {
        let completed = transcript(vec![turn("t1", 1, wire::TurnState::Completed)], None);
        assert!(transcript_rows(&completed).is_empty());

        let running = transcript(vec![turn("t2", 1, wire::TurnState::Running)], None);
        let rows = transcript_rows(&running);
        assert!(matches!(
            rows.as_slice(),
            [TranscriptRow::Turn {
                state: TurnState::Running,
                ..
            }]
        ));
    }

    #[test]
    fn a_running_turn_outside_the_page_gets_its_footer_after_the_rows() {
        let transcript = transcript(
            vec![item(
                "ask",
                9,
                wire::ChatBody::UserMessage {
                    text: "later".to_owned(),
                },
            )],
            Some(turn("t1", 2, wire::TurnState::Running)),
        );

        let rows = transcript_rows(&transcript);

        assert_eq!(ids(&rows), ["ask", "t1"]);
    }

    #[test]
    fn a_shell_command_is_asked_as_a_command_and_another_tool_by_name() {
        assert_eq!(approval_title("Bash"), "Run command?");
        assert_eq!(approval_title("WebFetch"), "Allow WebFetch?");
    }

    #[test]
    fn an_unknown_approval_state_is_drawn_as_cancelled_and_never_authorizes() {
        let transcript = transcript(
            vec![item(
                "req",
                1,
                wire::ChatBody::Approval(approval("Bash", wire::ApprovalState::Unknown)),
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            &rows[0],
            TranscriptRow::Approval { approval, .. } if approval.state == ApprovalState::Cancelled
        ));
    }

    #[test]
    fn a_pending_approval_keeps_its_request_and_whether_it_can_be_always_allowed() {
        let transcript = transcript(
            vec![item(
                "req",
                1,
                wire::ChatBody::Approval(approval("Bash", wire::ApprovalState::Pending)),
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            &rows[0],
            TranscriptRow::Approval { approval, .. }
                if approval.request_id == "req-1"
                    && approval.title == "Run command?"
                    && approval.state == ApprovalState::Pending
                    && approval.can_always
        ));
    }

    #[test]
    fn a_plan_keeps_its_steps_in_order_with_their_states() {
        let transcript = transcript(
            vec![item(
                "plan",
                1,
                wire::ChatBody::Plan {
                    entries: vec![
                        wire::PlanEntry {
                            text: "Read".to_owned(),
                            status: wire::PlanStatus::Completed,
                        },
                        wire::PlanEntry {
                            text: "Write".to_owned(),
                            status: wire::PlanStatus::InProgress,
                        },
                    ],
                },
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        let TranscriptRow::Plan { plan, .. } = &rows[0] else {
            panic!("expected a plan row, found {rows:?}");
        };
        let states: Vec<StepStatus> = plan.steps.iter().map(|step| step.status).collect();
        assert_eq!(states, [StepStatus::Done, StepStatus::InProgress]);
    }

    #[test]
    fn a_subagent_is_named_by_its_agent_type_and_carries_its_task() {
        let transcript = transcript(
            vec![item(
                "sub",
                1,
                wire::ChatBody::Subagent {
                    tool_use_id: "tu-1".to_owned(),
                    description: "Check the parser".to_owned(),
                    agent_type: Some("Explore".to_owned()),
                    status: wire::SubagentStatus::Running,
                },
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            &rows[0],
            TranscriptRow::Step { step, .. }
                if step.title == "Explore"
                    && step.detail == "Check the parser"
                    && step.state == StepState::Running
        ));
    }
}
