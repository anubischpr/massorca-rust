//! Table: a data table of comparable rows (docs/design/components/Table.md).
//!
//! `TableState` holds the rules without a window: the sort cycle, the roving cursor,
//! one selected row, row checkboxes and the loading, empty and error statuses. `TableView`
//! draws them and turns keys and clicks into events. Rows carry their own keys, so
//! selection and checkboxes survive a re-sort.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::checkbox::Checkbox;
use crate::components::empty_state::{EmptyState, EmptyStateVariant};
use crate::components::error_state::{ErrorState, ErrorStateVariant};
use crate::components::icon_button::IconButton;
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::components::tooltip::{Tooltip, tooltip_view};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    BORDER_WIDTH_THICK, Color, Palette, SIZE_CONTROL_COMFORTABLE, SIZE_CONTROL_DEFAULT, SPACE_1,
    SPACE_2, TYPE_BODY, TYPE_LABEL,
};
use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, Entity, EventEmitter, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Role, ScrollStrategy,
    SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div, px,
    uniform_list,
};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;

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

/// The roving cursor. Row 0 is the header row; rows 1.. are the body rows in display order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub column: usize,
}

/// A move between cells. Home and End span the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellMove {
    Up,
    Down,
    Left,
    Right,
    RowStart,
    RowEnd,
}

/// The keys that act on the cursor's cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Enter,
    Space,
}

/// What a press asks the table to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activation {
    /// Sort by a sortable header column.
    Sort(usize),
    /// Open a body row (Enter).
    Open(SharedString),
    /// Toggle a body row's checkbox (Space, in a selectable table).
    Check(SharedString),
}

/// The move a key asks for in a table, if any.
pub fn cell_move(key: &str) -> Option<CellMove> {
    match key {
        "up" => Some(CellMove::Up),
        "down" => Some(CellMove::Down),
        "left" => Some(CellMove::Left),
        "right" => Some(CellMove::Right),
        "home" => Some(CellMove::RowStart),
        "end" => Some(CellMove::RowEnd),
        _ => None,
    }
}

/// What the body shows in place of rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TableStatus {
    #[default]
    Ready,
    /// Five static skeleton rows, with no shimmer.
    Loading,
    /// An empty state in the body, with an optional action.
    Empty {
        title: SharedString,
        explanation: Option<SharedString>,
        action: Option<SharedString>,
    },
    /// "Could not load this table." with Retry; the headers stay.
    Error,
}

/// The rules of a table: sort, cursor, selection, checkboxes and status.
#[derive(Clone, Debug, PartialEq)]
pub struct TableState {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    /// Display order: indexes into `rows`.
    order: Vec<usize>,
    sort: Option<TableSort>,
    selected: Option<SharedString>,
    checked: HashSet<SharedString>,
    selectable: bool,
    cursor: Cursor,
    status: TableStatus,
}

impl TableState {
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow>) -> Self {
        let mut state = Self {
            columns,
            rows,
            order: Vec::new(),
            sort: None,
            selected: None,
            checked: HashSet::new(),
            selectable: false,
            cursor: Cursor::default(),
            status: TableStatus::Ready,
        };
        state.reorder();
        state
    }

    /// Adds the selection column, with a checkbox per row.
    pub fn selectable(mut self) -> Self {
        self.selectable = true;
        self
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

    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }

    pub fn is_selectable(&self) -> bool {
        self.selectable
    }

    pub fn is_checked(&self, key: &SharedString) -> bool {
        self.checked.contains(key)
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn status(&self) -> &TableStatus {
        &self.status
    }

    pub fn set_status(&mut self, status: TableStatus) {
        self.status = status;
    }

    /// Replaces the rows. The sort, selection and checkboxes stay where their keys still exist.
    pub fn set_rows(&mut self, rows: Vec<TableRow>) {
        self.rows = rows;
        if self
            .selected
            .as_ref()
            .is_some_and(|key| !self.contains(key))
        {
            self.selected = None;
        }
        self.checked
            .retain(|key| self.rows.iter().any(|row| &row.key == key));
        self.reorder();
        self.clamp_cursor();
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

    pub fn move_cursor(&mut self, movement: CellMove) {
        let last_row = self.rows.len();
        let last_column = self.columns.len().saturating_sub(1);
        let Cursor { row, column } = self.cursor;
        self.cursor = Cursor {
            row: match movement {
                CellMove::Up => row.saturating_sub(1),
                CellMove::Down => (row + 1).min(last_row),
                _ => row,
            },
            column: match movement {
                CellMove::Left => column.saturating_sub(1),
                CellMove::Right => (column + 1).min(last_column),
                CellMove::RowStart => 0,
                CellMove::RowEnd => last_column,
                _ => column,
            },
        };
    }

    /// Moves the cursor onto a body row; a pointer click does this too.
    pub fn focus_row(&mut self, key: &SharedString) -> bool {
        match self.position_of(key) {
            Some(index) => {
                self.cursor.row = index + 1;
                true
            }
            None => false,
        }
    }

    /// What Enter or Space does at the cursor. Enter or Space on a sortable header sorts;
    /// Enter opens a body row; Space toggles its checkbox in a selectable table.
    pub fn activate(&self, press: Press) -> Option<Activation> {
        let Cursor { row, column } = self.cursor;
        if row == 0 {
            return self
                .columns
                .get(column)
                .filter(|c| c.sortable)
                .map(|_| Activation::Sort(column));
        }
        let key = self.row_at(row - 1)?.key.clone();
        match press {
            Press::Enter => Some(Activation::Open(key)),
            Press::Space => self.selectable.then_some(Activation::Check(key)),
        }
    }

    /// Selects a body row. Selection is single in v1.
    pub fn select(&mut self, key: &SharedString) -> bool {
        if !self.contains(key) {
            return false;
        }
        self.selected = Some(key.clone());
        true
    }

    /// Toggles a row's checkbox and returns its new state. `None` when the table has no
    /// selection column or no such row.
    pub fn toggle_check(&mut self, key: &SharedString) -> Option<bool> {
        if !self.selectable || !self.contains(key) {
            return None;
        }
        if self.checked.remove(key) {
            Some(false)
        } else {
            self.checked.insert(key.clone());
            Some(true)
        }
    }

    fn contains(&self, key: &SharedString) -> bool {
        self.rows.iter().any(|row| &row.key == key)
    }

    fn position_of(&self, key: &SharedString) -> Option<usize> {
        self.order
            .iter()
            .position(|&row| &self.rows[row].key == key)
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

    fn clamp_cursor(&mut self) {
        self.cursor.row = self.cursor.row.min(self.rows.len());
        self.cursor.column = self.cursor.column.min(self.columns.len().saturating_sub(1));
    }
}

/// Row height of a table (Table.md, Variants and sizes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableDensity {
    /// 28 px rows (`size.control.default`).
    #[default]
    Default,
    /// 32 px rows (`size.control.comfortable`).
    Comfortable,
}

impl TableDensity {
    fn height(self) -> f32 {
        match self {
            Self::Default => SIZE_CONTROL_DEFAULT,
            Self::Comfortable => SIZE_CONTROL_COMFORTABLE,
        }
    }
}

/// What a table reports to its owner. The table has already sorted, selected or toggled
/// the row by the time the owner hears of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableEvent {
    /// The sort changed; `None` clears it.
    Sorted(Option<TableSort>),
    /// A body row was clicked and is now selected.
    Selected(SharedString),
    /// Enter opened a body row.
    Opened(SharedString),
    /// A row's checkbox changed.
    Checked { row: SharedString, checked: bool },
    /// A row's action button was pressed.
    Action { row: SharedString, column: usize },
    /// The empty state's action was pressed.
    EmptyAction,
    /// Retry was pressed in the error state.
    Retry,
}

/// Fully transparent: no fill at rest, and no focus ring until the cell has the cursor.
const CLEAR: Color = Color::rgba8(0, 0, 0, 0.0);
/// Skeleton rows shown while the rows load (Table.md, States).
const LOADING_ROWS: usize = 5;
/// The selection column is one 28 px control wide.
const SELECTION_WIDTH: f32 = SIZE_CONTROL_DEFAULT;

/// A data table. The entity owns its [`TableState`] and emits [`TableEvent`]s; the owner
/// subscribes with `cx.subscribe`.
pub struct TableView {
    id: ElementId,
    label: SharedString,
    state: TableState,
    density: TableDensity,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<TableEvent> for TableView {}

impl fmt::Debug for TableView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableView")
            .field("id", &self.id)
            .field("cursor", &self.state.cursor())
            .field("status", self.state.status())
            .finish_non_exhaustive()
    }
}

impl TableView {
    /// `label` names the table for assistive tech, usually its view heading.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        state: TableState,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state,
            density: TableDensity::default(),
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    pub fn density(mut self, density: TableDensity) -> Self {
        self.density = density;
        self
    }

    pub fn state(&self) -> &TableState {
        &self.state
    }

    /// The keyboard focus of the table, which is one Tab stop.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// Replaces the rows. The sort, selection and checkboxes stay where their keys still exist.
    pub fn set_rows(&mut self, rows: Vec<TableRow>, cx: &mut Context<Self>) {
        self.state.set_rows(rows);
        cx.notify();
    }

    pub fn set_status(&mut self, status: TableStatus, cx: &mut Context<Self>) {
        self.state.set_status(status);
        cx.notify();
    }

    fn sort_by(&mut self, column: usize, cx: &mut Context<Self>) {
        if self.state.cycle_sort(column) {
            cx.emit(TableEvent::Sorted(self.state.sort()));
            cx.notify();
        }
    }

    fn select_row(&mut self, key: &SharedString, cx: &mut Context<Self>) {
        if self.state.select(key) {
            self.state.focus_row(key);
            cx.emit(TableEvent::Selected(key.clone()));
            cx.notify();
        }
    }

    fn toggle_check(&mut self, key: &SharedString, cx: &mut Context<Self>) {
        if let Some(checked) = self.state.toggle_check(key) {
            cx.emit(TableEvent::Checked {
                row: key.clone(),
                checked,
            });
            cx.notify();
        }
    }

    fn press(&mut self, press: Press, cx: &mut Context<Self>) {
        match self.state.activate(press) {
            Some(Activation::Sort(column)) => self.sort_by(column, cx),
            Some(Activation::Open(row)) => cx.emit(TableEvent::Opened(row)),
            Some(Activation::Check(row)) => self.toggle_check(&row, cx),
            None => {}
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let handled = match (cell_move(key), key) {
            (Some(movement), _) => {
                self.state.move_cursor(movement);
                let row = self.state.cursor().row;
                if row > 0 {
                    self.scroll.scroll_to_item(row - 1, ScrollStrategy::Nearest);
                }
                true
            }
            (None, "enter") => {
                self.press(Press::Enter, cx);
                true
            }
            (None, "space") => {
                self.press(Press::Space, cx);
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn header_cell(
        &self,
        index: usize,
        column: &TableColumn,
        focused: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = theme.palette;
        let at_cursor = focused
            && self.state.cursor()
                == (Cursor {
                    row: 0,
                    column: index,
                });
        let direction = self
            .state
            .sort()
            .filter(|sort| sort.column == index)
            .map(|sort| sort.direction);
        let label = match self.state.sort_word(index) {
            Some(word) => format!("{}, sorted {word}", column.label),
            None => column.label.to_string(),
        };
        let mut cell = div()
            .id(("table-header", index))
            .debug_selector(|| format!("table-header-{index}"))
            .role(Role::ColumnHeader)
            .aria_label(label)
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .flex_1()
            .min_w(px(column.min_width))
            .min_w_0()
            .h_full()
            .px(px(SPACE_2))
            .border_2()
            .border_color(if at_cursor { p.border.focus } else { CLEAR })
            .text_style(TYPE_LABEL)
            .text_color(p.text.secondary);
        if column.align == TableAlign::End {
            cell = cell.justify_end();
        }
        if column.sortable {
            cell = cell
                .cursor_pointer()
                .hover(move |s| s.bg(p.fill.hover_canvas))
                .active(move |s| s.bg(p.fill.pressed_canvas))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.sort_by(index, cx);
                }));
        }
        let glyph = direction.map(|direction| match direction {
            SortDirection::Ascending => IconName::ChevronUp,
            SortDirection::Descending => IconName::ChevronDown,
        });
        cell.child(div().min_w_0().truncate().child(column.label.clone()))
            .children(glyph.map(|glyph| {
                Icon::new(glyph)
                    .size(IconSize::XSmall)
                    .color(p.text.secondary)
            }))
            .into_any_element()
    }

    fn render_row(
        &self,
        entity: &Entity<Self>,
        index: usize,
        focused: bool,
        cx: &App,
    ) -> AnyElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let Some(row) = self.state.row_at(index) else {
            return div().into_any_element();
        };
        let key = row.key.clone();
        let selected = self.state.selected() == Some(&key);
        let fills = Surface::Canvas.fills(&theme);
        let (rest, hover) = if selected {
            (fills.selected, fills.selected)
        } else {
            (CLEAR, fills.hover)
        };
        let cursor = self.state.cursor();
        let column_count = self.state.columns().len();
        let mut line = div()
            .id(("table-row", index))
            .debug_selector(|| format!("table-row-{index}"))
            .role(Role::Row)
            .aria_row_index(index + 2)
            .aria_selected(selected)
            .flex()
            .items_center()
            .w_full()
            .h(px(self.density.height()))
            .border_b_1()
            .border_color(p.border.subtle)
            .bg(rest)
            .hover(move |s| s.bg(hover))
            .active(move |s| s.bg(fills.pressed))
            .cursor_pointer()
            .on_click({
                let (entity, key) = (entity.clone(), key.clone());
                move |_, _, cx| entity.update(cx, |table, cx| table.select_row(&key, cx))
            });
        if selected {
            line = line.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(px(BORDER_WIDTH_THICK))
                    .bg(p.accent.fg),
            );
        }
        if self.state.is_selectable() {
            line = line.child(self.check_cell(&key, index, entity));
        }
        for (column_index, column) in self.state.columns().iter().enumerate() {
            let at_cursor = focused
                && cursor
                    == (Cursor {
                        row: index + 1,
                        column: column_index,
                    });
            let cell_id = index * column_count + column_index;
            let mut cell = div()
                .id(("table-cell", cell_id))
                .role(Role::Cell)
                .flex()
                .items_center()
                .flex_1()
                .min_w(px(column.min_width))
                .min_w_0()
                .h_full()
                .px(px(SPACE_2))
                .border_2()
                .border_color(if at_cursor { p.border.focus } else { CLEAR });
            if column.align == TableAlign::End {
                cell = cell.justify_end();
            }
            if let Some(content) = row.cells.get(column_index) {
                cell = cell.child(Self::cell_content(
                    content,
                    &key,
                    cell_id,
                    column_index,
                    entity,
                    &p,
                ));
            }
            line = line.child(cell);
        }
        line.into_any_element()
    }

    fn check_cell(&self, key: &SharedString, index: usize, entity: &Entity<Self>) -> AnyElement {
        let (entity, row) = (entity.clone(), key.clone());
        div()
            .id(("table-check-slot", index))
            .flex_none()
            .w(px(SELECTION_WIDTH))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                Checkbox::new(("table-check", index), self.state.is_checked(key)).on_change(
                    move |_, _, cx| entity.update(cx, |table, cx| table.toggle_check(&row, cx)),
                ),
            )
            .into_any_element()
    }

    fn cell_content(
        cell: &TableCell,
        key: &SharedString,
        cell_id: usize,
        column: usize,
        entity: &Entity<Self>,
        p: &Palette,
    ) -> AnyElement {
        match cell {
            TableCell::Text(text) => div()
                .id(("table-value", cell_id))
                .min_w_0()
                .truncate()
                .tooltip(tooltip_view(Tooltip::new(text.clone())))
                .child(text.clone())
                .into_any_element(),
            TableCell::Numeric { text, .. } => div()
                .id(("table-value", cell_id))
                .min_w_0()
                .truncate()
                .tabular_figures()
                .tooltip(tooltip_view(Tooltip::new(text.clone())))
                .child(text.clone())
                .into_any_element(),
            TableCell::Status(state) => div()
                .id(("table-value", cell_id))
                .flex()
                .items_center()
                .gap(px(SPACE_1))
                .min_w_0()
                .text_color(p.text.primary)
                .child(AgentStatus::new(*state).size(AgentStatusSize::XSmall))
                .child(div().min_w_0().truncate().child(state.label()))
                .into_any_element(),
            TableCell::Agent(kind) => div()
                .id(("table-value", cell_id))
                .flex()
                .items_center()
                .child(AgentIcon::new(*kind).size(AgentIconSize::Small))
                .tooltip(tooltip_view(Tooltip::new(kind.label())))
                .into_any_element(),
            TableCell::Action {
                icon,
                name,
                disabled,
            } => {
                let (entity, row) = (entity.clone(), key.clone());
                let button = IconButton::new(("table-action", cell_id), *icon, name.clone())
                    .size(ButtonSize::Compact)
                    .disabled(*disabled)
                    .on_click(move |_, cx| {
                        entity.update(cx, |_, cx| {
                            cx.emit(TableEvent::Action {
                                row: row.clone(),
                                column,
                            });
                        });
                    });
                div()
                    .id(("table-action-slot", cell_id))
                    .flex_none()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(button)
                    .into_any_element()
            }
            TableCell::Blank => div().into_any_element(),
        }
    }

    fn loading_body(height: f32) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .children((0..LOADING_ROWS).map(|_| {
                div()
                    .flex()
                    .items_center()
                    .h(px(height))
                    .px(px(SPACE_2))
                    .child(Skeleton::line(LineWidth::Most))
            }))
            .into_any_element()
    }

    fn empty_body(
        &self,
        title: &SharedString,
        explanation: Option<&SharedString>,
        action: Option<&SharedString>,
        entity: &Entity<Self>,
    ) -> AnyElement {
        let mut state = EmptyState::new((self.id.clone(), "empty"), title.clone())
            .variant(EmptyStateVariant::Section);
        if let Some(text) = explanation {
            state = state.explanation(text.clone());
        }
        if let Some(label) = action {
            let entity = entity.clone();
            let button = Button::new((self.id.clone(), "empty-action"), label.clone())
                .variant(ButtonVariant::Secondary)
                .on_click(move |_, cx| {
                    entity.update(cx, |_, cx| cx.emit(TableEvent::EmptyAction));
                });
            state = state.action(button);
        }
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .p(px(SPACE_2))
            .child(state)
            .into_any_element()
    }

    fn error_body(&self, entity: &Entity<Self>) -> AnyElement {
        let entity = entity.clone();
        let error = ErrorState::new((self.id.clone(), "error"), "Could not load this table.")
            .variant(ErrorStateVariant::Section)
            .on_retry(move |_, cx| entity.update(cx, |_, cx| cx.emit(TableEvent::Retry)));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .p(px(SPACE_2))
            .child(error)
            .into_any_element()
    }
}

impl Render for TableView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let focused = self.focus.is_focused(window);
        let selectable = self.state.is_selectable();
        let height = self.density.height();
        let width = self
            .state
            .columns()
            .iter()
            .map(|column| column.min_width)
            .sum::<f32>()
            + if selectable { SELECTION_WIDTH } else { 0.0 };

        let mut header = div()
            .id((self.id.clone(), "header"))
            .role(Role::Row)
            .aria_row_index(1)
            .flex()
            .items_center()
            .h(px(height))
            .border_b_1()
            .border_color(p.border.subtle);
        if selectable {
            header = header.child(div().flex_none().w(px(SELECTION_WIDTH)).h_full());
        }
        for (index, column) in self.state.columns().iter().enumerate() {
            header = header.child(self.header_cell(index, column, focused, &theme, cx));
        }

        let entity = cx.entity();
        let body = match self.state.status() {
            TableStatus::Ready => {
                let view = entity.clone();
                uniform_list(
                    (self.id.clone(), "body"),
                    self.state.row_count(),
                    move |range, _, cx| {
                        let table = view.read(cx);
                        range
                            .map(|index| table.render_row(&view, index, focused, cx))
                            .collect()
                    },
                )
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0()
                .into_any_element()
            }
            TableStatus::Loading => Self::loading_body(height),
            TableStatus::Empty {
                title,
                explanation,
                action,
            } => self.empty_body(title, explanation.as_ref(), action.as_ref(), &entity),
            TableStatus::Error => self.error_body(&entity),
        };

        div()
            .id(self.id.clone())
            .track_focus(&self.focus)
            .tab_stop(true)
            .role(Role::Table)
            .aria_label(self.label.clone())
            .aria_row_count(self.state.row_count() + 1)
            .aria_column_count(self.state.columns().len() + usize::from(selectable))
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .text_style(TYPE_BODY)
            .text_color(p.text.primary)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.on_key(event, cx);
            }))
            .child(
                div()
                    .id((self.id.clone(), "grid"))
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .min_w(px(width))
                            .child(header)
                            .child(body),
                    ),
            )
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
        (0..state.rows.len())
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
    fn arrow_moves_stop_at_the_edges_and_home_and_end_span_the_row() {
        let mut state = table();
        state.move_cursor(CellMove::Up);
        assert_eq!(state.cursor(), Cursor { row: 0, column: 0 });
        for _ in 0..3 {
            state.move_cursor(CellMove::Right);
        }
        assert_eq!(state.cursor().column, 2, "stops at the last column");
        state.move_cursor(CellMove::RowStart);
        assert_eq!(state.cursor().column, 0);
        state.move_cursor(CellMove::RowEnd);
        assert_eq!(state.cursor().column, 2);
        for _ in 0..10 {
            state.move_cursor(CellMove::Down);
        }
        assert_eq!(state.cursor().row, 3, "stops at the last body row");
    }

    #[test]
    fn enter_or_space_on_a_sortable_header_sorts_and_other_headers_do_nothing() {
        let mut state = table();
        assert_eq!(state.activate(Press::Enter), Some(Activation::Sort(0)));
        assert_eq!(state.activate(Press::Space), Some(Activation::Sort(0)));
        state.move_cursor(CellMove::RowEnd);
        assert_eq!(
            state.activate(Press::Enter),
            None,
            "the actions header does not sort"
        );
    }

    #[test]
    fn enter_opens_the_body_row_under_the_cursor_in_display_order() {
        let mut state = table();
        state.cycle_sort(1);
        state.cycle_sort(1);
        state.move_cursor(CellMove::Down);
        assert_eq!(
            state.activate(Press::Enter),
            Some(Activation::Open("c".into()))
        );
        state.move_cursor(CellMove::Down);
        assert_eq!(
            state.activate(Press::Enter),
            Some(Activation::Open("b".into()))
        );
    }

    #[test]
    fn space_toggles_a_checkbox_only_in_a_selectable_table() {
        let mut plain = table();
        plain.move_cursor(CellMove::Down);
        assert_eq!(plain.activate(Press::Space), None);
        assert_eq!(plain.toggle_check(&"b".into()), None);

        let mut state = table().selectable();
        state.move_cursor(CellMove::Down);
        assert_eq!(
            state.activate(Press::Space),
            Some(Activation::Check("b".into()))
        );
        assert_eq!(state.toggle_check(&"b".into()), Some(true));
        assert!(state.is_checked(&"b".into()));
        assert_eq!(state.toggle_check(&"b".into()), Some(false));
        assert_eq!(state.toggle_check(&"missing".into()), None);
    }

    #[test]
    fn selection_is_single_and_follows_its_row_through_a_resort() {
        let mut state = table();
        assert!(state.select(&"a".into()));
        assert!(state.select(&"c".into()));
        assert_eq!(state.selected(), Some(&SharedString::from("c")));
        state.cycle_sort(0);
        assert_eq!(state.selected(), Some(&SharedString::from("c")));
        state.set_rows(vec![TableRow::new(
            "a",
            vec![text("Alpha"), number(3.0), TableCell::Blank],
        )]);
        assert_eq!(
            state.selected(),
            None,
            "a removed row is no longer selected"
        );
        assert!(!state.select(&"c".into()));
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

    #[test]
    fn only_arrow_home_and_end_keys_move_the_cursor() {
        assert_eq!(cell_move("left"), Some(CellMove::Left));
        assert_eq!(cell_move("end"), Some(CellMove::RowEnd));
        assert_eq!(cell_move("enter"), None);
    }
}
