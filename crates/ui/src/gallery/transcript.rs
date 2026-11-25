//! The transcript story: one chat session with every kind of row, in the current theme.

use std::rc::Rc;

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::Story;
use crate::components::diff_view::{DiffModel, FileDiff, FileStatus};
use crate::components::plan_view::{Plan, PlanStep, StepStatus};
use crate::components::transcript::{
    ApprovalRow, ApprovalState, NoticeLevel, StepRow, StepState, TranscriptRow, TranscriptView,
    TurnState,
};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Transcript",
        spec: "ConversationMessageList.md",
        render: transcript,
    }]
}

fn transcript(_: &mut Window, _: &mut App) -> AnyElement {
    let plan = Rc::new(Plan {
        steps: vec![
            PlanStep::new("Read the spec", StepStatus::Done),
            PlanStep::new("Write the view", StepStatus::InProgress),
            PlanStep::new("Run the tests", StepStatus::Pending),
        ],
        document: None,
    });
    let diff = Rc::new(DiffModel::new(FileDiff {
        path: "src/parser.rs".into(),
        status: FileStatus::Modified,
        hunks: Vec::new(),
        hidden_after: Vec::new(),
    }));
    let rows = vec![
        TranscriptRow::UserMessage {
            id: "ask".into(),
            text: "Fix the cursor in the parser.".into(),
        },
        TranscriptRow::Thinking {
            id: "think".into(),
            text: "The cursor is stale after a split.".into(),
            streaming: false,
        },
        TranscriptRow::Step {
            id: "read".into(),
            step: StepRow {
                title: "Read".into(),
                detail: "src/parser.rs".into(),
                state: StepState::Done,
                output: None,
                truncated: false,
            },
        },
        TranscriptRow::Step {
            id: "test".into(),
            step: StepRow {
                title: "Bash".into(),
                detail: "cargo test -p parser".into(),
                state: StepState::Failed,
                output: Some("1 failed".into()),
                truncated: false,
            },
        },
        TranscriptRow::Prose {
            id: "answer".into(),
            text: "The cursor moved after the split. The fix is in the next step.".into(),
        },
        TranscriptRow::Plan {
            id: "plan".into(),
            plan,
        },
        TranscriptRow::Diff {
            id: "diff".into(),
            diff,
            truncated: true,
        },
        TranscriptRow::Approval {
            id: "approval".into(),
            approval: ApprovalRow {
                request_id: "req-1".into(),
                title: "Run command?".into(),
                input: "cargo test -p parser".into(),
                truncated: false,
                state: ApprovalState::Pending,
                can_always: true,
            },
        },
        TranscriptRow::Notice {
            id: "notice".into(),
            level: NoticeLevel::Warning,
            text: "The service retried the prompt once.".into(),
        },
        TranscriptRow::Turn {
            id: "turn".into(),
            state: TurnState::Running,
        },
    ];
    div()
        .h(px(640.0))
        .child(TranscriptView::new("gallery-transcript", rows))
        .into_any_element()
}
