//! What the surface paints: the visible cells with their colours resolved, the
//! cursor and the input modes of the emulator.

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors as Overrides;
use alacritty_terminal::term::{RenderableContent, TermMode, point_to_viewport};
use alacritty_terminal::vte::ansi::CursorShape;

use super::TerminalFeed;
use crate::components::terminal_surface::colors;
use crate::tokens::{Color, TerminalColors};

/// The visible grid, as the surface paints it.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSnapshot {
    pub columns: usize,
    pub lines: usize,
    /// `lines * columns` cells, row by row from the top of the view.
    pub cells: Vec<RenderCell>,
    /// Where the cursor is in the view; `None` when hidden or scrolled out of view.
    pub cursor: Option<RenderCursor>,
    /// The input modes the program has set.
    pub modes: TermMode,
    /// How many lines the view is scrolled back into the history.
    pub display_offset: usize,
}

/// One cell, with its colours already resolved (inverse, dim and selection applied).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderCell {
    pub ch: char,
    /// Combining marks drawn with `ch`.
    pub combining: Vec<char>,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    /// The first cell of a wide character, whose glyph spans this cell and the next.
    pub wide: bool,
    /// The second cell of a wide character: nothing is drawn here.
    pub spacer: bool,
}

/// The cursor's cell in the view and its shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderCursor {
    pub column: usize,
    pub line: usize,
    pub shape: CursorShape,
}

impl TerminalFeed {
    /// The visible grid with its colours resolved against `palette`.
    pub fn snapshot(&self, palette: &TerminalColors) -> RenderSnapshot {
        let (columns, lines) = (self.size.columns, self.size.lines);
        let RenderableContent {
            display_iter,
            selection,
            cursor: term_cursor,
            display_offset,
            colors,
            mode,
        } = self.term.renderable_content();
        let mut cells = vec![blank_cell(palette); columns * lines];
        for indexed in display_iter {
            let Some(point) = point_to_viewport(display_offset, indexed.point) else {
                continue;
            };
            if point.line >= lines || point.column.0 >= columns {
                continue;
            }
            let selected = selection
                .as_ref()
                .is_some_and(|range| range.contains(indexed.point));
            cells[point.line * columns + point.column.0] =
                render_cell(indexed.cell, palette, colors, selected);
        }
        let cursor = (term_cursor.shape != CursorShape::Hidden)
            .then(|| point_to_viewport(display_offset, term_cursor.point))
            .flatten()
            .filter(|point| point.line < lines && point.column.0 < columns)
            .map(|point| RenderCursor {
                column: point.column.0,
                line: point.line,
                shape: term_cursor.shape,
            });
        RenderSnapshot {
            columns,
            lines,
            cells,
            cursor,
            modes: mode,
            display_offset,
        }
    }
}

/// An empty cell in the default colours.
fn blank_cell(palette: &TerminalColors) -> RenderCell {
    RenderCell {
        ch: ' ',
        combining: Vec::new(),
        fg: palette.fg,
        bg: palette.bg,
        bold: false,
        italic: false,
        underline: false,
        strikeout: false,
        wide: false,
        spacer: false,
    }
}

/// A cell as the surface paints it: inverse, dim and hidden applied, and the selection
/// drawn as its background.
fn render_cell(
    cell: &Cell,
    palette: &TerminalColors,
    colors: &Overrides,
    selected: bool,
) -> RenderCell {
    let flags = cell.flags;
    let mut fg = colors::resolve(palette, cell.fg, colors);
    let mut bg = colors::resolve(palette, cell.bg, colors);
    if flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if flags.intersects(Flags::DIM) {
        fg = colors::mix(fg, bg, 0.5);
    }
    if flags.contains(Flags::HIDDEN) {
        fg = bg;
    }
    if selected {
        bg = palette.selection;
    }
    RenderCell {
        ch: cell.c,
        combining: cell.zerowidth().map(<[char]>::to_vec).unwrap_or_default(),
        fg,
        bg,
        bold: flags.contains(Flags::BOLD),
        italic: flags.contains(Flags::ITALIC),
        underline: flags.intersects(
            Flags::UNDERLINE
                | Flags::DOUBLE_UNDERLINE
                | Flags::UNDERCURL
                | Flags::DOTTED_UNDERLINE
                | Flags::DASHED_UNDERLINE,
        ),
        strikeout: flags.contains(Flags::STRIKEOUT),
        wide: flags.contains(Flags::WIDE_CHAR),
        spacer: flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER),
    }
}

#[cfg(test)]
mod tests {
    use super::super::ViewPoint;
    use super::*;
    use crate::components::terminal_surface::feed::FeedEvent;
    use crate::tokens::Palette;
    use alacritty_terminal::grid::Scroll;
    use alacritty_terminal::index::Side;
    use alacritty_terminal::selection::SelectionType;

    fn painted(cols: u16, rows: u16, bytes: &[u8]) -> TerminalFeed {
        let mut feed = TerminalFeed::new(cols, rows);
        feed.apply(FeedEvent::Attached { offset: 0 })
            .expect("attach");
        feed.apply(FeedEvent::Output { offset: 0, bytes })
            .expect("output");
        feed
    }

    fn cell(snapshot: &RenderSnapshot, row: usize, column: usize) -> &RenderCell {
        &snapshot.cells[row * snapshot.columns + column]
    }

    #[test]
    fn the_cells_hold_the_visible_text_row_by_row() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(5, 2, b"ab\r\ncd").snapshot(&palette);
        assert_eq!((snapshot.columns, snapshot.lines), (5, 2));
        assert_eq!(snapshot.cells.len(), 10);
        assert_eq!(cell(&snapshot, 0, 0).ch, 'a');
        assert_eq!(cell(&snapshot, 0, 1).ch, 'b');
        assert_eq!(cell(&snapshot, 1, 0).ch, 'c');
        assert_eq!(cell(&snapshot, 1, 1).ch, 'd');
    }

    #[test]
    fn the_cursor_is_a_block_after_the_text() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, b"ab").snapshot(&palette);
        assert_eq!(
            snapshot.cursor,
            Some(RenderCursor {
                column: 2,
                line: 0,
                shape: CursorShape::Block,
            })
        );
    }

    #[test]
    fn a_hidden_cursor_is_not_reported() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, b"\x1b[?25l").snapshot(&palette);
        assert_eq!(snapshot.cursor, None);
    }

    #[test]
    fn program_colours_resolve_against_the_palette() {
        let palette = Palette::DARK.terminal;
        let snapshot =
            painted(10, 3, b"\x1b[31mR\x1b[38;2;1;2;3mT\x1b[38;5;196mX").snapshot(&palette);
        assert_eq!(cell(&snapshot, 0, 0).fg, palette.ansi_red);
        assert_eq!(cell(&snapshot, 0, 1).fg, Color::rgba8(1, 2, 3, 1.0));
        assert_eq!(cell(&snapshot, 0, 2).fg, colors::indexed(196));
    }

    #[test]
    fn inverse_swaps_the_colours_and_dim_mixes_towards_the_background() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, b"\x1b[7mI\x1b[0m\x1b[2mD").snapshot(&palette);
        assert_eq!(cell(&snapshot, 0, 0).fg, palette.bg);
        assert_eq!(cell(&snapshot, 0, 0).bg, palette.fg);
        assert_eq!(
            cell(&snapshot, 0, 1).fg,
            colors::mix(palette.fg, palette.bg, 0.5)
        );
    }

    #[test]
    fn hidden_text_is_drawn_in_the_background_colour() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, b"\x1b[8mH").snapshot(&palette);
        assert_eq!(cell(&snapshot, 0, 0).fg, palette.bg);
    }

    #[test]
    fn bold_italic_underline_and_strikeout_are_reported() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, b"\x1b[1;3;4;9mA").snapshot(&palette);
        let a = cell(&snapshot, 0, 0);
        assert!(a.bold && a.italic && a.underline && a.strikeout);
        let plain = cell(&snapshot, 0, 1);
        assert!(!plain.bold && !plain.italic && !plain.underline && !plain.strikeout);
    }

    #[test]
    fn a_wide_character_takes_two_cells() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, "日a".as_bytes()).snapshot(&palette);
        assert_eq!(cell(&snapshot, 0, 0).ch, '日');
        assert!(cell(&snapshot, 0, 0).wide);
        assert!(cell(&snapshot, 0, 1).spacer);
        assert_eq!(cell(&snapshot, 0, 2).ch, 'a');
    }

    #[test]
    fn a_selection_is_painted_in_the_selection_colour() {
        let palette = Palette::DARK.terminal;
        let mut feed = painted(10, 3, b"hello");
        feed.begin_selection(
            SelectionType::Simple,
            ViewPoint { row: 0, column: 0 },
            Side::Left,
        );
        feed.extend_selection(ViewPoint { row: 0, column: 1 }, Side::Right);
        let snapshot = feed.snapshot(&palette);
        assert_eq!(cell(&snapshot, 0, 0).bg, palette.selection);
        assert_eq!(cell(&snapshot, 0, 1).bg, palette.selection);
        assert_eq!(cell(&snapshot, 0, 2).bg, palette.bg);
    }

    #[test]
    fn the_view_follows_the_scrollback() {
        let palette = Palette::DARK.terminal;
        let mut feed = painted(4, 2, b"1\r\n2\r\n3\r\n4");
        feed.scroll(Scroll::Delta(1));
        let snapshot = feed.snapshot(&palette);
        assert_eq!(snapshot.display_offset, 1);
        assert_eq!(cell(&snapshot, 0, 0).ch, '2');
        assert_eq!(cell(&snapshot, 1, 0).ch, '3');
    }

    #[test]
    fn the_input_modes_reflect_what_the_program_set() {
        let palette = Palette::DARK.terminal;
        let snapshot = painted(10, 3, b"\x1b[?1h\x1b[?2004h").snapshot(&palette);
        assert!(snapshot.modes.contains(TermMode::APP_CURSOR));
        assert!(snapshot.modes.contains(TermMode::BRACKETED_PASTE));
    }
}
