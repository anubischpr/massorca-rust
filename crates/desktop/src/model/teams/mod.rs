//! Bot Teams as plain data: teams, their Boards, projects, routines, memory and files (screens 4).
//! No GPUI types here, so the rules test as Rust.

mod access;
mod board;
mod board_move;
mod bot_edit;
mod bot_options;
mod card_menu;
mod filters;
mod live;
mod new_project;
mod page;
mod room;
mod routine_form;
mod routine_history;
mod sample;
mod sidebar;
mod state;
mod team_edit;
mod team_switch;
mod types;

pub use access::{access_change, access_line, new_team_access_hint};
pub use board::{BoardView, CardView, ColumnView, Landing, PickStep};
pub use board_move::{BoardMove, board_move};
pub use bot_edit::{BotChange, BotForm};
pub use bot_options::{
    KindOptions, OptionRow, effort_rows, kind_options, model_rows, options_of, row_index,
};
pub use card_menu::{CardMenuRow, card_menu_rows};
pub use filters::{BoardFilterRow, ProjectFilter, board_filter_rows};
pub use live::{agent_of_kind, live_teams};
pub use new_project::{NewProject, ProjectDraft};
pub use page::TeamPage;
pub use room::{
    EMPTY_ROOM, NO_BOTS, RoomLine, RoomTask, RoomView, Speaker, clock_text, receiver, room_view,
};
pub use routine_form::{
    MONTH_DAYS, ROUTINE_NAME_LIMIT, ROUTINE_PROMPT_LIMIT, RoutineDraft, RoutineField,
    RoutineProblem, ScheduleKind, WEEKDAYS, schedule_words,
};
pub use routine_history::{RoutineHistory, RunRow, RunsState, local_time, run_rows};
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
