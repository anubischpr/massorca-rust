//! The emulator behind a terminal surface, and the offset rules of the attach
//! stream (docs/architecture/terminals.md; decisions 0034 and 0035).
//!
//! A feed holds the emulator and the next output offset it has read up to. It
//! skips output the emulator already has, applies the rest, and reports a gap
//! instead of applying bytes it never received. It has no GPUI types: the
//! surface reads it through a render snapshot and drives it through `apply`.

use std::fmt;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{
    Config, MIN_COLUMNS, MIN_SCREEN_LINES, Term, TermMode, viewport_to_point,
};
use alacritty_terminal::vte::ansi::Processor;

mod snapshot;

pub use snapshot::{RenderCell, RenderCursor, RenderSnapshot};

/// Scrollback kept per terminal: the default of the desktop setting.
const SCROLLBACK_LINES: usize = 10_000;
/// The service's size limits (decision 0033), so a resize cannot ask for more.
pub const MAX_COLUMNS: usize = 1000;
/// The service's line limit (decision 0033).
pub const MAX_LINES: usize = 500;
/// Replies the program has not taken yet. A cursor report is 6 bytes, so this is generous.
const MAX_REPLY_BYTES: usize = 4096;

/// One item of an attach stream, decoded from its JSON event or binary chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedEvent<'a> {
    /// The first item: output continues from `offset`.
    Attached { offset: u64 },
    /// A redraw of `len` snapshot bytes follows; output continues from `offset`.
    Snapshot {
        offset: u64,
        cols: u16,
        rows: u16,
        len: u64,
    },
    /// One chunk of the snapshot's bytes, in order.
    SnapshotBytes(&'a [u8]),
    /// PTY output, starting at `offset`.
    Output { offset: u64, bytes: &'a [u8] },
    /// The terminal changed size.
    Resized { cols: u16, rows: u16 },
    /// The process ended; `code` is its exit code when it had one.
    Exited { code: Option<i32> },
}

/// What applying an event did to the emulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedOutcome {
    /// The bytes or the change were applied.
    Applied,
    /// The output lies wholly before the offset already read: nothing changed.
    Skipped,
    /// The output starts past the offset already read, so bytes are missing.
    /// Nothing was applied; the owner must re-attach without `since`.
    Gap { expected: u64, got: u64 },
}

/// An event that breaks the stream's protocol. The feed keeps its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedError {
    /// Output or snapshot bytes arrived before `Attached`.
    NotAttached,
    /// Output arrived while the snapshot still needed `missing` bytes.
    SnapshotIncomplete { missing: u64 },
    /// Snapshot bytes arrived without a `Snapshot` item.
    UnexpectedSnapshot,
    /// More snapshot bytes arrived than the `Snapshot` item announced.
    SnapshotOverflow { remaining: u64, got: usize },
}

/// How the process ended, as far as the feed knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    /// `None` when the process ended by a signal or the code is unknown.
    pub code: Option<i32>,
}

/// A cell of the visible grid. `row` counts from the top of the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewPoint {
    pub row: usize,
    pub column: usize,
}

/// The emulator of one terminal and the output offset it has read up to.
pub struct TerminalFeed {
    term: Term<Listener>,
    parser: Processor,
    reports: Arc<Mutex<Reports>>,
    size: Size,
    next_offset: Option<u64>,
    pending: Option<Pending>,
    exit: Option<Exit>,
}

impl TerminalFeed {
    /// An emulator of `cols` x `rows` that has not attached to a terminal yet.
    pub fn new(cols: u16, rows: u16) -> Self {
        let reports = Arc::default();
        let size = Size::clamped(cols, rows);
        Self {
            term: new_term(size, &reports),
            parser: Processor::new(),
            reports,
            size,
            next_offset: None,
            pending: None,
            exit: None,
        }
    }

    /// Applies one item of the attach stream.
    pub fn apply(&mut self, event: FeedEvent<'_>) -> Result<FeedOutcome, FeedError> {
        match event {
            FeedEvent::Attached { offset } => {
                self.pending = None;
                self.next_offset = Some(offset);
                Ok(FeedOutcome::Applied)
            }
            FeedEvent::Snapshot {
                offset,
                cols,
                rows,
                len,
            } => {
                self.reset(Size::clamped(cols, rows));
                self.pending = (len > 0).then_some(Pending {
                    offset,
                    remaining: len,
                });
                if len == 0 {
                    self.next_offset = Some(offset);
                }
                Ok(FeedOutcome::Applied)
            }
            FeedEvent::SnapshotBytes(bytes) => self.snapshot_bytes(bytes),
            FeedEvent::Output { offset, bytes } => self.output(offset, bytes),
            FeedEvent::Resized { cols, rows } => {
                self.resize(Size::clamped(cols, rows));
                Ok(FeedOutcome::Applied)
            }
            FeedEvent::Exited { code } => {
                self.exit = Some(Exit { code });
                Ok(FeedOutcome::Applied)
            }
        }
    }

    /// Columns of the emulator.
    pub fn columns(&self) -> usize {
        self.size.columns
    }

    /// Lines of the emulator.
    pub fn lines(&self) -> usize {
        self.size.lines
    }

    /// The offset output continues from, or `None` before the first attach.
    pub fn next_offset(&self) -> Option<u64> {
        self.next_offset
    }

    /// How the process ended, once it has.
    pub fn exit(&self) -> Option<Exit> {
        self.exit
    }

    /// The text of one visible row of the live screen, trailing blanks removed.
    pub fn row_text(&self, row: usize) -> String {
        let grid = self.term.grid();
        let text: String = (0..self.columns())
            .map(|column| grid[Line(row as i32)][Column(column)].c)
            .collect();
        text.trim_end().to_owned()
    }

    /// The window title the program set with OSC 0 or 2.
    pub fn title(&self) -> Option<String> {
        self.reports
            .lock()
            .ok()
            .and_then(|reports| reports.title.clone())
    }

    /// The bytes the emulator owes the program (cursor reports and the like), taken once. An
    /// attached client answers queries (decision 0034), so the owner sends these as input.
    pub fn take_replies(&mut self) -> Vec<u8> {
        self.reports
            .lock()
            .map(|mut reports| std::mem::take(&mut reports.replies))
            .unwrap_or_default()
    }

    /// Moves the view through the scrollback.
    pub fn scroll(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    /// Starts a selection at a visible cell.
    pub fn begin_selection(&mut self, kind: SelectionType, at: ViewPoint, side: Side) {
        let point = self.to_grid(at);
        self.term.selection = Some(Selection::new(kind, point, side));
    }

    /// Moves the free end of the selection to a visible cell.
    pub fn extend_selection(&mut self, at: ViewPoint, side: Side) {
        let point = self.to_grid(at);
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    /// Drops the selection.
    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// The selected text, if any.
    pub fn selected_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    /// The input modes the program has set.
    pub fn modes(&self) -> TermMode {
        *self.term.mode()
    }

    fn output(&mut self, offset: u64, bytes: &[u8]) -> Result<FeedOutcome, FeedError> {
        if let Some(pending) = self.pending {
            return Err(FeedError::SnapshotIncomplete {
                missing: pending.remaining,
            });
        }
        let next = self.next_offset.ok_or(FeedError::NotAttached)?;
        let end = offset + bytes.len() as u64;
        if end <= next {
            return Ok(FeedOutcome::Skipped);
        }
        if offset > next {
            return Ok(FeedOutcome::Gap {
                expected: next,
                got: offset,
            });
        }
        // `offset <= next < end`: the bytes before `next` are already in the emulator.
        let already = (next - offset) as usize;
        self.parser.advance(&mut self.term, &bytes[already..]);
        self.next_offset = Some(end);
        Ok(FeedOutcome::Applied)
    }

    fn snapshot_bytes(&mut self, bytes: &[u8]) -> Result<FeedOutcome, FeedError> {
        let pending = self.pending.as_mut().ok_or(FeedError::UnexpectedSnapshot)?;
        let got = bytes.len() as u64;
        if got > pending.remaining {
            return Err(FeedError::SnapshotOverflow {
                remaining: pending.remaining,
                got: bytes.len(),
            });
        }
        pending.remaining -= got;
        let (offset, done) = (pending.offset, pending.remaining == 0);
        self.parser.advance(&mut self.term, bytes);
        if done {
            self.pending = None;
            self.next_offset = Some(offset);
        }
        Ok(FeedOutcome::Applied)
    }

    /// Starts a fresh emulator at `size`: the state a snapshot redraws onto.
    fn reset(&mut self, size: Size) {
        self.size = size;
        self.term = new_term(size, &self.reports);
        self.parser = Processor::new();
    }

    fn resize(&mut self, size: Size) {
        self.size = size;
        self.term.resize(size);
    }

    fn to_grid(&self, at: ViewPoint) -> Point {
        viewport_to_point(
            self.term.grid().display_offset(),
            Point::new(at.row, Column(at.column)),
        )
    }
}

impl fmt::Debug for TerminalFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalFeed")
            .field("size", &self.size)
            .field("next_offset", &self.next_offset)
            .field("pending", &self.pending)
            .field("exit", &self.exit)
            .finish_non_exhaustive()
    }
}

fn new_term(size: Size, reports: &Arc<Mutex<Reports>>) -> Term<Listener> {
    let config = Config {
        scrolling_history: SCROLLBACK_LINES,
        ..Config::default()
    };
    Term::new(config, &size, Listener(Arc::clone(reports)))
}

/// The grid size the emulator is built for, within the service's limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Size {
    columns: usize,
    lines: usize,
}

impl Size {
    fn clamped(cols: u16, rows: u16) -> Self {
        Self {
            columns: usize::from(cols).clamp(MIN_COLUMNS, MAX_COLUMNS),
            lines: usize::from(rows).clamp(MIN_SCREEN_LINES, MAX_LINES),
        }
    }
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// A snapshot still waiting for bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    offset: u64,
    remaining: u64,
}

/// What the emulator reports besides the screen.
#[derive(Debug, Default)]
struct Reports {
    title: Option<String>,
    replies: Vec<u8>,
}

/// Hands the emulator's reports to the feed that owns it.
#[derive(Clone, Debug)]
struct Listener(Arc<Mutex<Reports>>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let Ok(mut reports) = self.0.lock() else {
            return;
        };
        match event {
            Event::Title(title) => reports.title = Some(title),
            Event::ResetTitle => reports.title = None,
            Event::PtyWrite(text) if reports.replies.len() + text.len() <= MAX_REPLY_BYTES => {
                reports.replies.extend_from_slice(text.as_bytes());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
