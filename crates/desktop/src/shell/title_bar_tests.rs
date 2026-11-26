//! The Title bar in a test window: the title and the Back control each page draws (D-SH-03).

use super::Shell;
use super::service_tests::{click, rig};
use crate::data::SampleDataSource;
use crate::model::canvas::Canvas;
use crate::model::navigation::Page;
use crate::model::regions::Region;
use crate::model::teams::{SampleTeamsSource, TeamId, TeamPage};
use crate::model::title_bar::title_bar;
use crate::service::fixtures;
use crate::service::{Command, Refusal, Script, StoreUpdate, TerminalCommand};
use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_protocol::methods::terminals::{Terminal, TerminalCreateParams};
use massorca_ui::{Appearance, Theme as UiTheme};
use tokio::sync::oneshot;

type TerminalReply = oneshot::Sender<Result<Terminal, Refusal>>;

fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| {
        UiTheme::set(Appearance::Dark, cx);
        crate::shell::bind_keys(cx);
    });
    cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx))
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn each_bot_teams_page_is_titled_by_its_team_and_draws_no_back(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.model.press_mode_switch();
        cx.notify();
    });
    let team = TeamId(1);
    let pages = [
        (TeamPage::Board(team), "Desarrollo · Board"),
        (TeamPage::Conference(team), "Desarrollo · Conference room"),
        (TeamPage::Routines(team), "Desarrollo · Routines"),
        (TeamPage::NewRoutine(team), "Desarrollo · New routine"),
    ];
    for (page, title) in pages {
        shell.update(cx, |shell, cx| {
            shell.model.open_team_page(page);
            cx.notify();
        });
        draw(cx);
        assert!(
            cx.debug_bounds("title-text").is_some(),
            "{title}: the title is drawn"
        );
        assert!(
            cx.debug_bounds("title-back").is_none(),
            "{title}: a Bot Teams page draws no back control"
        );
        shell.read_with(cx, |shell, _| {
            assert_eq!(title_bar(shell.model()).title, title);
        });
    }
}

#[gpui::test]
fn a_full_page_draws_its_title_and_one_back_control(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx);
    for (page, title) in [
        (Page::Settings, "Settings"),
        (Page::Calendar, "Calendar"),
        (Page::Tasks, "Tasks"),
    ] {
        shell.update(cx, |shell, cx| {
            shell.model.open_page(page);
            cx.notify();
        });
        draw(cx);
        assert!(
            cx.debug_bounds("title-text").is_some(),
            "{title}: the title is drawn"
        );
        assert!(
            cx.debug_bounds("title-back").is_some(),
            "{title}: a full page draws its back control"
        );
        shell.read_with(cx, |shell, _| {
            assert_eq!(title_bar(shell.model()).title, title);
        });
    }
}

#[gpui::test]
fn a_session_draws_its_back_and_forward_controls(cx: &mut TestAppContext) {
    let (_shell, cx) = shell(cx);
    draw(cx);
    assert!(
        cx.debug_bounds("title-back").is_some(),
        "a session draws Back and Forward"
    );
}

#[gpui::test]
fn the_actions_without_a_service_or_a_terminal_are_not_drawn(cx: &mut TestAppContext) {
    let (_shell, cx) = shell(cx);
    draw(cx);
    assert!(
        cx.debug_bounds("title-new-chat").is_some(),
        "New chat is drawn on a session"
    );
    assert!(
        cx.debug_bounds("title-terminal").is_none(),
        "no service, so no Terminal"
    );
    assert!(
        cx.debug_bounds("title-open-in-work-panel").is_none(),
        "the sample session has no terminal"
    );
}

#[gpui::test]
fn new_chat_opens_the_start_page_of_the_session_workspace(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![fixtures::agent(
            "s1", "w1", 10,
        )]))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, cx| {
        shell.model.open_session("s1");
        cx.notify();
    });
    click(cx, "title-new-chat");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.model().canvas(),
            Canvas::Workspace("w1".to_owned()),
            "the start page of the session's workspace"
        );
        assert_eq!(shell.model().snapshot().active_session, None);
    });
}

#[gpui::test]
fn terminal_opens_a_terminal_in_the_work_panel_of_the_current_workspace(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell.update(cx, |shell, _| {
        shell.model.select_workspace(Some("w1".to_owned()))
    });
    cx.run_until_parked();
    click(cx, "title-terminal");
    frame(cx);
    frame(cx);
    let (params, _reply) =
        create_call(&mut rig.script).expect("a terminal.create call once the pane has painted");
    assert_eq!(params.workspace, "w1");
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.model().region_visible(Region::WorkPanel),
            "the Work panel shows"
        );
    });
}

#[gpui::test]
fn open_in_work_panel_reveals_the_open_sessions_terminal(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let mut session = fixtures::agent("s1", "w1", 10);
    session.terminal_id = Some("t1".to_owned());
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![session]))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, cx| {
        shell.model.open_session("s1");
        cx.notify();
    });
    click(cx, "title-open-in-work-panel");
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.model().region_visible(Region::WorkPanel),
            "the Work panel shows"
        );
        assert_eq!(
            shell.model().terminal_selection().selected("w1"),
            Some("t1"),
            "the session's terminal is selected"
        );
    });
}

/// One frame as the platform runs it: the callbacks scheduled for it, then the paint.
fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    draw(cx);
    cx.run_until_parked();
}

/// The first `terminal.create` the script holds, with the reply the test may answer.
fn create_call(script: &mut Script) -> Option<(TerminalCreateParams, TerminalReply)> {
    while let Some(command) = script.next() {
        if let Command::Terminal(TerminalCommand::Create { params, reply }) = command {
            return Some((params, reply));
        }
    }
    None
}
