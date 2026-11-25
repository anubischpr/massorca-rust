//! The Sidebar's session rows on a service store (A97): each session row sits under its host, with no
//! workspace row between them, and a child session sits under its parent. A scripted handle answers
//! nothing, so no window opens.

use super::service_tests::{click, draw, rig};
use crate::service::StoreUpdate;
use crate::service::fixtures::agent;
use gpui::{Modifiers, MouseButton, TestAppContext};
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

#[gpui::test]
fn right_clicking_a_session_row_opens_its_menu(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![agent("a1", "w1", 1)]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    let row = cx
        .debug_bounds("sidebar-session-a1")
        .expect("the session row is drawn");
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(row.center(), MouseButton::Right, Modifiers::default());
    draw(cx);
    assert!(cx.debug_bounds("menu-panel").is_some());
}

#[gpui::test]
fn clicking_a_project_row_folds_its_sessions_and_clicking_again_shows_them(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![agent("a1", "w1", 1)]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-a1").is_some());

    click(cx, "sidebar-project-p1");
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-a1").is_none(), "collapsed");
    assert!(
        cx.debug_bounds("sidebar-project-p1").is_some(),
        "the project row stays"
    );

    click(cx, "sidebar-project-p1");
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-session-a1").is_some(),
        "expanded again"
    );
}
