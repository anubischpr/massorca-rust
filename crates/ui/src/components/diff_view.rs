//! DiffView: one file's changes, opened as a Work panel tab (docs/design/components/DiffView.md).
//!
//! This module holds the model: the file's hunks as plain data, and the display rows, change
//! blocks, split pairs, notes and counts derived from them. The core computes the hunks off the
//! render path; the view only draws what the model says.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::badge::Badge;
use crate::components::banner::{Banner, BannerVariant};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::error_state::ErrorState;
use crate::components::icon_button::IconButton;
use crate::components::progress::Spinner;
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    Color, Palette, SIZE_DIFF_GUTTER, SIZE_DIFF_MARKER, SIZE_TOOLBAR, SPACE_1, SPACE_2, SPACE_3,
    SPACE_4, TYPE_BODY, TYPE_BODY_SM, TYPE_CAPTION, TYPE_MONO, TYPE_MONO_SM,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Div, ElementId, Entity, FocusHandle, HighlightStyle, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, RenderOnce, Role, ScrollStrategy, SharedString,
    Stateful, StatefulInteractiveElement, Styled, StyledText, UniformListScrollHandle, Window, div,
    px, relative, uniform_list,
};
use std::ops::Range;
use std::rc::Rc;

/// Unchanged lines that one click on a collapsed block reveals.
pub const REVEAL_STEP: usize = 20;

/// Narrowest Work panel tab that can show the split layout, in logical pixels.
pub const SPLIT_MIN_WIDTH: f32 = 640.0;

/// How the file changed; the letter on the file chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

/// What a line is in the diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

impl LineKind {
    /// The glyph in the marker column.
    pub fn marker(self) -> &'static str {
        match self {
            Self::Context => " ",
            Self::Added => "+",
            Self::Removed => "-",
        }
    }

    /// How a screen reader names the line.
    pub fn spoken(self) -> &'static str {
        match self {
            Self::Context => "Unchanged line",
            Self::Added => "Added line",
            Self::Removed => "Removed line",
        }
    }

    pub fn is_change(self) -> bool {
        self != Self::Context
    }
}

/// One line of a hunk. A changed line carries the byte ranges that changed inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: SharedString,
    /// Byte ranges of `text` that changed, from the word-level diff.
    pub changed: Vec<Range<usize>>,
}

impl DiffLine {
    pub fn context(old_no: u32, new_no: u32, text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Context, Some(old_no), Some(new_no), text)
    }

    pub fn added(new_no: u32, text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Added, None, Some(new_no), text)
    }

    pub fn removed(old_no: u32, text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Removed, Some(old_no), None, text)
    }

    fn new(
        kind: LineKind,
        old_no: Option<u32>,
        new_no: Option<u32>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            kind,
            old_no,
            new_no,
            text: text.into(),
            changed: Vec::new(),
        }
    }

    /// The same line with the ranges that changed inside it.
    pub fn with_changed(mut self, changed: Vec<Range<usize>>) -> Self {
        self.changed = changed;
        self
    }
}

/// A hunk: a run of changed lines with their context, and the unchanged lines hidden before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// The section or function named after the second `@@`, if the core found one.
    pub heading: Option<SharedString>,
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    /// Unchanged lines between the previous hunk and this one, in file order.
    pub hidden_before: Vec<DiffLine>,
    /// The hunk's own lines: context, removed and added, in file order.
    pub lines: Vec<DiffLine>,
}

/// The diff of one file: its hunks and the unchanged lines after the last one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDiff {
    pub path: SharedString,
    pub status: FileStatus,
    pub hunks: Vec<Hunk>,
    pub hidden_after: Vec<DiffLine>,
}

/// One display row. Indexes point into the file data, so a reveal or a note keeps its meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// The `@@` header of a hunk.
    Hunk { hunk: usize },
    /// A line of a hunk.
    Line { hunk: usize, line: usize },
    /// An unchanged line that a click revealed: `index` into the gap's hidden lines.
    Revealed { gap: usize, index: usize },
    /// The collapsed "Show N unchanged lines" row of a gap.
    Collapsed { gap: usize, hidden: usize },
}

/// A run of changed lines: the unit that Previous and Next change move between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeBlock {
    /// First and last display row of the run, inclusive.
    pub first: usize,
    pub last: usize,
    pub added: usize,
    pub removed: usize,
}

/// A review note on one changed line. Drafts stay local until they are sent to the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub hunk: usize,
    pub line: usize,
    pub text: SharedString,
    pub sent: bool,
}

/// The diff of one file, with its display rows, change blocks, counts and notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffModel {
    file: FileDiff,
    /// Unchanged lines revealed per gap: gap `i` is before hunk `i`, gap `n` after the last.
    revealed: Vec<usize>,
    rows: Vec<Row>,
    changes: Vec<ChangeBlock>,
    added: usize,
    removed: usize,
    notes: Vec<Note>,
}

impl DiffModel {
    pub fn new(file: FileDiff) -> Self {
        let gaps = file.hunks.len() + 1;
        let (added, removed) = count_lines(&file);
        let mut model = Self {
            file,
            revealed: vec![0; gaps],
            rows: Vec::new(),
            changes: Vec::new(),
            added,
            removed,
            notes: Vec::new(),
        };
        model.rebuild();
        model
    }

    pub fn file(&self) -> &FileDiff {
        &self.file
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn changes(&self) -> &[ChangeBlock] {
        &self.changes
    }

    /// Added and removed line counts over the whole file, revealed or not.
    pub fn counts(&self) -> (usize, usize) {
        (self.added, self.removed)
    }

    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// Unchanged lines that a gap still hides.
    pub fn hidden(&self, gap: usize) -> usize {
        self.gap_total(gap)
            .map_or(0, |total| total - self.revealed[gap])
    }

    /// Reveals the next block of unchanged lines in a gap, nearest the hunk. Returns whether
    /// anything more was shown.
    pub fn reveal(&mut self, gap: usize) -> bool {
        let Some(total) = self.gap_total(gap) else {
            return false;
        };
        let shown = self.revealed[gap];
        if shown >= total {
            return false;
        }
        self.revealed[gap] = (shown + REVEAL_STEP).min(total);
        self.rebuild();
        true
    }

    pub fn line(&self, hunk: usize, line: usize) -> Option<&DiffLine> {
        self.file.hunks.get(hunk)?.lines.get(line)
    }

    /// The line a row shows, for hunk lines and revealed lines.
    pub fn row_line(&self, row: Row) -> Option<&DiffLine> {
        match row {
            Row::Line { hunk, line } => self.line(hunk, line),
            Row::Revealed { gap, index } => self.gap_lines(gap).get(index),
            Row::Hunk { .. } | Row::Collapsed { .. } => None,
        }
    }

    /// The next change after `current`, or the first one when nothing is current yet.
    pub fn next_change(&self, current: Option<usize>) -> Option<usize> {
        match current {
            None => (!self.changes.is_empty()).then_some(0),
            Some(index) => (index + 1 < self.changes.len()).then_some(index + 1),
        }
    }

    pub fn previous_change(&self, current: Option<usize>) -> Option<usize> {
        current?
            .checked_sub(1)
            .filter(|index| *index < self.changes.len())
    }

    /// The change block that holds a display row.
    pub fn change_at(&self, row: usize) -> Option<usize> {
        self.changes
            .iter()
            .position(|block| block.first <= row && row <= block.last)
    }

    /// The position of a change for the toolbar and screen readers: "Change 3 of 12".
    pub fn change_label(index: usize, total: usize) -> String {
        format!("Change {} of {}", index + 1, total)
    }

    /// Adds a draft note to a changed line. Context lines and lines that already have a note
    /// refuse it.
    pub fn add_draft(&mut self, hunk: usize, line: usize, text: impl Into<SharedString>) -> bool {
        let is_change = self
            .line(hunk, line)
            .is_some_and(|line| line.kind.is_change());
        if !is_change || self.note_on(hunk, line).is_some() {
            return false;
        }
        self.notes.push(Note {
            hunk,
            line,
            text: text.into(),
            sent: false,
        });
        true
    }

    pub fn note_on(&self, hunk: usize, line: usize) -> Option<&Note> {
        self.notes
            .iter()
            .find(|note| note.hunk == hunk && note.line == line)
    }

    pub fn drafts(&self) -> usize {
        self.notes.iter().filter(|note| !note.sent).count()
    }

    /// `path:line` for a changed line: the new line number, or the old one for a removal.
    pub fn reference(&self, hunk: usize, line: usize) -> Option<String> {
        let line = self.line(hunk, line)?;
        let number = line.new_no.or(line.old_no)?;
        Some(format!("{}:{}", self.file.path, number))
    }

    /// Marks every draft sent and returns the one message that carries them.
    pub fn send_drafts(&mut self) -> Option<String> {
        let body: Vec<String> = self
            .notes
            .iter()
            .filter(|note| !note.sent)
            .filter_map(|note| {
                let reference = self.reference(note.hunk, note.line)?;
                Some(format!("{reference} {}", note.text))
            })
            .collect();
        if body.is_empty() {
            return None;
        }
        for note in self.notes.iter_mut().filter(|note| !note.sent) {
            note.sent = true;
        }
        Some(format!("Review notes:\n{}", body.join("\n")))
    }

    /// The hidden lines of a gap: before hunk `gap`, or after the last hunk when `gap` is the
    /// hunk count.
    fn gap_lines(&self, gap: usize) -> &[DiffLine] {
        match self.file.hunks.get(gap) {
            Some(hunk) => &hunk.hidden_before,
            None => &self.file.hidden_after,
        }
    }

    fn gap_total(&self, gap: usize) -> Option<usize> {
        (gap <= self.file.hunks.len()).then(|| self.gap_lines(gap).len())
    }

    fn rebuild(&mut self) {
        self.rows = build_rows(&self.file, &self.revealed);
        self.changes = change_blocks(&self.rows, &self.file);
    }
}

fn count_lines(file: &FileDiff) -> (usize, usize) {
    file.hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .fold((0, 0), |(added, removed), line| match line.kind {
            LineKind::Added => (added + 1, removed),
            LineKind::Removed => (added, removed + 1),
            LineKind::Context => (added, removed),
        })
}

fn build_rows(file: &FileDiff, revealed: &[usize]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (hunk_index, hunk) in file.hunks.iter().enumerate() {
        push_before(
            &mut rows,
            hunk_index,
            hunk.hidden_before.len(),
            revealed[hunk_index],
        );
        rows.push(Row::Hunk { hunk: hunk_index });
        rows.extend((0..hunk.lines.len()).map(|line| Row::Line {
            hunk: hunk_index,
            line,
        }));
    }
    let after = file.hunks.len();
    push_after(&mut rows, after, file.hidden_after.len(), revealed[after]);
    rows
}

/// A gap above a hunk: the collapsed row first, then the revealed lines nearest the hunk.
fn push_before(rows: &mut Vec<Row>, gap: usize, total: usize, shown: usize) {
    let hidden = total - shown.min(total);
    if hidden > 0 {
        rows.push(Row::Collapsed { gap, hidden });
    }
    rows.extend((hidden..total).map(|index| Row::Revealed { gap, index }));
}

/// The gap after the last hunk: the revealed lines nearest the hunk, then the collapsed row.
fn push_after(rows: &mut Vec<Row>, gap: usize, total: usize, shown: usize) {
    let shown = shown.min(total);
    rows.extend((0..shown).map(|index| Row::Revealed { gap, index }));
    let hidden = total - shown;
    if hidden > 0 {
        rows.push(Row::Collapsed { gap, hidden });
    }
}

fn row_kind(row: Row, file: &FileDiff) -> Option<LineKind> {
    match row {
        Row::Line { hunk, line } => file.hunks.get(hunk)?.lines.get(line).map(|line| line.kind),
        Row::Revealed { .. } => Some(LineKind::Context),
        Row::Hunk { .. } | Row::Collapsed { .. } => None,
    }
}

fn change_blocks(rows: &[Row], file: &FileDiff) -> Vec<ChangeBlock> {
    let mut blocks = Vec::new();
    let mut current: Option<ChangeBlock> = None;
    for (index, row) in rows.iter().copied().enumerate() {
        match row_kind(row, file) {
            Some(kind) if kind.is_change() => {
                let block = current.get_or_insert(ChangeBlock {
                    first: index,
                    last: index,
                    added: 0,
                    removed: 0,
                });
                block.last = index;
                match kind {
                    LineKind::Added => block.added += 1,
                    _ => block.removed += 1,
                }
            }
            _ => blocks.extend(current.take()),
        }
    }
    blocks.extend(current);
    blocks
}

/// The `@@` header of a hunk, as the diff tool prints it.
pub fn hunk_title(hunk: &Hunk) -> String {
    let mut title = format!(
        "@@ -{},{} +{},{} @@",
        hunk.old_start, hunk.old_len, hunk.new_start, hunk.new_len
    );
    if let Some(heading) = &hunk.heading {
        title.push(' ');
        title.push_str(heading);
    }
    title
}

impl FileStatus {
    /// The letter on the file chip (M, A, D, R).
    pub fn letter(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }
}

/// One row of the split layout: the old side on the left, the new side on the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitRow {
    Hunk {
        hunk: usize,
    },
    /// Display-row indexes for each side; `None` leaves that side empty.
    Pair {
        left: Option<usize>,
        right: Option<usize>,
    },
    Collapsed {
        gap: usize,
        hidden: usize,
    },
}

/// One mark on the change ruler, as fractions of the full height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulerMark {
    pub top: f32,
    pub height: f32,
    pub added: bool,
    pub removed: bool,
    pub current: bool,
}

impl DiffModel {
    /// One mark per change block over the full height; `current` is outlined.
    pub fn ruler(&self, current: Option<usize>) -> Vec<RulerMark> {
        let total = self.rows.len();
        if total == 0 {
            return Vec::new();
        }
        let total = total as f32;
        self.changes
            .iter()
            .enumerate()
            .map(|(index, block)| RulerMark {
                top: block.first as f32 / total,
                height: (block.last + 1 - block.first) as f32 / total,
                added: block.added > 0,
                removed: block.removed > 0,
                current: current == Some(index),
            })
            .collect()
    }

    /// The rows of the split layout: removed and added lines of one change pair up in order.
    pub fn split_rows(&self) -> Vec<SplitRow> {
        let mut out = Vec::new();
        let (mut removed, mut added) = (Vec::new(), Vec::new());
        for (index, row) in self.rows.iter().copied().enumerate() {
            match row_kind(row, &self.file) {
                Some(LineKind::Removed) => removed.push(index),
                Some(LineKind::Added) => added.push(index),
                _ => {
                    flush_pairs(&mut out, &mut removed, &mut added);
                    out.push(match row {
                        Row::Hunk { hunk } => SplitRow::Hunk { hunk },
                        Row::Collapsed { gap, hidden } => SplitRow::Collapsed { gap, hidden },
                        Row::Line { .. } | Row::Revealed { .. } => SplitRow::Pair {
                            left: Some(index),
                            right: Some(index),
                        },
                    });
                }
            }
        }
        flush_pairs(&mut out, &mut removed, &mut added);
        out
    }
}

fn flush_pairs(out: &mut Vec<SplitRow>, removed: &mut Vec<usize>, added: &mut Vec<usize>) {
    let rows = removed.len().max(added.len());
    out.extend((0..rows).map(|k| SplitRow::Pair {
        left: removed.get(k).copied(),
        right: added.get(k).copied(),
    }));
    removed.clear();
    added.clear();
}

/// The layout of the diff body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// One column: removed lines above added lines.
    #[default]
    Inline,
    /// The old file on the left, the new file on the right.
    Split,
}

/// What the reader asked for. The owner of the diff acts on it; cursor and layout stay in the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffAction {
    Reveal { gap: usize },
    Comment { hunk: usize, line: usize },
    SendToAgent,
    Edit,
    More,
    Retry,
    ShowFile,
    ShowAnyway,
    OpenAsSource,
}

/// Receives the actions of a DiffView.
pub type DiffActionHandler = Rc<dyn Fn(DiffAction, &mut Window, &mut App)>;

/// Height of every body row, so the list virtualises with `uniform_list`.
const ROW_HEIGHT: f32 = 20.0;
/// Rows that Page Up and Page Down move the cursor by.
const PAGE_ROWS: usize = 20;
/// Width of the change ruler on the right edge.
const RULER_WIDTH: f32 = 8.0;

/// The view's own state, kept by its element id: the cursor, the current change and the layout.
struct ViewState {
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    cursor: usize,
    change: Option<usize>,
    mode: ViewMode,
}

/// What a body row needs from the view: colours, the cursor, the current change and the handler.
#[derive(Clone)]
struct Look {
    p: Palette,
    cursor: usize,
    change: Option<usize>,
    focused: bool,
    handler: Option<DiffActionHandler>,
    theme: Theme,
}

/// The changes of one file: toolbar, review bar, body and change ruler (DiffView.md).
fn emit(
    handler: &Option<DiffActionHandler>,
    action: DiffAction,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = handler {
        handler(action, window, cx);
    }
}

/// Moves the cursor to the first row of a change and centres it.
fn select_change(model: &DiffModel, state: &mut ViewState, index: usize) {
    if let Some(block) = model.changes().get(index) {
        state.change = Some(index);
        state.cursor = block.first;
        state
            .scroll
            .scroll_to_item(block.first, ScrollStrategy::Center);
    }
}

fn move_cursor(model: &DiffModel, state: &mut ViewState, to: usize) {
    state.cursor = to.min(model.rows().len().saturating_sub(1));
    state
        .scroll
        .scroll_to_item(state.cursor, ScrollStrategy::Center);
}

/// Applies a key to the view state. Returns whether the key was used, and the action it asks for.
fn handle_key(
    model: &DiffModel,
    state: &mut ViewState,
    key: &str,
    shift: bool,
    comment: bool,
) -> (bool, Option<DiffAction>) {
    let cursor = state.cursor;
    let last = model.rows().len().saturating_sub(1);
    match key {
        "down" => move_cursor(model, state, cursor + 1),
        "up" => move_cursor(model, state, cursor.saturating_sub(1)),
        "pagedown" => move_cursor(model, state, cursor + PAGE_ROWS),
        "pageup" => move_cursor(model, state, cursor.saturating_sub(PAGE_ROWS)),
        "home" => move_cursor(model, state, 0),
        "end" => move_cursor(model, state, last),
        "f7" => {
            let target = if shift {
                model.previous_change(state.change)
            } else {
                model.next_change(state.change)
            };
            if let Some(index) = target {
                select_change(model, state, index);
            }
        }
        "enter" => {
            let action = match model.rows().get(cursor) {
                Some(Row::Collapsed { gap, .. }) => Some(DiffAction::Reveal { gap: *gap }),
                _ => None,
            };
            return (action.is_some(), action);
        }
        "m" if comment => {
            let action = match model.rows().get(cursor) {
                Some(Row::Line { hunk, line }) => model
                    .line(*hunk, *line)
                    .filter(|line| line.kind.is_change())
                    .map(|_| DiffAction::Comment {
                        hunk: *hunk,
                        line: *line,
                    }),
                _ => None,
            };
            return (action.is_some(), action);
        }
        _ => return (false, None),
    }
    (true, None)
}

fn gutter_cell(number: Option<u32>, bg: Color, p: &Palette) -> Div {
    div()
        .flex_none()
        .w(px(SIZE_DIFF_GUTTER))
        .h_full()
        .flex()
        .items_center()
        .justify_end()
        .px(px(SPACE_1))
        .bg(bg)
        .text_style(TYPE_MONO)
        .tabular_figures()
        .text_color(p.text.secondary)
        .child(number.map(|n| n.to_string()).unwrap_or_default())
}

fn marker_cell(kind: LineKind, p: &Palette) -> Div {
    let color = match kind {
        LineKind::Added => p.diff.add_fg,
        LineKind::Removed => p.diff.remove_fg,
        LineKind::Context => p.text.tertiary,
    };
    div()
        .flex_none()
        .w(px(SIZE_DIFF_MARKER))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_style(TYPE_MONO)
        .text_color(color)
        .child(kind.marker())
}

/// The code of a line, with its changed ranges behind the text (word-level diff).
fn code_cell(line: &DiffLine, p: &Palette) -> Div {
    let word = match line.kind {
        LineKind::Added => p.diff.add_word,
        LineKind::Removed => p.diff.remove_word,
        LineKind::Context => p.surface.sunken,
    };
    let highlights = line.changed.iter().map(|range| {
        (
            range.clone(),
            HighlightStyle {
                background_color: Some(word.into()),
                ..Default::default()
            },
        )
    });
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .px(px(SPACE_1))
        .text_style(TYPE_MONO)
        .text_color(p.text.primary)
        .child(StyledText::new(line.text.clone()).with_highlights(highlights))
}

/// The row fill and the gutter fill of a line kind.
fn line_colours(kind: LineKind, p: &Palette) -> (Color, Color) {
    match kind {
        LineKind::Added => (p.diff.add_bg, p.diff.add_gutter),
        LineKind::Removed => (p.diff.remove_bg, p.diff.remove_gutter),
        LineKind::Context => (p.surface.sunken, p.surface.sunken),
    }
}

/// An inline line row. `current` marks it as part of the current change.
fn line_row(index: usize, line: &DiffLine, look: &Look, current: bool) -> Stateful<Div> {
    let p = &look.p;
    let (bg, gutter) = line_colours(line.kind, p);
    let mut row = div()
        .id(("diff-row", index))
        .relative()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .min_w_0()
        .bg(bg)
        .role(Role::ListItem)
        .aria_label(format!("{}: {}", line.kind.spoken(), line.text))
        .child(gutter_cell(line.old_no, gutter, p))
        .child(gutter_cell(line.new_no, gutter, p))
        .child(marker_cell(line.kind, p))
        .child(code_cell(line, p));
    if current {
        row = row.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.))
                .bg(p.accent.fg),
        );
    }
    if look.focused && look.cursor == index {
        row = row.border_2().border_color(p.border.focus);
    }
    row
}

/// The collapsed "Show N unchanged lines" row; a click reveals the next block.
fn collapsed_row(gap: usize, hidden: usize, look: &Look) -> AnyElement {
    let p = &look.p;
    let handler = look.handler.clone();
    div()
        .id(("diff-collapsed", gap))
        .debug_selector(move || format!("diff-collapsed-{gap}"))
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .px(px(SPACE_3))
        .cursor_pointer()
        .hover(move |s| s.bg(p.fill.hover_canvas))
        .role(Role::Button)
        .aria_label(format!("Show {hidden} unchanged lines"))
        .on_click(move |_, window, cx| emit(&handler, DiffAction::Reveal { gap }, window, cx))
        .child(
            Icon::new(IconName::ChevronsUpDown)
                .size(IconSize::XSmall)
                .color(p.text.secondary),
        )
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(p.text.secondary)
                .child(format!("Show {hidden} unchanged lines")),
        )
        .into_any_element()
}

/// A hunk header row: the `@@` line on the sunken surface.
fn hunk_row(index: usize, model: &DiffModel, hunk: usize, look: &Look) -> AnyElement {
    let title = model
        .file()
        .hunks
        .get(hunk)
        .map(hunk_title)
        .unwrap_or_default();
    div()
        .id(("diff-hunk", index))
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .px(px(SPACE_3))
        .bg(look.p.surface.sunken)
        .text_style(TYPE_MONO_SM)
        .text_color(look.p.text.tertiary)
        .whitespace_nowrap()
        .role(Role::Group)
        .child(title)
        .into_any_element()
}

/// Whether a display row belongs to the current change block.
fn in_change(model: &DiffModel, change: Option<usize>, row: usize) -> bool {
    change
        .and_then(|index| model.changes().get(index))
        .is_some_and(|block| block.first <= row && row <= block.last)
}

/// One row of the inline layout.
fn inline_row(model: &DiffModel, index: usize, look: &Look) -> AnyElement {
    let Some(row) = model.rows().get(index).copied() else {
        return div().into_any_element();
    };
    match row {
        Row::Hunk { hunk } => hunk_row(index, model, hunk, look),
        Row::Collapsed { gap, hidden } => collapsed_row(gap, hidden, look),
        Row::Line { .. } | Row::Revealed { .. } => match model.row_line(row) {
            Some(line) => {
                let current = in_change(model, look.change, index);
                line_row(index, line, look, current).into_any_element()
            }
            None => div().into_any_element(),
        },
    }
}

/// One half of a split row: a line on one side, or an empty cell.
fn split_side(model: &DiffModel, row: Option<usize>, old: bool, look: &Look) -> AnyElement {
    let line = row
        .and_then(|r| model.rows().get(r).copied())
        .and_then(|r| model.row_line(r));
    let Some(line) = line else {
        return div()
            .flex_1()
            .h(px(ROW_HEIGHT))
            .bg(look.p.surface.sunken)
            .into_any_element();
    };
    let number = if old { line.old_no } else { line.new_no };
    let (bg, gutter) = line_colours(line.kind, &look.p);
    div()
        .flex_1()
        .min_w_0()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .bg(bg)
        .child(gutter_cell(number, gutter, &look.p))
        .child(marker_cell(line.kind, &look.p))
        .child(code_cell(line, &look.p))
        .into_any_element()
}

/// One row of the split layout.
fn split_row(model: &DiffModel, index: usize, row: SplitRow, look: &Look) -> AnyElement {
    match row {
        SplitRow::Hunk { hunk } => hunk_row(index, model, hunk, look),
        SplitRow::Collapsed { gap, hidden } => collapsed_row(gap, hidden, look),
        SplitRow::Pair { left, right } => div()
            .id(("diff-split", index))
            .h(px(ROW_HEIGHT))
            .flex()
            .child(split_side(model, left, true, look))
            .child(div().w(px(1.)).h_full().bg(look.p.border.subtle))
            .child(split_side(model, right, false, look))
            .into_any_element(),
    }
}

/// The change ruler: one mark per change block, the current one outlined.
fn ruler_strip(model: &DiffModel, change: Option<usize>, p: &Palette) -> Div {
    let marks = model.ruler(change).into_iter().map(|mark| {
        let color = if mark.added {
            p.diff.add_fg
        } else {
            p.diff.remove_fg
        };
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(relative(mark.top))
            .h(relative(mark.height))
            .min_h(px(2.))
            .bg(color)
            .when(mark.current, |el| {
                el.border_1().border_color(p.text.primary)
            })
    });
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(RULER_WIDTH))
        .children(marks)
}

/// The scrolling body of a file that is ready to draw, with its keyboard handling.
fn ready_body(
    id: &ElementId,
    model: Rc<DiffModel>,
    state: Entity<ViewState>,
    look: Look,
    mode: ViewMode,
    focus: &FocusHandle,
    scroll: UniformListScrollHandle,
) -> AnyElement {
    let p = look.p;
    let theme = look.theme;
    let split_rows = match mode {
        ViewMode::Split => model.split_rows(),
        ViewMode::Inline => Vec::new(),
    };
    let count = match mode {
        ViewMode::Split => split_rows.len(),
        ViewMode::Inline => model.rows().len(),
    };
    let change = look.change;
    let key_handler = look.handler.clone();
    let ruler = ruler_strip(&model, change, &p);
    let rows_model = model.clone();
    let rows = uniform_list((id.clone(), "lines"), count, move |range, _, _| {
        range
            .map(|index| match mode {
                ViewMode::Split => match split_rows.get(index) {
                    Some(row) => split_row(&rows_model, index, *row, &look),
                    None => div().into_any_element(),
                },
                ViewMode::Inline => inline_row(&rows_model, index, &look),
            })
            .collect::<Vec<AnyElement>>()
    })
    .track_scroll(&scroll)
    .size_full();
    let key_model = model.clone();
    div()
        .id((id.clone(), "body"))
        .relative()
        .flex_1()
        .min_h_0()
        .w_full()
        .bg(p.surface.sunken)
        .track_focus(focus)
        .tab_stop(true)
        .focus_ring(&theme)
        .role(Role::Document)
        .aria_label(model.file().path.clone())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let modifiers = event.keystroke.modifiers;
            let comment = modifiers.alt && modifiers.secondary(cx);
            let key = event.keystroke.key.as_str();
            let (used, action) = state.update(cx, |s, _| {
                handle_key(&key_model, s, key, modifiers.shift, comment)
            });
            if used {
                cx.stop_propagation();
                window.refresh();
                if let Some(action) = action {
                    emit(&key_handler, action, window, cx);
                }
            }
        })
        .child(rows)
        .child(ruler)
        .into_any_element()
}

/// What the body shows in place of the diff while the file is not ready to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffStatus {
    /// The diff draws.
    Ready,
    /// Skeleton lines, and a spinner after 200 ms.
    Loading,
    /// "No changes in this file." with "Show file".
    Empty,
    /// "Could not load this diff", with the reason and Retry.
    Failed(SharedString),
    /// A diff too large to draw before the reader confirms it.
    Large { lines: usize },
    /// A binary file, with its size before and after.
    Binary {
        before: SharedString,
        after: SharedString,
    },
    /// A submodule whose recorded commit moved: git's pointer line, and whether its checkout has changes.
    Submodule {
        summary: SharedString,
        dirty: Option<bool>,
    },
}

/// The changes of one file: toolbar, review bar, body and change ruler (DiffView.md).
#[derive(IntoElement)]
pub struct DiffView {
    id: ElementId,
    model: Rc<DiffModel>,
    status: DiffStatus,
    split_available: bool,
    disconnected: bool,
    read_only: bool,
    conflicts_skipped: bool,
    compare: Option<SharedString>,
    on_action: Option<DiffActionHandler>,
}

impl std::fmt::Debug for DiffView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffView")
            .field("id", &self.id)
            .field("status", &self.status)
            .finish()
    }
}

impl DiffView {
    pub fn new(id: impl Into<ElementId>, model: impl Into<Rc<DiffModel>>) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            status: DiffStatus::Ready,
            split_available: true,
            disconnected: false,
            read_only: false,
            conflicts_skipped: false,
            compare: None,
            on_action: None,
        }
    }

    pub fn status(mut self, status: DiffStatus) -> Self {
        self.status = status;
        self
    }

    /// Whether the tab is at least `SPLIT_MIN_WIDTH` wide; a narrower tab draws inline.
    pub fn split_available(mut self, available: bool) -> Self {
        self.split_available = available;
        self
    }

    /// The host cannot be reached: the last diff stays, and Edit and Send to agent are disabled.
    pub fn disconnected(mut self, disconnected: bool) -> Self {
        self.disconnected = disconnected;
        self
    }

    /// A compare view (an external change): no Edit and no notes.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn conflicts_skipped(mut self, skipped: bool) -> Self {
        self.conflicts_skipped = skipped;
        self
    }

    /// The base the diff compares with, such as "vs origin/main".
    pub fn compare(mut self, base: impl Into<SharedString>) -> Self {
        self.compare = Some(base.into());
        self
    }

    pub fn on_action(
        mut self,
        listener: impl Fn(DiffAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

/// A click listener that sends one action to the handler.
fn on(
    handler: &Option<DiffActionHandler>,
    action: DiffAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| emit(&handler, action, window, cx)
}

/// The directory part (with its slash) and the file name of a path.
fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(index) => path.split_at(index + 1),
        None => ("", path),
    }
}

/// The file chip's icon: the agent mark for CLAUDE.md and AGENTS.md, else a grey document.
pub(crate) fn file_icon(path: &str, p: &Palette) -> AnyElement {
    match split_path(path).1 {
        "CLAUDE.md" => AgentIcon::new(AgentKind::Claude)
            .size(AgentIconSize::Small)
            .into_any_element(),
        "AGENTS.md" => AgentIcon::new(AgentKind::Codex)
            .size(AgentIconSize::Small)
            .into_any_element(),
        _ => Icon::new(IconName::File)
            .size(IconSize::Small)
            .color(p.text.tertiary)
            .into_any_element(),
    }
}

/// "12,431": thousands separated by commas.
fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// "1 draft note" and "3 draft notes".
fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The skeleton of the body and the spinner that shows after 200 ms (Loading).
fn loading_body(id: &ElementId, p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .p(px(SPACE_3))
        .child(
            div()
                .flex()
                .justify_end()
                .child(Spinner::new().delayed((id.clone(), "spinner"))),
        )
        .children((0..14).map(|i| {
            let width = if i % 2 == 0 {
                LineWidth::Full
            } else {
                LineWidth::Most
            };
            Skeleton::line(width)
        }))
        .bg(p.surface.sunken)
        .into_any_element()
}

impl RenderOnce for DiffView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let state = window.use_keyed_state(id.clone(), cx, |_, cx| ViewState {
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            cursor: 0,
            change: None,
            mode: ViewMode::Inline,
        });
        let (focus, scroll, cursor, change, stored) = {
            let s = state.read(cx);
            (
                s.focus.clone(),
                s.scroll.clone(),
                s.cursor,
                s.change,
                s.mode,
            )
        };
        let mode = if self.split_available {
            stored
        } else {
            ViewMode::Inline
        };
        let model = self.model.clone();
        let handler = self.on_action.clone();
        let editable = !self.read_only && !self.disconnected;
        let (added, removed) = model.counts();
        let (dir, name) = split_path(&model.file().path);
        let (dir, name) = (dir.to_owned(), name.to_owned());
        let look = Look {
            p,
            cursor,
            change,
            focused: focus.is_focused(window),
            handler: handler.clone(),
            theme,
        };

        let split_toggle = IconButton::new("diff-split", IconName::Columns2, "Split view")
            .toggle(mode == ViewMode::Split)
            .disabled(!self.split_available)
            .disabled_reason("Widen the Work panel to use split view")
            .on_click({
                let state = state.clone();
                move |window, cx| {
                    state.update(cx, |s, _| {
                        s.mode = match s.mode {
                            ViewMode::Inline => ViewMode::Split,
                            ViewMode::Split => ViewMode::Inline,
                        }
                    });
                    window.refresh();
                }
            });
        let step = |next: bool| {
            let (state, model) = (state.clone(), model.clone());
            move |window: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| {
                    let target = if next {
                        model.next_change(s.change)
                    } else {
                        model.previous_change(s.change)
                    };
                    if let Some(index) = target {
                        select_change(&model, s, index);
                    }
                });
                window.refresh();
            }
        };
        let toolbar = div()
            .id("diff-toolbar")
            .role(Role::Toolbar)
            .aria_label("Diff toolbar")
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .h(px(SIZE_TOOLBAR))
            .px(px(SPACE_2))
            .min_w_0()
            .border_b_1()
            .border_color(p.border.subtle)
            .child(Badge::text(model.file().status.letter()))
            .child(file_icon(&model.file().path, &p))
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_MONO_SM)
                    .child(div().text_color(p.text.tertiary).child(dir))
                    .child(div().text_color(p.text.primary).child(name)),
            )
            .when_some(self.compare.clone(), |el, base| {
                el.child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.tertiary)
                        .whitespace_nowrap()
                        .child(base),
                )
            })
            .child(
                div()
                    .text_style(TYPE_MONO_SM)
                    .tabular_figures()
                    .text_color(p.diff.add_fg)
                    .id("diff-counts")
                    .aria_label(format!("{added} added, {removed} removed"))
                    .child(format!("+{added}")),
            )
            .child(
                div()
                    .text_style(TYPE_MONO_SM)
                    .tabular_figures()
                    .text_color(p.diff.remove_fg)
                    .child(format!("-{removed}")),
            )
            .child(div().flex_1())
            .child(split_toggle)
            .child(
                IconButton::new("diff-previous", IconName::ArrowUp, "Previous change")
                    .disabled(model.previous_change(change).is_none())
                    .on_click(step(false)),
            )
            .child(
                IconButton::new("diff-next", IconName::ArrowDown, "Next change")
                    .disabled(model.next_change(change).is_none())
                    .on_click(step(true)),
            )
            // A read-only compare has no Edit; an unreachable host disables it.
            .when(!self.read_only, |bar| {
                bar.child(
                    IconButton::new("diff-edit", IconName::Pencil, "Edit")
                        .disabled(!editable)
                        .on_click(on(&handler, DiffAction::Edit)),
                )
            })
            .child(
                IconButton::new("diff-more", IconName::Ellipsis, "More")
                    .on_click(on(&handler, DiffAction::More)),
            );

        let drafts = model.drafts();
        let review = (drafts > 0).then(|| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(SPACE_3))
                .py(px(SPACE_2))
                .bg(p.surface.raised)
                .border_b_1()
                .border_color(p.border.subtle)
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.secondary)
                        .child(plural(drafts, "draft note", "draft notes")),
                )
                .child(
                    div().debug_selector(|| "diff-send-slot".into()).child(
                        Button::new("diff-send", "Send to agent")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Compact)
                            .disabled(self.disconnected)
                            .on_click(on(&handler, DiffAction::SendToAgent)),
                    ),
                )
        });
        let conflicts = self.conflicts_skipped.then(|| {
            Banner::new(
                "diff-conflicts",
                BannerVariant::Info,
                "Conflicted hunks are skipped. Resolve them in the merge view.",
            )
        });
        let offline = self.disconnected.then(|| {
            Banner::new(
                "diff-offline",
                BannerVariant::Unreachable,
                "Showing the last loaded diff. VPS can't be reached; changes made since may be missing.",
            )
        });

        let body = match self.status {
            DiffStatus::Ready => ready_body(
                &id,
                model.clone(),
                state.clone(),
                look,
                mode,
                &focus,
                scroll,
            ),
            DiffStatus::Loading => loading_body(&id, &p),
            // DiffView.md States (empty): the line in text.secondary, not an EmptyState title.
            DiffStatus::Empty => div()
                .id("diff-empty")
                .role(Role::Group)
                .aria_label("No changes in this file.")
                .flex()
                .flex_col()
                .items_start()
                .gap(px(SPACE_2))
                .p(px(SPACE_4))
                .child(
                    div()
                        .text_style(TYPE_BODY)
                        .text_color(p.text.secondary)
                        .child("No changes in this file."),
                )
                .child(
                    Button::new("diff-show-file", "Show file")
                        .variant(ButtonVariant::Ghost)
                        .on_click(on(&handler, DiffAction::ShowFile)),
                )
                .into_any_element(),
            DiffStatus::Failed(reason) => ErrorState::new("diff-error", "Could not load this diff")
                .explanation(reason)
                .on_retry(on(&handler, DiffAction::Retry))
                .into_any_element(),
            DiffStatus::Large { lines } => Banner::new(
                "diff-large",
                BannerVariant::Info,
                format!(
                    "This diff is large ({} lines). Show it anyway?",
                    grouped(lines)
                ),
            )
            .action(
                Button::new("diff-show-all", "Show diff")
                    .variant(ButtonVariant::Primary)
                    .on_click(on(&handler, DiffAction::ShowAnyway)),
            )
            .action(
                Button::new("diff-as-source", "Open as source")
                    .on_click(on(&handler, DiffAction::OpenAsSource)),
            )
            .into_any_element(),
            DiffStatus::Submodule { summary, dirty } => div()
                .debug_selector(|| "diff-submodule".to_owned())
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .p(px(SPACE_3))
                .child(
                    div()
                        .text_color(p.text.secondary)
                        .child("Submodule pointer changed"),
                )
                .child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.tertiary)
                        .child(summary.clone()),
                )
                .when(matches!(dirty, Some(true)), |body| {
                    body.child(
                        div()
                            .debug_selector(|| "diff-submodule-dirty".to_owned())
                            .text_style(TYPE_CAPTION)
                            .text_color(p.text.tertiary)
                            .child("The submodule has uncommitted changes."),
                    )
                })
                .into_any_element(),
            DiffStatus::Binary { before, after } => div()
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .p(px(SPACE_3))
                .child(
                    div()
                        .text_color(p.text.secondary)
                        .child("Binary file changed"),
                )
                .child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.tertiary)
                        .child(format!("{before} to {after}")),
                )
                .into_any_element(),
        };

        div()
            .id(id)
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(p.surface.canvas)
            .child(toolbar)
            .children(review)
            .children(conflicts)
            .children(offline)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(n: u32) -> DiffLine {
        DiffLine::context(n, n, format!("line {n}"))
    }

    fn hunk(start: u32, lines: Vec<DiffLine>, hidden_before: Vec<DiffLine>) -> Hunk {
        Hunk {
            heading: None,
            old_start: start,
            old_len: 1,
            new_start: start,
            new_len: 1,
            hidden_before,
            lines,
        }
    }

    /// Hunk one changes line 2; hunk two adds line 41 after 30 hidden lines; 5 lines follow.
    fn sample() -> FileDiff {
        FileDiff {
            path: "src/lib.rs".into(),
            status: FileStatus::Modified,
            hunks: vec![
                hunk(
                    1,
                    vec![
                        ctx(1),
                        DiffLine::removed(2, "old"),
                        DiffLine::added(2, "new"),
                        ctx(3),
                    ],
                    vec![],
                ),
                hunk(
                    40,
                    vec![ctx(40), DiffLine::added(41, "y"), ctx(41)],
                    (10..40).map(ctx).collect(),
                ),
            ],
            hidden_after: (60..65).map(ctx).collect(),
        }
    }

    fn revealed(model: &DiffModel, gap: usize) -> Vec<usize> {
        model
            .rows()
            .iter()
            .filter_map(|row| match row {
                Row::Revealed { gap: g, index } if *g == gap => Some(*index),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn counts_cover_every_hunk_line() {
        assert_eq!(DiffModel::new(sample()).counts(), (2, 1));
    }

    #[test]
    fn collapsed_rows_hide_unchanged_lines_before_each_hunk_and_after_the_last() {
        let model = DiffModel::new(sample());
        assert_eq!(model.rows()[0], Row::Hunk { hunk: 0 });
        assert!(
            model
                .rows()
                .contains(&Row::Collapsed { gap: 1, hidden: 30 })
        );
        assert_eq!(
            model.rows().last(),
            Some(&Row::Collapsed { gap: 2, hidden: 5 })
        );
        assert_eq!(
            (model.hidden(0), model.hidden(1), model.hidden(2)),
            (0, 30, 5)
        );
    }

    #[test]
    fn reveal_shows_twenty_lines_nearest_the_hunk_and_stops_when_all_are_shown() {
        let mut model = DiffModel::new(sample());
        assert!(model.reveal(1));
        assert_eq!(model.hidden(1), 10);
        assert_eq!(revealed(&model, 1), (10..30).collect::<Vec<_>>());
        assert!(model.reveal(1));
        assert_eq!(model.hidden(1), 0);
        assert!(!model.reveal(1));
        assert!(!model.reveal(9), "no gap exists past the last hunk");
        let first = model.row_line(Row::Revealed { gap: 1, index: 0 });
        assert_eq!(
            first.map(|line| line.text.to_string()).as_deref(),
            Some("line 10")
        );
    }

    #[test]
    fn reveal_after_the_last_hunk_grows_from_the_hunk_side() {
        let mut model = DiffModel::new(sample());
        assert!(model.reveal(2));
        assert_eq!(revealed(&model, 2), vec![0, 1, 2, 3, 4]);
        assert_eq!(model.hidden(2), 0);
    }

    #[test]
    fn change_blocks_are_runs_of_changed_lines() {
        let blocks: Vec<_> = DiffModel::new(sample())
            .changes()
            .iter()
            .map(|b| (b.first, b.last, b.added, b.removed))
            .collect();
        assert_eq!(blocks, vec![(2, 3, 1, 1), (8, 8, 1, 0)]);
    }

    #[test]
    fn next_and_previous_walk_the_change_blocks() {
        let model = DiffModel::new(sample());
        assert_eq!(model.next_change(None), Some(0));
        assert_eq!(model.next_change(Some(0)), Some(1));
        assert_eq!(model.next_change(Some(1)), None);
        assert_eq!(model.previous_change(Some(1)), Some(0));
        assert_eq!(model.previous_change(Some(0)), None);
        assert_eq!(model.previous_change(None), None);
        assert_eq!((model.change_at(3), model.change_at(4)), (Some(0), None));
        assert_eq!(DiffModel::change_label(0, 2), "Change 1 of 2");
    }

    #[test]
    fn drafts_attach_to_changed_lines_and_send_once_with_references() {
        let mut model = DiffModel::new(sample());
        assert!(!model.add_draft(0, 0, "context takes no note"));
        assert!(model.add_draft(0, 1, "why remove this"));
        assert!(!model.add_draft(0, 1, "second note on one line"));
        assert!(model.add_draft(1, 1, "check the bounds"));
        assert_eq!(model.drafts(), 2);
        assert_eq!(
            model.send_drafts().as_deref(),
            Some("Review notes:\nsrc/lib.rs:2 why remove this\nsrc/lib.rs:41 check the bounds")
        );
        assert_eq!(model.drafts(), 0);
        assert_eq!(model.send_drafts(), None);
        assert!(model.note_on(0, 1).is_some_and(|note| note.sent));
    }

    #[test]
    fn an_empty_file_has_no_rows_no_changes_and_no_counts() {
        let model = DiffModel::new(FileDiff {
            path: "empty.rs".into(),
            status: FileStatus::Added,
            hunks: Vec::new(),
            hidden_after: Vec::new(),
        });
        assert!(model.rows().is_empty() && model.changes().is_empty());
        assert_eq!(model.counts(), (0, 0));
        assert_eq!(model.next_change(None), None);
    }
}

#[cfg(test)]
mod split_tests {
    use super::*;

    /// One hunk: two removed lines and one added line, so the split layout pairs the first.
    fn pair_file() -> FileDiff {
        FileDiff {
            path: "a.txt".into(),
            status: FileStatus::Modified,
            hunks: vec![Hunk {
                heading: Some("fn one()".into()),
                old_start: 1,
                old_len: 2,
                new_start: 1,
                new_len: 1,
                hidden_before: Vec::new(),
                lines: vec![
                    DiffLine::removed(1, "one"),
                    DiffLine::removed(2, "two"),
                    DiffLine::added(1, "uno"),
                ],
            }],
            hidden_after: Vec::new(),
        }
    }

    #[test]
    fn split_pairs_removed_lines_with_added_lines_in_order() {
        let split = DiffModel::new(pair_file()).split_rows();
        assert_eq!(
            split,
            vec![
                SplitRow::Hunk { hunk: 0 },
                SplitRow::Pair {
                    left: Some(1),
                    right: Some(3)
                },
                SplitRow::Pair {
                    left: Some(2),
                    right: None
                },
            ]
        );
    }

    #[test]
    fn ruler_marks_each_change_block_and_outlines_the_current_one() {
        let marks = DiffModel::new(pair_file()).ruler(Some(0));
        assert_eq!(marks.len(), 1);
        assert!(marks[0].removed && marks[0].added && marks[0].current);
        assert!((marks[0].top - 0.25).abs() < 1e-6);
        assert!((marks[0].height - 0.75).abs() < 1e-6);
    }

    #[test]
    fn hunk_titles_print_like_the_diff_tool_and_letters_match_the_chip() {
        assert_eq!(
            hunk_title(&pair_file().hunks[0]),
            "@@ -1,2 +1,1 @@ fn one()"
        );
        assert_eq!(FileStatus::Renamed.letter(), "R");
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext};
    use std::cell::RefCell;

    /// One hunk with 30 unchanged lines hidden before it, so the first row is a collapsed block.
    fn hidden_model() -> DiffModel {
        let hunk = Hunk {
            heading: None,
            old_start: 31,
            old_len: 2,
            new_start: 31,
            new_len: 2,
            hidden_before: (1..31u32)
                .map(|n| DiffLine::context(n, n, format!("line {n}")))
                .collect(),
            lines: vec![DiffLine::removed(31, "old"), DiffLine::added(31, "new")],
        };
        DiffModel::new(FileDiff {
            path: "src/lib.rs".into(),
            status: FileStatus::Modified,
            hunks: vec![hunk],
            hidden_after: Vec::new(),
        })
    }

    /// Draws a diff whose file is a submodule pointer change, in the status given.
    struct PointerHarness {
        status: DiffStatus,
    }

    impl Render for PointerHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(640.))
                .h(px(400.))
                .child(DiffView::new("diff", hidden_model()).status(self.status.clone()))
        }
    }

    #[gpui::test]
    fn a_submodule_pointer_change_draws_its_pointer_and_a_dirty_note(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| PointerHarness {
            status: DiffStatus::Submodule {
                summary: "Subproject commit 1111111aaaaaaa..2222222bbbbbbb".into(),
                dirty: Some(true),
            },
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("diff-submodule").is_some(),
            "the pointer is drawn"
        );
        assert!(
            cx.debug_bounds("diff-submodule-dirty").is_some(),
            "a checkout with changes says so"
        );
    }

    struct Harness {
        model: DiffModel,
        disconnected: bool,
        actions: Rc<RefCell<Vec<DiffAction>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            div().w(px(640.)).h(px(400.)).child(
                DiffView::new("diff", self.model.clone())
                    .disconnected(self.disconnected)
                    .on_action(move |action, _, _| actions.borrow_mut().push(action)),
            )
        }
    }

    #[gpui::test]
    fn a_collapsed_block_click_asks_for_the_next_lines(cx: &mut TestAppContext) {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| Harness {
            model: hidden_model(),
            disconnected: false,
            actions: actions.clone(),
        });
        cx.run_until_parked();
        let center = cx
            .debug_bounds("diff-collapsed-0")
            .expect("the collapsed block is drawn")
            .center();
        cx.simulate_click(center, Modifiers::none());
        assert_eq!(actions.borrow().as_slice(), [DiffAction::Reveal { gap: 0 }]);
    }

    #[gpui::test]
    fn send_to_agent_asks_the_owner_when_the_host_is_reachable(cx: &mut TestAppContext) {
        let mut model = hidden_model();
        assert!(model.add_draft(0, 1, "check this"));
        let actions = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| Harness {
            model,
            disconnected: false,
            actions: actions.clone(),
        });
        cx.run_until_parked();
        let send = cx
            .debug_bounds("diff-send-slot")
            .expect("send is drawn")
            .center();
        cx.simulate_click(send, Modifiers::none());
        assert_eq!(actions.borrow().as_slice(), [DiffAction::SendToAgent]);
    }

    #[gpui::test]
    fn send_to_agent_does_nothing_while_the_host_is_unreachable(cx: &mut TestAppContext) {
        let mut model = hidden_model();
        assert!(model.add_draft(0, 1, "check this"));
        let actions = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| Harness {
            model,
            disconnected: true,
            actions: actions.clone(),
        });
        cx.run_until_parked();
        let send = cx
            .debug_bounds("diff-send-slot")
            .expect("send is drawn")
            .center();
        cx.simulate_click(send, Modifiers::none());
        assert!(actions.borrow().is_empty());
    }
}
