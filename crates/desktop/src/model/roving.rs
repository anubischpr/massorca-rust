//! Roving focus in a composite control: one tab stop, and the arrow keys move it (handoff 2.2).

/// A move between the items of a composite control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusStep {
    Previous,
    Next,
    First,
    Last,
}

/// The item a step lands on. Arrows stop at the ends instead of wrapping.
pub fn step(index: usize, len: usize, movement: FocusStep) -> usize {
    let Some(last) = len.checked_sub(1) else {
        return 0;
    };
    match movement {
        FocusStep::Previous => index.saturating_sub(1).min(last),
        FocusStep::Next => (index + 1).min(last),
        FocusStep::First => 0,
        FocusStep::Last => last,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_move_one_item_and_stop_at_the_ends() {
        assert_eq!(step(1, 3, FocusStep::Next), 2);
        assert_eq!(step(2, 3, FocusStep::Next), 2);
        assert_eq!(step(1, 3, FocusStep::Previous), 0);
        assert_eq!(step(0, 3, FocusStep::Previous), 0);
    }

    #[test]
    fn home_and_end_jump_to_the_ends() {
        assert_eq!(step(2, 4, FocusStep::First), 0);
        assert_eq!(step(1, 4, FocusStep::Last), 3);
    }

    #[test]
    fn an_empty_group_stays_at_zero() {
        assert_eq!(step(0, 0, FocusStep::Next), 0);
        assert_eq!(step(0, 0, FocusStep::Last), 0);
    }
}
