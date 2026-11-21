//! The Board's keyboard move (D-WS-30, BoardColumn): pick a card up, move it between columns and
//! places, then drop it or cancel. A move changes the column only; it starts and stops nothing.

use super::state::{TeamError, TeamsState};
use super::types::{BoardOwner, Column, Pick, TaskId};

/// An arrow key, as the Board reads it while a card is picked up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickStep {
    Left,
    Right,
    Up,
    Down,
}

/// Where a card landed, for the announcement "Moved to Running, position 2 of 3."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub column: Column,
    /// Counted from 1 among the cards in the column.
    pub position: usize,
    pub total: usize,
}

impl Landing {
    pub fn announcement(self) -> String {
        format!(
            "Moved to {}, position {} of {}.",
            self.column.title(),
            self.position,
            self.total
        )
    }
}

impl TeamsState {
    /// Picks a card up. Only one card can be picked up at a time.
    pub fn pick_task(&mut self, board: BoardOwner, task: TaskId) -> Result<(), TeamError> {
        self.require_connected()?;
        let tasks = self.tasks(board).ok_or(TeamError::Missing)?;
        let at = tasks
            .iter()
            .position(|item| item.id == task)
            .ok_or(TeamError::Missing)?;
        let column = tasks[at].column;
        let index = tasks[..at]
            .iter()
            .filter(|item| item.column == column)
            .count();
        self.set_pick(Some(Pick {
            board,
            task,
            column,
            index,
        }));
        Ok(())
    }

    /// Moves the picked card one step. Returns false at an edge or with nothing picked up.
    pub fn step_pick(&mut self, step: PickStep) -> bool {
        let Some(pick) = self.pick().cloned() else {
            return false;
        };
        let Some(tasks) = self.tasks(pick.board) else {
            return false;
        };
        let others = |column: Column| {
            tasks
                .iter()
                .filter(|item| item.column == column && item.id != pick.task)
                .count()
        };
        let next = match step {
            PickStep::Left | PickStep::Right => {
                let column = match step {
                    PickStep::Left => pick.column.previous(),
                    _ => pick.column.next(),
                };
                column.map(|column| (column, pick.index.min(others(column))))
            }
            PickStep::Up => (pick.index > 0).then(|| (pick.column, pick.index - 1)),
            PickStep::Down => {
                let last = others(pick.column);
                (pick.index < last).then(|| (pick.column, pick.index + 1))
            }
        };
        let Some((column, index)) = next else {
            return false;
        };
        self.set_pick(Some(Pick {
            column,
            index,
            ..pick
        }));
        true
    }

    /// Drops the picked card where it stands and returns where it landed.
    pub fn drop_pick(&mut self) -> Result<Landing, TeamError> {
        let pick = self.pick().cloned().ok_or(TeamError::Missing)?;
        self.set_pick(None);
        let tasks = self.tasks_mut(pick.board).ok_or(TeamError::Missing)?;
        let at = tasks
            .iter()
            .position(|item| item.id == pick.task)
            .ok_or(TeamError::Missing)?;
        let mut task = tasks.remove(at);
        task.column = pick.column;
        let places: Vec<usize> = tasks
            .iter()
            .enumerate()
            .filter(|(_, item)| item.column == pick.column)
            .map(|(position, _)| position)
            .collect();
        let insert_at = places
            .get(pick.index)
            .copied()
            .or_else(|| places.last().map(|last| last + 1))
            .unwrap_or(tasks.len());
        tasks.insert(insert_at, task);
        Ok(Landing {
            column: pick.column,
            position: pick.index + 1,
            total: places.len() + 1,
        })
    }

    /// Puts the picked card back without a change.
    pub fn cancel_pick(&mut self) {
        self.set_pick(None);
    }
}

#[cfg(test)]
mod tests {
    use super::super::sample::SampleTeamsSource;
    use super::super::sample::TeamsSource;
    use super::super::types::TeamId;
    use super::*;

    fn team() -> TeamId {
        TeamId(1)
    }

    fn running_titles(state: &TeamsState) -> Vec<String> {
        state.teams()[0]
            .tasks
            .iter()
            .filter(|task| task.column == Column::Running)
            .map(|task| task.title.clone())
            .collect()
    }

    #[test]
    fn a_picked_card_moves_between_columns_and_lands_where_the_keys_put_it() {
        let mut state = SampleTeamsSource.teams();
        // "Write release notes for oppm" is the only card in Open.
        state
            .pick_task(BoardOwner::Team(team()), TaskId(1))
            .unwrap();
        assert!(state.step_pick(PickStep::Right));
        assert_eq!(state.pick().unwrap().column, Column::Running);
        // Running holds three cards; the lifted one lands at the end of them.
        for _ in 0..3 {
            assert!(state.step_pick(PickStep::Down));
        }
        assert!(
            !state.step_pick(PickStep::Down),
            "the column ends at its last card"
        );
        let landing = state.drop_pick().unwrap();
        assert_eq!(landing.announcement(), "Moved to Running, position 4 of 4.");
        assert_eq!(
            running_titles(&state).last().map(String::as_str),
            Some("Write release notes for oppm")
        );
        assert!(state.pick().is_none());
    }

    #[test]
    fn moving_past_the_first_or_last_column_does_nothing() {
        let mut state = SampleTeamsSource.teams();
        state
            .pick_task(BoardOwner::Team(team()), TaskId(1))
            .unwrap();
        assert!(!state.step_pick(PickStep::Left));
        assert!(!state.step_pick(PickStep::Up));
        assert_eq!(state.pick().unwrap().column, Column::Open);
    }

    #[test]
    fn cancelling_a_pick_leaves_the_board_as_it_was() {
        let mut state = SampleTeamsSource.teams();
        let before = state.clone();
        state
            .pick_task(BoardOwner::Team(team()), TaskId(1))
            .unwrap();
        state.step_pick(PickStep::Right);
        state.cancel_pick();
        assert_eq!(state.teams(), before.teams());
        assert!(state.pick().is_none());
    }

    #[test]
    fn a_pick_needs_a_connected_service_and_an_existing_card() {
        let mut offline = TeamsState::unavailable();
        assert_eq!(
            offline.pick_task(BoardOwner::Team(team()), TaskId(1)),
            Err(TeamError::Unavailable)
        );
        let mut state = SampleTeamsSource.teams();
        assert_eq!(
            state.pick_task(BoardOwner::Team(team()), TaskId(999)),
            Err(TeamError::Missing)
        );
    }

    #[test]
    fn the_announcement_names_the_column_and_the_place() {
        let landing = Landing {
            column: Column::Running,
            position: 2,
            total: 3,
        };
        assert_eq!(landing.announcement(), "Moved to Running, position 2 of 3.");
    }
}
