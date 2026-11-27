//! The rows a chat session's transcript draws, mapped from the items the desktop holds (decision 0101, D3-D6).

use std::rc::Rc;

use gpui::SharedString;
use massorca_protocol::methods::chat as wire;
use massorca_ui::components::diff_view::{DiffModel, FileStatus};
use massorca_ui::components::plan_view::{Plan, PlanStep, StepStatus};
use massorca_ui::components::transcript::{
    ApprovalRow, ApprovalState, NoticeLevel, StepRow, StepState, TranscriptRow, TurnState,
};

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
        wire::ChatItemBody::UserMessage { text } => TranscriptRow::UserMessage {
            id,
            text: text.clone().into(),
        },
        wire::ChatItemBody::AssistantMessage { text, .. } => TranscriptRow::Prose {
            id,
            text: text.clone().into(),
        },
        wire::ChatItemBody::Thinking { text, state } => TranscriptRow::Thinking {
            id,
            text: text.clone().into(),
            streaming: *state == wire::MessageState::Running,
        },
        wire::ChatItemBody::ToolUse(tool) => TranscriptRow::Step {
            id,
            step: StepRow {
                title: tool.name.clone().into(),
                detail: tool.input_summary.clone().into(),
                state: tool_state(tool.status),
                output: tool.output_summary.clone().map(Into::into),
                truncated: tool.truncated,
            },
        },
        wire::ChatItemBody::Subagent {
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
        wire::ChatItemBody::Plan { entries, .. } => TranscriptRow::Plan {
            id,
            plan: Rc::new(Plan {
                steps: entries
                    .iter()
                    .map(|entry| PlanStep::new(entry.text.clone(), plan_status(entry.status)))
                    .collect(),
                document: None,
            }),
        },
        wire::ChatItemBody::Diff {
            path,
            patch,
            truncated,
            ..
        } => TranscriptRow::Diff {
            id,
            diff: Rc::new(DiffModel::new(parse_patch(
                path,
                FileStatus::Modified,
                patch,
            ))),
            truncated: *truncated,
        },
        wire::ChatItemBody::Approval(approval) => TranscriptRow::Approval {
            id,
            approval: ApprovalRow {
                request_id: approval.request_id.clone().into(),
                title: approval_title(&approval.tool).into(),
                input: approval.input_summary.clone().into(),
                truncated: approval.truncated,
                state: approval_state(approval.state),
                can_always: approval.can_always,
                suggestions: suggestion_lines(&approval.suggestions),
            },
        },
        wire::ChatItemBody::Notice { level, text } => TranscriptRow::Notice {
            id,
            level: notice_level(*level),
            text: text.clone().into(),
        },
        wire::ChatItemBody::Turn(turn) => TranscriptRow::Turn {
            id,
            state: turn_state(turn.state)?,
        },
        wire::ChatItemBody::Unknown => return None,
    })
}

/// A status this build does not know is drawn as interrupted, so it never reads as done or still running.
fn tool_state(status: wire::ToolStatus) -> StepState {
    match status {
        wire::ToolStatus::Started | wire::ToolStatus::Updated => StepState::Running,
        wire::ToolStatus::Completed => StepState::Done,
        wire::ToolStatus::Failed => StepState::Failed,
        wire::ToolStatus::Interrupted | wire::ToolStatus::Unknown => StepState::Interrupted,
    }
}

/// A status this build does not know is drawn as interrupted, as a tool call's is.
fn subagent_state(status: wire::SubagentStatus) -> StepState {
    match status {
        wire::SubagentStatus::Running => StepState::Running,
        wire::SubagentStatus::Completed => StepState::Done,
        wire::SubagentStatus::Failed => StepState::Failed,
        wire::SubagentStatus::Interrupted | wire::SubagentStatus::Unknown => StepState::Interrupted,
    }
}

/// A status this build does not know is pending, so it is never shown as done.
fn plan_status(status: wire::PlanStatus) -> StepStatus {
    match status {
        wire::PlanStatus::Pending | wire::PlanStatus::Unknown => StepStatus::Pending,
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

/// A level this build does not know is drawn as a warning, never as plain information.
fn notice_level(level: wire::NoticeLevel) -> NoticeLevel {
    match level {
        wire::NoticeLevel::Info => NoticeLevel::Info,
        wire::NoticeLevel::Warning | wire::NoticeLevel::Unknown => NoticeLevel::Warning,
        wire::NoticeLevel::Error => NoticeLevel::Error,
    }
}

/// A completed turn has no footer; the others say how they stand, and an unknown state is unverifiable.
fn turn_state(state: wire::TurnState) -> Option<TurnState> {
    match state {
        wire::TurnState::Running => Some(TurnState::Running),
        wire::TurnState::Completed => None,
        wire::TurnState::Interrupted => Some(TurnState::Interrupted),
        wire::TurnState::Unverifiable | wire::TurnState::Unknown => Some(TurnState::Unverifiable),
    }
}

/// What an Always answer writes, one line per rule and where it is written.
fn suggestion_lines(suggestions: &[wire::ApprovalSuggestion]) -> Vec<SharedString> {
    suggestions
        .iter()
        .flat_map(|suggestion| {
            let place = match suggestion.destination.as_str() {
                "session" => "for this session".to_owned(),
                "localSettings" => "in this project's local settings".to_owned(),
                other => format!("in {other}"),
            };
            suggestion
                .rules
                .iter()
                .map(move |rule| SharedString::from(format!("{rule} {place}")))
        })
        .collect()
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
    use massorca_protocol::methods::chat::{ChatCursor, ChatEvent, ChatItem, ChatSnapshot};

    fn item(id: &str, position: u64, body: wire::ChatItemBody) -> ChatItem {
        ChatItem {
            id: id.to_owned(),
            position,
            seq: position,
            body,
            ..crate::service::fixtures::base_chat_item()
        }
    }

    fn transcript(items: Vec<ChatItem>, latest_turn: Option<ChatItem>) -> ChatTranscript {
        let mut transcript = ChatTranscript::default();
        transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: ChatCursor {
                seq: 1,
                ..crate::service::fixtures::base_chat_cursor()
            },
            items,
            latest_turn,
            ..crate::service::fixtures::base_chat_snapshot()
        })));
        transcript
    }

    fn turn(id: &str, position: u64, state: wire::TurnState) -> ChatItem {
        item(
            id,
            position,
            wire::ChatItemBody::Turn(wire::Turn {
                state,
                user_item: None,
                completed_at_ms: None,
                outcome: None,
                usage: None,
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
            suggestions: Vec::new(),
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
                    wire::ChatItemBody::UserMessage {
                        text: "Fix it.".to_owned(),
                    },
                ),
                item(
                    "answer",
                    2,
                    wire::ChatItemBody::AssistantMessage {
                        text: "Done.".to_owned(),
                        state: wire::MessageState::Completed,
                    },
                ),
                item("gone", 3, wire::ChatItemBody::Unknown),
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
                wire::ChatItemBody::UserMessage {
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
                wire::ChatItemBody::Approval(approval("Bash", wire::ApprovalState::Unknown)),
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
                wire::ChatItemBody::Approval(approval("Bash", wire::ApprovalState::Pending)),
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
                wire::ChatItemBody::Plan {
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
                    document: None,
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
                wire::ChatItemBody::Subagent {
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

    #[test]
    fn a_status_this_build_does_not_know_is_drawn_as_interrupted_never_as_done() {
        let transcript = transcript(
            vec![
                item(
                    "tool",
                    1,
                    wire::ChatItemBody::ToolUse(wire::ToolUse {
                        status: wire::ToolStatus::Unknown,
                        ..crate::service::fixtures::base_tool_use()
                    }),
                ),
                item(
                    "sub",
                    2,
                    wire::ChatItemBody::Subagent {
                        tool_use_id: "tu-2".to_owned(),
                        description: "Check the parser".to_owned(),
                        agent_type: None,
                        status: wire::SubagentStatus::Unknown,
                    },
                ),
            ],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            &rows[..],
            [
                TranscriptRow::Step { step: tool, .. },
                TranscriptRow::Step { step: subagent, .. },
            ] if tool.state == StepState::Interrupted && subagent.state == StepState::Interrupted
        ));
    }

    #[test]
    fn an_unknown_plan_status_is_drawn_as_pending() {
        let transcript = transcript(
            vec![item(
                "plan",
                1,
                wire::ChatItemBody::Plan {
                    entries: vec![wire::PlanEntry {
                        text: "Read".to_owned(),
                        status: wire::PlanStatus::Unknown,
                    }],
                    document: None,
                },
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        let TranscriptRow::Plan { plan, .. } = &rows[0] else {
            panic!("expected a plan row, found {rows:?}");
        };
        assert_eq!(plan.steps[0].status, StepStatus::Pending);
    }

    #[test]
    fn an_unknown_notice_level_is_drawn_as_a_warning() {
        let transcript = transcript(
            vec![item(
                "note",
                1,
                wire::ChatItemBody::Notice {
                    level: wire::NoticeLevel::Unknown,
                    text: "The CLI retried.".to_owned(),
                },
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            &rows[0],
            TranscriptRow::Notice { level, .. } if *level == NoticeLevel::Warning
        ));
    }

    #[test]
    fn an_unknown_turn_state_keeps_a_footer_that_says_it_is_unverifiable() {
        let transcript = transcript(vec![turn("t3", 1, wire::TurnState::Unknown)], None);

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            rows.as_slice(),
            [TranscriptRow::Turn {
                state: TurnState::Unverifiable,
                ..
            }]
        ));
    }

    #[test]
    fn an_unknown_message_state_is_read_as_completed_so_its_thinking_is_not_streaming() {
        let transcript = transcript(
            vec![item(
                "thought",
                1,
                wire::ChatItemBody::Thinking {
                    text: "Weighing the options".to_owned(),
                    state: wire::MessageState::Unknown,
                },
            )],
            None,
        );

        let rows = transcript_rows(&transcript);

        assert!(matches!(
            &rows[0],
            TranscriptRow::Thinking {
                streaming: false,
                ..
            }
        ));
    }

    #[test]
    fn an_approval_lists_each_rule_an_always_answer_writes_with_where() {
        let mut offered = approval("Bash", wire::ApprovalState::Pending);
        offered.suggestions = vec![
            wire::ApprovalSuggestion {
                rules: vec!["Bash(touch probe.txt)".to_owned()],
                destination: "session".to_owned(),
                ..crate::service::fixtures::base_approval_suggestion()
            },
            wire::ApprovalSuggestion {
                rules: vec!["Read".to_owned(), "Edit".to_owned()],
                ..crate::service::fixtures::base_approval_suggestion()
            },
        ];
        let transcript = transcript(
            vec![item("ask", 1, wire::ChatItemBody::Approval(offered))],
            None,
        );

        let rows = transcript_rows(&transcript);

        let TranscriptRow::Approval { approval, .. } = &rows[0] else {
            panic!("an approval row");
        };
        let lines: Vec<&str> = approval
            .suggestions
            .iter()
            .map(|line| line.as_ref())
            .collect();
        assert_eq!(
            lines,
            [
                "Bash(touch probe.txt) for this session",
                "Read in this project's local settings",
                "Edit in this project's local settings",
            ]
        );
    }
}
