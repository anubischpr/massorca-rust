//! Element tests of a session's composer: what it sends for the session (a prompt, a stop, a resume), what it holds
//! while the call is in flight, and what it says about the session's state. A scripted handle stands in for the
//! service, and each test answers its calls by hand.

use super::*;
use crate::model::agent_queue::row_id;
use crate::model::marketplace::UnavailableMarketplace;
use crate::model::teams::{TeamsSource, UnavailableTeams};
use crate::service::fixtures::{agent, agent_kind, project, queued, workspace};
use crate::service::{
    Command, ConnectionState, QueueCommand, Refusal, Script, ServiceHandle, StoreUpdate,
};
use crate::store::ServiceStore;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, AgentKind, AgentModeSet, AgentPromptParams,
    AgentQueueList, AgentSession, AgentSetModeParams, AgentStartParams, ModeApplied, ModeSource,
    PackageState, PromptReceipt, QUEUE_REASON_PAUSED_STOPPED, QueuedPrompt, ReceiptKind,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_ui::components::composer::Composer;
use massorca_ui::components::composer::ComposerEvent;
use massorca_ui::components::queued_message::{QueuedMessages, QueuedMessagesEvent};
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::terminal_surface::feed::FeedEvent;
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

/// A shell on a connected scripted service, with project p1 and workspace w1, these kinds and sessions, and s1 open.
struct Rig {
    shell: Entity<Shell>,
    updates: UnboundedSender<StoreUpdate>,
    script: Script,
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn session_rig(
    cx: &mut TestAppContext,
    kinds: Vec<AgentKind>,
    sessions: Vec<AgentSession>,
) -> (Rig, &mut VisualTestContext) {
    queue_rig(cx, kinds, sessions, None)
}

/// As `session_rig`, on a service that keeps these queues, or none when `queues` is `None`.
fn queue_rig(
    cx: &mut TestAppContext,
    kinds: Vec<AgentKind>,
    sessions: Vec<AgentSession>,
    queues: Option<Vec<AgentQueueList>>,
) -> (Rig, &mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(
            store,
            Some(handle),
            None,
            UnavailableTeams.teams(),
            Box::new(UnavailableMarketplace),
            window,
            cx,
        )
    });
    let listed = [
        StoreUpdate::Connection(ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }),
        StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: vec![workspace("w1", "p1", "fix-login", 1)],
        },
        StoreUpdate::AgentKindsResync(kinds),
        StoreUpdate::AgentsResync(sessions),
        StoreUpdate::AgentQueuesResync(queues),
    ];
    for update in listed {
        updates.send(update).expect("the store listens");
    }
    cx.run_until_parked();
    shell.update(cx, |shell, _| {
        shell.model.open_started_session("s1".to_owned())
    });
    cx.run_until_parked();
    draw(cx);
    (
        Rig {
            shell,
            updates,
            script,
        },
        cx,
    )
}

/// An agent kind of the service's, able or unable to resume.
fn kind(resumable: bool) -> AgentKind {
    let mut kind = agent_kind("claude", "Claude Code", PackageState::Enabled);
    kind.resumable = resumable;
    kind
}

/// Session s1 of kind `claude` in w1: live or ended, and reporting `state` when given.
fn s1(ended: bool, state: Option<ActivityState>) -> AgentSession {
    let mut session = agent("s1", "w1", 10);
    if ended {
        session.execution.state = TerminalState::Exited;
    }
    session.activity = state.map(|state| Activity {
        state,
        outcome: None,
        source: ActivitySource::Hook,
        at_ms: 1,
        seen_at_ms: None,
    });
    session
}

fn composer_of(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell
        .update(cx, |shell, cx| shell.session_composer("s1", cx))
}

fn draft_of(composer: &Entity<Composer>, cx: &VisualTestContext) -> String {
    composer.read_with(cx, |composer, cx| composer.draft(cx))
}

fn reason_of(composer: &Entity<Composer>, cx: &VisualTestContext) -> Option<String> {
    composer.read_with(cx, |composer, _| {
        composer.disabled_reason().map(ToString::to_string)
    })
}

/// Puts `text` in the session's composer and sends it, as Enter does there. A terminal-arm session draws no composer,
/// so the test emits the event the composer emits.
fn type_and_send(rig: &Rig, cx: &mut VisualTestContext, text: &str) {
    let composer = composer_of(rig, cx);
    composer.update(cx, |composer, cx| {
        composer.set_draft(text, cx);
        cx.emit(ComposerEvent::Send(text.to_owned()));
    });
    cx.run_until_parked();
}

/// Puts `text` in the session's composer and queues it, as Send does while a queueing session's agent is busy.
fn type_and_queue(rig: &Rig, cx: &mut VisualTestContext, text: &str) {
    let composer = composer_of(rig, cx);
    composer.update(cx, |composer, cx| {
        composer.set_draft(text, cx);
        cx.emit(ComposerEvent::Queue(text.to_owned()));
    });
    cx.run_until_parked();
}

/// The ended session's bar as the shell offers it: its reason, whether Resume is offered, whether Start a new session
/// here is offered.
fn ended_of(rig: &Rig, cx: &mut VisualTestContext) -> Option<(&'static str, bool, bool)> {
    rig.shell.update(cx, |shell, cx| {
        shell
            .session_frame(cx)
            .and_then(|frame| frame.ended)
            .map(|ended| (ended.reason, ended.resume, ended.start_new))
    })
}

/// Clicks the element `selector` and lets the call it starts reach the script.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::default());
    cx.run_until_parked();
}

/// An `agent.prompt` the shell sent, with the reply that answers it.
type PromptCall = (
    AgentPromptParams,
    oneshot::Sender<Result<PromptReceipt, Refusal>>,
);

/// A session call the shell sent, with the reply that answers it.
type SessionCallReply = (String, oneshot::Sender<Result<AgentSession, Refusal>>);

fn prompt_call(script: &mut Script) -> Option<PromptCall> {
    while let Some(command) = script.next() {
        if let Command::AgentPrompt { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

fn stop_call(script: &mut Script) -> Option<SessionCallReply> {
    while let Some(command) = script.next() {
        if let Command::AgentStop { session, reply } = command {
            return Some((session, reply));
        }
    }
    None
}

fn resume_call(script: &mut Script) -> Option<SessionCallReply> {
    while let Some(command) = script.next() {
        if let Command::AgentResume { params, reply } = command {
            return Some((params.session, reply));
        }
    }
    None
}

fn refused(code: &str, message: &str) -> Refusal {
    Refusal::Failed {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[gpui::test]
fn sending_a_prompt_types_the_text_into_the_session_without_force(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = prompt_call(&mut rig.script).expect("the prompt is sent to the service");
    assert_eq!(params.session, "s1");
    assert_eq!(params.text, "fix the login");
    assert!(
        !params.force,
        "a prompt never overrides the agent's readiness"
    );
}

#[gpui::test]
fn the_draft_stays_read_only_until_the_service_accepts_the_prompt(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (_, reply) = prompt_call(&mut rig.script).expect("the prompt is sent to the service");
    draw(cx);
    let composer = composer_of(&rig, cx);
    assert_eq!(draft_of(&composer, cx), "fix the login", "not accepted yet");
    assert_eq!(reason_of(&composer, cx).as_deref(), Some("Sending…"));
    reply
        .send(Ok(PromptReceipt {
            receipt: ReceiptKind::HandedToTerminal,
            bytes: 15,
        }))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(draft_of(&composer, cx), "", "accepted: the text is gone");
    assert_eq!(reason_of(&composer, cx), None);
}

#[gpui::test]
fn a_refused_prompt_keeps_the_draft_and_shows_the_services_sentence(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (_, reply) = prompt_call(&mut rig.script).expect("the prompt is sent to the service");
    let refusal = refused(
        codes::AGENT_BUSY,
        "session s1 is not ready for a prompt: wait for its agent to report, or use force",
    );
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    let composer = composer_of(&rig, cx);
    let error = composer.read_with(cx, |composer, _| {
        composer.error_reason().map(ToString::to_string)
    });
    assert_eq!(error, Some(refusal.sentence()));
    assert_eq!(draft_of(&composer, cx), "fix the login");
}

#[gpui::test]
fn while_the_agent_works_the_header_stop_stops_it_at_once(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
    );
    assert!(
        cx.debug_bounds("composer-primary").is_none(),
        "a terminal-arm session draws no composer"
    );
    assert!(cx.debug_bounds("button:session-stop").is_some());
    click(cx, "button:session-stop");
    let (session, _reply) =
        stop_call(&mut rig.script).expect("the stop is sent at once, with no confirm");
    assert_eq!(session, "s1");
}

#[gpui::test]
fn a_live_run_without_status_hooks_keeps_a_header_stop(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    click(cx, "button:session-stop");
    let (session, _reply) =
        stop_call(&mut rig.script).expect("the header Stop stops the live run at once");
    assert_eq!(session, "s1");
}

#[gpui::test]
fn a_refused_stop_says_why_above_the_terminal(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
    );
    click(cx, "button:session-stop");
    let (_, reply) = stop_call(&mut rig.script).expect("the stop is sent to the service");
    let refusal = refused(codes::AGENT_NOT_LIVE, "session s1 is not live");
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("session-notice").is_some(),
        "the refusal is drawn"
    );
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(
        notice,
        Some(format!("Could not stop the agent: {}", refusal.sentence()))
    );
}

#[gpui::test]
fn an_ended_resumable_session_offers_resume_and_sends_agent_resume(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(true, Some(ActivityState::Done))],
    );
    assert_eq!(
        ended_of(&rig, cx),
        Some(("This session has ended.", true, false)),
        "the ended terminal offers Resume and nothing else"
    );
    click(cx, "button:session-resume");
    let (session, _reply) =
        resume_call(&mut rig.script).expect("the resume is sent to the service");
    assert_eq!(session, "s1");
}

/// The next `agent.resume` the shell sends, with its whole params.
fn resume_params(
    script: &mut Script,
) -> Option<massorca_protocol::methods::agents::AgentResumeParams> {
    while let Some(command) = script.next() {
        if let Command::AgentResume { params, .. } = command {
            return Some(params);
        }
    }
    None
}

#[gpui::test]
fn a_resume_names_the_session_and_carries_a_pending_bypass_mode(cx: &mut TestAppContext) {
    let mut claude = kind(true);
    claude.permission_modes = vec![massorca_protocol::methods::agents::PermissionMode {
        id: "bypassPermissions".to_owned(),
        label: "Bypass permissions".to_owned(),
        description: None,
        bypass: true,
        live_switch: false,
        may_fall_back: false,
    }];
    let mut session = s1(true, Some(ActivityState::Done));
    session.pending_permission_mode = Some("bypassPermissions".to_owned());
    let (mut rig, cx) = session_rig(cx, vec![claude], vec![session]);
    click(cx, "button:session-resume");
    let params = resume_params(&mut rig.script).expect("the resume is sent to the service");
    assert_eq!(params.session, "s1");
    assert_eq!(params.permission_mode.as_deref(), Some("bypassPermissions"));
}

#[gpui::test]
fn a_resume_starts_the_terminal_at_the_grid_the_session_view_last_drew(cx: &mut TestAppContext) {
    let mut session = s1(true, Some(ActivityState::Done));
    session.terminal_id = Some("t1".to_owned());
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![session]);
    rig.shell.update(cx, |shell, cx| {
        shell.note_terminal_grid("t1", 150, 70, cx);
        // A terminal the session view does not show leaves the size alone.
        shell.note_terminal_grid("t2", 90, 20, cx);
    });
    assert_eq!(
        rig.shell.read_with(cx, |shell, _| shell.session_grid()),
        Some((150, 70))
    );
    click(cx, "button:session-resume");
    let params = resume_params(&mut rig.script).expect("the resume is sent to the service");
    assert_eq!((params.cols, params.rows), (Some(150), Some(70)));
}

#[gpui::test]
fn a_resume_before_any_drawn_grid_starts_at_the_grid_the_conversation_fits(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(true, Some(ActivityState::Done))],
    );
    click(cx, "button:session-resume");
    let params = resume_params(&mut rig.script).expect("the resume is sent to the service");
    let (width, height) = rig
        .shell
        .read_with(cx, |shell, _| shell.conversation_size.get())
        .expect("the canvas measured the Conversation as it drew");
    let cell = cx
        .update(|_, cx| massorca_ui::components::terminal_surface::grid_paint::cell_metrics(cx))
        .expect("the terminal font has cell metrics");
    let fitted = crate::shell::session_grid::session_pane_grid(width, height, cell);
    assert_eq!((params.cols, params.rows), (Some(fitted.0), Some(fitted.1)));
}

#[gpui::test]
fn a_refused_resume_says_why_above_the_composer(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(true, Some(ActivityState::Done))],
    );
    click(cx, "button:session-resume");
    let (session, reply) = resume_call(&mut rig.script).expect("the resume is sent to the service");
    assert_eq!(session, "s1");
    let refusal = refused(codes::AGENT_NOT_LIVE, "the agent is gone");
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(
        notice,
        Some(format!(
            "Could not resume the session: {}",
            refusal.sentence()
        ))
    );
}

#[gpui::test]
fn an_ended_session_that_cannot_resume_says_so_and_offers_a_new_start_only(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![s1(true, None)]);
    assert_eq!(
        ended_of(&rig, cx),
        Some(("This session has ended and cannot be resumed.", false, true))
    );
    assert!(cx.debug_bounds("button:session-resume").is_none());
    assert!(cx.debug_bounds("button:session-start-new").is_some());
    assert!(cx.debug_bounds("button:session-stop").is_none());
}

#[gpui::test]
fn after_a_resume_the_session_view_attaches_the_terminal_of_the_new_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(true, Some(ActivityState::Done))],
    );
    click(cx, "button:session-resume");
    let (_, reply) = resume_call(&mut rig.script).expect("the resume is sent to the service");
    let mut resumed = s1(false, None);
    resumed.terminal_id = Some("t2".to_owned());
    reply
        .send(Ok(resumed.clone()))
        .expect("the shell waits for the answer");
    // The bridge hears the answer and hands the store the session it names.
    rig.updates
        .send(StoreUpdate::AgentChanged(resumed))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.terminal_views.contains_key("t2")),
        "the canvas follows the session to its new terminal"
    );
}

/// s1's queue as the service lists it.
fn s1_queue(items: Vec<QueuedPrompt>) -> Option<Vec<AgentQueueList>> {
    Some(vec![AgentQueueList {
        session: "s1".to_owned(),
        items,
    }])
}

fn dock_of(rig: &Rig, cx: &mut VisualTestContext) -> Entity<QueuedMessages> {
    rig.shell
        .read_with(cx, |shell, _| shell.session_composers["s1"].dock.clone())
}

fn queue_call(script: &mut Script) -> Option<QueueCommand> {
    while let Some(command) = script.next() {
        if let Command::AgentQueue(command) = command {
            return Some(command);
        }
    }
    None
}

#[gpui::test]
fn send_while_the_agent_works_queues_the_text_for_the_session(cx: &mut TestAppContext) {
    let (mut rig, cx) = queue_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
        s1_queue(Vec::new()),
    );
    type_and_queue(&rig, cx, "also add tests");
    cx.run_until_parked();
    let Some(QueueCommand::Add { params, reply }) = queue_call(&mut rig.script) else {
        panic!("the text is queued through agent.queue");
    };
    assert_eq!(params.session, "s1");
    assert_eq!(params.text, "also add tests");
    draw(cx);
    let composer = composer_of(&rig, cx);
    assert_eq!(reason_of(&composer, cx).as_deref(), Some("Queueing…"));
    reply
        .send(Ok(queued("q1", "s1", 1)))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    assert_eq!(
        draft_of(&composer, cx),
        "",
        "accepted: the draft is cleared"
    );
}

#[gpui::test]
fn a_refused_queue_keeps_the_draft_with_the_services_sentence(cx: &mut TestAppContext) {
    let (mut rig, cx) = queue_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
        s1_queue(Vec::new()),
    );
    type_and_queue(&rig, cx, "also add tests");
    cx.run_until_parked();
    let Some(QueueCommand::Add { reply, .. }) = queue_call(&mut rig.script) else {
        panic!("the text is queued through agent.queue");
    };
    let refusal = refused(codes::AGENT_BUSY, "the queue of session s1 is full");
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    let composer = composer_of(&rig, cx);
    let error = composer.read_with(cx, |composer, _| {
        composer.error_reason().map(ToString::to_string)
    });
    assert_eq!(error, Some(refusal.sentence()));
    assert_eq!(draft_of(&composer, cx), "also add tests");
}

/// The texts the dock of s1 holds.
fn dock_texts(rig: &Rig, cx: &mut VisualTestContext) -> Vec<String> {
    dock_of(rig, cx).read_with(cx, |dock, _| {
        dock.items()
            .iter()
            .map(|item| item.text.to_string())
            .collect()
    })
}

#[gpui::test]
fn the_dock_draws_the_stores_queue_and_follows_its_changes(cx: &mut TestAppContext) {
    let (rig, cx) = queue_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
        s1_queue(vec![queued("q1", "s1", 1), queued("q2", "s1", 2)]),
    );
    draw(cx);
    assert!(
        cx.debug_bounds("session-queue").is_some(),
        "the dock is drawn"
    );
    assert_eq!(dock_texts(&rig, cx), ["prompt q1", "prompt q2"]);
    rig.updates
        .send(StoreUpdate::AgentQueueChanged(AgentQueueList {
            session: "s1".to_owned(),
            items: vec![queued("q2", "s1", 2)],
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        dock_texts(&rig, cx),
        ["prompt q2"],
        "the delivered head left"
    );
    rig.updates
        .send(StoreUpdate::AgentQueueChanged(AgentQueueList {
            session: "s1".to_owned(),
            items: Vec::new(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("session-queue").is_none(),
        "an empty queue has no dock"
    );
}

#[gpui::test]
fn the_docks_actions_ask_the_service_by_prompt_id(cx: &mut TestAppContext) {
    let (mut rig, cx) = queue_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
        s1_queue(vec![queued("q1", "s1", 1), queued("q2", "s1", 2)]),
    );
    let dock = dock_of(&rig, cx);
    let emit = |event: QueuedMessagesEvent, cx: &mut VisualTestContext| {
        dock.update(cx, |_, cx| cx.emit(event));
        cx.run_until_parked();
    };
    emit(
        QueuedMessagesEvent::Edited {
            id: row_id("q2"),
            text: "new words".into(),
        },
        cx,
    );
    let Some(QueueCommand::Edit { params, .. }) = queue_call(&mut rig.script) else {
        panic!("an edit is sent as agent.queue.edit");
    };
    assert_eq!(
        (params.id.as_str(), params.text.as_str()),
        ("q2", "new words")
    );
    emit(QueuedMessagesEvent::Removed(row_id("q1")), cx);
    let Some(QueueCommand::Remove { id, .. }) = queue_call(&mut rig.script) else {
        panic!("a removal is sent as agent.queue.remove");
    };
    assert_eq!(id, "q1");
    emit(QueuedMessagesEvent::SendNow(row_id("q2")), cx);
    let Some(QueueCommand::SendNow { id, reply }) = queue_call(&mut rig.script) else {
        panic!("send now is sent as agent.queue.send_now");
    };
    assert_eq!(id, "q2");
    let refusal = refused(codes::AGENT_BUSY, "session s1 is asking a question");
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(
        notice,
        Some(format!(
            "Could not send the queued message: {}",
            refusal.sentence()
        ))
    );
}

#[gpui::test]
fn a_paused_queue_offers_resume_and_sends_agent_queue_resume(cx: &mut TestAppContext) {
    let mut paused = queued("q1", "s1", 1);
    paused.reason = Some(QUEUE_REASON_PAUSED_STOPPED.to_owned());
    let (mut rig, cx) = queue_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Done))],
        s1_queue(vec![paused]),
    );
    draw(cx);
    click(cx, "button:queued-resume");
    let Some(QueueCommand::Resume { session, .. }) = queue_call(&mut rig.script) else {
        panic!("Resume is sent as agent.queue.resume");
    };
    assert_eq!(session, "s1");
}

#[gpui::test]
fn without_the_queue_there_is_no_dock_and_nothing_queues(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
    );
    assert!(cx.debug_bounds("session-queue").is_none());
    type_and_send(&rig, cx, "later");
    cx.run_until_parked();
    assert!(queue_call(&mut rig.script).is_none(), "nothing is queued");
    assert_eq!(draft_of(&composer_of(&rig, cx), cx), "later");
}

/// Claude with two modes, Plan the vendor's own default and a bypass mode.
fn kind_with_modes() -> AgentKind {
    use massorca_protocol::methods::agents::PermissionMode;
    let mode = |id: &str, label: &str, bypass: bool| PermissionMode {
        id: id.into(),
        label: label.into(),
        description: None,
        bypass,
        live_switch: false,
        may_fall_back: false,
    };
    let mut kind = kind(true);
    kind.permission_modes = vec![
        mode("plan", "Plan", false),
        mode("bypassPermissions", "Bypass permissions", true),
    ];
    kind.default_permission_mode = Some("plan".into());
    kind
}

type SetModeCall = (
    AgentSetModeParams,
    oneshot::Sender<Result<AgentModeSet, Refusal>>,
);

fn set_mode_call(script: &mut Script) -> Option<SetModeCall> {
    while let Some(command) = script.next() {
        if let Command::AgentSetMode { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

/// The header's mode chip of session s1.
fn header_chip(rig: &Rig, cx: &VisualTestContext) -> Entity<SelectView> {
    rig.shell
        .read_with(cx, |shell, _| shell.session_mode_chips["s1"].entity.clone())
}

/// The header's mode chip of session s1: its entries, and the selected one.
fn header_chip_of(rig: &Rig, cx: &VisualTestContext) -> (Vec<String>, Option<usize>) {
    rig.shell.read_with(cx, |shell, cx| {
        let chip = &shell.session_mode_chips["s1"];
        (chip.names.clone(), chip.entity.read(cx).selected_index())
    })
}

/// The next `agent.start` the shell sends, with the reply that answers it.
type StartCall = (
    AgentStartParams,
    oneshot::Sender<Result<AgentSession, Refusal>>,
);

fn start_call(script: &mut Script) -> Option<StartCall> {
    while let Some(command) = script.next() {
        if let Command::AgentStart { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

#[gpui::test]
fn an_agent_runs_exit_draws_no_restart_because_its_session_view_answers_it(
    cx: &mut TestAppContext,
) {
    let mut session = s1(false, None);
    session.terminal_id = Some("t1".to_owned());
    let (rig, cx) = session_rig(cx, vec![kind(true)], vec![session]);
    let surface = rig
        .shell
        .read_with(cx, |shell, _| shell.terminal_views["t1"].surface.clone());
    surface.update(cx, |surface, cx| {
        surface
            .apply(FeedEvent::Exited { code: Some(0) }, cx)
            .expect("the exit is applied")
    });
    draw(cx);
    assert!(cx.debug_bounds("terminal-restart").is_none());
}

#[gpui::test]
fn start_a_new_session_here_starts_the_same_agent_in_the_same_workspace(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![s1(true, None)]);
    click(cx, "button:session-start-new");
    let (params, reply) =
        start_call(&mut rig.script).expect("the new session is started through agent.start");
    assert_eq!(params.workspace.as_deref(), Some("w1"));
    assert_eq!(params.kind, "claude");
    assert_eq!(
        params.prompt, None,
        "no prompt or saved conversation carries over"
    );
    let started = agent("s2", "w1", 20);
    reply
        .send(Ok(started.clone()))
        .expect("the shell waits for the answer");
    rig.updates
        .send(StoreUpdate::AgentChanged(started))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    let active = rig
        .shell
        .read_with(cx, |shell, _| shell.model.snapshot().active_session.clone());
    assert_eq!(active.as_deref(), Some("s2"), "the new session opens here");
}

#[gpui::test]
fn a_refused_start_a_new_session_says_why_above_the_terminal(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![s1(true, None)]);
    click(cx, "button:session-start-new");
    let (_, reply) = start_call(&mut rig.script).expect("the start is sent to the service");
    let refusal = refused(codes::AGENT_BUSY, "the workspace is busy");
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("session-notice").is_some());
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(
        notice,
        Some(format!(
            "Could not start a new session: {}",
            refusal.sentence()
        ))
    );
}

#[gpui::test]
fn a_resume_refused_for_a_missing_conversation_says_so_and_offers_a_new_start(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(true, Some(ActivityState::Done))],
    );
    click(cx, "button:session-resume");
    let (_, reply) = resume_call(&mut rig.script).expect("the resume is sent to the service");
    reply
        .send(Err(refused(
            codes::AGENT_RESUME_UNAVAILABLE,
            "its agent ref is not known",
        )))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        ended_of(&rig, cx),
        Some(("The saved conversation could not be found.", false, true))
    );
    assert!(
        cx.debug_bounds("session-notice").is_none(),
        "the reason replaces the refusal sentence"
    );
}

#[gpui::test]
fn a_session_in_the_vendors_default_shows_it_and_a_pick_asks_the_service(cx: &mut TestAppContext) {
    let mut session = s1(false, None);
    session.mode_source = Some(ModeSource::VendorDefault);
    let (mut rig, cx) = session_rig(cx, vec![kind_with_modes()], vec![session.clone()]);
    assert_eq!(
        header_chip_of(&rig, cx),
        (
            vec![
                "The CLI's default (Plan)".to_owned(),
                "Plan".to_owned(),
                "Bypass permissions".to_owned()
            ],
            Some(0)
        )
    );
    assert!(cx.debug_bounds("select-trigger").is_some());
    assert!(cx.debug_bounds("composer-primary").is_none());
    assert!(cx.debug_bounds("session-arm").is_some());

    header_chip(&rig, cx).update(cx, |_, cx| cx.emit(SelectEvent::Changed(1)));
    cx.run_until_parked();
    let (params, reply) =
        set_mode_call(&mut rig.script).expect("the switch is sent to the service");
    assert_eq!(params.session, "s1");
    assert_eq!(params.permission_mode, "plan");
    assert!(!params.force);

    session.pending_permission_mode = Some("plan".into());
    reply
        .send(Ok(AgentModeSet {
            applied: ModeApplied::NextResume,
            session: session.clone(),
        }))
        .expect("the shell waits for the answer");
    rig.updates
        .send(StoreUpdate::AgentChanged(session))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        header_chip_of(&rig, cx).1,
        Some(0),
        "the running mode is unchanged"
    );
    assert!(cx.debug_bounds("session-mode-note").is_some());
}

#[gpui::test]
fn picking_the_running_mode_or_the_clis_default_sends_nothing(cx: &mut TestAppContext) {
    let mut session = s1(false, None);
    session.mode_source = Some(ModeSource::VendorDefault);
    let (mut rig, cx) = session_rig(cx, vec![kind_with_modes()], vec![session]);
    header_chip(&rig, cx).update(cx, |_, cx| cx.emit(SelectEvent::Changed(0)));
    cx.run_until_parked();
    assert!(set_mode_call(&mut rig.script).is_none());
}

#[gpui::test]
fn a_refused_switch_says_why_above_the_composer(cx: &mut TestAppContext) {
    let mut session = s1(false, None);
    session.permission_mode = Some("plan".into());
    session.mode_source = Some(ModeSource::Chosen);
    session.mode_confirmed = true;
    let (mut rig, cx) = session_rig(cx, vec![kind_with_modes()], vec![session]);
    assert_eq!(header_chip_of(&rig, cx).1, Some(0));
    header_chip(&rig, cx).update(cx, |_, cx| cx.emit(SelectEvent::Changed(1)));
    cx.run_until_parked();
    let (params, reply) =
        set_mode_call(&mut rig.script).expect("the switch is sent to the service");
    assert_eq!(params.permission_mode, "bypassPermissions");
    let refusal = refused(codes::AGENT_MODE_UNAVAILABLE, "mode cannot be applied");
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(
        notice,
        Some(format!("Could not change the mode: {}", refusal.sentence()))
    );
    assert_eq!(
        header_chip_of(&rig, cx).1,
        Some(0),
        "the chip keeps the service's mode"
    );
}

#[gpui::test]
fn a_kind_without_modes_has_no_chip_in_the_session(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    assert!(
        !rig.shell
            .read_with(cx, |shell, _| shell.session_mode_chips.contains_key("s1"))
    );
    assert!(cx.debug_bounds("select-trigger").is_none());
}

/// The mode the next `agent.resume` names.
fn resume_mode(script: &mut Script) -> Option<Option<String>> {
    while let Some(command) = script.next() {
        if let Command::AgentResume { params, .. } = command {
            return Some(params.permission_mode);
        }
    }
    None
}

#[gpui::test]
fn resume_names_a_pending_bypass_mode_and_leaves_any_other_to_the_service(cx: &mut TestAppContext) {
    let mut session = s1(true, Some(ActivityState::Done));
    session.permission_mode = Some("plan".into());
    session.mode_source = Some(ModeSource::Chosen);
    session.pending_permission_mode = Some("bypassPermissions".into());
    let (mut rig, cx) = session_rig(cx, vec![kind_with_modes()], vec![session.clone()]);
    click(cx, "button:session-resume");
    assert_eq!(
        resume_mode(&mut rig.script),
        Some(Some("bypassPermissions".to_owned()))
    );

    let mut other = s1(true, Some(ActivityState::Done));
    other.permission_mode = Some("bypassPermissions".into());
    other.mode_source = Some(ModeSource::Chosen);
    other.pending_permission_mode = Some("plan".into());
    rig.updates
        .send(StoreUpdate::AgentChanged(other))
        .expect("the store listens");
    rig.shell.update(cx, |shell, _| {
        shell.session_calls.remove("s1");
    });
    cx.run_until_parked();
    draw(cx);
    click(cx, "button:session-resume");
    assert_eq!(resume_mode(&mut rig.script), Some(None));
}

/// The transcript a chat session holds after its first exchange: the person's ask and the agent's answer.
fn chat_transcript() -> crate::model::chat_transcript::ChatTranscript {
    use massorca_protocol::methods::chat::{
        ChatCursor, ChatEvent, ChatItem, ChatItemBody, ChatSnapshot, MessageState,
    };

    let item = |id: &str, position: u64, body: ChatItemBody| ChatItem {
        id: id.to_owned(),
        position,
        seq: position,
        turn: None,
        parent: None,
        at_ms: 0,
        body,
    };
    let mut transcript = crate::model::chat_transcript::ChatTranscript::default();
    transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
        cursor: ChatCursor {
            epoch: "epoch-1".to_owned(),
            seq: 2,
        },
        items: vec![
            item(
                "ask",
                1,
                ChatItemBody::UserMessage {
                    text: "Fix the parser.".to_owned(),
                },
            ),
            item(
                "answer",
                2,
                ChatItemBody::AssistantMessage {
                    text: "Reading it now.".to_owned(),
                    state: MessageState::Completed,
                },
            ),
        ],
        has_older: false,
        latest_turn: None,
        activity: None,
    })));
    transcript
}

#[gpui::test]
fn a_chat_arm_session_draws_its_transcript_and_composer_and_no_terminal(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![s1(false, None)]);
    rig.shell.update(cx, |shell, _| {
        shell
            .chat_transcripts
            .insert("s1".to_owned(), chat_transcript());
    });
    draw(cx);

    assert!(cx.debug_bounds("transcript-list").is_some());
    assert!(cx.debug_bounds("transcript-row-ask").is_some());
    assert!(cx.debug_bounds("composer-primary").is_some());
    assert!(cx.debug_bounds("session-terminal").is_none());
}
