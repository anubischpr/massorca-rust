//! The session view of a session a paired host runs (STAT-16): its header, its status and kind, and why its
//! terminal is not shown here. The terminal stays on the host until it can be attached over the host link.

use gpui::prelude::*;
use gpui::{AnyElement, App, div, px};
use massorca_ui::tokens::{SPACE_1, TYPE_BODY_SM, TYPE_HEADING};
use massorca_ui::{ActiveTheme, TextStyled};

use crate::model::remote_sessions::RemoteSessionView;
use crate::model::shell::AgentState;

/// The status word of a remote session. A host that cannot be reached is unverifiable, never exited.
fn status_word(state: AgentState) -> &'static str {
    match state {
        AgentState::Running => "Running",
        AgentState::Waiting => "Waiting for you",
        AgentState::Idle => "Idle",
        AgentState::Done => "Done",
        AgentState::Error => "Failed",
        AgentState::Unreachable => "Unverifiable",
    }
}

/// The page of a remote session: its title, its kind and status on its host, and the reason its terminal is not here.
pub(super) fn remote_session_page(view: &RemoteSessionView, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let facts = format!(
        "{} · {} · on {}",
        view.kind,
        status_word(view.state),
        view.host
    );
    let reason = format!(
        "This session runs on {}; its terminal opens from that computer for now.",
        view.host
    );
    let mut page = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_1))
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child(view.title.clone()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(facts),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(reason),
        );
    if !view.reached {
        page = page.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(format!(
                    "{} cannot be reached now; the status shown is the last known one.",
                    view.host
                )),
        );
    }
    page.into_any_element()
}
