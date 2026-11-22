//! Bot Teams as plain data: teams, their Boards, projects, routines, memory and files (screens 4).
//! No GPUI types here, so the rules test as Rust.

mod board;
mod live;
mod page;
mod sample;
mod sidebar;
mod state;
mod types;

pub use board::{BoardView, CardView, ColumnView, Landing, PickStep};
pub use live::{agent_of_kind, live_teams};
pub use page::TeamPage;
pub use sample::{SampleTeamsSource, TeamsSource, UnavailableTeams};
pub use sidebar::{
    ChildRow, ProjectRow, ProjectRowKind, ProjectsSection, TeamRow, TeamsSidebar, teams_sidebar,
};
pub use state::{NAME_LIMIT, NewBot, NewTeam, TeamError, TeamsState, check_name};
pub use types::{
    AddressId, BoardOwner, Bot, BotId, Cadence, Column, FileId, HostChoice, MemoryEntry, MemoryId,
    MemoryTab, Pick, Project, ProjectId, Routine, RoutineId, RunOutcome, Task, TaskId, Team,
    TeamFile, TeamId, WebAddress, Weekday,
};
