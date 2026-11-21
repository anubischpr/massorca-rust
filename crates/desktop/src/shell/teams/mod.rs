//! The Bot Teams pages (screens 4): the team list, each team's pages and their Sidebar.

mod board;
mod dialogs;
mod home;
mod project;
mod routines;
mod sidebar;

use crate::model::teams::TeamId;
use gpui::{IntoElement, ParentElement, Styled, div, px};
use massorca_ui::components::avatar::{Avatar, AvatarSize};
use massorca_ui::tokens::{SPACE_1, TYPE_BODY_STRONG};
use massorca_ui::{TextStyled, Theme};

pub(super) use board::{board_columns, board_page};
pub(super) use dialogs::{TeamConfirm, TeamDialog, team_confirm_view, team_dialog_view};
pub(super) use home::bot_teams_home;
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
}
