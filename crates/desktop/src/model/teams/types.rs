//! The plain data of Bot Teams: teams and what each one holds, as the pages show it (screens 4).

use crate::model::shell::{Agent, HostReach};
use massorca_protocol::methods::teams::{TeamAccess, TeamCharacter};

macro_rules! ids {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        $(
            $(#[$doc])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct $name(pub u32);
        )*
    };
}

ids! {
    /// A team: its Conference room, Board, projects, routines and memory (A55).
    TeamId,
    /// A Bot Teams project inside a team, with its own brief and Board (D-WS-31).
    ProjectId,
    /// A bot of a team (D-AG-33).
    BotId,
    /// A task on a Board (D-WS-30).
    TaskId,
    /// A routine of a team (D-AG-25).
    RoutineId,
    /// A web address that starts a task in a team (D-AG-25).
    AddressId,
    /// An entry of a team's memory (D-AG-26).
    MemoryId,
    /// A file of a team folder (D-ED-13).
    FileId,
}

/// The Board's columns, left to right (D-WS-30; the titles are the BoardColumn proposal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Column {
    Open,
    Running,
    Waiting,
    Done,
}

impl Column {
    pub const ALL: [Column; 4] = [Column::Open, Column::Running, Column::Waiting, Column::Done];

    pub fn title(self) -> &'static str {
        match self {
            Column::Open => "Open",
            Column::Running => "Running",
            Column::Waiting => "Waiting for you",
            Column::Done => "Done",
        }
    }

    /// The column to the left, if any.
    pub fn previous(self) -> Option<Column> {
        match self {
            Column::Open => None,
            Column::Running => Some(Column::Open),
            Column::Waiting => Some(Column::Running),
            Column::Done => Some(Column::Waiting),
        }
    }

    /// The column to the right, if any.
    pub fn next(self) -> Option<Column> {
        match self {
            Column::Open => Some(Column::Running),
            Column::Running => Some(Column::Waiting),
            Column::Waiting => Some(Column::Done),
            Column::Done => None,
        }
    }
}

/// Whose Board a page shows: the team's, or one of its projects' (D-WS-31).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardOwner {
    Team(TeamId),
    Project(TeamId, ProjectId),
}

/// A card picked up and not yet dropped. The Board is untouched until the drop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub board: BoardOwner,
    pub task: TaskId,
    /// The column and place the card would land in if dropped now.
    pub column: Column,
    pub index: usize,
}

/// A bot of a team. Its agent gives it the icon shown on its rows (A16, A17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bot {
    pub id: BotId,
    pub name: String,
    pub role: String,
    /// The agent the bot runs as; `None` when the service does not say which one.
    pub agent: Option<Agent>,
    /// Why the bot's kind cannot run on its host (turned off, or not detected there); `None` when it can.
    pub kind_unavailable: Option<&'static str>,
}

/// A task on a Board. Its card shows the column's status word, the bot and the time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub column: Column,
    pub bot: Option<BotId>,
    /// The clock time of the last change, as the card shows it.
    pub time: Option<String>,
    /// The bot's host. An unreachable host shows the last known state (A10).
    pub reach: HostReach,
    /// A line the card shows after the bot, such as "Needs your reply in the Conference room".
    pub note: Option<String>,
}

/// A Bot Teams project: a brief, its own Board and an optional link to a Workspaces project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub brief: String,
    /// The Workspaces project the Board links to (D-WS-31), by name.
    pub link: Option<String>,
    /// Whether a link can be chosen here. The service keeps no link, so a project it reads has none.
    pub link_saved: bool,
    pub tasks: Vec<Task>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub fn name(self) -> &'static str {
        match self {
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
            Weekday::Sunday => "Sunday",
        }
    }
}

/// When a routine runs (D-AG-34, the plain-language schedule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    Daily,
    Weekdays,
    Weekly(Weekday),
    /// The day of the month, from 1 to 28.
    Monthly(u8),
}

impl Cadence {
    /// The schedule in words, with the time: "Every Monday at 09:00".
    pub fn words(self, time: &str) -> String {
        match self {
            Cadence::Daily => format!("Every day at {time}"),
            Cadence::Weekdays => format!("Every weekday at {time}"),
            Cadence::Weekly(day) => format!("Every {} at {time}", day.name()),
            Cadence::Monthly(day) => format!("On day {day} of every month at {time}"),
        }
    }
}

/// The last run of a routine, as its status word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Done,
    Failed,
    Running,
}

impl RunOutcome {
    pub fn word(self) -> &'static str {
        match self {
            RunOutcome::Done => "Done",
            RunOutcome::Failed => "Failed",
            RunOutcome::Running => "Running",
        }
    }
}

/// A routine: a task a bot starts on a schedule (A56). It is a simpler automation (decision 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Routine {
    pub id: RoutineId,
    pub name: String,
    pub instruction: String,
    pub cadence: Cadence,
    /// The 24-hour time, such as "09:00".
    pub time: String,
    pub bot: Option<BotId>,
    pub host: String,
    pub enabled: bool,
    pub last_run: Option<RunOutcome>,
}

/// A web address that starts a task in the team. The address carries a token, so only its
/// masked path is ever shown (decision 7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebAddress {
    pub id: AddressId,
    pub name: String,
    pub host: String,
    pub created: String,
    pub token: String,
}

impl WebAddress {
    /// The address with its token hidden: only the host and the path prefix show (decision 7).
    pub fn masked(&self) -> String {
        format!("https://{}/hook/••••••", self.host.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_address_never_shows_its_token() {
        let address = WebAddress {
            id: AddressId(1),
            name: "Release hook".into(),
            host: "vps".into(),
            created: "11 Nov 2026".into(),
            token: "sample-token".into(),
        };
        assert_eq!(address.masked(), "https://vps/hook/••••••");
        assert!(!address.masked().contains(&address.token));
    }
}

/// Which tab of the memory an entry sits under (D-AG-26).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MemoryTab {
    Bot,
    #[default]
    Team,
}

/// One thing the team remembers, with the bot that wrote it when there is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: MemoryId,
    pub tab: MemoryTab,
    pub text: String,
    pub author: Option<BotId>,
    pub date: String,
}

/// A file of the team folder, or a file of a linked Workspaces project (D-ED-13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamFile {
    pub id: FileId,
    pub path: String,
    /// The Workspaces project the file belongs to, when it is linked.
    pub linked_project: Option<String>,
}

/// A host a team can run on, with its state now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostChoice {
    pub name: String,
    pub reach: HostReach,
}

/// A team and everything it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub id: TeamId,
    pub name: String,
    /// The team's shape and eyes, drawn beside its name (A57, A59).
    pub character: TeamCharacter,
    pub purpose: String,
    pub host: String,
    pub reach: HostReach,
    pub bots: Vec<Bot>,
    pub lead: Option<BotId>,
    /// The permission mode the team's bots run in (decision 0072, amendment "team access").
    pub access: TeamAccess,
    /// Whether Workspaces is on in this team (D-AG-13, off by default).
    pub workspaces: bool,
    /// The team's own Board (D-WS-30).
    pub tasks: Vec<Task>,
    pub projects: Vec<Project>,
    pub routines: Vec<Routine>,
    pub addresses: Vec<WebAddress>,
    pub memory: Vec<MemoryEntry>,
    pub files: Vec<TeamFile>,
}

impl Team {
    pub fn bot(&self, id: BotId) -> Option<&Bot> {
        self.bots.iter().find(|bot| bot.id == id)
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    /// The lead bot's name, for the card and the Conference room header.
    pub fn lead_name(&self) -> Option<&str> {
        self.lead
            .and_then(|id| self.bot(id))
            .map(|bot| bot.name.as_str())
    }
}
