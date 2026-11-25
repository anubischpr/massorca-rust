//! A chat session's transcript: the rows of its items drawn in timeline order, with the cards of X-CH-02 (approval),
//! X-CH-05 (plan) and D-AG-03 (conversation). The owner maps the session's items to [`TranscriptRow`]s.

use std::rc::Rc;

use gpui::{App, SharedString, Window};

use crate::components::diff_view::DiffModel;
use crate::components::plan_view::Plan;

pub mod approval;
pub mod cards;

/// One row of a transcript, as the view draws it.
#[derive(Clone, Debug)]
pub enum TranscriptRow {
    UserMessage {
        id: SharedString,
        text: SharedString,
    },
    Prose {
        id: SharedString,
        text: SharedString,
    },
    /// The agent's reasoning, collapsed until the person opens it.
    Thinking {
        id: SharedString,
        text: SharedString,
        streaming: bool,
    },
    /// A tool call or a subagent.
    Step {
        id: SharedString,
        step: StepRow,
    },
    Plan {
        id: SharedString,
        plan: Rc<Plan>,
    },
    /// A file change; `truncated` when the patch was cut before it reached the view.
    Diff {
        id: SharedString,
        diff: Rc<DiffModel>,
        truncated: bool,
    },
    Approval {
        id: SharedString,
        approval: ApprovalRow,
    },
    Notice {
        id: SharedString,
        level: NoticeLevel,
        text: SharedString,
    },
    /// The footer of a turn that has not completed cleanly. A completed turn has no row.
    Turn {
        id: SharedString,
        state: TurnState,
    },
}

impl TranscriptRow {
    pub fn id(&self) -> &SharedString {
        match self {
            Self::UserMessage { id, .. }
            | Self::Prose { id, .. }
            | Self::Thinking { id, .. }
            | Self::Step { id, .. }
            | Self::Plan { id, .. }
            | Self::Diff { id, .. }
            | Self::Approval { id, .. }
            | Self::Notice { id, .. }
            | Self::Turn { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRow {
    /// The tool's name, as the vendor names it, or the subagent's agent type or "Subagent".
    pub title: SharedString,
    /// The tool input or the subagent's task, drawn in mono and cut to one line.
    pub detail: SharedString,
    pub state: StepState,
    pub output: Option<SharedString>,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Running,
    Done,
    Failed,
    Interrupted,
}

/// A permission request. The owner words `title` ("Run command?" for a shell command, otherwise "Allow <tool>?").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalRow {
    pub request_id: SharedString,
    pub title: SharedString,
    pub input: SharedString,
    pub truncated: bool,
    pub state: ApprovalState,
    /// The CLI offered rules for the `always` scope, so the card offers it.
    pub can_always: bool,
}

/// Only `Allowed` authorizes the tool. `Cancelled` covers every end without an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Allowed,
    Denied,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalAnswer {
    Allow,
    /// Allows this call and the rules the CLI offered with it; only drawn when `can_always`.
    Always,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnState {
    Running,
    Interrupted,
    /// The run ended with the turn open and nothing says how; it is not a success.
    Unverifiable,
}

/// What the person asked the transcript to do. The owner decides whether the request is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptAction {
    Answer {
        request_id: SharedString,
        answer: ApprovalAnswer,
    },
}

pub type TranscriptActionHandler = Rc<dyn Fn(TranscriptAction, &mut Window, &mut App)>;
