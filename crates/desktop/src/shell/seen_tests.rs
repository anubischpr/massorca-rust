//! Opening an unread session tells the service it was seen (WS-33). A scripted handle shows what was sent.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::{Activity, ActivitySource, ActivityState, Outcome};

use super::service_tests::{Rig, rig};
use crate::service::fixtures::agent;
use crate::service::{Command, StoreUpdate};

fn activity(state: ActivityState, seen_at_ms: Option<i64>) -> Activity {
    Activity {
        state,
        outcome: (state == ActivityState::Done).then_some(Outcome::Success),
        source: ActivitySource::Hook,
        at_ms: 7,
        seen_at_ms,
    }
}

/// A connected shell listing a done session `u`, a seen one `s` and a working one `w`.
fn sessions(cx: &mut TestAppContext, offered: bool) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let mut unread = agent("u", "w1", 10);
    unread.activity = Some(activity(ActivityState::Done, None));
    let mut seen = agent("s", "w1", 11);
    seen.activity = Some(activity(ActivityState::Done, Some(8)));
    let mut working = agent("w", "w1", 12);
    working.activity = Some(activity(ActivityState::Working, None));
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![unread, seen, working]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::AgentSeenOffered(offered))
        .expect("the store listens");
    cx.run_until_parked();
    (rig, cx)
}

#[gpui::test]
fn opening_an_unread_session_marks_the_activity_shown_as_seen(cx: &mut TestAppContext) {
    let (mut rig, cx) = sessions(cx, true);
    rig.shell
        .update(cx, |shell, cx| shell.open_session("u", cx));
    cx.run_until_parked();

    let Some(Command::MarkSeen { params, .. }) = rig.script.next() else {
        panic!("agent.mark_seen was expected");
    };
    assert_eq!(params.session, "u");
    assert_eq!(params.at_ms, Some(7));
    assert!(rig.script.next().is_none(), "one call only");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model.snapshot().active_session.as_deref(), Some("u"));
    });
}

#[gpui::test]
fn opening_a_seen_or_working_session_sends_nothing(cx: &mut TestAppContext) {
    let (mut rig, cx) = sessions(cx, true);
    rig.shell.update(cx, |shell, cx| {
        shell.open_session("s", cx);
        shell.open_session("w", cx);
    });
    cx.run_until_parked();

    assert!(rig.script.next().is_none());
}

#[gpui::test]
fn without_the_capability_nothing_is_sent_and_the_session_still_opens(cx: &mut TestAppContext) {
    let (mut rig, cx) = sessions(cx, false);
    rig.shell
        .update(cx, |shell, cx| shell.open_session("u", cx));
    cx.run_until_parked();

    assert!(rig.script.next().is_none());
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model.snapshot().active_session.as_deref(), Some("u"));
    });
}
