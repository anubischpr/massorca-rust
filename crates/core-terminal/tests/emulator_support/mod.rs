//! Helpers shared by the snapshot tests: alacritty built as the desktop feed builds it, and vt100
//! as the model. Each test binary uses a different part of this module.

#![allow(
    dead_code,
    reason = "each test binary uses a different part of this module"
)]

use std::num::NonZeroU16;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use massorca_core_terminal::screen::SCROLLBACK_ROWS;

fn size(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n).expect("test sizes are non-zero")
}

/// A vt100 model fed `bytes`: the service's own model, or a snapshot rebuilt by it.
pub fn vt100_model(cols: u16, rows: u16, bytes: &[u8]) -> vt100::Parser {
    let mut parser = vt100::Parser::new(size(rows), size(cols), SCROLLBACK_ROWS);
    parser.process(bytes);
    parser
}

#[derive(Clone, Copy)]
struct Silent;

impl EventListener for Silent {
    fn send_event(&self, _event: Event) {}
}

#[derive(Clone, Copy)]
struct Shape {
    columns: usize,
    lines: usize,
}

impl Dimensions for Shape {
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

/// An alacritty emulator at a fixed size, fed as the desktop feed feeds it.
pub struct Client {
    term: Term<Silent>,
    parser: Processor,
    columns: usize,
    lines: usize,
}

impl Client {
    pub fn new(cols: u16, rows: u16) -> Self {
        let shape = Shape {
            columns: usize::from(cols),
            lines: usize::from(rows),
        };
        let config = Config {
            scrolling_history: SCROLLBACK_ROWS,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &shape, Silent),
            parser: Processor::new(),
            columns: shape.columns,
            lines: shape.lines,
        }
    }

    /// Takes a new size, as the desktop feed does when the terminal is resized.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let shape = Shape {
            columns: usize::from(cols),
            lines: usize::from(rows),
        };
        self.term.resize(shape);
        (self.columns, self.lines) = (shape.columns, shape.lines);
    }

    pub fn advance(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn alternate(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    /// (row, col), 0-based.
    pub fn cursor(&self) -> (u16, u16) {
        let point = self.term.grid().cursor.point;
        (
            u16::try_from(point.line.0).expect("the cursor is on screen"),
            u16::try_from(point.column.0).expect("the cursor is on screen"),
        )
    }

    /// The visible rows, each without its trailing blanks.
    pub fn rows(&self) -> Vec<String> {
        let grid = self.term.grid();
        (0..self.lines)
            .map(|line| {
                let row = &grid[Line(i32::try_from(line).expect("a line index"))];
                (0..self.columns)
                    .map(|column| row[Column(column)].c)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    /// The first visible row whose whole text is `text`.
    pub fn row_of(&self, text: &str) -> Option<usize> {
        self.rows().iter().position(|row| row == text)
    }
}

/// The client shows what `reference` shows: the active screen, the cursor and every row.
pub fn assert_client_matches(what: &str, reference: &vt100::Parser, client: &Client) {
    let screen = reference.screen();
    assert_eq!(
        client.alternate(),
        screen.alternate_screen(),
        "{what}: alternate screen"
    );
    assert_eq!(client.cursor(), screen.cursor_position(), "{what}: cursor");
    assert_eq!(client.rows(), trimmed_rows(screen), "{what}: rows");
}

/// A vt100 model shows what `reference` shows, as `assert_client_matches` checks.
pub fn assert_model_matches(what: &str, reference: &vt100::Parser, model: &vt100::Parser) {
    let (want, got) = (reference.screen(), model.screen());
    assert_eq!(
        got.alternate_screen(),
        want.alternate_screen(),
        "{what}: alternate screen"
    );
    assert_eq!(
        got.cursor_position(),
        want.cursor_position(),
        "{what}: cursor"
    );
    assert_eq!(trimmed_rows(got), trimmed_rows(want), "{what}: rows");
}

fn trimmed_rows(screen: &vt100::Screen) -> Vec<String> {
    let (_, cols) = screen.size();
    screen
        .rows(0, cols.get())
        .map(|row| row.trim_end().to_owned())
        .collect()
}
