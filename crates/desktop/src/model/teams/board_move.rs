//! What a card's move asks the service for (decision 0072): a drop into another column starts, stops,
//! finishes or reopens its task, or says why the service cannot make that move from here.

use super::types::Column;

/// Shown when a card is dropped on Waiting from Open: only a run can leave a card waiting for you.
pub const WAIT_NEEDS_RUN: &str = "A card waits for you only after its bot has run. Start it first.";
/// Shown when a running card is dropped on Open: the run has to end before the card can return.
pub const STOP_FIRST: &str = "Move the card to Waiting to stop its bot first, then to Open.";
/// Shown when a stopped or finished card is dropped on Running: a start only begins from Open.
pub const START_AGAIN: &str = "Move the card to Open first, then to Running to start it again.";
/// Shown when a finished card is dropped on Waiting: a finished card does not wait for anyone.
pub const DONE_NOT_WAITING: &str = "A finished card cannot wait for you. Move it to Open first.";

/// The call a drop makes, or the refusal the Board shows instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardMove {
    /// Dropped in the column it came from: nothing to send.
    Stay,
    /// `task.start`: the bot starts the task (Open to Running).
    Start,
    /// `task.stop`: the bot's run ends and the card waits for you (Running to Waiting).
    Stop,
    /// `task.done`: the card is marked Done. The service refuses while its run still goes on.
    Finish,
    /// `task.reopen`: the card returns to Open (Done or Waiting to Open).
    Reopen,
    /// No call: the words the Board shows.
    Refused(&'static str),
}

/// The answer to a card dropped in `to` after it was picked up in `from`.
pub fn board_move(from: Column, to: Column) -> BoardMove {
    use Column::{Done, Open, Running, Waiting};
    match (from, to) {
        (Open, Open) | (Running, Running) | (Waiting, Waiting) | (Done, Done) => BoardMove::Stay,
        (Open, Running) => BoardMove::Start,
        (Running, Waiting) => BoardMove::Stop,
        (_, Done) => BoardMove::Finish,
        (Waiting | Done, Open) => BoardMove::Reopen,
        (Running, Open) => BoardMove::Refused(STOP_FIRST),
        (Open, Waiting) => BoardMove::Refused(WAIT_NEEDS_RUN),
        (Waiting | Done, Running) => BoardMove::Refused(START_AGAIN),
        (Done, Waiting) => BoardMove::Refused(DONE_NOT_WAITING),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_dropped_in_its_own_column_sends_nothing() {
        for column in Column::ALL {
            assert_eq!(board_move(column, column), BoardMove::Stay);
        }
    }

    #[test]
    fn the_board_answers_every_move_between_two_columns() {
        use Column::{Done, Open, Running, Waiting};
        let table = [
            (Open, Running, BoardMove::Start),
            (Open, Waiting, BoardMove::Refused(WAIT_NEEDS_RUN)),
            (Open, Done, BoardMove::Finish),
            (Running, Open, BoardMove::Refused(STOP_FIRST)),
            (Running, Waiting, BoardMove::Stop),
            (Running, Done, BoardMove::Finish),
            (Waiting, Open, BoardMove::Reopen),
            (Waiting, Running, BoardMove::Refused(START_AGAIN)),
            (Waiting, Done, BoardMove::Finish),
            (Done, Open, BoardMove::Reopen),
            (Done, Running, BoardMove::Refused(START_AGAIN)),
            (Done, Waiting, BoardMove::Refused(DONE_NOT_WAITING)),
        ];
        for (from, to, expected) in table {
            assert_eq!(board_move(from, to), expected, "from {from:?} to {to:?}");
        }
    }
}
