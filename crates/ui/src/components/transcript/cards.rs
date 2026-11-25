//! The transcript rows other than the approval card: messages, reasoning, tool and subagent steps, plan, diff, notices and
//! turn footers. Each draws in the tokens of ConversationMessageList.md, AgentStatus.md and DiffView.md.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, SharedString, Window, div, px};

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::diff_view::{DiffModel, DiffView};
use crate::components::plan_view::{Plan, PlanStatus, PlanView};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::TextStyled;
use crate::tokens::{
    LAYOUT_CONVERSATION_READING, Palette, RADIUS_LG, RADIUS_MD, SPACE_1, SPACE_2, SPACE_3,
    TYPE_BODY_SM, TYPE_CAPTION, TYPE_CHAT, TYPE_MONO_SM,
};

use super::{NoticeLevel, StepRow, StepState, TurnState};

/// A diff in a transcript is a snippet; the Work panel opens the whole file.
const SNIPPET_HEIGHT: f32 = 240.0;

/// The person's message, right-aligned in a raised bubble.
pub fn user_message(text: &SharedString, p: Palette) -> AnyElement {
    div()
        .flex()
        .justify_end()
        .child(
            div()
                .max_w(px(LAYOUT_CONVERSATION_READING))
                .px(px(SPACE_3))
                .py(px(SPACE_2))
                .rounded(px(RADIUS_LG))
                .bg(p.surface.raised)
                .text_style(TYPE_CHAT)
                .text_color(p.text.primary)
                .child(text.clone()),
        )
        .into_any_element()
}

/// The agent's text, in the chat style on the canvas.
// TODO: draw Markdown once crates/ui has a renderer; until then the text is plain.
pub fn prose(text: &SharedString, p: Palette) -> AnyElement {
    div()
        .text_style(TYPE_CHAT)
        .text_color(p.text.primary)
        .child(text.clone())
        .into_any_element()
}

/// Opens or closes a thinking row.
pub type ThinkingToggle = Rc<dyn Fn(&mut Window, &mut App)>;

/// The agent's reasoning: a toggle that opens the text. The toggle carries a running glyph while the agent thinks.
pub fn thinking(
    id: &SharedString,
    text: &SharedString,
    streaming: bool,
    expanded: bool,
    toggle: ThinkingToggle,
    p: Palette,
) -> AnyElement {
    let toggle_key = format!("{id}-thinking-toggle");
    let toggle_row = div()
        .id(toggle_key.clone())
        .debug_selector(move || toggle_key)
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .cursor_pointer()
        .when(streaming, |row| {
            row.child(AgentStatus::new(AgentState::Running).size(AgentStatusSize::XSmall))
        })
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.tertiary)
                .child("Thinking"),
        )
        .child(
            Icon::new(if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .size(IconSize::XSmall)
            .color(p.text.tertiary),
        )
        .on_click(move |_, window, cx| toggle(window, cx));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(toggle_row)
        .when(expanded, |col| {
            let text_key = format!("{id}-thinking-text");
            col.child(
                div()
                    .debug_selector(move || text_key)
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(text.clone()),
            )
        })
        .into_any_element()
}

/// A tool call or a subagent: its status glyph and word, its input in mono, and its output in a sunken well. A failed
/// step is drawn on the error tint.
pub fn step(step: &StepRow, p: Palette) -> AnyElement {
    let (agent, word) = match step.state {
        StepState::Running => (AgentState::Running, "Running"),
        StepState::Done => (AgentState::Done, "Done"),
        StepState::Failed => (AgentState::Error, "Failed"),
        StepState::Interrupted => (AgentState::Idle, "Interrupted"),
    };
    let tint = match step.state {
        StepState::Failed => p.agent.error_bg,
        _ => p.surface.raised,
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .rounded(px(RADIUS_LG))
        .bg(tint)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(AgentStatus::new(agent).size(AgentStatusSize::Small))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_style(TYPE_MONO_SM)
                        .text_color(p.text.primary)
                        .child(step.title.clone()),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .flex_none()
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.tertiary)
                        .child(word),
                ),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_MONO_SM)
                .text_color(p.text.secondary)
                .child(step.detail.clone()),
        )
        .when_some(step.output.clone(), |card, output| {
            card.child(sunken_text(&output, step.truncated, p))
        })
        .into_any_element()
}

/// The plan the agent reported, drawn by PlanView with its own id.
pub fn plan(id: &SharedString, plan: &Rc<Plan>) -> AnyElement {
    PlanView::new(format!("{id}-plan"), plan.clone())
        .status(PlanStatus::Ready)
        .into_any_element()
}

/// A file change as a read-only snippet of the diff; `truncated` says the patch was cut before the view.
pub fn diff(id: &SharedString, model: &Rc<DiffModel>, truncated: bool, p: Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(
            div()
                .h(px(SNIPPET_HEIGHT))
                .child(DiffView::new(format!("{id}-diff"), model.clone()).read_only(true)),
        )
        .when(truncated, |col| col.child(caption("Diff truncated", p)))
        .into_any_element()
}

/// A notice from the service or the CLI, coloured by its level.
pub fn notice(level: NoticeLevel, text: &SharedString, p: Palette) -> AnyElement {
    let colour = match level {
        NoticeLevel::Info => p.text.secondary,
        NoticeLevel::Warning => p.status.warning_fg,
        NoticeLevel::Error => p.status.danger_fg,
    };
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(colour)
        .child(text.clone())
        .into_any_element()
}

/// The footer of a turn that is running, was interrupted, or ended without a result.
pub fn turn_footer(state: TurnState, p: Palette) -> AnyElement {
    let (agent, word) = match state {
        TurnState::Running => (Some(AgentState::Running), "Working"),
        TurnState::Interrupted => (Some(AgentState::Idle), "Interrupted"),
        TurnState::Unverifiable => (None, "Status unknown"),
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .when_some(agent, |row, agent| {
            row.child(AgentStatus::new(agent).size(AgentStatusSize::Small))
        })
        .child(caption(word, p))
        .into_any_element()
}

/// Text in a sunken well, with a note when it was cut.
pub(super) fn sunken_text(text: &SharedString, truncated: bool, p: Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(
            div()
                .rounded(px(RADIUS_MD))
                .bg(p.surface.sunken)
                .px(px(SPACE_2))
                .py(px(SPACE_1))
                .text_style(TYPE_MONO_SM)
                .text_color(p.text.secondary)
                .child(text.clone()),
        )
        .when(truncated, |col| col.child(caption("Output truncated", p)))
        .into_any_element()
}

pub(super) fn caption(text: impl Into<SharedString>, p: Palette) -> AnyElement {
    div()
        .text_style(TYPE_CAPTION)
        .text_color(p.text.tertiary)
        .child(text.into())
        .into_any_element()
}
