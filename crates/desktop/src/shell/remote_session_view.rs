//! The session view of a session a paired host runs (STAT-16): its header, its status and kind, and its terminal, which is
//! attached over the host's own connection. A session with no terminal says why its terminal is not shown here.

use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, WeakEntity, div, px};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::tokens::{SPACE_1, SPACE_2, TYPE_BODY_SM, TYPE_HEADING};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use super::work_panel::{SESSION_ATTACH_FAILED, attach_failed_banner};
use crate::model::remote_sessions::RemoteSessionView;
use crate::model::shell::AgentState;

/// What the remote session page draws: its view, and its terminal's surface with the state of that attach.
pub(super) struct RemoteSessionFrame {
    pub(super) view: RemoteSessionView,
    /// The terminal's surface, while the terminal is attached.
    pub(super) surface: Option<Entity<TerminalSurface>>,
    pub(super) out_of_contact: bool,
    /// Why the host refused the terminal; Retry attaches it again.
    pub(super) failure: Option<String>,
    pub(super) shell: WeakEntity<Shell>,
}

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

/// The page of a remote session: its title, its kind and status on its host, and its terminal. A session with no
/// terminal says why the terminal is not here.
pub(super) fn remote_session_page(frame: RemoteSessionFrame, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let RemoteSessionFrame {
        view,
        surface,
        out_of_contact,
        failure,
        shell,
    } = frame;
    let facts = format!(
        "{} · {} · on {}",
        view.kind,
        status_word(view.state),
        view.host
    );
    let mut header = div()
        .flex_none()
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
        );
    if !view.reached {
        header = header.child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(format!(
                    "{} cannot be reached now; the status shown is the last known one.",
                    view.host
                )),
        );
    }
    let body = match (surface, failure) {
        (_, Some(detail)) => {
            let retry = shell;
            attach_failed_banner(SESSION_ATTACH_FAILED, detail, move |_, cx| {
                let _ = retry.update(cx, |shell, cx| shell.retry_remote_terminal(cx));
            })
        }
        (Some(surface), None) => div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .when(out_of_contact, |pane| {
                pane.child(unreachable_line(&view.host))
            })
            .child(div().relative().flex_1().min_h_0().child(surface))
            .into_any_element(),
        (None, None) => div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(theme.palette.text.tertiary)
                    .child(format!(
                        "This session runs on {}; its terminal opens from that computer for now.",
                        view.host
                    )),
            )
            .into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .size_full()
        .gap(px(SPACE_2))
        .child(header)
        .child(body)
        .into_any_element()
}

/// The line a paired host's terminal shows while the host is out of contact. It stays plain: the session is not ended.
fn unreachable_line(host: &str) -> impl IntoElement {
    div()
        .debug_selector(|| "remote-unreachable".to_owned())
        .child(Banner::new(
            "remote-unreachable-banner",
            BannerVariant::Unreachable,
            format!("Can't reach {host}; reconnecting."),
        ))
}
