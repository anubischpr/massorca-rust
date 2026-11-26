//! The approval card of a transcript (X-CH-02): a permission request the agent waits on, drawn on the waiting tint with
//! Deny and Approve, and one line once it is settled. Only an allowed request authorizes the tool.

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::TextStyled;
use crate::tokens::{
    Palette, RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION,
    TYPE_MONO_SM,
};

use super::cards::sunken_text;
use super::{
    ApprovalAnswer, ApprovalRow, ApprovalState, TranscriptAction, TranscriptActionHandler,
};

/// The card for `approval`. A pending request has Deny, Always (when the CLI offered it) and Approve; `host_unreachable`
/// names the host when it cannot be reached, which disables the answers, because they would not reach the agent.
pub fn approval_card(
    id: &SharedString,
    approval: &ApprovalRow,
    host_unreachable: Option<&SharedString>,
    on_action: Option<TranscriptActionHandler>,
    p: Palette,
) -> AnyElement {
    match approval.state {
        ApprovalState::Pending => pending_card(id, approval, host_unreachable, on_action, p),
        settled => settled_line(approval, settled, p),
    }
}

fn pending_card(
    id: &SharedString,
    approval: &ApprovalRow,
    host_unreachable: Option<&SharedString>,
    on_action: Option<TranscriptActionHandler>,
    p: Palette,
) -> AnyElement {
    let unreachable = host_unreachable.is_some();
    let answer = |label: &'static str, answer: ApprovalAnswer, variant: ButtonVariant| {
        let handler = on_action.clone();
        let request_id = approval.request_id.clone();
        Button::new(format!("{id}-{label}"), label)
            .variant(variant)
            .size(ButtonSize::Compact)
            .disabled(unreachable)
            .on_click(move |window, cx| {
                if let Some(handler) = &handler {
                    let action = TranscriptAction::Answer {
                        request_id: request_id.clone(),
                        answer,
                    };
                    handler(action, window, cx);
                }
            })
    };
    let actions = div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(SPACE_2))
        .child(answer(
            "Deny",
            ApprovalAnswer::Deny,
            ButtonVariant::Secondary,
        ))
        .when(approval.can_always, |row| {
            row.child(answer(
                "Always",
                ApprovalAnswer::Always,
                ButtonVariant::Secondary,
            ))
        })
        .child(answer(
            "Approve",
            ApprovalAnswer::Allow,
            ButtonVariant::Primary,
        ));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .p(px(SPACE_3))
        .rounded(px(RADIUS_LG))
        .bg(p.agent.waiting_bg)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(AgentStatus::new(AgentState::Waiting).size(AgentStatusSize::Medium))
                .child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(p.agent.waiting)
                        .child("Waiting for you"),
                ),
        )
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(p.text.primary)
                .child(approval.title.clone()),
        )
        .child(sunken_text(&approval.input, approval.truncated, p))
        .when(
            approval.can_always && !approval.suggestions.is_empty(),
            |card| card.child(always_rules(id, &approval.suggestions, p)),
        )
        .when_some(host_unreachable, |card, host| {
            card.child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(format!(
                        "Can't reach {host}. Your answer would not reach the agent."
                    )),
            )
        })
        .child(actions)
        .into_any_element()
}

/// What an Always answer writes: each rule on a line of its own, so the person sees it before accepting.
fn always_rules(id: &SharedString, rules: &[SharedString], p: Palette) -> AnyElement {
    div()
        .debug_selector({
            let id = id.clone();
            move || format!("approval-always-{id}")
        })
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.tertiary)
                .child("Always allows"),
        )
        .children(rules.iter().map(|rule| {
            div()
                .text_style(TYPE_MONO_SM)
                .text_color(p.text.secondary)
                .child(rule.clone())
        }))
        .into_any_element()
}

/// The settled card in one line: its word, the request's title and, when allowed, a check.
fn settled_line(approval: &ApprovalRow, state: ApprovalState, p: Palette) -> AnyElement {
    let (word, colour) = match state {
        ApprovalState::Allowed => ("Approved", p.status.success_fg),
        ApprovalState::Denied => ("Denied", p.text.secondary),
        ApprovalState::Cancelled | ApprovalState::Pending => ("Cancelled", p.text.tertiary),
    };
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .text_style(TYPE_BODY_SM)
        .when(matches!(state, ApprovalState::Allowed), |row| {
            row.child(
                Icon::new(IconName::Check)
                    .size(IconSize::XSmall)
                    .color(colour),
            )
        })
        .child(div().flex_none().text_color(colour).child(word))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_MONO_SM)
                .text_color(p.text.secondary)
                .child(approval.title.clone()),
        )
        .into_any_element()
}
