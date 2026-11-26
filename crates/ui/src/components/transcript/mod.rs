//! A chat session's transcript: the rows of its items drawn in timeline order, with the cards of X-CH-02 (approval),
//! X-CH-05 (plan) and D-AG-03 (conversation). The owner maps the session's items to [`TranscriptRow`]s.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, ElementId, Entity, FollowMode, ListAlignment, ListState, SharedString, Window,
    div, list, px,
};

use crate::components::diff_view::DiffModel;
use crate::components::plan_view::Plan;
use crate::theme::ActiveTheme;
use crate::tokens::{LAYOUT_CONVERSATION_READING, Palette, SPACE_3, SPACE_4, SPACE_12};

use cards::ThinkingToggle;

pub mod approval;
pub mod cards;
#[cfg(test)]
mod tests;

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

/// Rows drawn beyond the viewport, so a short scroll does not wait for them.
const OVERDRAW: f32 = SPACE_12;

/// The view's own state, kept by its element id: the list that places the rows and the thinking rows opened so far.
struct TranscriptUi {
    list: ListState,
    expanded: HashSet<SharedString>,
}

/// Draws a chat session's transcript. Rows are bottom-aligned and the list follows the newest one while the agent
/// writes; scrolling up releases it, and scrolling back to the end resumes it (ConversationMessageList.md).
#[derive(IntoElement)]
pub struct TranscriptView {
    id: ElementId,
    rows: Rc<Vec<TranscriptRow>>,
    host_unreachable: Option<SharedString>,
    on_action: Option<TranscriptActionHandler>,
}

impl std::fmt::Debug for TranscriptView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranscriptView")
            .field("id", &self.id)
            .field("rows", &self.rows.len())
            .finish()
    }
}

impl TranscriptView {
    pub fn new(id: impl Into<ElementId>, rows: impl Into<Rc<Vec<TranscriptRow>>>) -> Self {
        Self {
            id: id.into(),
            rows: rows.into(),
            host_unreachable: None,
            on_action: None,
        }
    }

    /// The host the session runs on cannot be reached: the answers are disabled, since they would not reach the agent.
    pub fn host_unreachable(mut self, host: Option<SharedString>) -> Self {
        self.host_unreachable = host;
        self
    }

    /// Receives what the person asks for, such as an answer to an approval request.
    pub fn on_action(
        mut self,
        handler: impl Fn(TranscriptAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TranscriptView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p: Palette = cx.theme().palette;
        let ui = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            let list = ListState::new(0, ListAlignment::Bottom, px(OVERDRAW));
            list.set_follow_mode(FollowMode::Tail);
            TranscriptUi {
                list,
                expanded: HashSet::new(),
            }
        });
        let list_state = ui.read(cx).list.clone();
        // Rows that arrived are measured by the list; rows that changed in place are measured again, and the scroll holds.
        if list_state.item_count() == self.rows.len() {
            list_state.remeasure();
        } else {
            list_state.reset(self.rows.len());
        }
        let rows = self.rows;
        let host = self.host_unreachable;
        let on_action = self.on_action;
        div()
            .debug_selector(|| "transcript-list".to_owned())
            .size_full()
            .child(
                list(list_state, move |ix, _window, cx| {
                    let row = &rows[ix];
                    let expanded = ui.read(cx).expanded.contains(row.id());
                    let key = row.id().clone();
                    let body =
                        render_row(row, ix, expanded, &ui, host.as_ref(), on_action.clone(), p);
                    div()
                        .debug_selector(move || format!("transcript-row-{key}"))
                        .w_full()
                        .flex()
                        .justify_center()
                        .px(px(SPACE_4))
                        .pb(px(SPACE_3))
                        .child(
                            div()
                                .w_full()
                                .max_w(px(LAYOUT_CONVERSATION_READING))
                                .child(body),
                        )
                        .into_any_element()
                })
                .size_full(),
            )
    }
}

fn render_row(
    row: &TranscriptRow,
    ix: usize,
    expanded: bool,
    ui: &Entity<TranscriptUi>,
    host: Option<&SharedString>,
    on_action: Option<TranscriptActionHandler>,
    p: Palette,
) -> AnyElement {
    match row {
        TranscriptRow::UserMessage { text, .. } => cards::user_message(text, p),
        TranscriptRow::Prose { text, .. } => cards::prose(text, p),
        TranscriptRow::Thinking {
            id,
            text,
            streaming,
        } => {
            let toggle: ThinkingToggle = {
                let (ui, id) = (ui.clone(), id.clone());
                Rc::new(move |window, cx| {
                    ui.update(cx, |ui, _| {
                        if !ui.expanded.remove(&id) {
                            ui.expanded.insert(id.clone());
                        }
                        ui.list.remeasure_items(ix..ix + 1);
                    });
                    window.refresh();
                })
            };
            cards::thinking(id, text, *streaming, expanded, toggle, p)
        }
        TranscriptRow::Step { step, .. } => cards::step(step, p),
        TranscriptRow::Plan { id, plan } => cards::plan(id, plan),
        TranscriptRow::Diff {
            id,
            diff,
            truncated,
        } => cards::diff(id, diff, *truncated, p),
        TranscriptRow::Approval { id, approval } => {
            approval::approval_card(id, approval, host, on_action, p)
        }
        TranscriptRow::Notice { level, text, .. } => cards::notice(*level, text, p),
        TranscriptRow::Turn { state, .. } => cards::turn_footer(*state, p),
    }
}
