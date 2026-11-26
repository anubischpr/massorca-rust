//! The session view on the Canvas (ws-session, D-AG-03): the open session's header, its live terminal, attached
//! through the same view the Work panel uses, and the composer under it. The composer types the next message
//! into the session, stops a running agent and resumes an ended session, all through the service. When the service
//! keeps a prompt queue (decision 0084), Send while the agent is busy queues the text, and the dock above the
//! composer draws the session's queue from the store.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, SharedString, Subscription, WeakEntity, div, px};
use massorca_protocol::methods::agents::{
    AgentKind, AgentModeSet, AgentPromptParams, AgentQueueEditParams, AgentQueueParams,
    AgentSession, AgentSetModeParams, QueuedPrompt,
};
use massorca_protocol::methods::chat::{
    self, ApprovalDecision, ApprovalScope, ApprovalState, ChatApprovalAnswerParams,
    ChatHistoryParams, ChatItem, ChatItemBody, RunType,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_ui::components::agent_status::{
    AgentState as GlyphState, AgentStatus, AgentStatusSize,
};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::composer::{Composer, ComposerEvent};
use massorca_ui::components::model_picker::{Choice, PickerState};
use massorca_ui::components::queued_message::{QueuedMessages, QueuedMessagesEvent};
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::components::transcript::{
    ApprovalAnswer, TranscriptAction, TranscriptRow, TranscriptView,
};
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, SIZE_TOOLBAR, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM,
    TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use super::remote_session_view::RemoteSessionFrame;
use super::session_child_wait::{ChildWaitStrip, child_wait_strip};
use super::sidebar_rows::glyph_state;
use super::transcript_rows::transcript_rows;
use super::work_panel::{SESSION_ATTACH_FAILED, attach_failed_banner};
use crate::model::agent_queue::{prompt_id, queued_items};
use crate::model::agent_sessions::{agent_of_kind, session_state};
use crate::model::canvas::Canvas;
use crate::model::chat_transcript::ChatTranscript;
use crate::model::pairing::now_ms;
use crate::model::parent_wait::{child_wait, elapsed_words};
use crate::model::permission_modes::{SessionModes, mode_to_set, session_modes};
use crate::model::session_ended::{
    ENDED, ENDED_NOT_RESUMABLE, START_NEW, ended_offer, refusal_says_conversation_gone,
    screen_says_conversation_gone,
};
use crate::model::shell::AgentState;
use crate::model::start_page::{VOICE_LATER, start_again_params};
use crate::service::{Refusal, ServiceHandle};

/// What the composer of a session waits for. A session has at most one call in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionCall {
    /// A prompt is being typed into the session.
    Sending,
    /// A prompt is being queued behind the busy agent.
    Queueing,
    /// The session's run is being stopped.
    Stopping,
    /// The ended session is starting a new run.
    Resuming,
    /// A new session is starting in place of the ended one, in the same workspace with the same agent.
    Starting,
}

/// A session's composer and queue dock as the Shell keeps them, with the subscriptions that turn their events into
/// calls.
#[derive(Debug)]
pub(super) struct SessionComposer {
    pub entity: Entity<Composer>,
    pub dock: Entity<QueuedMessages>,
    /// Whether the composer was built to queue; it is built again when the service's answer changes.
    queueing: bool,
    _events: Subscription,
    _dock_events: Subscription,
}

/// The header's mode chip of a session: a select over the kind's modes, kept while the kind lists the same names.
#[derive(Debug)]
pub(super) struct SessionModeChip {
    pub entity: Entity<SelectView>,
    pub(super) names: Vec<String>,
    _events: Subscription,
}

const SENDING: &str = "Sending…";
const QUEUEING: &str = "Queueing…";
const RESUMING: &str = "Resuming…";
const STARTING: &str = "Starting…";

/// Whether a session runs in its terminal alone, with no composer (owner request 2026-11-24). A session whose `run` is
/// `chat` opens the transcript view instead; an absent or unknown run is a terminal session (decision 0101, D2).
pub(super) fn is_terminal_arm(session: &AgentSession) -> bool {
    session.run != RunType::Chat
}

/// Whether a session's run is live: its terminal runs, or may still, so it can be stopped.
fn is_live(session: &AgentSession) -> bool {
    matches!(
        session.execution.state,
        TerminalState::Live | TerminalState::Unverifiable
    )
}

/// What the composer of a session shows: whether the agent runs, why sending is off, and whether Resume is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ComposerTerms {
    /// The agent runs or waits for the person, so the primary button stops it.
    pub busy: bool,
    /// Why sending is off. The text is read-only while a reason is shown.
    pub reason: Option<&'static str>,
    /// An ended session of a kind that can resume offers Resume beside its reason.
    pub resume: bool,
    /// The run is live but nothing says the agent works, as with a kind without status hooks: Send stays the
    /// primary button and the header keeps a Stop, so a live run can always be stopped.
    pub stop_aside: bool,
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
    let live = is_live(session);
    let (reason, resume) = match call {
        Some(SessionCall::Sending) => (Some(SENDING), false),
        Some(SessionCall::Queueing) => (Some(QUEUEING), false),
        Some(SessionCall::Resuming) => (Some(RESUMING), false),
        Some(SessionCall::Starting) => (Some(STARTING), false),
        _ if !ended => (None, false),
        _ if resumable => (Some(ENDED), true),
        _ => (Some(ENDED_NOT_RESUMABLE), false),
    };
    ComposerTerms {
        busy,
        reason,
        resume,
        stop_aside: live && !busy,
    }
}

/// The bar of an ended terminal-arm session: its reason and the one action its state offers.
pub(super) struct EndedFrame {
    pub reason: &'static str,
    pub resume: bool,
    pub start_new: bool,
    /// A call is in flight, or the service is not connected: the actions wait.
    pub busy: bool,
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
    /// The other sessions that also write in this folder, named under the header (decision 0169).
    pub shared_folder: Option<String>,
    /// The header's Stop for a live run the composer does not stop, and whether a call in flight holds it.
    pub stop_aside: Option<bool>,
    /// A terminal-arm session has no composer: its header names the arm and carries the mode chip instead.
    pub terminal_arm: bool,
    /// The ended session's bar, when the session has ended in the terminal arm.
    pub ended: Option<EndedFrame>,
    pub mode_chip: Option<Entity<SelectView>>,
    /// The caption after the mode chip, such as "Requested" or "Next resume: Plan".
    pub mode_note: Option<String>,
    pub composer: Entity<Composer>,
    /// The strip above the composer while a child writes in this session's own worktree (decision 0170).
    pub child_wait: Option<ChildWaitStrip>,
    /// The session's queue dock, while the service keeps a queue and it holds a prompt.
    pub queue: Option<Entity<QueuedMessages>>,
    pub shell: WeakEntity<Shell>,
    /// The open session's id, for the header's Delegate and Stop.
    pub session: String,
    /// Whether the header offers Delegate: a service is connected and the session's run has not ended.
    pub can_delegate: bool,
    /// Whether the service cannot be reached, so a chat's answers would not reach the agent.
    pub host_unreachable: bool,
    /// A chat session's transcript rows; a session with rows shows them in place of the terminal.
    pub transcript: Option<Vec<TranscriptRow>>,
}

/// The height of the session view's header, the band above the terminal's pane (`start_grid` fits the pane below it).
pub(super) const SESSION_HEADER_HEIGHT: f32 = SIZE_TOOLBAR;

/// A chat session's body: its transcript, whose answers go to the shell's answer hook and whose top asks for the
/// older page.
fn transcript_body(
    rows: Vec<TranscriptRow>,
    shell: &WeakEntity<Shell>,
    session: &str,
    host_unreachable: bool,
) -> AnyElement {
    let (answers, older, session) = (shell.clone(), shell.clone(), session.to_owned());
    let host = host_unreachable.then(|| SharedString::from("the host"));
    let for_answers = session.clone();
    let view = TranscriptView::new(format!("session-transcript-{session}"), rows)
        .host_unreachable(host)
        .on_action(move |action, _, cx| {
            let _ = answers.update(cx, |shell, cx| {
                shell.answer_transcript(&for_answers, action, cx)
            });
        })
        .on_reach_top(move |_, cx| {
            let _ = older.update(cx, |shell, cx| shell.load_older_chat(&session, cx));
        });
    div().flex_1().min_h_0().child(view).into_any_element()
}

/// The session view: its header, then the terminal's pane, or the refusal of its attach with Retry, then the notice of
/// a refused stop or resume, and the composer.
pub(super) fn session_page(frame: SessionFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = theme.palette;
    let wait_shell = frame.shell.clone();
    let ended_bar = frame.ended.map(|ended| {
        let (resume_shell, resume_session) = (frame.shell.clone(), frame.session.clone());
        let (start_shell, start_session) = (frame.shell.clone(), frame.session.clone());
        div()
            .debug_selector(|| "session-ended".to_owned())
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(SPACE_2))
            .px(px(SPACE_4))
            .pb(px(SPACE_2))
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.secondary)
            .child(ended.reason)
            .child(div().flex_1())
            .when(ended.resume, |bar| {
                bar.child(
                    Button::new("session-resume", "Resume")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Compact)
                        .disabled(ended.busy)
                        .on_click(move |_, cx| {
                            let _ = resume_shell
                                .update(cx, |shell, cx| shell.resume_session(&resume_session, cx));
                        }),
                )
            })
            .when(ended.start_new, |bar| {
                bar.child(
                    Button::new("session-start-new", START_NEW)
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Compact)
                        .disabled(ended.busy)
                        .on_click(move |_, cx| {
                            let _ = start_shell.update(cx, |shell, cx| {
                                shell.start_new_session(&start_session, cx)
                            });
                        }),
                )
            })
    });
    let delegate = frame.can_delegate.then(|| {
        let (shell, session) = (frame.shell.clone(), frame.session.clone());
        Button::new("session-delegate", "Delegate")
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Compact)
            .on_click(move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_delegate(&session, window, cx));
            })
    });
    let header = div()
        .debug_selector(|| "session-header".to_owned())
        .flex()
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .h(px(SESSION_HEADER_HEIGHT))
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
        )
        .when(frame.terminal_arm, |header| {
            header.child(
                div()
                    .debug_selector(|| "session-arm".to_owned())
                    .flex_none()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child("Terminal"),
            )
        })
        .child(div().flex_1())
        .when_some(frame.mode_chip, |header, chip| {
            header.child(div().flex_none().child(chip))
        })
        .when_some(frame.mode_note, |header, note| {
            header.child(
                div()
                    .debug_selector(|| "session-mode-note".to_owned())
                    .flex_none()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(note),
            )
        })
        .when_some(delegate, |header, button| header.child(button))
        .when_some(frame.stop_aside, |header, held| {
            let shell = frame.shell.clone();
            let session = frame.session.clone();
            header.child(
                Button::new("session-stop", "Stop")
                    .size(ButtonSize::Compact)
                    .variant(ButtonVariant::Secondary)
                    .disabled(held)
                    .on_click(move |_, cx| {
                        let _ = shell.update(cx, |shell, cx| shell.stop_session(&session, cx));
                    }),
            )
        });
    let body = match (
        frame.transcript,
        frame.failure,
        frame.terminal,
        frame.surface,
    ) {
        (Some(rows), _, _, _) => {
            transcript_body(rows, &frame.shell, &frame.session, frame.host_unreachable)
        }
        (_, Some(detail), Some(terminal), _) => {
            let (shell, workspace) = (frame.shell, frame.workspace);
            attach_failed_banner(SESSION_ATTACH_FAILED, detail, move |_, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    shell.retry_attach(&workspace, &terminal, cx);
                });
            })
        }
        (_, _, _, Some(surface)) => div()
            .debug_selector(|| "session-terminal".to_owned())
            .flex_1()
            .min_h_0()
            .child(surface)
            .into_any_element(),
        (_, _, None, _) => div()
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
        .when_some(frame.shared_folder, |page, notice| {
            page.child(
                div()
                    .debug_selector(|| "session-shared-folder".to_owned())
                    .flex_none()
                    .px(px(SPACE_4))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(notice),
            )
        })
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
        .when_some(ended_bar, |page, bar| page.child(bar))
        .when_some(frame.queue, |page, dock| {
            page.child(
                div()
                    .debug_selector(|| "session-queue".to_owned())
                    .flex_none()
                    .px(px(SPACE_4))
                    .pb(px(SPACE_2))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(LAYOUT_CONVERSATION_READING))
                            .mx_auto()
                            .child(dock),
                    ),
            )
        })
        .when_some(frame.child_wait, |page, wait| {
            page.child(child_wait_strip(&wait, wait_shell, &theme))
        })
        .when(!frame.terminal_arm, |page| {
            page.child(div().flex_none().pb(px(SPACE_3)).child(frame.composer))
        })
        .into_any_element()
}

impl Shell {
    /// The open session when it is a chat: its transcript is what the bridge follows while the view shows it.
    fn open_chat_session(&self, cx: &App) -> Option<String> {
        if !matches!(self.model.canvas(), Canvas::Session(_)) {
            return None;
        }
        let id = self.model.snapshot().active_session.clone()?;
        let state = self.store.read(cx).state();
        let session = state.agents.iter().find(|session| session.id == id)?;
        (!is_terminal_arm(session)).then_some(id)
    }

    /// Follows the transcript of the chat session the view shows, and stops following the one it no longer shows.
    pub(super) fn sync_chat_watch(&mut self, cx: &mut Context<Self>) {
        let wanted = self.open_chat_session(cx);
        if wanted == self.chat_watched {
            return;
        }
        if let Some(service) = &self.service {
            if let Some(old) = &self.chat_watched {
                service.unwatch_chat(old);
            }
            if let Some(new) = &wanted {
                service.watch_chat(new);
            }
        }
        self.chat_history_pending.clear();
        self.chat_watched = wanted;
    }

    /// Asks for the page of items before the oldest one the transcript holds, when the service has older ones. The store
    /// adds the page when the bridge relays it; a refusal other than a stale cursor is kept above the composer.
    pub(super) fn load_older_chat(&mut self, session: &str, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        if self.chat_history_pending.contains(session) {
            return;
        }
        let before = {
            let state = self.store.read(cx).state();
            state
                .chat
                .get(session)
                .filter(|transcript| transcript.has_older())
                .and_then(|transcript| transcript.history_cursor())
        };
        let Some(before) = before else {
            return;
        };
        self.chat_history_pending.insert(session.to_owned());
        let params = ChatHistoryParams {
            session: session.to_owned(),
            before: Some(before),
            limit: None,
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_chat_history(params).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.chat_history_pending.remove(&id);
                // A stale cursor means the stream resets and its snapshot brings the transcript back.
                if let Err(refusal) = answer
                    && refusal.code() != Some(chat::CURSOR_STALE)
                {
                    shell.session_notices.insert(
                        id,
                        format!("Could not load older messages: {}", refusal.sentence()),
                    );
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The open session's view. Its terminal is attached the first time the view is drawn, the way the Work panel
    /// attaches a terminal it selects. Its composer follows the session's state and the call it has in flight.
    pub(super) fn session_frame(&mut self, cx: &mut Context<Self>) -> Option<SessionFrame> {
        if !matches!(self.model.canvas(), Canvas::Session(_)) {
            return None;
        }
        let id = self.model.snapshot().active_session.clone()?;
        let (session, kinds, queue, waiting, shared_folder) = {
            let state = self.store.read(cx).state();
            let session = state
                .agents
                .iter()
                .find(|session| session.id == id)?
                .clone();
            let queue = state
                .agent_queues
                .as_ref()
                .and_then(|queues| queues.get(&id).cloned());
            let waiting = child_wait(&state.agents, &session);
            let shared_folder = shared_folder_notice(&session, &state.agents, &state.agent_kinds);
            (
                session,
                state.agent_kinds.clone(),
                queue,
                waiting,
                shared_folder,
            )
        };
        // A chat session draws its transcript from its first frame, empty until the stream's snapshot arrives; a
        // terminal session draws one only where a test holds a transcript for it. The secret requests the agent asked
        // for sit in the transcript by their time (X-CH-07).
        let agent = kinds
            .iter()
            .find(|kind| kind.id == session.kind)
            .map_or("The agent", |kind| kind.name.as_str());
        let secrets = self.session_secret_cards(&id, agent, cx);
        let held = {
            let state = self.store.read(cx).state();
            self.chat_transcripts
                .get(&id)
                .or_else(|| state.chat.get(&id))
                .map(|transcript| transcript_rows(transcript, &secrets))
        };
        let transcript = match held {
            Some(rows) => Some(rows),
            None if !is_terminal_arm(&session) => {
                Some(transcript_rows(&ChatTranscript::default(), &secrets))
            }
            None => None,
        };
        if let Some(terminal) = session.terminal_id.as_deref()
            && !self.terminal_views.contains_key(terminal)
            && transcript.is_none()
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
        let call = self.session_calls.get(&id).copied();
        let terms = composer_terms(&session, resumable, call);
        let modes = kinds
            .iter()
            .find(|kind| kind.id == session.kind)
            .and_then(|kind| session_modes(kind, &session));
        let composer = self.session_composer(&id, cx);
        composer.update(cx, |composer, cx| {
            composer.set_busy(terms.busy, cx);
            composer.set_disabled_reason(terms.reason.map(SharedString::from), cx);
            composer.set_resume_offered(terms.resume, cx);
            // The chip always shows the service's mode; a pick only asks for a change.
            match &modes {
                Some(modes) => {
                    composer.set_access_modes(
                        super::start_page::access_modes(&modes.choices),
                        modes.selected,
                        cx,
                    );
                    composer.set_access_note(modes.note.clone().map(SharedString::from), cx);
                }
                None => {
                    composer.set_access_modes(Vec::new(), 0, cx);
                    composer.set_access_note(None, cx);
                }
            }
        });
        let terminal_arm = is_terminal_arm(&session) && transcript.is_none();
        let (mode_chip, mode_note) = match (&modes, terminal_arm) {
            (Some(modes), true) => (
                Some(self.session_mode_chip(&id, modes, cx)),
                modes.note.clone(),
            ),
            _ => (None, None),
        };
        // A terminal-arm run has no composer to stop it, so its header keeps Stop for any live run.
        let stop_aside = if terminal_arm {
            is_live(&session)
        } else {
            terms.stop_aside
        };
        let queue = queue.and_then(|prompts| {
            let dock = self.session_composers.get(&id)?.dock.clone();
            let runs = session.execution.state != TerminalState::Exited;
            dock.update(cx, |dock, cx| {
                dock.set_items(queued_items(&prompts), cx);
                dock.set_send_now_enabled(runs, cx);
            });
            Some(dock)
        });
        let ended = (terminal_arm && session.execution.state == TerminalState::Exited).then(|| {
            // The vendor CLI prints "No conversation found" and exits when the saved conversation is missing.
            let shown_gone = surface.as_ref().is_some_and(|surface| {
                let feed = surface.read(cx).feed();
                screen_says_conversation_gone((0..feed.lines()).map(|row| feed.row_text(row)))
            });
            let gone = self.session_resume_gone.contains(&id) || shown_gone;
            let offer = ended_offer(resumable, gone);
            let reason = match call {
                Some(SessionCall::Resuming) => RESUMING,
                Some(SessionCall::Starting) => STARTING,
                _ => offer.reason,
            };
            EndedFrame {
                reason,
                resume: offer.resume,
                start_new: offer.start_new,
                busy: call.is_some() || self.service.is_none(),
            }
        });
        let wait_strip = waiting.map(|wait| ChildWaitStrip {
            agent: kind_name(&wait.kind, &kinds),
            elapsed: elapsed_words(wait.started_at_ms, now_ms()),
            child: wait.child,
        });
        Some(SessionFrame {
            child_wait: wait_strip,
            stop_aside: stop_aside.then_some(call.is_some()),
            terminal_arm,
            ended,
            mode_chip,
            mode_note,
            title: session_title(&session, &kinds),
            kind: kind_name(&session.kind, &kinds),
            status: glyph_state(session_state(&session)),
            workspace: session.workspace_id.clone(),
            terminal: session.terminal_id.clone(),
            surface,
            failure,
            notice: self.session_notices.get(&id).cloned(),
            shared_folder,
            composer,
            queue,
            shell: cx.entity().downgrade(),
            session: id.clone(),
            can_delegate: self.service.is_some()
                && session.execution.state != TerminalState::Exited,
            host_unreachable: self.service.is_none(),
            transcript,
        })
    }

    /// The view of the open remote session, while the Canvas shows a session This PC does not run (STAT-16). Its
    /// terminal is attached the first time the view is drawn, as a local session's is, and dropped with the view.
    pub(super) fn remote_session_frame(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<RemoteSessionFrame> {
        let open = self
            .model
            .open_remote_session()
            .filter(|_| matches!(self.model.canvas(), Canvas::Session(_)))
            .map(|(host, id)| (host.to_owned(), id.to_owned()));
        let view = open.and_then(|(host, id)| {
            crate::model::remote_sessions::remote_session_view(
                self.store.read(cx).state(),
                &host,
                &id,
            )
        });
        let Some(view) = view else {
            self.remote_terminal = None;
            return None;
        };
        let terminal = view.terminal.clone();
        if let Some(terminal) = terminal.as_deref()
            && !self
                .remote_terminal
                .as_ref()
                .is_some_and(|attached| attached.is(&view.host, terminal))
        {
            self.attach_remote_terminal(&view.host, terminal, cx);
        }
        let shown = terminal.as_deref().and_then(|terminal| {
            self.remote_terminal
                .as_ref()
                .filter(|attached| attached.is(&view.host, terminal))
        });
        Some(RemoteSessionFrame {
            surface: shown.map(|attached| attached.surface.clone()),
            out_of_contact: shown.is_some_and(|attached| attached.out_of_contact),
            failure: shown.and_then(|attached| attached.failure.clone()),
            shell: cx.entity().downgrade(),
            view,
        })
    }

    /// The header's mode chip of `session` for its kind's modes, kept while the kind lists the same names. It shows the
    /// mode the service reports, and a pick calls `set_session_mode`, as the composer's chip does.
    fn session_mode_chip(
        &mut self,
        session: &str,
        modes: &SessionModes,
        cx: &mut Context<Self>,
    ) -> Entity<SelectView> {
        let names: Vec<String> = modes
            .choices
            .iter()
            .map(|choice| choice.name.clone())
            .collect();
        let kept = self
            .session_mode_chips
            .get(session)
            .filter(|chip| chip.names == names)
            .map(|chip| chip.entity.clone());
        let entity = match kept {
            Some(entity) => entity,
            None => {
                let id = session.to_owned();
                let select = cx.new(|cx| {
                    SelectView::new(names.clone(), cx)
                        .label("Mode")
                        .selected(Some(modes.selected))
                });
                let events =
                    cx.subscribe(
                        &select,
                        move |shell, _, event: &SelectEvent, cx| match event {
                            SelectEvent::Changed(index) => shell.set_session_mode(&id, *index, cx),
                        },
                    );
                self.session_mode_chips.insert(
                    session.to_owned(),
                    SessionModeChip {
                        entity: select.clone(),
                        names,
                        _events: events,
                    },
                );
                select
            }
        };
        entity.update(cx, |select, cx| {
            select.set_selected(Some(modes.selected), cx)
        });
        entity
    }

    /// The composer of a session, created on first use and then kept, so its draft survives redraws and switches. It
    /// queues while the agent is busy only when the service keeps a queue; when that changes it is built again,
    /// keeping its draft.
    pub(super) fn session_composer(
        &mut self,
        session: &str,
        cx: &mut Context<Self>,
    ) -> Entity<Composer> {
        let queueing = self.store.read(cx).state().agent_queues.is_some();
        let draft = match self.session_composers.get(session) {
            Some(kept) if kept.queueing == queueing => return kept.entity.clone(),
            Some(kept) => Some(kept.entity.read(cx).draft(cx)),
            None => None,
        };
        let built = self.build_session_composer(session, queueing, cx);
        if let Some(draft) = draft {
            built
                .entity
                .update(cx, |composer, cx| composer.set_draft(&draft, cx));
        }
        let entity = built.entity.clone();
        self.session_composers.insert(session.to_owned(), built);
        entity
    }

    fn build_session_composer(
        &mut self,
        session: &str,
        queueing: bool,
        cx: &mut Context<Self>,
    ) -> SessionComposer {
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
                .queueing(queueing)
        });
        let events = cx.subscribe(
            &entity,
            move |shell, composer, event: &ComposerEvent, cx| match event {
                ComposerEvent::Send(text) => {
                    shell.send_prompt(&id, text.clone(), composer.clone(), cx);
                }
                ComposerEvent::Queue(text) => {
                    shell.queue_prompt(&id, text.clone(), composer.clone(), cx);
                }
                ComposerEvent::RetrySend => {
                    let text = composer.read(cx).draft(cx);
                    shell.send_prompt(&id, text, composer.clone(), cx);
                }
                ComposerEvent::Stop => shell.stop_session(&id, cx),
                ComposerEvent::Resume => shell.resume_session(&id, cx),
                ComposerEvent::AccessChanged(index) => shell.set_session_mode(&id, *index, cx),
                _ => {}
            },
        );
        let dock = cx.new(|_| QueuedMessages::new());
        let id = session.to_owned();
        let dock_events = cx.subscribe(&dock, move |shell, _, event: &QueuedMessagesEvent, cx| {
            shell.on_queue_event(&id, event, cx);
        });
        SessionComposer {
            entity,
            dock,
            queueing,
            _events: events,
            _dock_events: dock_events,
        }
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

    /// The service's answer to a prompt or a queued prompt: an accepted one clears the draft; a refused one keeps it
    /// with the sentence.
    fn finish_prompt<T>(
        &mut self,
        session: &str,
        composer: Entity<Composer>,
        answer: Result<T, Refusal>,
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

    /// Queues the draft behind the busy agent. As with a prompt, the draft stays until the service accepts it.
    fn queue_prompt(
        &mut self,
        session: &str,
        text: String,
        composer: Entity<Composer>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.begin_session_call(session, SessionCall::Queueing, cx) else {
            return;
        };
        let params = AgentQueueParams {
            session: session.to_owned(),
            text,
            request_id: None,
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_queue(params).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_prompt(&id, composer, answer, cx);
            });
        })
        .detach();
    }

    /// Asks the service for what the person did in the dock. The dock changes only with the store's next read of the
    /// queue; a refusal is kept above the composer.
    fn on_queue_event(
        &mut self,
        session: &str,
        event: &QueuedMessagesEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            return;
        };
        let prompts: Vec<QueuedPrompt> = self
            .store
            .read(cx)
            .state()
            .agent_queues
            .as_ref()
            .and_then(|queues| queues.get(session).cloned())
            .unwrap_or_default();
        let prompt = |row: u64| prompt_id(&prompts, row);
        let call = match event {
            QueuedMessagesEvent::Edited { id, text } => prompt(*id).map(|id| QueueCall::Edit {
                id,
                text: text.to_string(),
            }),
            QueuedMessagesEvent::Removed(id) => prompt(*id).map(QueueCall::Remove),
            QueuedMessagesEvent::SendNow(id) | QueuedMessagesEvent::Retry(id) => {
                prompt(*id).map(QueueCall::SendNow)
            }
            QueuedMessagesEvent::Resume => Some(QueueCall::Resume(session.to_owned())),
        };
        let Some(call) = call else {
            return;
        };
        self.session_notices.remove(session);
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let (what, answer) = call.run(&service).await;
            if let Err(refusal) = answer {
                let _ = shell.update(cx, |shell, cx| {
                    shell
                        .session_notices
                        .insert(id, format!("{what}: {}", refusal.sentence()));
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// The `seq` of the pending approval item of `session` that asks `request_id`, as the person sees it. None when the
    /// request is not pending any more: there is nothing left to answer.
    fn pending_approval_seq(&self, session: &str, request_id: &str, cx: &App) -> Option<u64> {
        let state = self.store.read(cx).state();
        let transcript = self
            .chat_transcripts
            .get(session)
            .or_else(|| state.chat.get(session))?;
        transcript.items().iter().find_map(|item| match &item.body {
            ChatItemBody::Approval(approval)
                if approval.request_id == request_id
                    && approval.state == ApprovalState::Pending =>
            {
                Some(item.seq)
            }
            _ => None,
        })
    }

    /// Sends the person's answer to a permission request of a chat session, with the `seq` of the item they saw, so an
    /// answer to a request the CLI sent again since is refused (decision 0101, D6). The item changes through the stream.
    fn answer_transcript(
        &mut self,
        session: &str,
        action: TranscriptAction,
        cx: &mut Context<Self>,
    ) {
        let (request_id, answer) = match action {
            TranscriptAction::Answer { request_id, answer } => (request_id, answer),
            TranscriptAction::Secret { request_id, action } => {
                return self.secret_card_action(&request_id, action, cx);
            }
        };
        let request_id = request_id.to_string();
        let Some(service) = self.service.clone() else {
            return;
        };
        let Some(seq) = self.pending_approval_seq(session, &request_id, cx) else {
            return;
        };
        if !self.chat_answering.insert(request_id.clone()) {
            return;
        }
        let (decision, scope) = match answer {
            ApprovalAnswer::Deny => (ApprovalDecision::Deny, None),
            ApprovalAnswer::Allow => (ApprovalDecision::Allow, Some(ApprovalScope::Once)),
            ApprovalAnswer::Always => (ApprovalDecision::Allow, Some(ApprovalScope::Always)),
        };
        let params = ChatApprovalAnswerParams {
            session: session.to_owned(),
            request_id: request_id.clone(),
            decision,
            scope,
            message: None,
            seq: Some(seq),
        };
        self.session_notices.remove(session);
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_chat_answer(params).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.finish_approval(&id, &request_id, answer, cx);
            });
        })
        .detach();
        cx.notify();
    }

    /// The service's answer to an approval. The item itself comes through the stream; a refusal says in a short note why
    /// the answer did not count.
    fn finish_approval(
        &mut self,
        session: &str,
        request_id: &str,
        answer: Result<ChatItem, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.chat_answering.remove(request_id);
        if let Err(refusal) = answer {
            let note = match refusal.code() {
                Some(chat::APPROVAL_CHANGED) => {
                    "That request changed before your answer arrived. Read it again and answer."
                        .to_owned()
                }
                Some(chat::APPROVAL_SETTLED) => "That request was already answered.".to_owned(),
                _ => format!("Could not send your answer: {}", refusal.sentence()),
            };
            self.session_notices.insert(session.to_owned(), note);
        }
        cx.notify();
    }

    /// Whether `session` is a chat whose agent works or waits: its Stop ends the turn and keeps the session.
    fn chat_turn_runs(&self, session: &str, cx: &App) -> bool {
        let state = self.store.read(cx).state();
        state
            .agents
            .iter()
            .find(|record| record.id == session)
            .is_some_and(|record| {
                !is_terminal_arm(record)
                    && matches!(
                        session_state(record),
                        AgentState::Running | AgentState::Waiting
                    )
            })
    }

    /// Stops what the session is doing. A chat that works gets its turn interrupted and keeps its run (decision 0101,
    /// D8); any other Stop ends the run.
    fn stop_session(&mut self, session: &str, cx: &mut Context<Self>) {
        let interrupt = self.chat_turn_runs(session, cx);
        let Some(service) = self.begin_session_call(session, SessionCall::Stopping, cx) else {
            return;
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            if interrupt {
                let answer = service.agent_chat_interrupt(&id).await;
                let _ = shell.update(cx, |shell, cx| {
                    shell.finish_session_call(&id, "Could not stop the turn", answer, cx);
                });
            } else {
                let answer = service.agent_stop(&id).await;
                let _ = shell.update(cx, |shell, cx| {
                    shell.finish_session_call(&id, "Could not stop the agent", answer, cx);
                });
            }
        })
        .detach();
    }

    /// Starts a new run of the ended session. The store follows the session, and the view attaches its new terminal.
    fn resume_session(&mut self, session: &str, cx: &mut Context<Self>) {
        let Some(service) = self.begin_session_call(session, SessionCall::Resuming, cx) else {
            return;
        };
        let mode = self.pending_bypass(session, cx);
        // A chat has no terminal, so its run starts with no grid.
        let chat = self.is_chat_session(session, cx);
        let grid = (!chat).then(|| self.start_grid(cx));
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_resume(&id, mode, grid).await;
            let _ = shell.update(cx, |shell, cx| shell.finish_resume(&id, answer, cx));
        })
        .detach();
    }

    fn is_chat_session(&self, session: &str, cx: &App) -> bool {
        let state = self.store.read(cx).state();
        state
            .agents
            .iter()
            .find(|record| record.id == session)
            .is_some_and(|record| !is_terminal_arm(record))
    }

    /// The service's answer to a resume. A refusal that says the saved conversation is gone replaces the reason of the
    /// ended session and offers Start a new session here; any other refusal is kept above the terminal with its reason.
    /// A resumed chat follows its transcript again from the cursor it holds: the journal carries on (decision 0101, D9).
    fn finish_resume(
        &mut self,
        session: &str,
        answer: Result<AgentSession, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.session_calls.remove(session);
        match answer {
            Ok(resumed) => {
                self.session_resume_gone.remove(session);
                if resumed.run == RunType::Chat
                    && self.chat_watched.as_deref() == Some(session)
                    && let Some(service) = &self.service
                {
                    service.watch_chat(session);
                }
            }
            Err(refusal) if refusal_says_conversation_gone(refusal.code()) => {
                self.session_resume_gone.insert(session.to_owned());
            }
            Err(refusal) => {
                self.session_notices.insert(
                    session.to_owned(),
                    format!("Could not resume the session: {}", refusal.sentence()),
                );
            }
        }
        cx.notify();
    }

    /// Starts a new session in the ended session's workspace with its agent, without the saved conversation. The
    /// service's new session opens in this view; a refusal is kept above the terminal with its reason.
    fn start_new_session(&mut self, session: &str, cx: &mut Context<Self>) {
        let Some(service) = self.begin_session_call(session, SessionCall::Starting, cx) else {
            return;
        };
        let grid = self.start_grid(cx);
        let params = {
            let state = self.store.read(cx).state();
            state
                .agents
                .iter()
                .find(|record| record.id == session)
                .map(|record| start_again_params(&record.workspace_id, &record.kind, grid))
        };
        let Some(params) = params else {
            self.session_calls.remove(session);
            cx.notify();
            return;
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_start(params).await;
            let _ = shell.update(cx, |shell, cx| shell.finish_start_new(&id, answer, cx));
        })
        .detach();
    }

    /// The service's answer to Start a new session here: the new session opens, or the refusal is kept above the terminal.
    fn finish_start_new(
        &mut self,
        session: &str,
        answer: Result<AgentSession, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.session_calls.remove(session);
        match answer {
            Ok(started) => self.model.open_started_session(started.id),
            Err(refusal) => {
                self.session_notices.insert(
                    session.to_owned(),
                    format!("Could not start a new session: {}", refusal.sentence()),
                );
            }
        }
        cx.notify();
    }

    /// Asks the service to switch the session to the mode of the chip's entry `index`: at once where the kind has a
    /// live switch, else at the next resume. Picking the running mode, or the CLI's default, changes nothing.
    fn set_session_mode(&mut self, session: &str, index: usize, cx: &mut Context<Self>) {
        let mode = {
            let state = self.store.read(cx).state();
            state
                .agents
                .iter()
                .find(|record| record.id == session)
                .and_then(|record| {
                    let kind = state
                        .agent_kinds
                        .iter()
                        .find(|kind| kind.id == record.kind)?;
                    mode_to_set(&session_modes(kind, record)?, record, index)
                })
        };
        let (Some(mode), Some(service)) = (mode, self.service.clone()) else {
            cx.notify();
            return;
        };
        self.session_notices.remove(session);
        let params = AgentSetModeParams {
            session: session.to_owned(),
            permission_mode: mode,
            force: false,
        };
        let id = session.to_owned();
        let shell = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let answer = service.agent_set_mode(params).await;
            let _ = shell.update(cx, |shell, cx| shell.finish_set_mode(&id, answer, cx));
        })
        .detach();
        cx.notify();
    }

    /// The service's answer to a mode switch: the chip follows the session the store holds, and a refusal is kept
    /// above the composer with its reason.
    fn finish_set_mode(
        &mut self,
        session: &str,
        answer: Result<AgentModeSet, Refusal>,
        cx: &mut Context<Self>,
    ) {
        if let Err(refusal) = answer {
            self.session_notices.insert(
                session.to_owned(),
                format!("Could not change the mode: {}", refusal.sentence()),
            );
        }
        cx.notify();
    }

    /// A bypass mode the person picked for the next resume. The service carries a bypass mode into a run only when
    /// the resume names it, so Resume names it; any other pending mode the service applies by itself.
    fn pending_bypass(&self, session: &str, cx: &App) -> Option<String> {
        let state = self.store.read(cx).state();
        let record = state.agents.iter().find(|record| record.id == session)?;
        let pending = record.pending_permission_mode.as_deref()?;
        state
            .agent_kinds
            .iter()
            .find(|kind| kind.id == record.kind)?
            .permission_modes
            .iter()
            .any(|mode| mode.id == pending && mode.bypass)
            .then(|| pending.to_owned())
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
    fn finish_session_call<T>(
        &mut self,
        session: &str,
        what: &'static str,
        answer: Result<T, Refusal>,
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

/// A queue call the dock asked for, by prompt id or, for Resume, by session id.
enum QueueCall {
    Edit { id: String, text: String },
    Remove(String),
    SendNow(String),
    Resume(String),
}

impl QueueCall {
    /// Runs the call; the answer names what failed, for the notice.
    async fn run(self, service: &ServiceHandle) -> (&'static str, Result<(), Refusal>) {
        match self {
            Self::Edit { id, text } => (
                "Could not edit the queued message",
                service
                    .agent_queue_edit(AgentQueueEditParams { id, text })
                    .await
                    .map(drop),
            ),
            Self::Remove(id) => (
                "Could not remove the queued message",
                service.agent_queue_remove(&id).await.map(drop),
            ),
            Self::SendNow(id) => (
                "Could not send the queued message",
                service.agent_queue_send_now(&id).await.map(drop),
            ),
            Self::Resume(session) => (
                "Could not resume the queue",
                service.agent_queue_resume(&session).await.map(drop),
            ),
        }
    }
}

/// The line naming the other sessions that also write in the session's folder, or `None` when none does (decision
/// 0169). A writer the store does not hold is named generically.
pub(super) fn shared_folder_notice(
    session: &AgentSession,
    sessions: &[AgentSession],
    kinds: &[AgentKind],
) -> Option<String> {
    if session.shared_folder_with.is_empty() {
        return None;
    }
    let names: Vec<String> = session
        .shared_folder_with
        .iter()
        .map(|id| {
            sessions.iter().find(|other| &other.id == id).map_or_else(
                || "another session".to_owned(),
                |other| session_title(other, kinds),
            )
        })
        .collect();
    Some(format!(
        "Also writing in this folder: {}.",
        names.join(", ")
    ))
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
    use massorca_protocol::methods::agents::{Activity, ActivityState, PackageState};

    fn activity(state: ActivityState) -> Activity {
        Activity {
            state,
            ..crate::service::fixtures::base_activity()
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
                stop_aside: true,
            }
        );
    }

    #[test]
    fn a_live_run_without_status_reports_keeps_a_stop_beside_send() {
        let hookless = agent("s", "w1", 1);
        assert!(
            hookless.activity.is_none(),
            "a kind without status hooks never reports"
        );
        let terms = composer_terms(&hookless, false, None);
        assert!(!terms.busy, "Send stays the primary button");
        assert!(terms.stop_aside, "the run can still be stopped");
        let mut unverifiable = agent("s", "w1", 1);
        unverifiable.execution.state = TerminalState::Unverifiable;
        assert!(composer_terms(&unverifiable, false, None).stop_aside);
        assert!(!composer_terms(&ended("s"), true, None).stop_aside);
        let mut working = agent("s", "w1", 1);
        working.activity = Some(activity(ActivityState::Working));
        assert!(
            !composer_terms(&working, false, None).stop_aside,
            "the composer's primary button stops it"
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
