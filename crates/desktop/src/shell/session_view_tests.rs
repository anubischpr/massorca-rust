//! Element tests of a session's composer: what it sends for the session (a prompt, a stop, a resume), what it holds
//! while the call is in flight, and what it says about the session's state. A scripted handle stands in for the
//! service, and each test answers its calls by hand.

use super::*;
use crate::model::marketplace::UnavailableMarketplace;
use crate::model::teams::{TeamsSource, UnavailableTeams};
use crate::service::fixtures::{agent, agent_kind, project, workspace};
use crate::service::{Command, ConnectionState, Refusal, Script, ServiceHandle, StoreUpdate};
use crate::store::ServiceStore;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, AgentKind, AgentModeSet, AgentPromptParams,
    AgentSession, AgentSetModeParams, ModeApplied, ModeSource, PackageState, PromptReceipt,
    ReceiptKind,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_ui::components::composer::Composer;
use massorca_ui::components::composer::ComposerEvent;
use massorca_ui::components::composer::policy::PrimaryAction;
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

/// Focuses the session's composer, types `text` and presses Enter.
fn type_and_send(rig: &Rig, cx: &mut VisualTestContext, text: &str) {
    let composer = composer_of(rig, cx);
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    cx.run_until_parked();
    cx.simulate_input(text);
    cx.simulate_keystrokes("enter");
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
fn while_the_agent_works_stop_replaces_send_and_stops_it_at_once(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
    );
    let composer = composer_of(&rig, cx);
    assert_eq!(
        composer.read_with(cx, |composer, cx| composer.primary_action(cx)),
        PrimaryAction::Stop
    );
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    cx.simulate_input("later");
    cx.run_until_parked();
    assert_eq!(
        composer.read_with(cx, |composer, cx| composer.primary_action(cx)),
        PrimaryAction::Stop,
        "a draft cannot queue into a running agent"
    );
    click(cx, "composer-primary");
    let (session, _reply) =
        stop_call(&mut rig.script).expect("the stop is sent at once, with no confirm");
    assert_eq!(session, "s1");
}

#[gpui::test]
fn a_live_run_without_status_hooks_keeps_send_and_a_header_stop(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    let composer = composer_of(&rig, cx);
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    cx.simulate_input("status?");
    cx.run_until_parked();
    assert_eq!(
        composer.read_with(cx, |composer, cx| composer.primary_action(cx)),
        PrimaryAction::Send,
        "nothing says the agent works, so Send stays primary"
    );
    draw(cx);
    click(cx, "button:session-stop");
    let (session, _reply) =
        stop_call(&mut rig.script).expect("the header Stop stops the live run at once");
    assert_eq!(session, "s1");
}

#[gpui::test]
fn the_header_stop_is_drawn_only_for_a_live_run_the_composer_does_not_stop(
    cx: &mut TestAppContext,
) {
    let (_rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
    );
    draw(cx);
    assert!(cx.debug_bounds("button:session-stop").is_none());
}

#[gpui::test]
fn a_refused_stop_says_why_above_the_composer(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(false, Some(ActivityState::Working))],
    );
    click(cx, "composer-primary");
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
    let composer = composer_of(&rig, cx);
    assert_eq!(
        reason_of(&composer, cx).as_deref(),
        Some("This session has ended.")
    );
    assert!(composer.read_with(cx, |composer, _| composer.resume_offered()));
    click(cx, "composer-resume");
    let (session, _reply) =
        resume_call(&mut rig.script).expect("the resume is sent to the service");
    assert_eq!(session, "s1");
}

#[gpui::test]
fn an_ended_session_that_cannot_resume_says_so_and_offers_no_control(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![s1(true, None)]);
    let composer = composer_of(&rig, cx);
    assert_eq!(
        reason_of(&composer, cx).as_deref(),
        Some("This session has ended and cannot be resumed.")
    );
    assert!(!composer.read_with(cx, |composer, _| composer.resume_offered()));
}

#[gpui::test]
fn after_a_resume_the_session_view_attaches_the_terminal_of_the_new_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(
        cx,
        vec![kind(true)],
        vec![s1(true, Some(ActivityState::Done))],
    );
    click(cx, "composer-resume");
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

fn chip_of(composer: &Entity<Composer>, cx: &VisualTestContext) -> (Vec<String>, usize) {
    composer.read_with(cx, |composer, _| {
        (
            composer
                .access_modes()
                .iter()
                .map(|mode| mode.name.to_string())
                .collect(),
            composer.access_index(),
        )
    })
}

#[gpui::test]
fn a_session_in_the_vendors_default_shows_it_and_a_pick_asks_the_service(cx: &mut TestAppContext) {
    let mut session = s1(false, None);
    session.mode_source = Some(ModeSource::VendorDefault);
    let (mut rig, cx) = session_rig(cx, vec![kind_with_modes()], vec![session.clone()]);
    let composer = composer_of(&rig, cx);
    assert_eq!(
        chip_of(&composer, cx),
        (
            vec![
                "The CLI's default (Plan)".to_owned(),
                "Plan".to_owned(),
                "Bypass permissions".to_owned()
            ],
            0
        )
    );
    assert!(cx.debug_bounds("composer-access").is_some());

    composer.update(cx, |_, cx| cx.emit(ComposerEvent::AccessChanged(1)));
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
    assert_eq!(chip_of(&composer, cx).1, 0, "the running mode is unchanged");
    assert!(cx.debug_bounds("composer-access-note").is_some());
}

#[gpui::test]
fn picking_the_running_mode_or_the_clis_default_sends_nothing(cx: &mut TestAppContext) {
    let mut session = s1(false, None);
    session.mode_source = Some(ModeSource::VendorDefault);
    let (mut rig, cx) = session_rig(cx, vec![kind_with_modes()], vec![session]);
    let composer = composer_of(&rig, cx);
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::AccessChanged(0)));
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
    let composer = composer_of(&rig, cx);
    assert_eq!(chip_of(&composer, cx).1, 0);
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::AccessChanged(1)));
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
        chip_of(&composer, cx).1,
        0,
        "the chip keeps the service's mode"
    );
}

#[gpui::test]
fn a_kind_without_modes_has_no_chip_in_the_session(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    let composer = composer_of(&rig, cx);
    assert!(chip_of(&composer, cx).0.is_empty());
    assert!(cx.debug_bounds("composer-access").is_none());
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
    click(cx, "composer-resume");
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
    click(cx, "composer-resume");
    assert_eq!(resume_mode(&mut rig.script), Some(None));
}
