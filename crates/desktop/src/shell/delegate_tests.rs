//! The delegate form under a running session, on a scripted service: a submit sends `agent.delegate` with its fields
//! and a fresh id, a refusal shows its reason and keeps the input, the next submit sends the same id, and the form
//! offers the child it made.
//! Only the agents turned on and detected on this host are offered. No service runs.

use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentDelegated, AgentSession, PackageState};
use massorca_ui::components::text_field::TextField;

use super::delegate::DelegateForm;
use super::service_tests::{Rig, click, draw, rig};
use crate::model::agent_delegate::{PROMPT_REQUIRED, SessionDelegation, WritingChild};
use crate::service::fixtures::{agent, agent_kind};
use crate::service::{Command, Refusal, StoreUpdate};

/// A connected shell with one running session `s` in workspace `w1`, and the agents the host offers: Claude and
/// Codex turned on, Gemini turned off, Aider not installed.
fn with_running_session(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Enabled),
            agent_kind("gemini", "Gemini", PackageState::Disabled),
            agent_kind("aider", "Aider", PackageState::Available),
        ]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![agent("s", "w1", 10)]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

fn open_form(rig: &Rig, cx: &mut VisualTestContext) {
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_delegate("s", window, cx));
    draw(cx);
}

/// Declares the open form's session delegation, as a parent session that carries the field would.
fn declare(rig: &Rig, cx: &mut VisualTestContext, delegation: SessionDelegation) {
    rig.shell.update(cx, |shell, cx| {
        if let Some(form) = shell.delegate.as_mut() {
            form.delegation = Some(delegation);
        }
        cx.notify();
    });
    draw(cx);
}

/// A delegation whose writing child is `writing_child`, with a read-only child allowed and no reason.
fn writing(writing_child: WritingChild) -> SessionDelegation {
    SessionDelegation {
        writing_child,
        read_only_child: true,
        reason: None,
    }
}

/// Types `text` into one of the form's fields through the field's own entity, as the person's typing does.
fn type_into(
    rig: &Rig,
    cx: &mut VisualTestContext,
    field: impl Fn(&DelegateForm) -> &Entity<TextField>,
    text: &'static str,
) {
    rig.shell.update_in(cx, |shell, window, cx| {
        let form = shell.delegate.as_ref().expect("the delegate form is open");
        field(form).update(cx, |text_field, cx| text_field.set_text(text, window, cx));
    });
}

/// Types `text` into the form's multi-line prompt.
fn type_prompt(rig: &Rig, cx: &mut VisualTestContext, text: &'static str) {
    rig.shell.update_in(cx, |shell, window, cx| {
        let form = shell.delegate.as_ref().expect("the delegate form is open");
        form.prompt
            .update(cx, |area, cx| area.set_text(text, window, cx));
    });
}

/// The child the service made for a delegate call from `s`.
fn started_child() -> AgentDelegated {
    AgentDelegated {
        session: AgentSession {
            parent_id: Some("s".to_owned()),
            title: Some("Write the tests".to_owned()),
            ..agent("c", "w1", 20)
        },
        note: None,
    }
}

#[gpui::test]
fn a_submit_sends_its_fields_with_a_new_request_id_and_then_offers_the_child(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    type_prompt(&rig, cx, "Write the tests");
    type_into(&rig, cx, |form| &form.title, "Feed endpoints");
    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_kind(1, cx));
    click(cx, "dialog-action");

    let Some(Command::Delegate { params, reply }) = rig.script.next() else {
        panic!("an agent.delegate call was expected");
    };
    assert_eq!(
        (
            params.parent.as_str(),
            params.prompt.as_str(),
            params.kind.as_deref()
        ),
        ("s", "Write the tests", Some("codex"))
    );
    assert_eq!(
        (
            params.name.as_deref(),
            params.title.as_deref(),
            params.read_only
        ),
        (None, Some("Feed endpoints"), false)
    );
    assert!(
        params
            .request_id
            .as_deref()
            .is_some_and(|id| !id.is_empty())
    );
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.delegate.as_ref().expect("open").sending);
    });

    reply.send(Ok(started_child())).expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let started = shell
            .delegate
            .as_ref()
            .and_then(|form| form.started.as_ref());
        assert_eq!(started.map(|child| child.id.as_str()), Some("c"));
    });

    click(cx, "dialog-action");
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.delegate.is_none(),
            "the form closes when its child opens"
        );
    });
}

#[gpui::test]
fn a_refusal_shows_its_reason_and_keeps_delegate_live_with_the_same_request_id(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    type_prompt(&rig, cx, "Write the tests");
    click(cx, "dialog-action");
    let Some(Command::Delegate {
        params: first,
        reply,
    }) = rig.script.next()
    else {
        panic!("an agent.delegate call was expected");
    };
    reply
        .send(Err(Refusal::Failed {
            code: codes::AGENT_DELEGATE_NEEDS_GIT.to_owned(),
            message: "The parent workspace has no branch to start from.".to_owned(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, app| {
        let form = shell.delegate.as_ref().expect("the form stays open");
        assert_eq!(form.prompt.read(app).text(), "Write the tests");
        assert_eq!(
            form.error.as_deref(),
            Some("The parent workspace has no branch to start from.")
        );
        assert!(!form.sending);
    });

    click(cx, "dialog-action");
    let Some(Command::Delegate {
        params: retry,
        reply,
    }) = rig.script.next()
    else {
        panic!("a second agent.delegate call was expected");
    };
    assert!(first.request_id.is_some());
    assert_eq!(retry.request_id, first.request_id);
    reply.send(Ok(started_child())).expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell
                .delegate
                .as_ref()
                .is_some_and(|form| form.started.is_some())
        );
    });
}

#[gpui::test]
fn an_edit_after_a_refusal_makes_the_next_submit_a_new_call(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    type_prompt(&rig, cx, "Write the tests");
    click(cx, "dialog-action");
    let Some(Command::Delegate {
        params: first,
        reply,
    }) = rig.script.next()
    else {
        panic!("an agent.delegate call was expected");
    };
    reply
        .send(Err(Refusal::unavailable("The service is not connected.")))
        .expect("the shell waits");
    cx.run_until_parked();

    type_prompt(&rig, cx, "Write more tests");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.delegate.as_ref().expect("open").error,
            None,
            "an edit clears the refusal"
        );
    });

    click(cx, "dialog-action");
    let Some(Command::Delegate { params: second, .. }) = rig.script.next() else {
        panic!("a second agent.delegate call was expected");
    };
    assert_ne!(second.request_id, first.request_id);
}

#[gpui::test]
fn a_read_only_child_is_sent_without_a_name(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    type_prompt(&rig, cx, "Review the diff");
    type_into(&rig, cx, |form| &form.name, "docs");
    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_read_only(true, cx));
    click(cx, "dialog-action");

    let Some(Command::Delegate { params, .. }) = rig.script.next() else {
        panic!("an agent.delegate call was expected");
    };
    assert_eq!((params.read_only, params.name), (true, None));
}

#[gpui::test]
fn the_agents_offered_are_the_ones_turned_on_and_detected_here(cx: &mut TestAppContext) {
    let (rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    rig.shell.read_with(cx, |shell, _| {
        let form = shell.delegate.as_ref().expect("the form is open");
        let offered: Vec<&str> = form.kinds.iter().map(|kind| kind.id.as_str()).collect();
        assert_eq!(offered, ["claude", "codex"]);
        assert_eq!(form.kind, Some(0), "the form starts on the parent's agent");
    });
}

#[gpui::test]
fn a_blank_prompt_is_not_sent(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    rig.shell.update(cx, |shell, cx| shell.submit_delegate(cx));
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell
                .delegate
                .as_ref()
                .and_then(|form| form.error.as_deref()),
            Some(PROMPT_REQUIRED)
        );
    });
    assert!(rig.script.next().is_none(), "no call reaches the service");
}

#[gpui::test]
fn enter_adds_a_line_to_the_prompt_and_mod_enter_sends_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    rig.shell.update_in(cx, |shell, window, cx| {
        let form = shell.delegate.as_ref().expect("the delegate form is open");
        form.prompt.update(cx, |area, cx| area.focus(window, cx));
    });
    cx.run_until_parked();
    cx.simulate_input("Write the tests");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("for the feed");
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "Enter adds a line and sends nothing"
    );
    let typed = rig.shell.read_with(cx, |shell, app| {
        let form = shell.delegate.as_ref().expect("the delegate form is open");
        form.prompt.read(app).text().to_owned()
    });
    assert_eq!(
        typed,
        "Write the tests
for the feed"
    );
    draw(cx);
    assert!(
        cx.debug_bounds("delegate-prompt-hint").is_some(),
        "the hint says how to send"
    );
    let send = if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    };
    cx.simulate_keystrokes(send);
    cx.run_until_parked();
    let Some(Command::Delegate { params, .. }) = rig.script.next() else {
        panic!("the modified Enter sends agent.delegate");
    };
    assert_eq!(
        params.prompt,
        "Write the tests
for the feed"
    );
}

#[gpui::test]
fn an_own_worktree_child_is_described_before_the_person_types(cx: &mut TestAppContext) {
    let (rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    declare(&rig, cx, writing(WritingChild::OwnWorktree));
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the placement shows as the form opens"
    );
    rig.shell.read_with(cx, |shell, app| {
        let form = shell.delegate.as_ref().expect("the form is open");
        assert!(
            form.prompt.read(app).text().is_empty(),
            "nothing is typed yet"
        );
    });
}

#[gpui::test]
fn a_shared_folder_child_is_described_before_the_person_types(cx: &mut TestAppContext) {
    let (rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    declare(&rig, cx, writing(WritingChild::SharedFolder));
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the placement shows as the form opens"
    );
    rig.shell.read_with(cx, |shell, app| {
        let form = shell.delegate.as_ref().expect("the form is open");
        assert!(
            form.prompt.read(app).text().is_empty(),
            "nothing is typed yet"
        );
    });
}

#[gpui::test]
fn a_read_only_child_gets_no_isolation_line_and_a_writing_one_gets_it_back(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    declare(&rig, cx, writing(WritingChild::SharedFolder));
    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_read_only(true, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_none(),
        "a read-only child needs no isolation line"
    );
    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_read_only(false, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the line returns for a child that may write"
    );
}

#[gpui::test]
fn a_session_that_declares_no_placement_shows_no_line_and_sends_as_before(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_none(),
        "no session declares a placement yet"
    );
    type_prompt(&rig, cx, "Write the tests");
    click(cx, "dialog-action");
    let Some(Command::Delegate { params, .. }) = rig.script.next() else {
        panic!("an agent.delegate call was expected");
    };
    assert_eq!(params.prompt, "Write the tests");
}

#[gpui::test]
fn a_writing_unavailable_session_sends_a_read_only_child_only_with_read_only_on(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    declare(
        &rig,
        cx,
        SessionDelegation {
            writing_child: WritingChild::Unavailable,
            read_only_child: true,
            reason: Some("Writing is paused in this session.".to_owned()),
        },
    );
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the reason shows before typing"
    );
    type_prompt(&rig, cx, "Review the diff");
    click(cx, "dialog-action");
    rig.shell.update(cx, |shell, cx| shell.submit_delegate(cx));
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "with Read only off, a writing child cannot start"
    );

    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_read_only(true, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the reason stays with Read only on"
    );
    click(cx, "dialog-action");
    let Some(Command::Delegate { params, .. }) = rig.script.next() else {
        panic!("with Read only on, the session's read-only child is sent");
    };
    assert_eq!((params.read_only, params.name), (true, None));
}

#[gpui::test]
fn a_writing_unavailable_session_without_read_only_children_keeps_delegate_off(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    declare(
        &rig,
        cx,
        SessionDelegation {
            writing_child: WritingChild::Unavailable,
            read_only_child: false,
            reason: Some("This session has reached its delegation depth.".to_owned()),
        },
    );
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the reason shows before typing"
    );
    type_prompt(&rig, cx, "Review the diff");
    click(cx, "dialog-action");
    rig.shell.update(cx, |shell, cx| shell.submit_delegate(cx));
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "with Read only off, nothing can start"
    );

    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_read_only(true, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the reason stays with Read only on"
    );
    click(cx, "dialog-action");
    rig.shell.update(cx, |shell, cx| shell.submit_delegate(cx));
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "with Read only on, the session still starts no child"
    );
}

#[gpui::test]
fn an_own_worktree_session_that_starts_no_child_keeps_delegate_off(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    declare(
        &rig,
        cx,
        SessionDelegation {
            writing_child: WritingChild::OwnWorktree,
            read_only_child: false,
            reason: Some("This session has reached its delegation depth.".to_owned()),
        },
    );
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the reason shows before typing"
    );
    type_prompt(&rig, cx, "Write the tests");
    click(cx, "dialog-action");
    rig.shell.update(cx, |shell, cx| shell.submit_delegate(cx));
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "with Read only off, no child starts"
    );

    rig.shell
        .update(cx, |shell, cx| shell.set_delegate_read_only(true, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("delegate-placement-notice").is_some(),
        "the reason stays with Read only on"
    );
    click(cx, "dialog-action");
    rig.shell.update(cx, |shell, cx| shell.submit_delegate(cx));
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "with Read only on, no child starts either"
    );
}
