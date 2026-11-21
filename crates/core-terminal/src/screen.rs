//! The parsed screen model and the attach snapshot (decision 0034).
//!
//! A snapshot is an escape stream that rebuilds this screen in any emulator of the same size.

use std::fmt;
use std::num::NonZeroU16;

use vt100::{Callbacks, Cell, Color};

/// Scrollback rows kept per terminal.
pub const SCROLLBACK_ROWS: usize = 1000;

/// Longest window title kept, in characters.
const TITLE_MAX_CHARS: usize = 1024;

const RESET: &str = "\x1bc";
const CLEAR_ATTRS: &str = "\x1b[m";

#[derive(Debug, Default)]
struct TitleCallbacks {
    title: Option<String>,
}

impl Callbacks for TitleCallbacks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = Some(
            String::from_utf8_lossy(title)
                .chars()
                .take(TITLE_MAX_CHARS)
                .collect(),
        );
    }
}

/// A parsed terminal screen with scrollback, sized like the PTY.
pub struct Screen {
    parser: vt100::Parser<TitleCallbacks>,
}

impl fmt::Debug for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (cols, rows) = self.size();
        f.debug_struct("Screen")
            .field("cols", &cols)
            .field("rows", &rows)
            .field("title", &self.title())
            .finish_non_exhaustive()
    }
}

/// Zero is raised to 1: a terminal always has a cell.
fn dimension(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n).unwrap_or(NonZeroU16::MIN)
}

impl Screen {
    pub fn new(cols: u16, rows: u16) -> Self {
        let parser = vt100::Parser::new_with_callbacks(
            dimension(rows),
            dimension(cols),
            SCROLLBACK_ROWS,
            TitleCallbacks::default(),
        );
        Self { parser }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.parser.set_size(dimension(rows), dimension(cols));
    }

    /// (cols, rows)
    pub fn size(&self) -> (u16, u16) {
        let (rows, cols) = self.parser.screen().size();
        (cols.get(), rows.get())
    }

    pub fn title(&self) -> Option<&str> {
        self.parser.callbacks().title.as_deref()
    }

    /// Escape bytes that rebuild this screen in a fresh emulator of the same size.
    ///
    /// Reset, the scrollback oldest first, then the visible rows, then the cursor.
    /// Each row lands on the bottom of a blank screen, so scrollback scrolls off in order.
    pub fn snapshot(&mut self) -> Vec<u8> {
        let mut out = String::from(RESET);
        write_rows(&mut out, self.parser.screen_mut(), true);
        place_cursor(&mut out, self.parser.screen());
        out.into_bytes()
    }
}

/// Writes scrollback (oldest first, when `with_scrollback`) then the visible rows.
///
/// Each row after the first starts a new line unless the row before it soft-wraps into it. The
/// total line count is therefore rows-1 plus one per scrollback row, so exactly the scrollback
/// rows scroll off the top of the target.
fn write_rows(out: &mut String, screen: &mut vt100::Screen, with_scrollback: bool) {
    let (rows, cols) = screen.size();
    let (rows, cols) = (rows.get(), cols.get());
    let scrollback_len = if with_scrollback {
        screen.scrollback_len()
    } else {
        0
    };
    // The target may still hold a pen from an earlier section.
    out.push_str(CLEAR_ATTRS);
    let mut style = Style::default();
    let mut previous_wraps = false;
    for index in 0..scrollback_len + usize::from(rows) {
        let (view_row, offset) = if index < scrollback_len {
            // With the view at `len - index`, its first row is scrollback row `index`.
            (0, scrollback_len - index)
        } else {
            (u16::try_from(index - scrollback_len).unwrap_or(0), 0)
        };
        screen.set_scrollback(offset);
        let wrapped = screen.row_wrapped(view_row);
        if index > 0 && !previous_wraps {
            reset_style(out, &mut style);
            out.push_str("\r\n");
        }
        write_row(
            out,
            screen,
            view_row,
            cols,
            previous_wraps,
            wrapped,
            &mut style,
        );
        previous_wraps = wrapped;
    }
    screen.set_scrollback(0);
}

/// Writes one row's cells from column 0, leaving the cursor after its last written cell.
///
/// `continuation`: the previous row soft-wraps into this one, so the cursor is already at the
/// start of this row once something prints. `wrapped`: this row soft-wraps into the next one,
/// so it is written full width and has no line end.
fn write_row(
    out: &mut String,
    screen: &vt100::Screen,
    row: u16,
    cols: u16,
    continuation: bool,
    wrapped: bool,
    style: &mut Style,
) {
    let cell = |col: u16| screen.cell(row, col);
    let mut col: u16 = 0;
    if continuation && !cell(0).is_some_and(Cell::has_contents) {
        // A blank cannot advance the wrap, so print one, erase it, and step over the column.
        set_style(out, style, Style::of(cell(0)));
        out.push_str(" \x08\x1b[X\x1b[C");
        col = 1;
    }

    let text_end = (0..cols)
        .rev()
        .find_map(|c| {
            let cell = cell(c)?;
            cell.has_contents()
                .then(|| c.saturating_add(cell_width(cell)).min(cols))
        })
        .unwrap_or(0);
    let end = if wrapped { cols } else { text_end };
    while col < end {
        let Some(here) = cell(col) else { break };
        if here.has_contents() {
            set_style(out, style, Style::of(Some(here)));
            out.push_str(here.contents());
            col += cell_width(here);
            continue;
        }
        // A run of text-free cells with one style. Plain cells are stepped over, so the target
        // leaves them empty as the source does. Coloured ones are erased. Only a wrapped row
        // that runs to its last column prints them: the wrap needs a printed cell.
        let blank = Style::of(Some(here));
        let mut run_end = col + 1;
        while run_end < end
            && !cell(run_end).is_some_and(Cell::has_contents)
            && Style::of(cell(run_end)) == blank
        {
            run_end += 1;
        }
        let run = run_end - col;
        if wrapped && run_end == cols {
            set_style(out, style, blank);
            out.push_str(&" ".repeat(usize::from(run)));
        } else if blank == Style::default() {
            out.push_str(&format!("\x1b[{run}C"));
        } else {
            set_style(out, style, blank);
            out.push_str(&format!("\x1b[{run}X\x1b[{run}C"));
        }
        col = run_end;
    }
    if wrapped {
        return;
    }

    // Blank cells that carry colour (an erase with background) keep it through ECH; text-free
    // cells are never written, so the target's trailing blanks match.
    let mut at = col;
    let mut c = col;
    while c < cols {
        let here = Style::of(cell(c));
        if here == Style::default() {
            c += 1;
            continue;
        }
        let start = c;
        while c < cols && Style::of(cell(c)) == here {
            c += 1;
        }
        if start > at {
            out.push_str(&format!("\x1b[{}C", start - at));
        }
        set_style(out, style, here);
        out.push_str(&format!("\x1b[{}X", c - start));
        // ECH leaves the cursor where it was.
        at = start;
    }
    reset_style(out, style);
}

/// Cursor and pen, last. vt100 positions the cursor absolutely when given no previous position.
fn place_cursor(out: &mut String, screen: &vt100::Screen) {
    out.push_str(&screen.cursor_state_formatted());
    out.push_str(&screen.attributes_formatted());
}

fn cell_width(cell: &Cell) -> u16 {
    if cell.is_wide() { 2 } else { 1 }
}

/// The drawing attributes of a cell, as SGR can set them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
}

impl Style {
    fn of(cell: Option<&Cell>) -> Self {
        cell.map_or_else(Self::default, |cell| Self {
            fg: cell.fgcolor(),
            bg: cell.bgcolor(),
            bold: cell.bold(),
            dim: cell.dim(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        })
    }
}

fn set_style(out: &mut String, current: &mut Style, wanted: Style) {
    if *current != wanted {
        out.push_str(&sgr(wanted));
        *current = wanted;
    }
}

fn reset_style(out: &mut String, current: &mut Style) {
    set_style(out, current, Style::default());
}

fn sgr(style: Style) -> String {
    let mut params = String::from("0");
    for (on, code) in [
        (style.bold, "1"),
        (style.dim, "2"),
        (style.italic, "3"),
        (style.underline, "4"),
        (style.inverse, "7"),
    ] {
        if on {
            params.push(';');
            params.push_str(code);
        }
    }
    push_colour(&mut params, style.fg, 38);
    push_colour(&mut params, style.bg, 48);
    format!("\x1b[{params}m")
}

fn push_colour(params: &mut String, colour: Color, base: u8) {
    match colour {
        Color::Default => {}
        Color::Idx(index) => params.push_str(&format!(";{base};5;{index}")),
        Color::Rgb(r, g, b) => params.push_str(&format!(";{base};2;{r};{g};{b}")),
    }
}
