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
use crate::components::secret_request_card::{SecretAction, SecretState};
use crate::theme::ActiveTheme;
use crate::tokens::{LAYOUT_CONVERSATION_READING, Palette, SPACE_3, SPACE_4, SPACE_12};

use cards::ThinkingToggle;

pub mod approval;
pub mod cards;
pub mod secret;
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
    /// A secret an agent asked for: its card, with the request's state (X-CH-07).
    Secret {
        id: SharedString,
        secret: SecretRow,
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
            | Self::Secret { id, .. }
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
    /// What an `always` answer writes, one line each, shown above the Always button so the person sees it first.
    pub suggestions: Vec<SharedString>,
}

/// A secret request as its card draws it. The card holds a name and a state, never a value.
#[derive(Clone, Debug, PartialEq)]
pub struct SecretRow {
    pub request_id: SharedString,
    /// The variable name, as the agent asked for it.
    pub name: SharedString,
    /// "Claude needs a secret", naming the agent that asked.
    pub title: SharedString,
    /// Why the agent needs it, as plain text.
    pub reason: Option<SharedString>,
    /// Where the secret belongs, such as "for project oppm".
    pub scope: SharedString,
    pub state: SecretState,
    /// Whether this app can store a value for the request's scope; a team's cannot be answered here.
    pub storable: bool,
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
    /// A press on a secret card: Add and Retry open the Add dialog, Decline declines, Open vault opens the vault.
    Secret {
        request_id: SharedString,
        action: SecretAction,
    },
}

pub type TranscriptActionHandler = Rc<dyn Fn(TranscriptAction, &mut Window, &mut App)>;

/// Called when the list is scrolled to its first row, so the owner can load the older ones.
pub type ReachTopHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Rows drawn beyond the viewport, so a short scroll does not wait for them.
const OVERDRAW: f32 = SPACE_12;

/// The view's own state, kept by its element id: the list that places the rows and the thinking rows opened so far.
struct TranscriptUi {
    list: ListState,
    expanded: HashSet<SharedString>,
    /// The id of the first row the list last placed, to tell rows added above it from rows added below.
    first: Option<SharedString>,
}

/// Draws a chat session's transcript. Rows are bottom-aligned and the list follows the newest one while the agent
/// writes; scrolling up releases it, and scrolling back to the end resumes it (ConversationMessageList.md).
#[derive(IntoElement)]
pub struct TranscriptView {
    id: ElementId,
    rows: Rc<Vec<TranscriptRow>>,
    host_unreachable: Option<SharedString>,
    on_action: Option<TranscriptActionHandler>,
    on_reach_top: Option<ReachTopHandler>,
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
            on_reach_top: None,
        }
    }

    /// The host the session runs on cannot be reached: the answers are disabled, since they would not reach the agent.
    pub fn host_unreachable(mut self, host: Option<SharedString>) -> Self {
        self.host_unreachable = host;
        self
    }

    /// Called when the first row is in view, as often as the list is scrolled there; the owner loads what is older.
    pub fn on_reach_top(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reach_top = Some(Rc::new(handler));
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
                first: None,
            }
        });
        let list_state = ui.read(cx).list.clone();
        let first = self.rows.first().map(|row| row.id().clone());
        let previous_first = ui.read(cx).first.clone();
        let held = list_state.item_count();
        // Rows that arrived are measured by the list; rows that changed in place are measured again, and the scroll holds.
        // Rows added above the first one (an older page) are spliced in so the rows in view stay where they are.
        let added_above = previous_first
            .as_ref()
            .filter(|_| held > 0 && self.rows.len() > held)
            .and_then(|previous| self.rows.iter().position(|row| row.id() == previous))
            .filter(|above| *above > 0 && above + held == self.rows.len());
        if held == self.rows.len() {
            list_state.remeasure();
        } else if let Some(above) = added_above {
            list_state.splice(0..0, above);
            list_state.remeasure();
        } else {
            list_state.reset(self.rows.len());
        }
        ui.update(cx, |ui, _| ui.first = first);
        if let Some(reach_top) = self.on_reach_top.clone() {
            list_state.set_scroll_handler(move |event, window, cx| {
                if event.visible_range.start == 0 {
                    reach_top(window, cx);
                }
            });
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
        TranscriptRow::Secret { id, secret } => {
            secret::secret_card(id, secret, host.is_some(), on_action)
        }
        TranscriptRow::Notice { level, text, .. } => cards::notice(*level, text, p),
        TranscriptRow::Turn { state, .. } => cards::turn_footer(*state, p),
    }
}
