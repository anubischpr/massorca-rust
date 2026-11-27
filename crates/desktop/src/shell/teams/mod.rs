//! The Bot Teams pages (screens 4): the team list, each team's pages and their Sidebar.

mod access;
mod board;
mod bot_advanced;
mod bot_skills;
mod card_menu;
mod conference;
mod dialogs;
mod files;
mod filters;
mod home;
mod memory;
mod memory_menu;
mod new_project;
mod project;
mod routine_confirm;
mod routine_form;
mod routine_form_page;
mod routine_history;
mod routine_menu;
mod routine_service;
mod routines;
mod sidebar;
mod team_bots;
mod team_switch;

use super::Shell;
use super::sidebar::row_focus;
use crate::model::teams::TeamPage;
use gpui::{
    App, IntoElement, KeyDownEvent, ParentElement, Role, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_protocol::methods::teams::TeamCharacter;
use massorca_ui::components::team_character::{TeamCharacterMark, TeamShape};
use massorca_ui::style::FocusRing;
use massorca_ui::tokens::{
    RADIUS_FULL, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_HALF, TYPE_BODY_STRONG,
};
use massorca_ui::{ActiveTheme, Icon, IconName, IconSize, TextStyled};

pub(super) use board::{board_columns, board_page};
pub(super) use card_menu::OpenTeamMenu;
pub(super) use conference::{RoomFrame, RoomPage, conference_page};
pub(super) use dialogs::{TeamConfirm, TeamDialog, team_confirm_view, team_dialog_view};
pub(super) use files::files_page;
pub(super) use filters::OpenFilterMenu;
pub(super) use home::bot_teams_home;
pub(super) use memory::memory_page;
pub(super) use memory_menu::OpenMemoryMenu;
pub(super) use project::project_page;
pub(super) use routine_confirm::{RoutineConfirm, routine_confirm_view};
pub(super) use routine_form::RoutineForm;
pub(super) use routine_form_page::routine_form_page;
pub(super) use routine_history::routine_history_page;
pub(super) use routine_menu::OpenRoutineMenu;
pub(super) use routines::routines_page;
pub(super) use sidebar::teams_list;
pub(super) use team_switch::OpenTeamSwitch;

/// The team pill of a team page: the team's character, its name and a chevron, as bt-board draws it
/// (screens 4, A59). It is a button that opens the team switch.
fn team_pill(
    page: TeamPage,
    name: &str,
    character: TeamCharacter,
    shell: &WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = *cx.theme();
    let focus = row_focus(window, cx, "team-pill".to_owned());
    let (press, key) = (shell.clone(), shell.clone());
    let palette = theme.palette;
    div()
        .id("team-pill")
        .debug_selector(|| "team-pill".to_owned())
        .track_focus(&focus)
        .tab_stop(true)
        .focus_ring(&theme)
        .role(Role::Button)
        .aria_label(format!("Team: {name}. Switch team"))
        .flex()
        .items_center()
        .gap(px(SPACE_1 + SPACE_HALF))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_FULL))
        .cursor_pointer()
        .hover(move |style| style.bg(palette.fill.hover_canvas))
        .active(move |style| style.bg(palette.fill.pressed_canvas))
        .on_click(move |_, window, cx| {
            let _ = press.update(cx, |shell, cx| shell.open_team_switch(page, window, cx));
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                let _ = key.update(cx, |shell, cx| shell.open_team_switch(page, window, cx));
                cx.stop_propagation();
            }
        })
        .child(TeamCharacterMark::new(
            team_shape(character),
            name.to_owned(),
        ))
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(palette.text.primary)
                .child(name.to_owned()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(IconSize::XSmall)
                .color(palette.text.tertiary),
        )
}

/// The shape a team's character draws. A character the service does not name draws the triangle.
fn team_shape(character: TeamCharacter) -> TeamShape {
    match character {
        TeamCharacter::Triangle | TeamCharacter::Unknown => TeamShape::Triangle,
        TeamCharacter::Square => TeamShape::Square,
        TeamCharacter::Circle => TeamShape::Circle,
        TeamCharacter::Diamond => TeamShape::Diamond,
        TeamCharacter::Hexagon => TeamShape::Hexagon,
        TeamCharacter::Star => TeamShape::Star,
    }
}

#[cfg(test)]
mod routine_tests;

#[cfg(test)]
mod tests {
    use super::team_shape;
    use crate::data::SampleDataSource;
    use crate::model::canvas::Canvas;
    use crate::model::teams::{ProjectId, RoutineId, SampleTeamsSource, TeamId, TeamPage};
    use massorca_protocol::methods::teams::TeamCharacter;
    use massorca_ui::components::team_character::TeamShape;

    #[test]
    fn every_team_character_draws_its_own_shape_and_an_unknown_one_the_triangle() {
        assert_eq!(team_shape(TeamCharacter::Triangle), TeamShape::Triangle);
        assert_eq!(team_shape(TeamCharacter::Square), TeamShape::Square);
        assert_eq!(team_shape(TeamCharacter::Circle), TeamShape::Circle);
        assert_eq!(team_shape(TeamCharacter::Diamond), TeamShape::Diamond);
        assert_eq!(team_shape(TeamCharacter::Hexagon), TeamShape::Hexagon);
        assert_eq!(team_shape(TeamCharacter::Star), TeamShape::Star);
        assert_eq!(team_shape(TeamCharacter::Unknown), TeamShape::Triangle);
    }
    use crate::model::teams::{TeamsSource, TeamsState};
    use crate::shell::Shell;
    use gpui::{Modifiers, TestAppContext};
    use massorca_ui::components::menu::{MenuChoice, MenuEvent};
    use massorca_ui::{Appearance, Theme};

    #[gpui::test]
    fn every_team_page_opens_and_draws_with_the_sample_team(cx: &mut TestAppContext) {
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        });
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            cx.notify();
        });
        let team = TeamId(1);
        let pages = [
            TeamPage::Conference(team),
            TeamPage::Board(team),
            TeamPage::Routines(team),
            TeamPage::Memory(team),
            TeamPage::Files(team),
            TeamPage::Project(team, ProjectId(1)),
            TeamPage::NewRoutine(team),
            TeamPage::EditRoutine(team, RoutineId(3)),
            TeamPage::RoutineHistory(team, RoutineId(3)),
        ];
        for page in pages {
            shell.update(cx, |shell, cx| {
                shell.model.open_team_page(page);
                cx.notify();
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            shell.read_with(cx, |shell, _| {
                assert_eq!(shell.model().canvas(), Canvas::Team(page));
            });
        }
    }
    /// The sample team and a second one, so the switch has somewhere to go.
    fn two_teams() -> TeamsState {
        let sample = SampleTeamsSource.teams();
        let mut other = sample.teams()[0].clone();
        other.id = TeamId(2);
        other.name = "Operaciones".to_owned();
        let mut teams = sample.teams().to_vec();
        teams.push(other);
        TeamsState::connected(sample.hosts().to_vec(), teams)
    }

    #[gpui::test]
    fn the_team_pill_switches_to_the_same_page_of_the_team_chosen(cx: &mut TestAppContext) {
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        });
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.set_teams(two_teams());
            shell.model.open_team_page(TeamPage::Routines(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let pill = cx
            .debug_bounds("team-pill")
            .expect("the Routines header draws the team pill");
        cx.simulate_click(pill.center(), Modifiers::default());
        cx.run_until_parked();
        let menu = shell.read_with(cx, |shell, _| {
            shell
                .team_switch
                .as_ref()
                .map(|open| open.menu.clone())
                .expect("the pill opens the team switch")
        });
        menu.update(cx, |_, cx| {
            cx.emit(MenuEvent::Chosen(MenuChoice {
                row: 1,
                child: None,
            }));
        });
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert!(shell.team_switch.is_none(), "the switch closes");
            assert_eq!(
                shell.model().canvas(),
                Canvas::Team(TeamPage::Routines(TeamId(2)))
            );
        });
    }

    #[gpui::test]
    fn the_sidebar_and_the_team_card_draw_the_team_character(cx: &mut TestAppContext) {
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        });
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let name = SampleTeamsSource.teams().teams()[0].name.clone();
        assert!(
            cx.debug_bounds("team-character").is_some(),
            "the team card and the Sidebar row draw {name}'s character"
        );
    }
}
