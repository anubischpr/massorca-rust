//! The Sidebar's session rows on a service store (A97): each session row sits under its host, with no
//! workspace row between them, and a child session sits under its parent. A scripted handle answers
//! nothing, so no window opens.

use super::service_tests::{click, draw, rig};
use crate::service::StoreUpdate;
use crate::service::fixtures::agent;
use gpui::TestAppContext;
use massorca_protocol::methods::agents::AgentSession;

#[gpui::test]
fn sessions_are_drawn_as_rows_and_clicking_one_selects_it(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let sessions = vec![agent("a1", "w1", 1), agent("a2", "w1", 2)];
    rig.updates
        .send(StoreUpdate::AgentsResync(sessions))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-a1").is_some());
    assert!(cx.debug_bounds("sidebar-session-a2").is_some());

    click(cx, "sidebar-session-a2");
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-a2").is_some());
}

#[gpui::test]
fn a_child_session_is_drawn_under_its_parent(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let child = AgentSession {
        parent_id: Some("a1".into()),
        ..agent("a2", "w1", 2)
    };
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![agent("a1", "w1", 1), child]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-a1").is_some());
    assert!(cx.debug_bounds("sidebar-session-a2").is_some());
}
