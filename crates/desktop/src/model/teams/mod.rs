//! Bot Teams as plain data: teams, their Boards, projects, routines, memory and files (screens 4).
//! No GPUI types here, so the rules test as Rust.

mod access;
mod board;
mod bot_edit;
mod card_menu;
mod live;
mod new_project;
mod page;
mod sample;
mod sidebar;
mod state;
mod team_edit;
mod team_switch;
mod types;

pub use access::{access_change, access_line, new_team_access_hint};
pub use board::{BoardView, CardView, ColumnView, Landing, PickStep};
pub use bot_edit::{BotChange, BotForm};
pub use card_menu::{CardMenuRow, card_menu_rows};
pub use live::{agent_of_kind, live_teams};
pub use new_project::{NewProject, ProjectDraft};
pub use page::TeamPage;
pub use sample::{SampleTeamsSource, TeamsSource, UnavailableTeams};
pub use sidebar::{
    ChildRow, ProjectRow, ProjectRowKind, ProjectsSection, TeamRow, TeamsSidebar, teams_sidebar,
};
pub use state::{LINK_NOT_SAVED, NAME_LIMIT, NewBot, NewTeam, TeamError, TeamsState, check_name};
pub use team_edit::{CHARACTER_CHOICES, TeamChange, TeamForm};
pub use team_switch::{TeamSwitchRow, team_switch_rows};
pub use types::{
    AddressId, BoardOwner, Bot, BotId, Cadence, Column, FileId, HostChoice, MemoryEntry, MemoryId,
    MemoryTab, Pick, Project, ProjectId, Routine, RoutineId, RunOutcome, Task, TaskId, Team,
    TeamFile, TeamId, WebAddress, Weekday,
};
