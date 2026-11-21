//! The Bot Teams pages (screens 4): the team list, each team's pages and their Sidebar.

mod dialogs;
mod home;
mod sidebar;

pub(super) use dialogs::{TeamDialog, team_dialog_view};
pub(super) use home::bot_teams_home;
pub(super) use sidebar::teams_list;
