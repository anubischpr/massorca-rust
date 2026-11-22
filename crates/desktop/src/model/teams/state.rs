//! The Bot Teams state and the changes made to it. Changes land here, in the sample store today;
//! nothing in this module talks to a host, and every refusal names its reason.

use super::live::agent_of_kind;
use super::page::TeamPage;
use super::types::{
    AddressId, BoardOwner, Bot, BotId, HostChoice, MemoryId, MemoryTab, Pick, Project, ProjectId,
    RoutineId, Task, Team, TeamId,
};

/// Names hold at most this many characters (screens 4.5, 4.7).
pub const NAME_LIMIT: usize = 40;

/// Why a change was refused. The messages are the screens' copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamError {
    /// No Bot Teams service is connected, so nothing can change.
    Unavailable,
    /// The team, project or item is no longer there.
    Missing,
    /// A name is required.
    NameRequired,
    /// The name is longer than `NAME_LIMIT`.
    NameTooLong,
    /// Another team on the same host has this name.
    NameTaken,
}

impl TeamError {
    pub fn message(self) -> &'static str {
        match self {
            TeamError::Unavailable => {
                "Can't reach Bot Teams. Nothing can be changed until it is back."
            }
            TeamError::Missing => "That item is gone. Nothing was changed.",
            TeamError::NameRequired => "Name is required.",
            TeamError::NameTooLong => "Use up to 40 characters.",
            TeamError::NameTaken => "Name is already used in this host.",
        }
    }
}

/// What the New team dialog sends. `host` indexes the hosts the state lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTeam {
    pub name: String,
    pub purpose: String,
    pub host: usize,
    pub workspaces: bool,
}

/// What the Add bot dialog sends. `kind` is the service's id of the agent kind the bot runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewBot {
    pub name: String,
    pub role: String,
    pub kind: String,
}

/// All Bot Teams data the shell shows. `connected` is false when no service backs it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TeamsState {
    connected: bool,
    hosts: Vec<HostChoice>,
    teams: Vec<Team>,
    /// The team the last team page belongs to; the Sidebar lists its projects.
    selected: Option<TeamId>,
    /// A Board card picked up with the keyboard and not yet dropped.
    pick: Option<Pick>,
    /// The memory tab the Memory page shows.
    memory_tab: MemoryTab,
    /// Why the last card move was refused, shown on the Board until the next pick.
    notice: Option<String>,
    next_id: u32,
}

impl TeamsState {
    /// The state when no Bot Teams service is connected: the pages say so and change nothing.
    pub fn unavailable() -> Self {
        Self::default()
    }

    /// A connected state holding these teams. Ids handed out later never repeat one in use.
    pub fn connected(hosts: Vec<HostChoice>, teams: Vec<Team>) -> Self {
        let next_id = highest_id(&teams) + 1;
        Self {
            connected: true,
            hosts,
            teams,
            selected: None,
            pick: None,
            memory_tab: MemoryTab::default(),
            notice: None,
            next_id,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn hosts(&self) -> &[HostChoice] {
        &self.hosts
    }

    pub fn teams(&self) -> &[Team] {
        &self.teams
    }

    pub fn team(&self, id: TeamId) -> Option<&Team> {
        self.teams.iter().find(|team| team.id == id)
    }

    pub fn selected(&self) -> Option<TeamId> {
        self.selected
    }

    /// Remembers the team the user is working in, so the Sidebar keeps its projects open.
    pub fn select(&mut self, team: TeamId) {
        if self.team(team).is_some() {
            self.selected = Some(team);
        }
    }

    /// The state after a service read: `fresh`'s teams, with the team, the memory tab and the picked card
    /// kept where they still exist.
    pub fn refreshed(&self, mut fresh: TeamsState) -> TeamsState {
        let selected = self.selected.filter(|team| fresh.team(*team).is_some());
        let pick = self.pick.clone().filter(|pick| {
            fresh
                .tasks(pick.board)
                .is_some_and(|tasks| tasks.iter().any(|task| task.id == pick.task))
        });
        fresh.selected = selected;
        fresh.pick = pick;
        fresh.memory_tab = self.memory_tab;
        fresh.notice = self.notice.clone();
        fresh
    }

    pub fn pick(&self) -> Option<&Pick> {
        self.pick.as_ref()
    }

    pub fn memory_tab(&self) -> MemoryTab {
        self.memory_tab
    }

    pub fn set_memory_tab(&mut self, tab: MemoryTab) {
        self.memory_tab = tab;
    }

    pub fn create_team(&mut self, new: NewTeam) -> Result<TeamId, TeamError> {
        self.require_connected()?;
        let name = check_name(&new.name)?;
        let host = self
            .hosts
            .get(new.host)
            .cloned()
            .ok_or(TeamError::Missing)?;
        if self
            .teams
            .iter()
            .any(|team| team.host == host.name && team.name.eq_ignore_ascii_case(&name))
        {
            return Err(TeamError::NameTaken);
        }
        let id = TeamId(self.fresh());
        self.teams.push(Team {
            id,
            name,
            purpose: new.purpose.trim().to_owned(),
            host: host.name,
            reach: host.reach,
            bots: Vec::new(),
            lead: None,
            workspaces: new.workspaces,
            tasks: Vec::new(),
            projects: Vec::new(),
            routines: Vec::new(),
            addresses: Vec::new(),
            memory: Vec::new(),
            files: Vec::new(),
        });
        self.selected = Some(id);
        Ok(id)
    }

    /// Adds a bot to a team. The first bot added leads the team, as the Conference room needs one.
    pub fn add_bot(&mut self, team: TeamId, new: NewBot) -> Result<BotId, TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let name = check_name(&new.name)?;
        let id = BotId(self.fresh());
        let team = &mut self.teams[index];
        team.bots.push(Bot {
            id,
            name,
            role: new.role.trim().to_owned(),
            agent: agent_of_kind(&new.kind),
            kind_unavailable: false,
        });
        if team.lead.is_none() {
            team.lead = Some(id);
        }
        Ok(id)
    }

    pub fn create_project(&mut self, team: TeamId, name: &str) -> Result<ProjectId, TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let name = check_name(name)?;
        let id = ProjectId(self.fresh());
        self.teams[index].projects.push(Project {
            id,
            name,
            brief: String::new(),
            link: None,
            tasks: Vec::new(),
        });
        Ok(id)
    }

    pub fn save_brief(
        &mut self,
        team: TeamId,
        project: ProjectId,
        brief: &str,
    ) -> Result<(), TeamError> {
        self.require_connected()?;
        self.project_mut(team, project)?.brief = brief.trim().to_owned();
        Ok(())
    }

    /// Links a project's Board to a Workspaces project, or clears the link when `None`.
    pub fn link_project(
        &mut self,
        team: TeamId,
        project: ProjectId,
        link: Option<String>,
    ) -> Result<(), TeamError> {
        self.require_connected()?;
        self.project_mut(team, project)?.link = link;
        Ok(())
    }

    pub fn delete_project(&mut self, team: TeamId, project: ProjectId) -> Result<(), TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let team = &mut self.teams[index];
        let at = team
            .projects
            .iter()
            .position(|item| item.id == project)
            .ok_or(TeamError::Missing)?;
        team.projects.remove(at);
        if self
            .pick
            .as_ref()
            .is_some_and(|pick| pick.board == BoardOwner::Project(team.id, project))
        {
            self.pick = None;
        }
        Ok(())
    }

    pub fn set_routine_enabled(
        &mut self,
        team: TeamId,
        routine: RoutineId,
        enabled: bool,
    ) -> Result<(), TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let item = self.teams[index]
            .routines
            .iter_mut()
            .find(|item| item.id == routine)
            .ok_or(TeamError::Missing)?;
        item.enabled = enabled;
        Ok(())
    }

    pub fn delete_routine(&mut self, team: TeamId, routine: RoutineId) -> Result<(), TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let routines = &mut self.teams[index].routines;
        let at = routines
            .iter()
            .position(|item| item.id == routine)
            .ok_or(TeamError::Missing)?;
        routines.remove(at);
        Ok(())
    }

    pub fn delete_address(&mut self, team: TeamId, address: AddressId) -> Result<(), TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let addresses = &mut self.teams[index].addresses;
        let at = addresses
            .iter()
            .position(|item| item.id == address)
            .ok_or(TeamError::Missing)?;
        addresses.remove(at);
        Ok(())
    }

    pub fn delete_memory(&mut self, team: TeamId, memory: MemoryId) -> Result<(), TeamError> {
        self.require_connected()?;
        let index = self.index_of(team)?;
        let entries = &mut self.teams[index].memory;
        let at = entries
            .iter()
            .position(|item| item.id == memory)
            .ok_or(TeamError::Missing)?;
        entries.remove(at);
        Ok(())
    }

    /// The Board tasks of an owner, in column order as stored.
    pub fn tasks(&self, board: BoardOwner) -> Option<&[Task]> {
        match board {
            BoardOwner::Team(team) => self.team(team).map(|team| team.tasks.as_slice()),
            BoardOwner::Project(team, project) => self
                .team(team)?
                .project(project)
                .map(|project| project.tasks.as_slice()),
        }
    }

    pub fn tasks_mut(&mut self, board: BoardOwner) -> Option<&mut Vec<Task>> {
        match board {
            BoardOwner::Team(team) => {
                let index = self.index_of(team).ok()?;
                Some(&mut self.teams[index].tasks)
            }
            BoardOwner::Project(team, project) => {
                let index = self.index_of(team).ok()?;
                let at = self.teams[index]
                    .projects
                    .iter()
                    .position(|item| item.id == project)?;
                Some(&mut self.teams[index].projects[at].tasks)
            }
        }
    }

    pub fn set_pick(&mut self, pick: Option<Pick>) {
        self.pick = pick;
    }

    /// Takes the picked card back, leaving the Board as it is.
    pub fn take_pick(&mut self) -> Option<Pick> {
        self.pick.take()
    }

    /// Why the last card move was refused, if it was.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn set_notice(&mut self, notice: Option<String>) {
        self.notice = notice;
    }

    /// The team a page shows, when it belongs to a team that exists.
    pub fn page_team(&self, page: TeamPage) -> Option<&Team> {
        self.team(page.team())
    }

    pub(super) fn require_connected(&self) -> Result<(), TeamError> {
        if self.connected {
            Ok(())
        } else {
            Err(TeamError::Unavailable)
        }
    }

    fn index_of(&self, team: TeamId) -> Result<usize, TeamError> {
        self.teams
            .iter()
            .position(|item| item.id == team)
            .ok_or(TeamError::Missing)
    }

    fn project_mut(&mut self, team: TeamId, project: ProjectId) -> Result<&mut Project, TeamError> {
        let index = self.index_of(team)?;
        self.teams[index]
            .projects
            .iter_mut()
            .find(|item| item.id == project)
            .ok_or(TeamError::Missing)
    }

    fn fresh(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// Trims a name and checks its length; a blank name is refused.
pub fn check_name(name: &str) -> Result<String, TeamError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Err(TeamError::NameRequired)
    } else if trimmed.chars().count() > NAME_LIMIT {
        Err(TeamError::NameTooLong)
    } else {
        Ok(trimmed.to_owned())
    }
}

/// The largest id any item of these teams holds, so new ids start above it.
fn highest_id(teams: &[Team]) -> u32 {
    let mut highest = 0;
    let mut seen = |id: u32| highest = highest.max(id);
    for team in teams {
        seen(team.id.0);
        team.bots.iter().for_each(|bot| seen(bot.id.0));
        team.tasks.iter().for_each(|task| seen(task.id.0));
        team.routines.iter().for_each(|routine| seen(routine.id.0));
        team.addresses.iter().for_each(|address| seen(address.id.0));
        team.memory.iter().for_each(|entry| seen(entry.id.0));
        team.files.iter().for_each(|file| seen(file.id.0));
        for project in &team.projects {
            seen(project.id.0);
            project.tasks.iter().for_each(|task| seen(task.id.0));
        }
    }
    highest
}

#[cfg(test)]
mod tests {
    use super::super::sample::{SampleTeamsSource, TeamsSource};
    use super::super::types::{BoardOwner, TaskId};
    use super::*;
    use crate::model::shell::Agent;

    fn new_team(name: &str, host: usize) -> NewTeam {
        NewTeam {
            name: name.into(),
            purpose: "  Ops  ".into(),
            host,
            workspaces: true,
        }
    }

    #[test]
    fn a_team_needs_a_trimmed_name_up_to_the_limit_and_a_free_name_on_its_host() {
        let mut state = SampleTeamsSource.teams();
        assert_eq!(
            state.create_team(new_team("   ", 0)),
            Err(TeamError::NameRequired)
        );
        let long = "x".repeat(NAME_LIMIT + 1);
        assert_eq!(
            state.create_team(new_team(&long, 0)),
            Err(TeamError::NameTooLong)
        );
        assert_eq!(
            state.create_team(new_team("desarrollo", 1)),
            Err(TeamError::NameTaken),
            "the name is taken on VPS, case aside"
        );
        let id = state.create_team(new_team("  Ops  ", 0)).unwrap();
        let created = state.team(id).unwrap();
        assert_eq!(
            (created.name.as_str(), created.purpose.as_str()),
            ("Ops", "Ops")
        );
        assert!(created.workspaces && created.bots.is_empty());
        assert_eq!(state.selected(), Some(id));
    }

    #[test]
    fn the_first_bot_added_leads_the_team() {
        let mut state = SampleTeamsSource.teams();
        let id = state.create_team(new_team("Ops", 0)).unwrap();
        assert_eq!(state.team(id).unwrap().lead, None);
        let bot = state
            .add_bot(
                id,
                NewBot {
                    name: "Nora".into(),
                    role: String::new(),
                    kind: "codex".into(),
                },
            )
            .unwrap();
        let team = state.team(id).unwrap();
        assert_eq!(team.lead, Some(bot));
        assert_eq!(team.lead_name(), Some("Nora"));
        let nora = team.bots.iter().find(|b| b.id == bot).unwrap();
        assert_eq!(nora.agent, Some(Agent::Codex));
    }

    #[test]
    fn a_bot_of_a_kind_this_build_has_no_mark_for_gets_the_generic_mark() {
        let mut state = SampleTeamsSource.teams();
        let id = state.create_team(new_team("Ops", 0)).unwrap();
        let bot = state
            .add_bot(
                id,
                NewBot {
                    name: "Ada".into(),
                    role: String::new(),
                    kind: "my-own-kind".into(),
                },
            )
            .unwrap();
        let ada = state
            .team(id)
            .unwrap()
            .bots
            .iter()
            .find(|b| b.id == bot)
            .unwrap()
            .clone();
        assert_eq!(ada.agent, None);
    }

    #[test]
    fn deleting_a_project_removes_its_board_and_a_pick_on_it() {
        let mut state = SampleTeamsSource.teams();
        let team = TeamId(1);
        let project = ProjectId(1);
        state
            .pick_task(BoardOwner::Project(team, project), TaskId(7))
            .unwrap();
        state.delete_project(team, project).unwrap();
        assert!(state.team(team).unwrap().project(project).is_none());
        assert!(state.pick().is_none());
        assert_eq!(state.delete_project(team, project), Err(TeamError::Missing));
    }

    #[test]
    fn a_brief_is_saved_trimmed_and_a_link_can_be_set_and_cleared() {
        let mut state = SampleTeamsSource.teams();
        let (team, project) = (TeamId(1), ProjectId(1));
        state.save_brief(team, project, "  New brief  ").unwrap();
        state
            .link_project(team, project, Some("oppm".into()))
            .unwrap();
        let linked = state.team(team).unwrap().project(project).unwrap();
        assert_eq!(linked.brief, "New brief");
        assert_eq!(linked.link.as_deref(), Some("oppm"));
        state.link_project(team, project, None).unwrap();
        assert_eq!(
            state.team(team).unwrap().project(project).unwrap().link,
            None
        );
    }

    #[test]
    fn a_refresh_keeps_the_selected_team_and_the_picked_card_while_they_exist() {
        let mut state = SampleTeamsSource.teams();
        let task = state.team(TeamId(1)).unwrap().tasks[0].id;
        state.select(TeamId(1));
        state.pick_task(BoardOwner::Team(TeamId(1)), task).unwrap();
        let kept = state.refreshed(SampleTeamsSource.teams());
        assert_eq!(kept.selected(), Some(TeamId(1)));
        assert_eq!(kept.pick().map(|pick| pick.task), Some(task));
        let gone = state.refreshed(TeamsState::unavailable());
        assert_eq!(gone.selected(), None);
        assert!(gone.pick().is_none());
    }

    #[test]
    fn taking_a_pick_leaves_the_board_as_it_was() {
        let mut state = SampleTeamsSource.teams();
        let before = state.clone();
        let task = state.team(TeamId(1)).unwrap().tasks[0].id;
        state.pick_task(BoardOwner::Team(TeamId(1)), task).unwrap();
        assert!(state.take_pick().is_some());
        assert!(state.pick().is_none());
        assert_eq!(state.teams(), before.teams());
    }

    #[test]
    fn nothing_changes_without_a_connected_service() {
        let mut offline = TeamsState::unavailable();
        assert_eq!(
            offline.create_team(new_team("Ops", 0)),
            Err(TeamError::Unavailable)
        );
        assert_eq!(
            offline.save_brief(TeamId(1), ProjectId(1), "x"),
            Err(TeamError::Unavailable)
        );
        assert!(offline.teams().is_empty());
    }
}
