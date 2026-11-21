//! The pages of one team. Each keeps the Sidebar open, unlike a full page (screens 3, decision 1).

use super::types::{ProjectId, TeamId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamPage {
    /// The Conference room, the team's chat (D-AG-24).
    Conference(TeamId),
    /// The team Board (D-WS-30).
    Board(TeamId),
    /// The routines of the team (D-AG-25).
    Routines(TeamId),
    /// The team memory (D-AG-26).
    Memory(TeamId),
    /// The team files (D-ED-13).
    Files(TeamId),
    /// A team project: its brief, Board and link (D-WS-31).
    Project(TeamId, ProjectId),
    /// The plain-language routine form (D-AG-34).
    NewRoutine(TeamId),
}

impl TeamPage {
    /// The team the page belongs to; the Sidebar lists its projects.
    pub fn team(self) -> TeamId {
        match self {
            TeamPage::Conference(team)
            | TeamPage::Board(team)
            | TeamPage::Routines(team)
            | TeamPage::Memory(team)
            | TeamPage::Files(team)
            | TeamPage::Project(team, _)
            | TeamPage::NewRoutine(team) => team,
        }
    }

    /// The page's name in the Title bar. A project's name comes from its team, not from here.
    pub fn title(self) -> &'static str {
        match self {
            TeamPage::Conference(_) => "Conference room",
            TeamPage::Board(_) => "Board",
            TeamPage::Routines(_) => "Routines",
            TeamPage::Memory(_) => "Memory",
            TeamPage::Files(_) => "Files",
            TeamPage::Project(_, _) => "Project",
            TeamPage::NewRoutine(_) => "New routine",
        }
    }
}
