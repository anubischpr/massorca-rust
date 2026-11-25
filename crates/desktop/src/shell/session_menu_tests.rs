//! A session row's menu (A97): the rows it offers, and what Stop, Copy path and Reveal in Work panel reach. The
//! scripted service of the composer tests stands in for the service.

use super::session_view_tests::{Rig, draw, session_rig, setup, stop_call};
use super::workspace_dialogs::MenuTarget;
use crate::model::regions::Region;
use crate::model::session_menu::SessionMenuId;
use crate::service::fixtures::{agent, agent_kind};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::{AgentKind, AgentSession, PackageState};
use massorca_protocol::methods::terminals::TerminalState;

fn kinds() -> Vec<AgentKind> {
    vec![agent_kind("claude", "Claude Code", PackageState::Enabled)]
}

/// Session s1 in w1, running, with its terminal.
fn running_s1() -> AgentSession {
    let mut session = agent("s1", "w1", 10);
    session.terminal_id = Some("t1".to_owned());
    session
}

/// Opens the menu of the row of `session` (None for a workspace with no session) and returns the rows it offers.
fn open_menu(
    rig: &Rig,
    cx: &mut VisualTestContext,
    session: Option<&str>,
    local: bool,
) -> Vec<SessionMenuId> {
    let session = session.map(str::to_owned);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_session_menu(session, "w1".to_owned(), local, window, cx);
    });
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        match shell.dialogs.menu.as_ref().map(|menu| &menu.target) {
            Some(MenuTarget::Session { rows, .. }) => rows.clone(),
            _ => panic!("the session menu is not open"),
        }
    })
}

#[gpui::test]
fn a_running_session_offers_stop_copy_open_folder_and_reveal(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![running_s1()]);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), true),
        [
            SessionMenuId::Stop,
            SessionMenuId::CopyPath,
            SessionMenuId::OpenFolder,
            SessionMenuId::RevealInWorkPanel,
        ]
    );
}

#[gpui::test]
fn an_ended_session_offers_no_stop(cx: &mut TestAppContext) {
    setup(cx);
    let mut session = running_s1();
    session.execution.state = TerminalState::Exited;
    let (rig, cx) = session_rig(cx, kinds(), vec![session]);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), true),
        [
            SessionMenuId::CopyPath,
            SessionMenuId::OpenFolder,
            SessionMenuId::RevealInWorkPanel,
        ]
    );
}

#[gpui::test]
fn a_session_on_another_host_offers_no_open_folder(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![running_s1()]);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), false),
        [
            SessionMenuId::Stop,
            SessionMenuId::CopyPath,
            SessionMenuId::RevealInWorkPanel,
        ]
    );
}

#[gpui::test]
fn a_workspace_without_a_session_offers_only_the_folder_rows(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![]);
    assert_eq!(
        open_menu(&rig, cx, None, true),
        [SessionMenuId::CopyPath, SessionMenuId::OpenFolder]
    );
}

#[gpui::test]
fn stop_sends_agent_stop_for_the_session(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![running_s1()]);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Stop, cx);
    });
    cx.run_until_parked();
    let (session, _reply) = stop_call(&mut rig.script).expect("a stop call");
    assert_eq!(session, "s1");
}

#[gpui::test]
fn copy_path_writes_the_workspace_folder_to_the_clipboard(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![running_s1()]);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::CopyPath, cx);
    });
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("/repos/fix-login"));
}

#[gpui::test]
fn reveal_shows_the_work_panel_with_the_session_terminal(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![running_s1()]);
    rig.shell.update(cx, |shell, _| {
        if shell.model.region_visible(Region::WorkPanel) {
            shell.model.toggle_region(Region::WorkPanel);
        }
    });
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::RevealInWorkPanel, cx);
    });
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.model.region_visible(Region::WorkPanel));
    });
    draw(cx);
}
