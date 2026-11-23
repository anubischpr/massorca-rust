//! The session view on the Canvas (ws-session, D-AG-03): the open session's header and its live terminal,
//! attached through the same view the Work panel uses. The composer for the next message comes later.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, WeakEntity, div, px};
use massorca_protocol::methods::agents::{AgentKind, AgentSession};
use massorca_ui::components::agent_status::{
    AgentState as GlyphState, AgentStatus, AgentStatusSize,
};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::tokens::{SIZE_TOOLBAR, SPACE_2, SPACE_4, TYPE_BODY_SM, TYPE_TITLE_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use super::sidebar_rows::glyph_state;
use super::work_panel::{SESSION_ATTACH_FAILED, attach_failed_banner};
use crate::model::agent_sessions::{agent_of_kind, session_state};
use crate::model::canvas::Canvas;

/// What the session view draws: the header's facts, and the terminal's pane or the refusal of its attach.
pub(super) struct SessionFrame {
    pub title: String,
    pub kind: String,
    pub status: GlyphState,
    pub workspace: String,
    /// The session's terminal; a session without one shows its header only.
    pub terminal: Option<String>,
    pub surface: Option<Entity<TerminalSurface>>,
    pub failure: Option<String>,
    pub shell: WeakEntity<Shell>,
}

/// The session view: its header, then the terminal's pane, or the refusal of its attach with Retry.
pub(super) fn session_page(frame: SessionFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = theme.palette;
    let header = div()
        .debug_selector(|| "session-header".to_owned())
        .flex()
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .h(px(SIZE_TOOLBAR))
        .px(px(SPACE_4))
        .child(AgentStatus::new(frame.status).size(AgentStatusSize::XSmall))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_TITLE_SM)
                .text_color(p.text.primary)
                .child(frame.title),
        )
        .child(
            div()
                .flex_none()
                .text_style(TYPE_BODY_SM)
                .text_color(p.text.tertiary)
                .child(frame.kind),
        );
    let body = match (frame.failure, frame.terminal, frame.surface) {
        (Some(detail), Some(terminal), _) => {
            let (shell, workspace) = (frame.shell, frame.workspace);
            attach_failed_banner(SESSION_ATTACH_FAILED, detail, move |_, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.retry_attach(&workspace, &terminal, cx);
                });
            })
        }
        (_, _, Some(surface)) => div().flex_1().min_h_0().child(surface).into_any_element(),
        (_, None, _) => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.tertiary)
            .child("This session has no terminal to show.")
            .into_any_element(),
        _ => div().flex_1().into_any_element(),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(body)
        .into_any_element()
}

impl Shell {
    /// The open session's view. Its terminal is attached the first time the view is drawn, the way the
    /// Work panel attaches a terminal it selects.
    pub(super) fn session_frame(&mut self, cx: &mut Context<Self>) -> Option<SessionFrame> {
        if !matches!(self.model.canvas(), Canvas::Session(_)) {
            return None;
        }
        let id = self.model.snapshot().active_session.clone()?;
        let (session, kinds) = {
            let state = self.store.read(cx).state();
            let session = state
                .agents
                .iter()
                .find(|session| session.id == id)?
                .clone();
            (session, state.agent_kinds.clone())
        };
        if let Some(terminal) = session.terminal_id.as_deref()
            && !self.terminal_views.contains_key(terminal)
        {
            self.attach_view(&session.workspace_id, terminal, cx);
        }
        let view = session
            .terminal_id
            .as_deref()
            .and_then(|terminal| self.terminal_views.get(terminal));
        Some(SessionFrame {
            title: session_title(&session, &kinds),
            kind: kind_name(&session.kind, &kinds),
            status: glyph_state(session_state(&session)),
            workspace: session.workspace_id.clone(),
            terminal: session.terminal_id.clone(),
            surface: view.map(|view| view.surface.clone()),
            failure: view.and_then(|view| view.failure.clone()),
            shell: cx.entity().downgrade(),
        })
    }
}

/// The session's name in its header: its title, or its agent's name when it has none.
fn session_title(session: &AgentSession, kinds: &[AgentKind]) -> String {
    session
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map_or_else(|| kind_name(&session.kind, kinds), str::to_owned)
}

/// An agent kind's name: the service's name for it, or the desktop's own when the service does not declare it.
fn kind_name(kind: &str, kinds: &[AgentKind]) -> String {
    kinds
        .iter()
        .find(|declared| declared.id == kind)
        .map_or_else(
            || {
                super::rail::agent_kind(agent_of_kind(kind))
                    .label()
                    .to_owned()
            },
            |declared| declared.name.clone(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::{agent, agent_kind};
    use massorca_protocol::methods::agents::PackageState;

    #[test]
    fn a_session_is_named_by_its_title_and_an_untitled_one_by_its_agent() {
        let kinds = [agent_kind("claude", "Claude Code", PackageState::Enabled)];
        let mut titled = agent("s1", "w1", 1);
        titled.title = Some("Fix the login".to_owned());
        assert_eq!(session_title(&titled, &kinds), "Fix the login");
        assert_eq!(session_title(&agent("s2", "w1", 2), &kinds), "Claude Code");
    }

    #[test]
    fn a_kind_the_service_does_not_declare_is_named_by_the_desktop() {
        let mut session = agent("s3", "w1", 3);
        session.kind = "gentle-shell".to_owned();
        assert_eq!(session_title(&session, &[]), "Gentle Shell");
    }
}
