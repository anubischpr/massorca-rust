//! The session view on the Canvas (ws-session, D-AG-03): the open session's header, its live terminal, attached
//! through the same view the Work panel uses, and the composer under it. The composer types the next message
//! into the session, stops a running agent and resumes an ended session, all through the service.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, SharedString, Subscription, WeakEntity, div, px};
use massorca_protocol::methods::agents::{
    AgentKind, AgentPromptParams, AgentSession, PromptReceipt,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_ui::components::agent_status::{
    AgentState as GlyphState, AgentStatus, AgentStatusSize,
};
use massorca_ui::components::composer::{Composer, ComposerEvent};
use massorca_ui::components::model_picker::{Choice, PickerState};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::tokens::{SIZE_TOOLBAR, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_TITLE_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use super::sidebar_rows::glyph_state;
use super::work_panel::{SESSION_ATTACH_FAILED, attach_failed_banner};
use crate::model::agent_sessions::{agent_of_kind, session_state};
use crate::model::canvas::Canvas;
use crate::model::shell::AgentState;
use crate::model::start_page::VOICE_LATER;
use crate::service::{Refusal, ServiceHandle};

/// What the composer of a session waits for. A session has at most one call in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionCall {
    /// A prompt is being typed into the session.
    Sending,
    /// The session's run is being stopped.
    Stopping,
    /// The ended session is starting a new run.
    Resuming,
}

/// A session's composer as the Shell keeps it, with the subscription that turns its events into calls.
#[derive(Debug)]
pub(super) struct SessionComposer {
    pub entity: Entity<Composer>,
    _events: Subscription,
}

const SENDING: &str = "Sending…";
const RESUMING: &str = "Resuming…";
const ENDED: &str = "This session has ended.";
const ENDED_NOT_RESUMABLE: &str = "This session has ended and cannot be resumed.";

/// What the composer of a session shows: whether the agent runs, why sending is off, and whether Resume is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ComposerTerms {
    /// The agent runs or waits for the person, so the primary button stops it.
    pub busy: bool,
    /// Why sending is off. The text is read-only while a reason is shown.
    pub reason: Option<&'static str>,
    /// An ended session of a kind that can resume offers Resume beside its reason.
    pub resume: bool,
}

/// The composer's terms for `session`, given whether its kind can resume and the call it has in flight.
pub(super) fn composer_terms(
    session: &AgentSession,
    resumable: bool,
    call: Option<SessionCall>,
) -> ComposerTerms {
    let busy = matches!(
        session_state(session),
        AgentState::Running | AgentState::Waiting
    );
    let ended = session.execution.state == TerminalState::Exited;
    let (reason, resume) = match call {
        Some(SessionCall::Sending) => (Some(SENDING), false),
        Some(SessionCall::Resuming) => (Some(RESUMING), false),
        _ if !ended => (None, false),
        _ if resumable => (Some(ENDED), true),
        _ => (Some(ENDED_NOT_RESUMABLE), false),
    };
    ComposerTerms {
        busy,
        reason,
        resume,
    }
}

/// What the session view draws: the header's facts, the terminal's pane or the refusal of its attach, the refusal of
/// the last stop or resume, and the composer.
pub(super) struct SessionFrame {
    pub title: String,
    pub kind: String,
    pub status: GlyphState,
    pub workspace: String,
    /// The session's terminal; a session without one shows its header only.
    pub terminal: Option<String>,
    pub surface: Option<Entity<TerminalSurface>>,
    pub failure: Option<String>,
    /// Why the last stop or resume of the session was refused.
    pub notice: Option<String>,
    pub composer: Entity<Composer>,
    pub shell: WeakEntity<Shell>,
}

/// The session view: its header, then the terminal's pane, or the refusal of its attach with Retry, then the notice of
/// a refused stop or resume, and the composer.
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
        .when_some(frame.notice, |page, notice| {
            page.child(
                div()
                    .debug_selector(|| "session-notice".to_owned())
                    .flex_none()
                    .px(px(SPACE_4))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.status.danger_fg)
                    .child(notice),
            )
        })
        .child(div().flex_none().pb(px(SPACE_3)).child(frame.composer))
        .into_any_element()
}

impl Shell {
    /// The open session's view. Its terminal is attached the first time the view is drawn, the way the Work panel
    /// attaches a terminal it selects. Its composer follows the session's state and the call it has in flight.
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
        let (surface, failure) = match session
            .terminal_id
            .as_deref()
            .and_then(|terminal| self.terminal_views.get(terminal))
        {
            Some(view) => (Some(view.surface.clone()), view.failure.clone()),
            None => (None, None),
        };
        let resumable = kinds
            .iter()
            .any(|kind| kind.id == session.kind && kind.resumable);
        let terms = composer_terms(&session, resumable, self.session_calls.get(&id).copied());
        let composer = self.session_composer(&id, cx);
        composer.update(cx, |composer, cx| {
            composer.set_busy(terms.busy, cx);
            composer.set_disabled_reason(terms.reason.map(SharedString::from), cx);
            composer.set_resume_offered(terms.resume, cx);
        });
        Some(SessionFrame {
            title: session_title(&session, &kinds),
            kind: kind_name(&session.kind, &kinds),
            status: glyph_state(session_state(&session)),
            workspace: session.workspace_id.clone(),
            terminal: session.terminal_id.clone(),
            surface,
            failure,
            notice: self.session_notices.get(&id).cloned(),
            composer,
            shell: cx.entity().downgrade(),
        })
    }

    /// The composer of a session, created on first use and then kept, so its draft survives redraws and switches.
    pub(super) fn session_composer(
        &mut self,
        session: &str,
        cx: &mut Context<Self>,
    ) -> Entity<Composer> {
        self.session_composers
            .entry(session.to_owned())
            .or_insert_with(|| {
                let id = session.to_owned();
                let picker = PickerState::new(
                    Vec::new(),
                    Choice {
                        model: SharedString::default(),
                        effort: None,
                        fast: false,
                    },
                );
                let entity = cx.new(|cx| {
                    Composer::new(picker, Vec::new(), cx)
                        .voice_unavailable(VOICE_LATER)
                        .attach(false)
                        .model_picker(false)
                        .queueing(false)
                });
                let events = cx.subscribe(
                    &entity,
                    move |shell, composer, event: &ComposerEvent, cx| match event {
                        ComposerEvent::Send(text) => {
                            shell.send_prompt(&id, text.clone(), composer.clone(), cx);
                        }
                        ComposerEvent::RetrySend => {
                            let text = composer.read(cx).draft(cx);
                            shell.send_prompt(&id, text, composer.clone(), cx);
                        }
                        ComposerEvent::Stop => shell.stop_session(&id, cx),
                        ComposerEvent::Resume => shell.resume_session(&id, cx),
                        _ => {}
                    },
                );
                SessionComposer {
                    entity,
                    _events: events,
                }
            })
            .entity
            .clone()
    }

    /// Types the draft into the session. The draft stays, read-only, until the service accepts the prompt.
    fn send_prompt(
        &mut self,
        session: &str,
        text: String,
        composer: Entity<Composer>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.begin_session_call(session, SessionCall::Sending, cx) else {
            return;
        };
        let params = AgentPromptParams {
            session: session.to_owned(),
            text,
            force: false,
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_prompt(params).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_prompt(&id, composer, answer, cx);
            });
        })
        .detach();
    }

    /// The service's answer to a prompt: an accepted one clears the draft; a refused one keeps it with the sentence.
    fn finish_prompt(
        &mut self,
        session: &str,
        composer: Entity<Composer>,
        answer: Result<PromptReceipt, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.session_calls.remove(session);
        composer.update(cx, |composer, cx| match answer {
            Ok(_) => {
                composer.set_disabled_reason(None, cx);
                composer.set_error(None, cx);
                composer.set_draft("", cx);
            }
            Err(refusal) => composer.set_error(Some(refusal.sentence().into()), cx),
        });
        cx.notify();
    }

    /// Stops the session's run at once, without a confirm. The store follows the service's answer.
    fn stop_session(&mut self, session: &str, cx: &mut Context<Self>) {
        let Some(service) = self.begin_session_call(session, SessionCall::Stopping, cx) else {
            return;
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_stop(&id).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_session_call(&id, "Could not stop the agent", answer, cx);
            });
        })
        .detach();
    }

    /// Starts a new run of the ended session. The store follows the session, and the view attaches its new terminal.
    fn resume_session(&mut self, session: &str, cx: &mut Context<Self>) {
        let Some(service) = self.begin_session_call(session, SessionCall::Resuming, cx) else {
            return;
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_resume(&id).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_session_call(&id, "Could not resume the session", answer, cx);
            });
        })
        .detach();
    }

    /// Marks a call in flight for `session`. None when one is already in flight or there is no service to ask.
    fn begin_session_call(
        &mut self,
        session: &str,
        call: SessionCall,
        cx: &mut Context<Self>,
    ) -> Option<ServiceHandle> {
        if self.session_calls.contains_key(session) {
            return None;
        }
        let service = self.service.clone()?;
        self.session_calls.insert(session.to_owned(), call);
        self.session_notices.remove(session);
        cx.notify();
        Some(service)
    }

    /// The service's answer to a stop or resume: a refusal is kept above the composer with its reason.
    fn finish_session_call(
        &mut self,
        session: &str,
        what: &'static str,
        answer: Result<AgentSession, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.session_calls.remove(session);
        if let Err(refusal) = answer {
            self.session_notices.insert(
                session.to_owned(),
                format!("{what}: {}", refusal.sentence()),
            );
        }
        cx.notify();
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
    use massorca_protocol::methods::agents::{
        Activity, ActivitySource, ActivityState, PackageState,
    };

    fn activity(state: ActivityState) -> Activity {
        Activity {
            state,
            outcome: None,
            source: ActivitySource::Hook,
            at_ms: 1,
        }
    }

    fn ended(id: &str) -> AgentSession {
        let mut session = agent(id, "w1", 1);
        session.execution.state = TerminalState::Exited;
        session
    }

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

    #[test]
    fn a_working_or_waiting_agent_is_busy_and_an_idle_one_is_not() {
        let mut session = agent("s", "w1", 1);
        assert!(!composer_terms(&session, false, None).busy, "no report yet");
        session.activity = Some(activity(ActivityState::Working));
        assert!(composer_terms(&session, false, None).busy);
        session.activity = Some(activity(ActivityState::Waiting));
        assert!(
            composer_terms(&session, false, None).busy,
            "a question to the person still holds the run"
        );
        session.activity = Some(activity(ActivityState::Done));
        assert!(!composer_terms(&session, false, None).busy);
    }

    #[test]
    fn a_live_session_holds_no_text_and_offers_no_resume() {
        let terms = composer_terms(&agent("s", "w1", 1), true, None);
        assert_eq!(
            terms,
            ComposerTerms {
                busy: false,
                reason: None,
                resume: false,
            }
        );
    }

    #[test]
    fn an_ended_session_offers_resume_only_when_its_kind_can_resume() {
        let resumable = composer_terms(&ended("s"), true, None);
        assert_eq!(resumable.reason, Some(ENDED));
        assert!(resumable.resume);
        let stuck = composer_terms(&ended("s"), false, None);
        assert_eq!(stuck.reason, Some(ENDED_NOT_RESUMABLE));
        assert!(!stuck.resume, "one line says so and no control is offered");
    }

    #[test]
    fn a_prompt_or_a_resume_in_flight_holds_the_text_and_says_so() {
        let sending = composer_terms(&agent("s", "w1", 1), false, Some(SessionCall::Sending));
        assert_eq!(sending.reason, Some(SENDING));
        let resuming = composer_terms(&ended("s"), true, Some(SessionCall::Resuming));
        assert_eq!(resuming.reason, Some(RESUMING));
        assert!(!resuming.resume, "no second Resume while one runs");
    }

    #[test]
    fn a_stop_in_flight_leaves_the_state_to_the_service() {
        let mut session = agent("s", "w1", 1);
        session.activity = Some(activity(ActivityState::Working));
        let terms = composer_terms(&session, false, Some(SessionCall::Stopping));
        assert!(terms.busy, "the run ends when the service says so");
        assert_eq!(terms.reason, None);
    }
}
