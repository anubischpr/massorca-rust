//! A session row's menu (A97): the rows it offers, and what Stop, Copy path, Reveal in Work panel and Delete session…
//! reach. The scripted service of the composer tests stands in for the service.

use super::session_view_tests::{Rig, draw, session_rig, setup, stop_call};
use super::workspace_dialogs::MenuTarget;
use crate::model::regions::Region;
use crate::model::session_delete::SessionDeleteStage;
use crate::model::session_menu::{SessionMenuId, SessionMenuRow};
use crate::service::Script;
use crate::service::fixtures::{agent, agent_kind};
use crate::service::{Command, ForgetSessionParams, Refusal, StoreUpdate};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::{AgentKind, AgentSession, PackageState};
use massorca_protocol::methods::terminals::TerminalState;
use tokio::sync::oneshot;

type ForgetCall = (
    ForgetSessionParams,
    oneshot::Sender<Result<AgentSession, Refusal>>,
);

fn kinds() -> Vec<AgentKind> {
    vec![agent_kind("claude", "Claude Code", PackageState::Enabled)]
}

/// Session s1 in w1, running, with its terminal.
fn running_s1() -> AgentSession {
    let mut session = agent("s1", "w1", 10);
    session.terminal_id = Some("t1".to_owned());
    session
}

/// Session s1 in w1 after its run has ended; its terminal stays.
fn ended_s1() -> AgentSession {
    let mut session = running_s1();
    session.execution.state = TerminalState::Exited;
    session
}

/// Tells the store that the service offers `agent-archive/1`, as a connect does.
fn offer_archive(rig: &Rig, cx: &mut VisualTestContext) {
    rig.updates
        .send(StoreUpdate::AgentArchiveOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
}

/// Opens the menu of the row of `session` (None for a workspace with no session) and returns the rows it offers.
fn open_menu(
    rig: &Rig,
    cx: &mut VisualTestContext,
    session: Option<&str>,
    local: bool,
) -> Vec<SessionMenuRow> {
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

fn item(id: SessionMenuId) -> SessionMenuRow {
    SessionMenuRow::Item(id)
}

fn forget_call(script: &mut Script) -> Option<ForgetCall> {
    while let Some(command) = script.next() {
        if let Command::AgentForget { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

fn delete_title(rig: &Rig, cx: &mut VisualTestContext) -> Option<String> {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .dialogs
            .delete_session
            .as_ref()
            .map(|flow| flow.copy().title)
    })
}

#[gpui::test]
fn a_running_session_offers_stop_copy_open_folder_and_reveal(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![running_s1()]);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), true),
        [
            item(SessionMenuId::Stop),
            item(SessionMenuId::CopyPath),
            item(SessionMenuId::OpenFolder),
            item(SessionMenuId::RevealInWorkPanel),
        ]
    );
}

#[gpui::test]
fn an_ended_session_offers_no_stop(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), true),
        [
            item(SessionMenuId::CopyPath),
            item(SessionMenuId::OpenFolder),
            item(SessionMenuId::RevealInWorkPanel),
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
            item(SessionMenuId::Stop),
            item(SessionMenuId::CopyPath),
            item(SessionMenuId::RevealInWorkPanel),
        ]
    );
}

#[gpui::test]
fn a_workspace_without_a_session_offers_only_the_folder_rows(cx: &mut TestAppContext) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![]);
    assert_eq!(
        open_menu(&rig, cx, None, true),
        [
            item(SessionMenuId::CopyPath),
            item(SessionMenuId::OpenFolder)
        ]
    );
}

#[gpui::test]
fn an_ended_session_offers_delete_after_a_divider_once_the_service_offers_it(
    cx: &mut TestAppContext,
) {
    setup(cx);
    let (rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), true),
        [
            item(SessionMenuId::CopyPath),
            item(SessionMenuId::OpenFolder),
            item(SessionMenuId::RevealInWorkPanel),
            item(SessionMenuId::Rename),
            item(SessionMenuId::Archive),
            SessionMenuRow::Divider,
            item(SessionMenuId::Delete),
        ]
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

#[gpui::test]
fn delete_opens_the_confirm_and_sends_nothing_until_confirmed(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Delete, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        delete_title(&rig, cx).as_deref(),
        Some("Delete session feat/fix-login?")
    );
    assert!(
        forget_call(&mut rig.script).is_none(),
        "nothing is sent before the confirm"
    );
}

#[gpui::test]
fn cancel_closes_the_confirm_and_sends_nothing(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Delete, cx);
    });
    rig.shell
        .update(cx, |shell, cx| shell.close_session_delete(cx));
    cx.run_until_parked();
    assert_eq!(delete_title(&rig, cx), None);
    assert!(forget_call(&mut rig.script).is_none());
}

#[gpui::test]
fn confirm_sends_a_plain_forget_for_the_session(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Delete, cx);
    });
    rig.shell
        .update(cx, |shell, cx| shell.confirm_session_delete(cx));
    cx.run_until_parked();
    let (params, reply) = forget_call(&mut rig.script).expect("a forget call");
    assert_eq!(
        params,
        ForgetSessionParams {
            session: "s1".to_owned(),
            remove_workspace: false,
            force: false,
        }
    );
    reply.send(Ok(ended_s1())).expect("the shell listens");
    cx.run_until_parked();
    assert_eq!(
        delete_title(&rig, cx),
        None,
        "the dialog closes once deleted"
    );
}

#[gpui::test]
fn the_worktree_option_sends_remove_workspace(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Delete, cx);
    });
    rig.shell.update(cx, |shell, cx| {
        shell.set_session_delete_remove_workspace(true, cx)
    });
    rig.shell
        .update(cx, |shell, cx| shell.confirm_session_delete(cx));
    cx.run_until_parked();
    let (params, _reply) = forget_call(&mut rig.script).expect("a forget call");
    assert!(params.remove_workspace);
    assert!(!params.force);
}

#[gpui::test]
fn a_dirty_worktree_asks_to_discard_and_then_retries_with_force(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Delete, cx);
    });
    rig.shell.update(cx, |shell, cx| {
        shell.set_session_delete_remove_workspace(true, cx)
    });
    rig.shell
        .update(cx, |shell, cx| shell.confirm_session_delete(cx));
    cx.run_until_parked();
    let (_params, reply) = forget_call(&mut rig.script).expect("the first call");
    reply.send(Err(Refusal::Dirty)).expect("the shell listens");
    cx.run_until_parked();
    let stage = rig.shell.read_with(cx, |shell, _| {
        shell
            .dialogs
            .delete_session
            .as_ref()
            .map(|flow| flow.stage().clone())
    });
    assert_eq!(stage, Some(SessionDeleteStage::Dirty));

    rig.shell
        .update(cx, |shell, cx| shell.confirm_session_delete(cx));
    cx.run_until_parked();
    let (params, reply) = forget_call(&mut rig.script).expect("the retry");
    assert!(params.remove_workspace && params.force);
    reply.send(Ok(ended_s1())).expect("the shell listens");
    cx.run_until_parked();
    assert_eq!(delete_title(&rig, cx), None);
}

fn rename_call(script: &mut Script) -> Option<(String, String)> {
    while let Some(command) = script.next() {
        if let Command::AgentRename { session, title, .. } = command {
            return Some((session, title));
        }
    }
    None
}

fn archive_call(script: &mut Script) -> Option<(String, bool)> {
    while let Some(command) = script.next() {
        if let Command::AgentArchive {
            session, archived, ..
        } = command
        {
            return Some((session, archived));
        }
    }
    None
}

#[gpui::test]
fn an_archived_session_offers_unarchive_in_place_of_archive(cx: &mut TestAppContext) {
    setup(cx);
    let mut archived = ended_s1();
    archived.archived = true;
    let (rig, cx) = session_rig(cx, kinds(), vec![archived]);
    offer_archive(&rig, cx);
    assert_eq!(
        open_menu(&rig, cx, Some("s1"), true),
        [
            item(SessionMenuId::CopyPath),
            item(SessionMenuId::OpenFolder),
            item(SessionMenuId::RevealInWorkPanel),
            item(SessionMenuId::Rename),
            item(SessionMenuId::Unarchive),
            SessionMenuRow::Divider,
            item(SessionMenuId::Delete),
        ]
    );
}

#[gpui::test]
fn rename_opens_an_inline_editor_and_enter_sends_agent_rename(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Rename, cx);
    });
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-rename-s1").is_some(),
        "the session's row becomes a title editor"
    );
    cx.simulate_input("Fix login v2");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        rename_call(&mut rig.script),
        Some(("s1".to_owned(), "Fix login v2".to_owned()))
    );
}

#[gpui::test]
fn archive_and_unarchive_send_agent_archive_with_the_flag(cx: &mut TestAppContext) {
    setup(cx);
    let (mut rig, cx) = session_rig(cx, kinds(), vec![ended_s1()]);
    offer_archive(&rig, cx);
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Archive, cx);
    });
    cx.run_until_parked();
    assert_eq!(archive_call(&mut rig.script), Some(("s1".to_owned(), true)));
    rig.shell.update(cx, |shell, cx| {
        shell.run_session_action(Some("s1"), "w1", SessionMenuId::Unarchive, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        archive_call(&mut rig.script),
        Some(("s1".to_owned(), false))
    );
}
