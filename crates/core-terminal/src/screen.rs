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

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const SO: u8 = 0x0e;
const SI: u8 = 0x0f;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;

const RESET: &str = "\x1bc";
const CLEAR_ATTRS: &str = "\x1b[m";
const HOME: &str = "\x1b[H";
// `?47` switches grids without the side effects of `?1049`, which save the main cursor.
const ENTER_ALTERNATE_GRID: &str = "\x1b[?47h";
const LEAVE_ALTERNATE_GRID: &str = "\x1b[?47l";
const HIDE: &str = "\x1b[?25l";
const SHOW: &str = "\x1b[?25h";
/// A longer escape sequence is dropped whole, as vt100 drops it.
const MAX_SEQUENCE: usize = 64;
const MAIN: usize = 0;
const ALTERNATE: usize = 1;

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
    modes: Modes,
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
        Self {
            parser,
            modes: Modes::default(),
        }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        let input = self.modes.filter(bytes);
        self.parser.process(&input);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.parser.set_size(dimension(rows), dimension(cols));
    }

    /// (cols, rows)
    pub fn size(&self) -> (u16, u16) {
        let (rows, cols) = self.parser.screen().size();
        (cols.get(), rows.get())
    }

    /// (row, col), 0-based.
    pub fn cursor_position(&self) -> (u16, u16) {
        self.parser.screen().cursor_position()
    }

    pub fn title(&self) -> Option<&str> {
        self.parser.callbacks().title.as_deref()
    }

    /// Escape bytes that rebuild this screen in a fresh emulator of the same size: reset, the
    /// scrollback oldest first, each grid's rows with its margins and cursors (main first), then
    /// the character set, autowrap, pen and input modes.
    pub fn snapshot(&self) -> Vec<u8> {
        let (rows, cols) = self.parser.screen().size();
        // The reads and the rows come from a copy, so the live screen keeps its cursor and view.
        let mut probe = vt100::Parser::new(rows, cols, SCROLLBACK_ROWS);
        *probe.screen_mut() = self.parser.screen().clone();
        let alternate = probe.screen().alternate_screen();
        if alternate {
            // Leaving the grid without `?1049l` keeps the main cursor where it is.
            probe.process(LEAVE_ALTERNATE_GRID.as_bytes());
        }
        let mut out = String::from(RESET);
        write_rows(&mut out, probe.screen_mut(), true);
        let main = read_grid(&mut probe, self.modes.origin, MAIN);
        // Always written, so that a later `?47h` shows the alternate grid as it was.
        probe.process(ENTER_ALTERNATE_GRID.as_bytes());
        let mut alternate_rows = String::new();
        write_rows(&mut alternate_rows, probe.screen_mut(), false);
        let alternate_grid = read_grid(&mut probe, self.modes.origin, ALTERNATE);

        let (saved, rows) = (self.modes.saved_charsets, rows.get());
        write_grid(&mut out, &main, saved, rows);
        out.push_str(ENTER_ALTERNATE_GRID);
        out.push_str(HOME);
        out.push_str(&alternate_rows);
        write_grid(&mut out, &alternate_grid, saved, rows);
        if !alternate {
            out.push_str(LEAVE_ALTERNATE_GRID);
        }
        if self.modes.charsets != Charsets::default() {
            out.push_str(&charset_sequence(self.modes.charsets));
        }
        if self.modes.autowrap_off {
            out.push_str("\x1b[?7l");
        }
        let live = self.parser.screen();
        out.push_str(&live.attributes_formatted());
        out.push_str(if live.hide_cursor() { HIDE } else { SHOW });
        out.push_str(&live.input_mode_formatted());
        out.into_bytes()
    }
}

/// Where vt100 holds the cursor, and where it is drawn: one column back while it waits to wrap.
struct Placement {
    raw: (u16, u16),
    at: (u16, u16),
    reprint: Option<(Style, String)>,
}

impl Placement {
    fn of(screen: &vt100::Screen, (row, col): (u16, u16)) -> Self {
        let cols = screen.size().1.get();
        let wraps = col >= cols;
        let wide = cols >= 2 && screen.cell(row, cols - 2).is_some_and(Cell::is_wide);
        let last = cols - 1 - u16::from(wide);
        let at = if wraps { last } else { col };
        let reprint = screen
            .cell(row, at)
            .filter(|cell| wraps && cell.has_contents())
            .map(|cell| (Style::of(Some(cell)), cell.contents().to_owned()));
        Self {
            raw: (row, col),
            at: (row, at),
            reprint,
        }
    }
}

/// One grid as read from a copy of the screen.
struct GridView {
    cursor: Placement,
    cursor_bytes: String,
    margins: (u16, u16),
    origin: bool,
    saved: Placement,
    saved_bytes: String,
    saved_origin: bool,
    pen: String,
}

/// Reads the active grid of the copy; `origin` holds the live modes, `index` picks the grid.
fn read_grid(probe: &mut vt100::Parser, origin: [bool; 4], index: usize) -> GridView {
    let screen = probe.screen();
    let cursor = Placement::of(screen, screen.cursor_position());
    let cursor_bytes = screen.cursor_state_formatted();
    // Origin mode homes to the top margin, and `CUP` clamps to the bottom one.
    probe.process(b"\x1b[?6h");
    let top = probe.screen().cursor_position().0;
    probe.process(b"\x1b[999;1H");
    let bottom = probe.screen().cursor_position().0;
    probe.process(b"\x1b8");
    let screen = probe.screen();
    GridView {
        cursor,
        cursor_bytes,
        margins: (top, bottom),
        origin: origin[index],
        saved: Placement::of(screen, screen.cursor_position()),
        saved_bytes: screen.cursor_state_formatted(),
        saved_origin: origin[index + 2],
        pen: screen.attributes_formatted(),
    }
}

/// Writes a grid's margins, saved cursor and cursor after its rows, each before what it moves.
fn write_grid(out: &mut String, grid: &GridView, saved: Charsets, rows: u16) {
    // A fresh grid has the whole screen as its region and origin mode off.
    let mut region = (0, rows - 1);
    let mut origin = false;
    let (top, bottom) = grid.margins;
    // Origin mode cannot reach a row outside the margins: the saved cursor is written with the
    // whole screen as its region, and the margins come back after DECSC.
    let outside = grid.saved_origin && !(top..=bottom).contains(&grid.saved.at.0);
    let saved_region = if outside { region } else { grid.margins };
    if grid.saved.raw != (0, 0)
        || grid.saved_origin
        || grid.pen != CLEAR_ATTRS
        || saved != Charsets::default()
    {
        set_region(out, &mut region, saved_region);
        set_origin(out, &mut origin, grid.saved_origin);
        place(out, &grid.saved, &grid.saved_bytes, origin, region.0);
        out.push_str(&grid.pen);
        if saved != Charsets::default() {
            out.push_str(&charset_sequence(saved));
        }
        out.push_str("\x1b7");
        if saved != Charsets::default() {
            out.push_str(&charset_sequence(Charsets::default()));
        }
    }
    set_region(out, &mut region, grid.margins);
    set_origin(out, &mut origin, grid.origin);
    place(out, &grid.cursor, &grid.cursor_bytes, origin, region.0);
}

/// Puts the cursor at `placement`: vt100's `bytes` with origin mode off, else relative to `top`.
fn place(out: &mut String, placement: &Placement, bytes: &str, origin: bool, top: u16) {
    if !origin {
        // vt100 redraws a pending cell with the default pen, so the pen is reset first.
        out.push_str(CLEAR_ATTRS);
        out.push_str(bytes);
        return;
    }
    move_to(out, placement.at, top);
    if let Some((style, text)) = &placement.reprint {
        out.push_str(&sgr(*style));
        out.push_str(text);
    }
}

/// Positions the cursor at `(row, col)`, with the row counted from `top`.
fn move_to(out: &mut String, (row, col): (u16, u16), top: u16) {
    let (row, col) = (row.saturating_sub(top) + 1, col + 1);
    out.push_str(&format!("\x1b[{row};{col}H"));
}

/// Sets the scroll region unless it is already `wanted`. Setting it homes the cursor.
fn set_region(out: &mut String, current: &mut (u16, u16), wanted: (u16, u16)) {
    if *current != wanted {
        out.push_str(&format!("\x1b[{};{}r", wanted.0 + 1, wanted.1 + 1));
        *current = wanted;
    }
}

/// Sets origin mode, which homes the cursor, unless it is already `wanted`.
fn set_origin(out: &mut String, current: &mut bool, wanted: bool) {
    if *current != wanted {
        out.push_str(if wanted { "\x1b[?6h" } else { "\x1b[?6l" });
        *current = wanted;
    }
}

/// Designates G0 and G1 and selects GL, as `state` says.
fn charset_sequence(state: Charsets) -> String {
    let set = |line_drawing: bool| if line_drawing { '0' } else { 'B' };
    let shift = if state.shifted { SO } else { SI };
    let (g0, g1) = (set(state.g0), set(state.g1));
    format!("\x1b({g0}\x1b){g1}{}", char::from(shift))
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

/// G0 and G1 (`true` for DEC line drawing, otherwise ASCII), and whether GL is G1 (after SO).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Charsets {
    g0: bool,
    g1: bool,
    shifted: bool,
}

impl Charsets {
    fn gl_is_line_drawing(self) -> bool {
        if self.shifted { self.g1 } else { self.g0 }
    }
}

/// The byte scanner's state, following the parser's states. OSC is ended by BEL too, DCS and the
/// like only by ST; an ESC inside a string is held until a backslash shows whether it is ST.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ParseState {
    #[default]
    Ground,
    Escape,
    Csi,
    /// A sequence too long to keep, dropped up to its final byte.
    Ignored,
    Osc,
    Passthrough,
    StringEscape,
}

/// The modes vt100 ignores, followed from the bytes given to the screen. Bytes pass on unchanged,
/// except SCOSC and SCORC (as DECSC and DECRC) and line drawing (as the box characters it draws).
#[derive(Debug, Default)]
struct Modes {
    state: ParseState,
    /// The escape sequence in progress, from its ESC. It is held back until it ends.
    sequence: Vec<u8>,
    alternate: bool,
    /// Origin mode of the main and the alternate grid, then their saved origin modes.
    origin: [bool; 4],
    charsets: Charsets,
    saved_charsets: Charsets,
    autowrap_off: bool,
}

impl Modes {
    /// The bytes to pass to the parser for `input`, after noting the modes they set.
    fn filter(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &byte in input {
            self.step(byte, &mut out);
        }
        out
    }

    fn step(&mut self, byte: u8, out: &mut Vec<u8>) {
        match self.state {
            ParseState::Ground => match byte {
                ESC => self.begin(),
                SO | SI => self.shift(byte, out),
                0x5f..=0x7e if self.charsets.gl_is_line_drawing() => {
                    out.extend_from_slice(dec_graphics(byte).encode_utf8(&mut [0; 4]).as_bytes());
                }
                _ => out.push(byte),
            },
            ParseState::Osc | ParseState::Passthrough => match byte {
                ESC => {
                    self.state = ParseState::StringEscape;
                    self.sequence = vec![ESC];
                }
                BEL if self.state == ParseState::Osc => {
                    self.state = ParseState::Ground;
                    out.push(byte);
                }
                CAN | SUB => {
                    self.state = ParseState::Ground;
                    out.push(byte);
                }
                _ => out.push(byte),
            },
            ParseState::StringEscape if byte == b'\\' => {
                self.state = ParseState::Ground;
                self.sequence.clear();
                out.extend_from_slice(&[ESC, b'\\']);
            }
            ParseState::StringEscape => {
                // Not ST, so the held ESC starts a new sequence.
                self.state = ParseState::Escape;
                self.step(byte, out);
            }
            _ => {
                if !self.control(byte, out) {
                    self.sequence_byte(byte, out);
                }
            }
        }
    }

    /// What every escape state shares: ESC restarts, CAN and SUB cancel, SO and SI shift.
    fn control(&mut self, byte: u8, out: &mut Vec<u8>) -> bool {
        match byte {
            ESC => self.begin(),
            CAN | SUB => {
                self.state = ParseState::Ground;
                self.sequence.clear();
                out.push(byte);
            }
            SO | SI => self.shift(byte, out),
            0x00..=0x1f => out.push(byte),
            _ => return false,
        }
        true
    }

    fn sequence_byte(&mut self, byte: u8, out: &mut Vec<u8>) {
        match (self.state, byte) {
            (ParseState::Escape, 0x20..=0x2f) | (ParseState::Csi, 0x20..=0x3f) => {
                self.collect(byte);
            }
            (ParseState::Escape, 0x30..=0x7e) => self.escape_final(byte, out),
            (ParseState::Csi, 0x40..=0x7e) => self.csi_final(byte, out),
            (ParseState::Ignored, 0x40..=0x7e) => self.state = ParseState::Ground,
            _ => {}
        }
    }

    fn begin(&mut self) {
        self.state = ParseState::Escape;
        self.sequence = vec![ESC];
    }

    fn collect(&mut self, byte: u8) {
        if self.sequence.len() < MAX_SEQUENCE {
            self.sequence.push(byte);
        } else {
            self.state = ParseState::Ignored;
            self.sequence.clear();
        }
    }

    fn shift(&mut self, byte: u8, out: &mut Vec<u8>) {
        self.charsets.shifted = byte == SO;
        out.push(byte);
    }

    /// Passes the held sequence on, ended by `byte`.
    fn pass(&mut self, byte: u8, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.sequence);
        out.push(byte);
        self.sequence.clear();
    }

    fn escape_final(&mut self, byte: u8, out: &mut Vec<u8>) {
        self.state = ParseState::Ground;
        match (self.sequence.get(1).copied(), byte) {
            (Some(b'('), _) => self.charsets.g0 = byte == b'0',
            (Some(b')'), _) => self.charsets.g1 = byte == b'0',
            (None, b'[') => {
                self.state = ParseState::Csi;
                self.sequence.push(byte);
                return;
            }
            (None, b']') => self.state = ParseState::Osc,
            (None, b'P' | b'X' | b'^' | b'_') => self.state = ParseState::Passthrough,
            (None, b'7') => self.save(),
            (None, b'8') => self.restore(),
            (None, b'c') => self.reset(),
            _ => {}
        }
        self.pass(byte, out);
    }

    fn csi_final(&mut self, byte: u8, out: &mut Vec<u8>) {
        self.state = ParseState::Ground;
        let params = self.sequence.split_off(2);
        let replaced: Option<&[u8]> = match (params.as_slice(), byte) {
            ([], b's') => {
                self.save();
                Some(b"\x1b7")
            }
            ([], b'u') => {
                self.restore();
                Some(b"\x1b8")
            }
            ([b'?', modes @ ..], b'h' | b'l') => {
                let numbers = modes.split(|&b| b == b';');
                for mode in numbers.filter_map(|n| std::str::from_utf8(n).ok()?.parse().ok()) {
                    self.private_mode(mode, byte == b'h');
                }
                None
            }
            _ => None,
        };
        match replaced {
            Some(bytes) => {
                self.sequence.clear();
                out.extend_from_slice(bytes);
            }
            None => {
                self.sequence.extend(params);
                self.pass(byte, out);
            }
        }
    }

    fn private_mode(&mut self, mode: u16, set: bool) {
        let grid = self.grid();
        match mode {
            6 => self.origin[grid] = set,
            7 => self.autowrap_off = !set,
            47 => self.alternate = set,
            // As vt100 does it: save on the main grid, clear the alternate one, then switch.
            1049 if set => {
                self.save();
                self.origin[ALTERNATE] = false;
                self.origin[ALTERNATE + 2] = false;
                self.alternate = true;
            }
            1049 => {
                self.alternate = false;
                self.restore();
            }
            _ => {}
        }
    }

    fn grid(&self) -> usize {
        if self.alternate { ALTERNATE } else { MAIN }
    }

    /// DECSC: the origin mode of the active grid and the character sets.
    fn save(&mut self) {
        let grid = self.grid();
        self.origin[grid + 2] = self.origin[grid];
        self.saved_charsets = self.charsets;
    }

    fn restore(&mut self) {
        let grid = self.grid();
        self.origin[grid] = self.origin[grid + 2];
        self.charsets = self.saved_charsets;
    }

    /// RIS: every mode back to its start. The held sequence is not a mode, so it stays.
    fn reset(&mut self) {
        let sequence = std::mem::take(&mut self.sequence);
        *self = Self {
            sequence,
            ..Self::default()
        };
    }
}

/// The character DEC special graphics draws for a byte from 0x5f to 0x7e.
fn dec_graphics(byte: u8) -> char {
    const LINE_DRAWING: [char; 32] = [
        '\u{a0}', '\u{25c6}', '\u{2592}', '\u{2409}', '\u{240c}', '\u{240d}', '\u{240a}', '\u{b0}',
        '\u{b1}', '\u{2424}', '\u{240b}', '\u{2518}', '\u{2510}', '\u{250c}', '\u{2514}',
        '\u{253c}', '\u{23ba}', '\u{23bb}', '\u{2500}', '\u{23bc}', '\u{23bd}', '\u{251c}',
        '\u{2524}', '\u{2534}', '\u{252c}', '\u{2502}', '\u{2264}', '\u{2265}', '\u{3c0}',
        '\u{2260}', '\u{a3}', '\u{b7}',
    ];
    LINE_DRAWING[usize::from(byte - 0x5f)]
}
