//! The delegate form under a running session, on a scripted service: a submit sends `agent.delegate` with its fields
//! and a fresh id, a refusal keeps the input and a retry sends the same id, and the form offers the child it made.
//! Only the agents turned on and detected on this host are offered. No service runs.

use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentDelegated, AgentSession, PackageState};
use massorca_ui::components::text_field::TextField;

use super::delegate::DelegateForm;
use super::service_tests::{Rig, click, draw, rig};
use crate::model::agent_delegate::PROMPT_REQUIRED;
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
    type_into(&rig, cx, |form| &form.prompt, "Write the tests");
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
fn a_refused_submit_keeps_its_input_and_a_retry_sends_the_same_request_id(cx: &mut TestAppContext) {
    let (mut rig, cx) = with_running_session(cx);
    open_form(&rig, cx);
    type_into(&rig, cx, |form| &form.prompt, "Write the tests");
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
    type_into(&rig, cx, |form| &form.prompt, "Write the tests");
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

    type_into(&rig, cx, |form| &form.prompt, "Write more tests");
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
    type_into(&rig, cx, |form| &form.prompt, "Review the diff");
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
