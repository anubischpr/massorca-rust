//! The Bot Teams pages (screens 4): the team list, each team's pages and their Sidebar.

mod board;
mod dialogs;
mod files;
mod home;
mod memory;
mod project;
mod routines;
mod sidebar;

use crate::model::settings_sections::NOT_AVAILABLE;
use crate::model::teams::TeamId;
use gpui::{ElementId, IntoElement, ParentElement, SharedString, Styled, div, px};
use massorca_ui::components::avatar::{Avatar, AvatarSize};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::tokens::{SPACE_1, TYPE_BODY_STRONG};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

pub(super) use board::{board_columns, board_page};
pub(super) use dialogs::{TeamConfirm, TeamDialog, team_confirm_view, team_dialog_view};
pub(super) use files::files_page;
pub(super) use home::bot_teams_home;
pub(super) use memory::memory_page;
pub(super) use project::project_page;
pub(super) use routines::routines_page;
pub(super) use sidebar::teams_list;

/// The team pill of a team page: the team mark and its name (screens 4, A59).
fn team_pill(name: &str, team: TeamId, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .child(
            Avatar::team(format!("team-pill-{}", team.0), name.to_owned()).size(AvatarSize::Small),
        )
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(theme.palette.text.primary)
                .child(name.to_owned()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(IconSize::XSmall)
                .color(theme.palette.text.secondary),
        )
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
    use crate::data::SampleDataSource;
    use crate::model::canvas::Canvas;
    use crate::model::teams::{ProjectId, SampleTeamsSource, TeamId, TeamPage};
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
