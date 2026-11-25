//! The Sidebar in Bot Teams mode (D-WS-29, screens 4.1): every team with its Conference room and
//! Board, and the Projects of the team in view (Files, Memory, Routines, then its projects).

use super::page::TeamPage;
use super::state::TeamsState;
use super::types::{Column, ProjectId, Team, TeamId};
use crate::model::navigation::Page;
use crate::model::shell::HostReach;
use massorca_protocol::methods::teams::TeamCharacter;

/// The Bot Teams Sidebar, ready to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamsSidebar {
    /// False without a service: the Sidebar says so instead of listing teams.
    pub connected: bool,
    /// The bot teams page is on screen, so the "All teams" row is selected.
    pub all_teams_selected: bool,
    pub teams: Vec<TeamRow>,
    /// The Projects section of the team in view, if there is one.
    pub projects: Option<ProjectsSection>,
}

/// One team and its two rows underneath.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamRow {
    pub id: TeamId,
    pub name: String,
    /// The team's character, drawn before its name (A57).
    pub character: TeamCharacter,
    pub reach: HostReach,
    /// A page of this team is on screen.
    pub selected: bool,
    pub conference: ChildRow,
    pub board: ChildRow,
}

/// A row under a team: the Conference room or the Board, with its status hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildRow {
    pub label: &'static str,
    /// The hint at the right: "Nothing open", "5 tasks" or "No tasks".
    pub hint: String,
    pub page: TeamPage,
    pub selected: bool,
}

/// The Projects section of one team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectsSection {
    pub team: TeamId,
    pub rows: Vec<ProjectRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRow {
    pub kind: ProjectRowKind,
    pub label: String,
    pub page: TeamPage,
    pub selected: bool,
}

/// What a Projects row stands for; the view maps each kind to its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectRowKind {
    Files,
    Memory,
    Routines,
    Project(ProjectId),
}

/// Builds the teams sidebar for the page on screen. The team in view comes from that page, or
/// from the last team the user worked in.
pub fn teams_sidebar(state: &TeamsState, page: Page) -> TeamsSidebar {
    let current = match page {
        Page::Team(team_page) => Some(team_page),
        _ => None,
    };
    let context = current.map(TeamPage::team).or(state.selected());
    TeamsSidebar {
        connected: state.is_connected(),
        all_teams_selected: page == Page::Home,
        teams: state
            .teams()
            .iter()
            .map(|team| team_row(team, current))
            .collect(),
        projects: context
            .and_then(|id| state.team(id))
            .map(|team| projects_section(team, current)),
    }
}

fn team_row(team: &Team, current: Option<TeamPage>) -> TeamRow {
    let conference = TeamPage::Conference(team.id);
    let board = TeamPage::Board(team.id);
    TeamRow {
        id: team.id,
        name: team.name.clone(),
        character: team.character,
        reach: team.reach,
        selected: current.is_some_and(|page| page.team() == team.id),
        conference: ChildRow {
            label: "Conference room",
            hint: "Nothing open".into(),
            page: conference,
            selected: current == Some(conference),
        },
        board: ChildRow {
            label: "Board",
            hint: board_hint(team),
            page: board,
            selected: current == Some(board),
        },
    }
}

/// The Board's hint counts the cards that are not Done yet.
fn board_hint(team: &Team) -> String {
    let open = team
        .tasks
        .iter()
        .filter(|task| task.column != Column::Done)
        .count();
    match open {
        0 => "No tasks".into(),
        1 => "1 task".into(),
        count => format!("{count} tasks"),
    }
}

fn projects_section(team: &Team, current: Option<TeamPage>) -> ProjectsSection {
    let fixed = [
        (ProjectRowKind::Files, "Files", TeamPage::Files(team.id)),
        (ProjectRowKind::Memory, "Memory", TeamPage::Memory(team.id)),
        (
            ProjectRowKind::Routines,
            "Routines",
            TeamPage::Routines(team.id),
        ),
    ];
    let mut rows: Vec<ProjectRow> = fixed
        .into_iter()
        .map(|(kind, label, page)| ProjectRow {
            kind,
            label: label.into(),
            page,
            selected: current == Some(page),
        })
        .collect();
    rows.extend(team.projects.iter().map(|project| {
        let page = TeamPage::Project(team.id, project.id);
        ProjectRow {
            kind: ProjectRowKind::Project(project.id),
            label: project.name.clone(),
            page,
            selected: current == Some(page),
        }
    }));
    ProjectsSection {
        team: team.id,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::super::sample::{SampleTeamsSource, TeamsSource, UnavailableTeams};
    use super::*;

    #[test]
    fn each_team_lists_its_conference_room_and_board_with_their_hints() {
        let state = SampleTeamsSource.teams();
        let sidebar = teams_sidebar(&state, Page::Home);
        assert!(sidebar.all_teams_selected && sidebar.connected);
        let row = &sidebar.teams[0];
        assert_eq!(row.name, "Desarrollo");
        assert_eq!(row.conference.hint, "Nothing open");
        assert_eq!(row.board.hint, "5 tasks", "the Done card is not counted");
        assert_eq!(row.board.page, TeamPage::Board(TeamId(1)));
        assert!(!row.selected);
    }

    #[test]
    fn the_projects_follow_the_team_in_view_and_mark_the_open_page() {
        let state = SampleTeamsSource.teams();
        let page = TeamPage::Memory(TeamId(1));
        let sidebar = teams_sidebar(&state, Page::Team(page));
        assert!(!sidebar.all_teams_selected);
        assert!(sidebar.teams[0].selected);
        let section = sidebar.projects.expect("the team in view has projects");
        let labels: Vec<&str> = section.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Files", "Memory", "Routines", "Desarrollo onboarding"]
        );
        assert!(section.rows[1].selected, "Memory is the open page");
    }

    #[test]
    fn the_last_worked_team_keeps_its_projects_on_the_home_page() {
        let mut state = SampleTeamsSource.teams();
        state.select(TeamId(1));
        let sidebar = teams_sidebar(&state, Page::Home);
        assert!(sidebar.projects.is_some());
    }

    #[test]
    fn without_a_service_the_sidebar_lists_no_teams() {
        let sidebar = teams_sidebar(&UnavailableTeams.teams(), Page::Home);
        assert!(!sidebar.connected);
        assert!(sidebar.teams.is_empty() && sidebar.projects.is_none());
    }
}
