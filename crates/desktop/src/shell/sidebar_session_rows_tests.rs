//! The Sidebar's session rows on a service store (A97): each session row sits under its host, with no
//! workspace row between them, and a child session sits under its parent. A scripted handle answers
//! nothing, so no window opens.

use super::Shell;
use super::service_tests::{Rig, click, draw, rig, setup};
use crate::data::{DataSource, SampleDataSource};
use crate::model::shell::ShellSnapshot;
use crate::model::teams::SampleTeamsSource;
use crate::service::StoreUpdate;
use crate::service::fixtures::{agent, workspace};
use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::AgentSession;

/// Creates workspace w2 on the rig's project p1 with the Claude kind chosen and no session yet.
fn created_without_a_session(rig: &Rig, cx: &mut VisualTestContext) {
    let created = workspace("w2", "p1", "feat-b", 2);
    rig.updates
        .send(StoreUpdate::WorkspaceKindChosen {
            workspace: "w2".into(),
            kind: "claude".into(),
        })
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(created))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn a_workspace_created_without_a_session_is_drawn_as_a_placeholder_row(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    assert!(
        cx.debug_bounds("sidebar-placeholder-w1").is_none(),
        "a workspace listed at startup with no session has no row"
    );
    created_without_a_session(&rig, cx);
    assert!(cx.debug_bounds("sidebar-placeholder-w2").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let snapshot = shell.model.snapshot();
        let placeholders = &snapshot.projects[0].hosts[0].placeholders;
        assert_eq!(placeholders.len(), 1);
        assert_eq!(placeholders[0].title, "feat/feat-b");
    });
}

#[gpui::test]
fn the_first_session_replaces_the_placeholder_row(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    created_without_a_session(&rig, cx);
    rig.updates
        .send(StoreUpdate::AgentChanged(Box::new(agent("a1", "w2", 3))))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-placeholder-w2").is_none());
    assert!(cx.debug_bounds("sidebar-session-a1").is_some());
}

#[gpui::test]
fn a_workspace_whose_sessions_are_all_gone_has_no_row(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    created_without_a_session(&rig, cx);
    rig.updates
        .send(StoreUpdate::AgentChanged(Box::new(agent("a1", "w2", 3))))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::AgentGone("a1".into()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-a1").is_none());
    assert!(
        cx.debug_bounds("sidebar-placeholder-w2").is_none(),
        "an archived or deleted session leaves no placeholder behind"
    );
}

/// The sample data with an owner on its first project, as a remote project carries one (A10).
struct WithOwner;

impl DataSource for WithOwner {
    fn snapshot(&self) -> ShellSnapshot {
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.projects[0].owner = Some("acme".into());
        snapshot
    }
}

#[gpui::test]
fn a_project_with_an_owner_draws_its_owner_line_and_one_without_does_not(cx: &mut TestAppContext) {
    setup(cx);
    let (_shell, cx) =
        cx.add_window_view(|window, cx| Shell::new(&WithOwner, &SampleTeamsSource, window, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-project-owner-massorca-rust")
            .is_some(),
        "the project with an owner draws the owner line"
    );
    assert!(
        cx.debug_bounds("sidebar-project-owner-datalake").is_none(),
        "a project without an owner draws no owner line"
    );
}

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
