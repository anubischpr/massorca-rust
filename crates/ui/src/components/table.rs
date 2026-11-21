//! Table: a data table of comparable rows (docs/design/components/Table.md).
//!
//! `TableState` holds the rules without a window. This part covers the columns, the rows
//! and the sort; the cursor, selection and statuses follow in the next change.

use crate::components::agent_icon::AgentKind;
use crate::components::agent_status::AgentState;
use crate::icon::IconName;
use gpui::SharedString;
use std::cmp::Ordering;

/// Narrowest a column gets before the table scrolls sideways (Table.md, Responsive).
const COLUMN_MIN_WIDTH: f32 = 96.0;

/// Where a column's cells sit. Numeric columns align right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableAlign {
    #[default]
    Start,
    End,
}

/// A column: its header label, alignment, whether a click sorts it and its minimum width.
#[derive(Clone, Debug, PartialEq)]
pub struct TableColumn {
    pub label: SharedString,
    pub align: TableAlign,
    pub sortable: bool,
    pub min_width: f32,
}

impl TableColumn {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            align: TableAlign::Start,
            sortable: false,
            min_width: COLUMN_MIN_WIDTH,
        }
    }

    /// Right-aligned, for numbers.
    pub fn numeric(mut self) -> Self {
        self.align = TableAlign::End;
        self
    }

    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }

    pub fn min_width(mut self, pixels: f32) -> Self {
        self.min_width = pixels;
        self
    }
}

/// What one cell shows. Its variant also decides how the column sorts it.
#[derive(Clone, Debug, PartialEq)]
pub enum TableCell {
    /// Text, sorted alphabetically regardless of case.
    Text(SharedString),
    /// A number shown as `text` and sorted by `value`.
    Numeric { value: f64, text: SharedString },
    /// An agent's status glyph with its word; sorted by the word.
    Status(AgentState),
    /// An agent's mark only; its name is the tooltip and accessible label (A20).
    Agent(AgentKind),
    /// An icon button at the row end, drawn with a 24 px target. Never sorted.
    Action {
        icon: IconName,
        name: SharedString,
        disabled: bool,
    },
    /// Nothing; sorts before every other cell.
    Blank,
}

/// What a sort compares. Blanks and actions compare equal to each other, numbers come
/// before words, so any mix of cells still gives a total order.
#[derive(Clone, Copy)]
enum SortKey<'a> {
    Blank,
    Number(f64),
    Word(&'a str),
}

impl TableCell {
    fn sort_key(&self) -> SortKey<'_> {
        match self {
            Self::Blank | Self::Action { .. } => SortKey::Blank,
            Self::Numeric { value, .. } => SortKey::Number(*value),
            Self::Text(text) => SortKey::Word(text.as_ref()),
            Self::Status(state) => SortKey::Word(state.label()),
            Self::Agent(kind) => SortKey::Word(kind.label()),
        }
    }
}

fn sort_key_at(row: &TableRow, column: usize) -> SortKey<'_> {
    row.cells
        .get(column)
        .map_or(SortKey::Blank, |cell| cell.sort_key())
}

fn compare_keys(left: SortKey<'_>, right: SortKey<'_>) -> Ordering {
    use SortKey::{Blank, Number, Word};
    match (left, right) {
        (Blank, Blank) => Ordering::Equal,
        (Blank, _) => Ordering::Less,
        (_, Blank) => Ordering::Greater,
        (Number(a), Number(b)) => a.total_cmp(&b),
        (Number(_), Word(_)) => Ordering::Less,
        (Word(_), Number(_)) => Ordering::Greater,
        (Word(a), Word(b)) => a
            .chars()
            .flat_map(char::to_lowercase)
            .cmp(b.chars().flat_map(char::to_lowercase))
            .then_with(|| a.cmp(b)),
    }
}

/// One row. `key` names it in selection, checkboxes and events.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub key: SharedString,
    pub cells: Vec<TableCell>,
}

impl TableRow {
    pub fn new(key: impl Into<SharedString>, cells: Vec<TableCell>) -> Self {
        Self {
            key: key.into(),
            cells,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn word(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

/// The sorted column and its direction. No sort keeps the rows in the order they came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableSort {
    pub column: usize,
    pub direction: SortDirection,
}

/// The columns, rows and sort of a table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableState {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    /// Display order: indexes into `rows`.
    order: Vec<usize>,
    sort: Option<TableSort>,
}

impl TableState {
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow>) -> Self {
        let mut state = Self {
            columns,
            rows,
            order: Vec::new(),
            sort: None,
        };
        state.reorder();
        state
    }

    /// Starts sorted. A column that cannot sort is ignored.
    pub fn sorted_by(mut self, column: usize, direction: SortDirection) -> Self {
        if self.columns.get(column).is_some_and(|c| c.sortable) {
            self.sort = Some(TableSort { column, direction });
            self.reorder();
        }
        self
    }

    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// The body row at a display position; 0 is the top body row.
    pub fn row_at(&self, index: usize) -> Option<&TableRow> {
        self.order.get(index).map(|&row| &self.rows[row])
    }

    pub fn sort(&self) -> Option<TableSort> {
        self.sort
    }

    /// Replaces the rows and keeps the sort.
    pub fn set_rows(&mut self, rows: Vec<TableRow>) {
        self.rows = rows;
        self.reorder();
    }

    /// Clicking a sortable header: ascending, then descending, then none. Another column
    /// starts ascending. Returns false for a column that cannot sort.
    pub fn cycle_sort(&mut self, column: usize) -> bool {
        if !self.columns.get(column).is_some_and(|c| c.sortable) {
            return false;
        }
        self.sort = match self.sort {
            Some(TableSort {
                column: current,
                direction: SortDirection::Ascending,
            }) if current == column => Some(TableSort {
                column,
                direction: SortDirection::Descending,
            }),
            Some(TableSort {
                column: current,
                direction: SortDirection::Descending,
            }) if current == column => None,
            _ => Some(TableSort {
                column,
                direction: SortDirection::Ascending,
            }),
        };
        self.reorder();
        true
    }

    /// The sort word a header carries in its accessible label. GPUI has no aria-sort setter.
    pub fn sort_word(&self, column: usize) -> Option<&'static str> {
        self.sort
            .filter(|sort| sort.column == column)
            .map(|sort| sort.direction.word())
    }

    /// The line a screen reader hears after a sort change, such as "Sorted by Updated, descending."
    pub fn sort_announcement(&self) -> Option<String> {
        let sort = self.sort?;
        let column = self.columns.get(sort.column)?;
        Some(format!(
            "Sorted by {}, {}.",
            column.label,
            sort.direction.word()
        ))
    }

    fn reorder(&mut self) {
        let mut order: Vec<usize> = (0..self.rows.len()).collect();
        if let Some(TableSort { column, direction }) = self.sort {
            let rows = &self.rows;
            order.sort_by(|&a, &b| {
                let ordering =
                    compare_keys(sort_key_at(&rows[a], column), sort_key_at(&rows[b], column));
                match direction {
                    SortDirection::Ascending => ordering,
                    SortDirection::Descending => ordering.reverse(),
                }
            });
        }
        self.order = order;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> TableCell {
        TableCell::Text(value.into())
    }

    fn number(value: f64) -> TableCell {
        TableCell::Numeric {
            value,
            text: format!("{value}").into(),
        }
    }

    fn table() -> TableState {
        TableState::new(
            vec![
                TableColumn::new("Name").sortable(),
                TableColumn::new("Usage").numeric().sortable(),
                TableColumn::new("Actions"),
            ],
            vec![
                TableRow::new("b", vec![text("beta"), number(12.5), TableCell::Blank]),
                TableRow::new("a", vec![text("Alpha"), number(3.0), TableCell::Blank]),
                TableRow::new("c", vec![text("gamma"), number(40.0), TableCell::Blank]),
            ],
        )
    }

    fn keys(state: &TableState) -> Vec<String> {
        (0..state.row_count())
            .filter_map(|index| state.row_at(index))
            .map(|row| row.key.to_string())
            .collect()
    }

    #[test]
    fn a_header_click_cycles_ascending_descending_then_none() {
        let mut state = table();
        assert!(state.cycle_sort(0));
        assert_eq!(
            state.sort(),
            Some(TableSort {
                column: 0,
                direction: SortDirection::Ascending
            })
        );
        assert_eq!(keys(&state), ["a", "b", "c"]);
        state.cycle_sort(0);
        assert_eq!(keys(&state), ["c", "b", "a"]);
        state.cycle_sort(0);
        assert_eq!(state.sort(), None);
        assert_eq!(
            keys(&state),
            ["b", "a", "c"],
            "no sort keeps the source order"
        );
    }

    #[test]
    fn another_column_starts_ascending_and_an_unsortable_one_is_ignored() {
        let mut state = table();
        state.cycle_sort(0);
        state.cycle_sort(0);
        assert!(state.cycle_sort(1));
        assert_eq!(
            state.sort(),
            Some(TableSort {
                column: 1,
                direction: SortDirection::Ascending
            })
        );
        assert!(!state.cycle_sort(2), "the actions column does not sort");
        assert_eq!(state.sort().map(|sort| sort.column), Some(1));
    }

    #[test]
    fn sorted_by_starts_sorted_and_ignores_an_unsortable_column() {
        let state = table().sorted_by(0, SortDirection::Descending);
        assert_eq!(keys(&state), ["c", "b", "a"]);
        assert_eq!(table().sorted_by(2, SortDirection::Ascending).sort(), None);
    }

    #[test]
    fn numbers_sort_by_value() {
        let mut state = table();
        state.cycle_sort(1);
        assert_eq!(keys(&state), ["a", "b", "c"], "3, 12.5, 40");
    }

    #[test]
    fn words_sort_regardless_of_case() {
        let mut state = TableState::new(
            vec![TableColumn::new("Name").sortable()],
            vec![
                TableRow::new("z", vec![text("Zulu")]),
                TableRow::new("a", vec![text("alpha")]),
                TableRow::new("m", vec![text("Mike")]),
            ],
        );
        state.cycle_sort(0);
        assert_eq!(keys(&state), ["a", "m", "z"]);
    }

    #[test]
    fn equal_cells_keep_their_order_in_both_directions() {
        let mut state = TableState::new(
            vec![TableColumn::new("Usage").numeric().sortable()],
            vec![
                TableRow::new("x", vec![number(5.0)]),
                TableRow::new("y", vec![number(5.0)]),
                TableRow::new("z", vec![number(1.0)]),
            ],
        );
        state.cycle_sort(0);
        assert_eq!(keys(&state), ["z", "x", "y"]);
        state.cycle_sort(0);
        assert_eq!(keys(&state), ["x", "y", "z"], "descending keeps x before y");
    }

    #[test]
    fn mixed_cells_sort_in_a_total_order_with_blanks_first() {
        let mut state = TableState::new(
            vec![TableColumn::new("Mix").sortable()],
            vec![
                TableRow::new("w", vec![text("word")]),
                TableRow::new("n", vec![number(2.0)]),
                TableRow::new("b", vec![TableCell::Blank]),
            ],
        );
        state.cycle_sort(0);
        assert_eq!(keys(&state), ["b", "n", "w"]);
    }

    #[test]
    fn a_sort_change_is_announced_with_its_column_and_direction() {
        let mut state = table();
        assert_eq!(state.sort_announcement(), None);
        state.cycle_sort(1);
        state.cycle_sort(1);
        assert_eq!(
            state.sort_announcement().as_deref(),
            Some("Sorted by Usage, descending.")
        );
        assert_eq!(state.sort_word(1), Some("descending"));
        assert_eq!(state.sort_word(0), None);
    }
}
