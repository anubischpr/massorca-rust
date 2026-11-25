//! The filters of the Bot Teams views: the Board's project filter and the Sidebar's option to hide finished
//! projects. View state only; the service holds neither (decision 0072).

use super::types::{Column, Project, ProjectId, TaskId, Team};

/// The cards a team's Board shows: all of them, those of one project, or those no project holds (pv96's chips).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProjectFilter {
    #[default]
    All,
    Project(ProjectId),
    NoProject,
}

/// One row of the Board's filter menu: its label, the filter it chooses, and whether it is the one in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardFilterRow {
    pub label: String,
    pub filter: ProjectFilter,
    pub checked: bool,
}

/// The Board filter menu's rows: all tasks, each project of the team, then the tasks no project holds.
pub fn board_filter_rows(team: &Team, current: ProjectFilter) -> Vec<BoardFilterRow> {
    let row = |label: String, filter: ProjectFilter| BoardFilterRow {
        label,
        filter,
        checked: filter == current,
    };
    let mut rows = vec![row("All tasks".to_owned(), ProjectFilter::All)];
    rows.extend(
        team.projects
            .iter()
            .map(|project| row(project.name.clone(), ProjectFilter::Project(project.id))),
    );
    rows.push(row("No project".to_owned(), ProjectFilter::NoProject));
    rows
}

/// Whether a project is finished: its Board holds cards and every one of them is Done.
pub fn is_finished_project(project: &Project) -> bool {
    !project.tasks.is_empty() && project.tasks.iter().all(|task| task.column == Column::Done)
}

/// Whether a Board under `filter` keeps `task` of `team`. A card belongs to the project whose Board holds it.
pub(super) fn keeps(filter: ProjectFilter, team: &Team, task: TaskId) -> bool {
    let owner = team
        .projects
        .iter()
        .find(|project| project.tasks.iter().any(|item| item.id == task))
        .map(|project| project.id);
    match filter {
        ProjectFilter::All => true,
        ProjectFilter::Project(project) => owner == Some(project),
        ProjectFilter::NoProject => owner.is_none(),
    }
}

/// Desarrollo with its project's card on the team Board too, as the service's Board holds every card.
#[cfg(test)]
pub(super) fn desarrollo_with_project_card() -> Team {
    use super::sample::{SampleTeamsSource, TeamsSource};
    let mut team = SampleTeamsSource.teams().teams()[0].clone();
    let project_card = team.projects[0].tasks[0].clone();
    team.tasks.push(project_card);
    team
}

#[cfg(test)]
mod tests {
    use super::super::state::{TeamError, TeamsState};
    use super::super::types::TeamId;
    use super::*;

    #[test]
    fn a_project_filter_keeps_only_the_cards_its_project_holds() {
        let team = desarrollo_with_project_card();
        let project = ProjectFilter::Project(ProjectId(1));
        assert!(keeps(project, &team, TaskId(7)));
        assert!(!keeps(project, &team, TaskId(1)));
        assert!(keeps(ProjectFilter::All, &team, TaskId(1)));
    }

    #[test]
    fn no_project_keeps_the_cards_outside_every_project() {
        let team = desarrollo_with_project_card();
        assert!(keeps(ProjectFilter::NoProject, &team, TaskId(1)));
        assert!(!keeps(ProjectFilter::NoProject, &team, TaskId(7)));
    }

    #[test]
    fn the_board_filter_menu_lists_all_each_project_and_no_project_and_marks_the_one_in_use() {
        let team = desarrollo_with_project_card();
        let rows = board_filter_rows(&team, ProjectFilter::Project(ProjectId(1)));
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["All tasks", "Desarrollo onboarding", "No project"]);
        let checked: Vec<bool> = rows.iter().map(|row| row.checked).collect();
        assert_eq!(checked, [false, true, false]);
    }

    #[test]
    fn a_project_is_finished_when_it_holds_cards_and_every_one_is_done() {
        let mut project: Project = desarrollo_with_project_card().projects[0].clone();
        assert!(!is_finished_project(&project), "its card is still Open");
        project.tasks[0].column = Column::Done;
        assert!(is_finished_project(&project));
        project.tasks.clear();
        assert!(
            !is_finished_project(&project),
            "a project with no cards is not finished"
        );
    }

    #[test]
    fn the_filter_is_kept_per_team_and_a_project_that_is_gone_shows_every_card() {
        let mut state = TeamsState::connected(Vec::new(), vec![desarrollo_with_project_card()]);
        state
            .set_board_filter(TeamId(1), ProjectFilter::Project(ProjectId(1)))
            .unwrap();
        assert_eq!(
            state.board_filter(TeamId(1)),
            ProjectFilter::Project(ProjectId(1))
        );
        state
            .set_board_filter(TeamId(1), ProjectFilter::Project(ProjectId(99)))
            .unwrap();
        assert_eq!(state.board_filter(TeamId(1)), ProjectFilter::All);
        assert_eq!(
            state.set_board_filter(TeamId(9), ProjectFilter::All),
            Err(TeamError::Missing)
        );
    }

    #[test]
    fn the_filters_survive_a_service_read() {
        let mut state = TeamsState::connected(Vec::new(), vec![desarrollo_with_project_card()]);
        state
            .set_board_filter(TeamId(1), ProjectFilter::NoProject)
            .unwrap();
        state.set_hide_finished_projects(true);
        let fresh = state.refreshed(TeamsState::connected(
            Vec::new(),
            vec![desarrollo_with_project_card()],
        ));
        assert_eq!(fresh.board_filter(TeamId(1)), ProjectFilter::NoProject);
        assert!(fresh.hides_finished_projects());
    }
}
