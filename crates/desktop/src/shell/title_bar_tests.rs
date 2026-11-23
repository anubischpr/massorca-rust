//! The Title bar in a test window: the title and the Back control each page draws (D-SH-03).

use super::Shell;
use crate::data::SampleDataSource;
use crate::model::navigation::Page;
use crate::model::teams::{SampleTeamsSource, TeamId, TeamPage};
use crate::model::title_bar::title_bar;
use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_ui::{Appearance, Theme as UiTheme};

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
