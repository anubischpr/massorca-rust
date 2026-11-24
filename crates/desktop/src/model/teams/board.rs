//! The Board's keyboard move (D-WS-30, BoardColumn): pick a card up, move it between columns and
//! places, then drop it or cancel. A drop changes the column only; the shell sends the call it asks for.

use super::filters::keeps;
use super::state::{TeamError, TeamsState};
use super::types::{BoardOwner, Column, Pick, Task, TaskId};
use crate::model::shell::HostReach;

/// One card of a Board column, as the page draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardView {
    pub id: TaskId,
    pub title: String,
    pub bot: Option<String>,
    pub time: Option<String>,
    pub note: Option<String>,
    pub reach: HostReach,
    /// Why the bot on the card cannot run; the card says so. `None` when it can run.
    pub kind_unavailable: Option<&'static str>,
    /// The card is picked up for a move and drawn raised.
    pub lifted: bool,
}

/// One column of a Board and its cards, in the order they show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnView {
    pub column: Column,
    pub cards: Vec<CardView>,
}

/// A Board as the page draws it: its four columns, with a picked card already in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardView {
    pub columns: Vec<ColumnView>,
}

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
        self.set_notice(None);
        let tasks = self.tasks(board).ok_or(TeamError::Missing)?;
        let at = tasks
            .iter()
            .position(|item| item.id == task && self.shows(board, item.id))
            .ok_or(TeamError::Missing)?;
        let column = tasks[at].column;
        let index = tasks[..at]
            .iter()
            .filter(|item| item.column == column && self.shows(board, item.id))
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
                .filter(|item| {
                    item.column == column && item.id != pick.task && self.shows(pick.board, item.id)
                })
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
        let shown = self.shown_ids(pick.board);
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
            .filter(|(_, item)| item.column == pick.column && shown.contains(&item.id))
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

    /// The Board as the page draws it. A picked card sits where the keys would drop it.
    pub fn board_view(&self, board: BoardOwner) -> Option<BoardView> {
        let team = match board {
            BoardOwner::Team(team) | BoardOwner::Project(team, _) => self.team(team)?,
        };
        let tasks = self.tasks(board)?;
        let pick = self.pick().filter(|pick| pick.board == board);
        let card = |task: &Task| CardView {
            id: task.id,
            title: task.title.clone(),
            bot: task
                .bot
                .and_then(|id| team.bot(id))
                .map(|bot| bot.name.clone()),
            time: task.time.clone(),
            note: task.note.clone(),
            reach: task.reach,
            kind_unavailable: task
                .bot
                .and_then(|id| team.bot(id))
                .and_then(|bot| bot.kind_unavailable),
            lifted: pick.is_some_and(|pick| pick.task == task.id),
        };
        let columns = Column::ALL
            .into_iter()
            .map(|column| {
                let mut cards: Vec<CardView> = tasks
                    .iter()
                    .filter(|task| {
                        task.column == column
                            && pick.is_none_or(|pick| pick.task != task.id)
                            && self.shows(board, task.id)
                    })
                    .map(card)
                    .collect();
                if let Some(pick) = pick.filter(|pick| pick.column == column)
                    && let Some(task) = tasks.iter().find(|task| task.id == pick.task)
                {
                    cards.insert(pick.index.min(cards.len()), card(task));
                }
                ColumnView { column, cards }
            })
            .collect();
        Some(BoardView { columns })
    }

    /// Whether the Board `board` draws the card: a team Board draws the cards its project filter keeps.
    fn shows(&self, board: BoardOwner, task: TaskId) -> bool {
        match board {
            BoardOwner::Team(team) => self
                .team(team)
                .is_some_and(|team| keeps(self.board_filter(team.id), team, task)),
            BoardOwner::Project(..) => true,
        }
    }

    /// The cards of a Board the page draws, in the order they are stored.
    fn shown_ids(&self, board: BoardOwner) -> Vec<TaskId> {
        self.tasks(board)
            .map(|tasks| {
                tasks
                    .iter()
                    .filter(|item| self.shows(board, item.id))
                    .map(|item| item.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Puts the picked card back without a change.
    pub fn cancel_pick(&mut self) {
        self.set_pick(None);
    }
}

#[cfg(test)]
mod tests {
    use super::super::filters::{ProjectFilter, desarrollo_with_project_card};
    use super::super::sample::SampleTeamsSource;
    use super::super::sample::TeamsSource;
    use super::super::types::{ProjectId, TeamId};
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

    #[test]
    fn the_board_view_draws_a_picked_card_in_the_place_the_keys_chose() {
        let mut state = SampleTeamsSource.teams();
        let board = BoardOwner::Team(team());
        let before = state.board_view(board).unwrap();
        assert_eq!(before.columns[0].cards.len(), 1);
        state.pick_task(board, TaskId(1)).unwrap();
        state.step_pick(PickStep::Right);
        let during = state.board_view(board).unwrap();
        assert!(
            during.columns[0].cards.is_empty(),
            "the source column keeps the others only"
        );
        let running = &during.columns[1].cards;
        assert_eq!(running.len(), 4);
        assert!(running[0].lifted && running[0].title == "Write release notes for oppm");
        assert_eq!(running[1].bot.as_deref(), Some("Mara"));
    }

    #[test]
    fn a_filtered_board_draws_and_picks_only_the_cards_its_filter_keeps() {
        let mut state = TeamsState::connected(Vec::new(), vec![desarrollo_with_project_card()]);
        let board = BoardOwner::Team(team());
        state
            .set_board_filter(team(), ProjectFilter::Project(ProjectId(1)))
            .unwrap();
        let drawn: usize = state
            .board_view(board)
            .unwrap()
            .columns
            .iter()
            .map(|column| column.cards.len())
            .sum();
        assert_eq!(drawn, 1, "only the project's card is drawn");
        assert_eq!(
            state.pick_task(board, TaskId(1)),
            Err(TeamError::Missing),
            "a card the filter hides cannot be picked up"
        );
        state.pick_task(board, TaskId(7)).unwrap();
        assert!(state.step_pick(PickStep::Right));
        assert_eq!(
            state.drop_pick().unwrap().announcement(),
            "Moved to Running, position 1 of 1."
        );
    }
}
