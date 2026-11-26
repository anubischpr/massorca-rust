//! The team pill's switch (A59, screen 4.3): the teams it offers and the page a choice opens.

use super::{Team, TeamId, TeamPage};

/// One team the switch offers; `current` marks the team the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamSwitchRow {
    pub team: TeamId,
    pub name: String,
    pub current: bool,
}

/// Every team, in the Sidebar's order, the page's own team marked.
pub fn team_switch_rows(teams: &[Team], current: TeamId) -> Vec<TeamSwitchRow> {
    teams
        .iter()
        .map(|team| TeamSwitchRow {
            team: team.id,
            name: team.name.clone(),
            current: team.id == current,
        })
        .collect()
}

impl TeamPage {
    /// The same page of `team`. A project belongs to one team, so switching from it opens the other team's Board.
    pub fn for_team(self, team: TeamId) -> TeamPage {
        match self {
            TeamPage::Conference(_) => TeamPage::Conference(team),
            TeamPage::Board(_) | TeamPage::Project(_, _) => TeamPage::Board(team),
            TeamPage::Routines(_) => TeamPage::Routines(team),
            TeamPage::Memory(_) => TeamPage::Memory(team),
            TeamPage::Files(_) => TeamPage::Files(team),
            TeamPage::NewRoutine(_) => TeamPage::NewRoutine(team),
            TeamPage::EditRoutine(_, _) | TeamPage::RoutineHistory(_, _) => {
                TeamPage::Routines(team)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::teams::{ProjectId, SampleTeamsSource, TeamsSource};

    #[test]
    fn a_switch_keeps_the_page_and_a_project_falls_back_to_the_board() {
        let other = TeamId(7);
        assert_eq!(
            TeamPage::Routines(TeamId(1)).for_team(other),
            TeamPage::Routines(other)
        );
        assert_eq!(
            TeamPage::Memory(TeamId(1)).for_team(other),
            TeamPage::Memory(other)
        );
        assert_eq!(
            TeamPage::Project(TeamId(1), ProjectId(3)).for_team(other),
            TeamPage::Board(other)
        );
    }

    #[test]
    fn the_switch_lists_every_team_and_marks_the_current_one() {
        let state = SampleTeamsSource.teams();
        let team = &state.teams()[0];
        let rows = team_switch_rows(state.teams(), team.id);
        assert_eq!(rows.len(), state.teams().len());
        assert_eq!(rows[0].name, team.name);
        assert!(rows[0].current);
        assert!(rows.iter().filter(|row| row.current).count() == 1);
    }
}
