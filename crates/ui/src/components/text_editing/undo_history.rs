//! Undo and redo for the text inputs. Each step keeps the text and selection from before it, so
//! undoing restores what the user saw. Consecutive typed characters, or consecutive backspaces or
//! deletes, share one step while the caret stays where the last one left it; every other edit
//! (paste, cut, a line break, a word delete, a selection replaced) is a step of its own.

use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;

/// The most steps kept; the oldest go first.
const MAX_STEPS: usize = 100;
/// The most text the kept steps hold together, so one large paste cannot pin megabytes.
const MAX_BYTES: usize = 4 << 20;

/// The text and selection an undo step restores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) text: String,
    pub(crate) selected: Range<usize>,
    pub(crate) reversed: bool,
}

/// How an edit groups with the edits before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditKind {
    /// A typed character or a composition.
    Typing,
    /// Backspace over a collapsed caret.
    Backspace,
    /// Delete over a collapsed caret.
    Delete,
    /// Anything else: always a step of its own.
    Other,
}

impl EditKind {
    fn groups(self) -> bool {
        !matches!(self, Self::Other)
    }
}

/// The undo and redo stacks of one text buffer.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct UndoHistory {
    /// Off for a masked value, which must not keep copies of itself.
    enabled: bool,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    /// The kind of the last edit, while the step it started can still grow.
    open: Option<EditKind>,
}

impl fmt::Debug for UndoHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The counts only: the steps hold the typed text.
        f.debug_struct("UndoHistory")
            .field("enabled", &self.enabled)
            .field("undo", &self.undo.len())
            .field("redo", &self.redo.len())
            .finish()
    }
}

impl UndoHistory {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            undo: VecDeque::new(),
            redo: Vec::new(),
            open: None,
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Turns recording on or off; either way the steps held are dropped.
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.clear();
    }

    /// Drops every step, as a programmatic replacement of the text does.
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = None;
    }

    /// Whether an edit of `kind` continues the open step; `settled` means the caret is collapsed or composing.
    pub(crate) fn joins(&self, kind: EditKind, settled: bool) -> bool {
        kind.groups() && settled && self.open == Some(kind)
    }

    /// Records an edit that changed the text. `before` is the state it started from, or `None` when it joined the open step.
    pub(crate) fn record(&mut self, before: Option<Snapshot>, kind: EditKind) {
        if !self.enabled {
            return;
        }
        if let Some(before) = before {
            self.undo.push_back(before);
            self.redo.clear();
            self.trim();
        }
        self.open = kind.groups().then_some(kind);
    }

    /// Ends the open step, so the next edit starts a new one.
    pub(crate) fn break_group(&mut self) {
        self.open = None;
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Takes the step to undo; `current` is kept so redo can return to it.
    pub(crate) fn take_undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let previous = self.undo.pop_back()?;
        self.redo.push(current);
        self.open = None;
        self.trim();
        Some(previous)
    }

    /// Takes the step to redo; `current` is kept so undo can return to it.
    pub(crate) fn take_redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        self.undo.push_back(current);
        self.open = None;
        self.trim();
        Some(next)
    }

    /// Drops the oldest steps, undo before redo, until the steps and the text held are within
    /// their caps. The newest undo step always stays, so the last edit can be undone.
    fn trim(&mut self) {
        loop {
            let over_steps = self.undo.len() + self.redo.len() > MAX_STEPS;
            if !(over_steps || self.bytes() > MAX_BYTES) {
                break;
            }
            if self.undo.len() > 1 {
                self.undo.pop_front();
            } else if !self.redo.is_empty() {
                self.redo.remove(0);
            } else {
                break;
            }
        }
    }

    fn bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|step| step.text.len())
            .sum()
    }
}
