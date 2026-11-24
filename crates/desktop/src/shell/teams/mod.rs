//! The Bot Teams pages (screens 4): the team list, each team's pages and their Sidebar.

mod access;
mod board;
mod card_menu;
mod dialogs;
mod files;
mod home;
mod memory;
mod new_project;
mod project;
mod routines;
mod sidebar;

use crate::model::settings_sections::NOT_AVAILABLE;
use gpui::{ElementId, IntoElement, ParentElement, SharedString, Styled, div, px};
use massorca_protocol::methods::teams::TeamCharacter;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::team_character::{TeamCharacterMark, TeamShape};
use massorca_ui::tokens::{SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_HALF, TYPE_BODY_STRONG};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

pub(super) use board::{board_columns, board_page};
pub(super) use card_menu::OpenTeamMenu;
pub(super) use dialogs::{TeamConfirm, TeamDialog, team_confirm_view, team_dialog_view};
pub(super) use files::files_page;
pub(super) use home::bot_teams_home;
pub(super) use memory::memory_page;
pub(super) use project::project_page;
pub(super) use routines::routines_page;
pub(super) use sidebar::teams_list;

/// The team pill of a team page: the team's character, its name and a chevron, as bt-board draws it
/// (screens 4, A59).
fn team_pill(name: &str, character: TeamCharacter, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1 + SPACE_HALF))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .child(TeamCharacterMark::new(
            team_shape(character),
            name.to_owned(),
        ))
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(theme.palette.text.primary)
                .child(name.to_owned()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(IconSize::XSmall)
                .color(theme.palette.text.tertiary),
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

/// A control whose action waits for a layer that does not exist yet: drawn disabled, and its
/// tooltip says so.
fn waiting_icon(
    id: impl Into<ElementId>,
    icon: IconName,
    name: impl Into<SharedString>,
) -> IconButton {
    IconButton::new(id, icon, name)
        .disabled(true)
        .disabled_reason(NOT_AVAILABLE)
}

#[cfg(test)]
mod tests {
    use super::team_shape;
    use crate::data::SampleDataSource;
    use crate::model::canvas::Canvas;
    use crate::model::teams::{ProjectId, SampleTeamsSource, TeamId, TeamPage};
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
    use crate::shell::Shell;
    use gpui::TestAppContext;
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
}
