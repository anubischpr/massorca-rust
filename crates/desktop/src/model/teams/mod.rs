//! Bot Teams as plain data: teams, their Boards, projects, routines, memory and files (screens 4).
//! No GPUI types here, so the rules test as Rust.

mod page;
mod sample;
mod state;
mod types;

pub use page::TeamPage;
pub use sample::{SampleTeamsSource, TeamsSource, UnavailableTeams};
pub use state::{NAME_LIMIT, NewBot, NewTeam, TeamError, TeamsState, check_name};
pub use types::{
    AddressId, BoardOwner, Bot, BotId, Cadence, Column, FileId, HostChoice, MemoryEntry, MemoryId,
    MemoryTab, Pick, Project, ProjectId, Routine, RoutineId, RunOutcome, Task, TaskId, Team,
    TeamFile, TeamId, WebAddress, Weekday,
};
