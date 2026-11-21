//! DiffView: one file's changes, opened as a Work panel tab (docs/design/components/DiffView.md).
//!
//! This module holds the model: the file's hunks as plain data, and the display rows, change
//! blocks, split pairs, notes and counts derived from them. The core computes the hunks off the
//! render path; the view only draws what the model says.

use gpui::SharedString;
use std::ops::Range;

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
