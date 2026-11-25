//! The Sidebar's session rows on a service store (A97): each session row sits under its host, with no
//! workspace row between them, and a child session sits under its parent. A scripted handle answers
//! nothing, so no window opens.

use super::Shell;
use super::service_tests::{Rig, click, draw, rig, setup};
use crate::data::{DataSource, SampleDataSource};
use crate::model::canvas::Canvas;
use crate::model::session_menu::SessionMenuId;
use crate::model::shell::ShellSnapshot;
use crate::model::sidebar::{SidebarContent, TreeRow};
use crate::model::teams::SampleTeamsSource;
use crate::service::fixtures::{agent, workspace};
use crate::service::{Command, HostAgents, HostLink, HostSessionsRead, StoreUpdate};
use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
use massorca_client::HostRecord;
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
        .send(StoreUpdate::AgentChanged(agent("a1", "w2", 3)))
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
        .send(StoreUpdate::AgentChanged(agent("a1", "w2", 3)))
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

fn paired_host(name: &str) -> HostRecord {
    HostRecord {
        name: name.to_owned(),
        server: "vps-server".to_owned(),
        addresses: vec!["127.0.0.1:1".to_owned()],
        pin: "sha256:test".to_owned(),
        device_id: "device".to_owned(),
        device_name: "desk".to_owned(),
        scopes: Vec::new(),
        paired_at_ms: 0,
    }
}

/// The tree's session rows of host `vps` as (id, dimmed, state), read from the Sidebar model.
fn host_session_rows(shell: &Shell) -> Vec<(String, bool, crate::model::shell::AgentState)> {
    let SidebarContent::Workspaces { tree, .. } = shell.model.sidebar() else {
        panic!("the workspaces sidebar is shown");
    };
    let host = tree
        .iter()
        .position(|row| matches!(row, TreeRow::RemoteHost { name, .. } if name == "vps"))
        .expect("the paired host is listed");
    tree[host + 1..]
        .iter()
        .map_while(|row| match row {
            TreeRow::Session {
                id, dimmed, state, ..
            } => Some((id.clone(), *dimmed, *state)),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn a_paired_host_lists_its_sessions_under_its_row_and_keeps_them_unverifiable_when_unreached(
    cx: &mut TestAppContext,
) {
    use crate::model::shell::AgentState;
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::HostsResync(vec![paired_host("vps")]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::HostLinkChanged {
            name: "vps".into(),
            link: HostLink::Connected {
                server: "vps-server".into(),
                version: "0.0.0-test".into(),
            },
        })
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::HostSessions(HostSessionsRead {
            host: "vps".into(),
            request: 1,
            answer: Some(HostAgents {
                sessions: vec![agent("r1", "w1", 1)],
                workspaces: vec![workspace("w1", "p1", "feat", 1)],
            }),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-session-r1").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let rows = host_session_rows(shell);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].1, "a reached host is not dimmed");
    });

    rig.updates
        .send(StoreUpdate::HostSessions(HostSessionsRead {
            host: "vps".into(),
            request: 2,
            answer: None,
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-session-r1").is_some(),
        "the last known session stays listed while the host is unreached"
    );
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            host_session_rows(shell),
            vec![("r1".to_owned(), true, AgentState::Unreachable)],
            "dimmed and unverifiable, never exited"
        );
    });
}

/// Pairs host `vps` and lists its session `r1` as reached, so the Sidebar draws the host's row and session.
fn host_with_session(rig: &Rig, cx: &mut VisualTestContext) {
    rig.updates
        .send(StoreUpdate::HostsResync(vec![paired_host("vps")]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::HostLinkChanged {
            name: "vps".into(),
            link: HostLink::Connected {
                server: "vps-server".into(),
                version: "0.0.0-test".into(),
            },
        })
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::HostSessions(HostSessionsRead {
            host: "vps".into(),
            request: 1,
            answer: Some(HostAgents {
                sessions: vec![agent("r1", "w1", 1)],
                workspaces: vec![workspace("w1", "p1", "feat", 1)],
            }),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn a_click_on_a_paired_hosts_session_opens_it_in_the_canvas_with_its_host(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    host_with_session(&rig, cx);
    click(cx, "sidebar-session-r1");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model.open_remote_session(), Some(("vps", "r1")));
        assert!(matches!(shell.model.canvas(), Canvas::Session(_)));
    });
}

#[gpui::test]
fn stop_on_a_paired_hosts_session_reaches_that_host_and_never_this_pcs_service(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    host_with_session(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_remote_session_action("vps", "r1", None, SessionMenuId::Stop, cx)
    });
    cx.run_until_parked();
    let mut commands = Vec::new();
    while let Some(command) = rig.script.next() {
        commands.push(command);
    }
    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::HostAgentStop { host, session, .. } if host == "vps" && session == "r1"
        )),
        "the stop goes to the host's connection"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::AgentStop { .. })),
        "This PC's service is never asked to stop a paired host's session"
    );
}
