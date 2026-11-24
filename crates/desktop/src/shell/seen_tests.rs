//! Opening an unread session tells the service it was seen (WS-33). A scripted handle shows what was sent.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::{Activity, ActivitySource, ActivityState, Outcome};
use massorca_ui::tokens::{TYPE_BODY, TYPE_BODY_STRONG};

use super::service_tests::{Rig, click, rig};
use super::sidebar_rows::tree_row;
use crate::model::sidebar::{SidebarContent, TreeRow};
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

#[gpui::test]
fn clicking_an_unread_session_row_opens_it_and_marks_it_seen(cx: &mut TestAppContext) {
    let (mut rig, cx) = sessions(cx, true);
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model.snapshot().active_session, None);
    });

    click(cx, "sidebar-session-u");

    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model.snapshot().active_session.as_deref(), Some("u"));
    });
    let Some(Command::MarkSeen { params, .. }) = rig.script.next() else {
        panic!("agent.mark_seen was expected");
    };
    assert_eq!(params.session, "u");
    assert!(rig.script.next().is_none(), "one call only");
}

#[gpui::test]
fn an_unread_session_row_takes_a_strong_title_and_a_read_one_does_not(cx: &mut TestAppContext) {
    let (rig, cx) = sessions(cx, true);
    let styles = rig.shell.update(cx, |shell, cx| {
        let (focus, weak) = (cx.focus_handle(), cx.entity().downgrade());
        let SidebarContent::Workspaces { tree, .. } = shell.model.sidebar() else {
            panic!("the workspaces are listed");
        };
        tree.iter()
            .enumerate()
            .filter_map(|(index, row)| match row {
                TreeRow::Session { id, .. } => Some((
                    id.clone(),
                    tree_row(row, index, &focus, &weak).title_style(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    let style = |id: &str| {
        styles
            .iter()
            .find(|(session, _)| session == id)
            .map(|(_, style)| *style)
    };
    assert_eq!(style("u"), Some(TYPE_BODY_STRONG), "unread");
    assert_eq!(style("s"), Some(TYPE_BODY), "seen");
    assert_eq!(style("w"), Some(TYPE_BODY), "working");
}
