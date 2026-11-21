//! ListRow: one row in a list or a tree (docs/design/components/ListRow.md).
//!
//! The row answers its own keys: Enter and Space arrive as a click, Right and
//! Left toggle a tree row, and Shift+F10 or the Menu key open its context menu.
//! Up, Down, Home and End pass through to the list, which owns the roving focus
//! (`list_move` and `move_index`).

use crate::tokens::{
    SIZE_CONTROL_COMFORTABLE, SIZE_CONTROL_COMPACT, SIZE_CONTROL_DEFAULT, SIZE_ROW_TWO_LINE,
    SPACE_3,
};

/// Indent of one tree level (`space.3`).
pub const TREE_INDENT: f32 = SPACE_3;

/// Lines of text in a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowLines {
    /// The secondary line sits beside the title.
    #[default]
    One,
    /// The secondary line sits under the title (`size.row.two-line`).
    Two,
}

/// Height of a one-line row (`size.control.*`). Two-line rows are always 44 px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowDensity {
    Compact,
    #[default]
    Default,
    Comfortable,
}

impl RowDensity {
    fn height(self) -> f32 {
        match self {
            Self::Compact => SIZE_CONTROL_COMPACT,
            Self::Default => SIZE_CONTROL_DEFAULT,
            Self::Comfortable => SIZE_CONTROL_COMFORTABLE,
        }
    }
}

/// The height of a row with `lines` lines at `density`.
pub fn row_height(lines: RowLines, density: RowDensity) -> f32 {
    match lines {
        RowLines::One => density.height(),
        RowLines::Two => SIZE_ROW_TWO_LINE,
    }
}

/// A move between the rows of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMove {
    Previous,
    Next,
    First,
    Last,
}

/// The move a key asks for in a list. Left and Right belong to tree rows.
pub fn list_move(key: &str) -> Option<ListMove> {
    match key {
        "up" => Some(ListMove::Previous),
        "down" => Some(ListMove::Next),
        "home" => Some(ListMove::First),
        "end" => Some(ListMove::Last),
        _ => None,
    }
}

/// The row a move lands on. Moves stop at the ends; an empty list has no row.
pub fn move_index(current: usize, count: usize, movement: ListMove) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match movement {
        ListMove::Previous => current.saturating_sub(1).min(last),
        ListMove::Next => current.saturating_add(1).min(last),
        ListMove::First => 0,
        ListMove::Last => last,
    })
}

/// The expanded value a key sets on a tree row: Right opens a closed row and Left
/// closes an open one. `None` leaves the key to the list (Left goes to the parent).
pub fn tree_toggle(expanded: Option<bool>, key: &str) -> Option<bool> {
    match (expanded, key) {
        (Some(false), "right") => Some(true),
        (Some(true), "left") => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_rows_follow_the_density_and_two_line_rows_are_44_px() {
        assert_eq!(row_height(RowLines::One, RowDensity::Compact), 24.0);
        assert_eq!(row_height(RowLines::One, RowDensity::Default), 28.0);
        assert_eq!(row_height(RowLines::One, RowDensity::Comfortable), 32.0);
        assert_eq!(row_height(RowLines::Two, RowDensity::Compact), 44.0);
    }

    #[test]
    fn moves_stop_at_both_ends_and_an_empty_list_has_no_row() {
        assert_eq!(move_index(0, 0, ListMove::Next), None);
        assert_eq!(move_index(0, 3, ListMove::Previous), Some(0));
        assert_eq!(move_index(0, 3, ListMove::Next), Some(1));
        assert_eq!(move_index(2, 3, ListMove::Next), Some(2));
        assert_eq!(move_index(1, 3, ListMove::First), Some(0));
        assert_eq!(move_index(1, 3, ListMove::Last), Some(2));
        assert_eq!(
            move_index(9, 3, ListMove::Previous),
            Some(2),
            "a stale index clamps"
        );
    }

    #[test]
    fn only_up_down_home_and_end_are_list_moves() {
        assert_eq!(list_move("up"), Some(ListMove::Previous));
        assert_eq!(list_move("down"), Some(ListMove::Next));
        assert_eq!(list_move("home"), Some(ListMove::First));
        assert_eq!(list_move("end"), Some(ListMove::Last));
        assert_eq!(list_move("right"), None);
        assert_eq!(list_move("enter"), None);
    }

    #[test]
    fn right_opens_a_closed_row_and_left_closes_an_open_one() {
        assert_eq!(tree_toggle(Some(false), "right"), Some(true));
        assert_eq!(tree_toggle(Some(true), "left"), Some(false));
        assert_eq!(
            tree_toggle(Some(true), "right"),
            None,
            "the list moves to the child"
        );
        assert_eq!(
            tree_toggle(Some(false), "left"),
            None,
            "the list moves to the parent"
        );
        assert_eq!(
            tree_toggle(None, "right"),
            None,
            "a plain row does not toggle"
        );
    }
}
