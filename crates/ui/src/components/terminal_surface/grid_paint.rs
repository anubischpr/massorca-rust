//! Paints the visible grid: each row's background runs, its text runs, then the
//! cursor. Planning the runs is pure and tested; the painting is GPUI.

use std::sync::Arc;

use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
    App, BorderStyle, Bounds, Font, FontFeatures, FontStyle, FontWeight, Hsla, Pixels, Point,
    StrikethroughStyle, TextAlign, TextRun, UnderlineStyle, Window, fill, font, outline, point, px,
    size,
};

use super::feed::{RenderCell, RenderCursor, RenderSnapshot};
use super::geometry::CellSize;
use crate::text_preferences;
use crate::tokens::{
    BORDER_WIDTH_DEFAULT, BORDER_WIDTH_THICK, Color, TYPE_TERMINAL, TerminalColors,
};

/// Everything the paint pass needs, computed during prepaint.
#[derive(Debug)]
pub struct Frame {
    pub snapshot: RenderSnapshot,
    pub cell: CellSize,
    /// The cursor, or `None` while the process has exited.
    pub cursor: Option<RenderCursor>,
    pub focused: bool,
    pub palette: TerminalColors,
}

/// A background rectangle: `columns` cells from `column`, in `color`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub column: usize,
    pub columns: usize,
    pub color: Color,
}

/// The colour and decorations of one run of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub fg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

/// Text drawn in one style from `column`: a run of narrow cells, or one wide glyph.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub column: usize,
    pub columns: usize,
    pub wide: bool,
    pub text: String,
    pub style: TextStyle,
}

/// The background rectangles of one row. A focused block cursor at `cursor` is filled
/// with `cursor_colour`; cells in `default_bg` are left to the pane.
pub fn fills(
    row: &[RenderCell],
    cursor: Option<usize>,
    cursor_colour: Color,
    default_bg: Color,
) -> Vec<Fill> {
    let mut fills: Vec<Fill> = Vec::new();
    for (column, cell) in row.iter().enumerate() {
        let color = if cursor == Some(column) {
            cursor_colour
        } else {
            cell.bg
        };
        if color == default_bg {
            continue;
        }
        match fills.last_mut() {
            Some(last) if last.color == color && last.column + last.columns == column => {
                last.columns += 1;
            }
            _ => fills.push(Fill {
                column,
                columns: 1,
                color,
            }),
        }
    }
    fills
}

/// The text runs of one row. A block cursor at `cursor` is drawn in its cell's
/// background colour. Blank runs without decoration are left out.
pub fn spans(row: &[RenderCell], cursor: Option<usize>) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for (column, cell) in row.iter().enumerate() {
        if cell.spacer {
            continue;
        }
        let style = text_style(cell, cursor == Some(column));
        let text = glyph(cell);
        if cell.wide {
            spans.push(Span {
                column,
                columns: 2,
                wide: true,
                text,
                style,
            });
            continue;
        }
        match spans.last_mut() {
            Some(last)
                if !last.wide && last.style == style && last.column + last.columns == column =>
            {
                last.text.push_str(&text);
                last.columns += 1;
            }
            _ => spans.push(Span {
                column,
                columns: 1,
                wide: false,
                text,
                style,
            }),
        }
    }
    spans.retain(|span| {
        span.wide
            || span.style.underline
            || span.style.strikeout
            || span.text.chars().any(|c| c != ' ')
    });
    spans
}

/// The style of a cell's text; a block cursor's cell is drawn in its background colour.
fn text_style(cell: &RenderCell, block_cursor: bool) -> TextStyle {
    TextStyle {
        fg: if block_cursor { cell.bg } else { cell.fg },
        bold: cell.bold,
        italic: cell.italic,
        underline: cell.underline,
        strikeout: cell.strikeout,
    }
}

/// A cell's character followed by the combining marks it carries.
fn glyph(cell: &RenderCell) -> String {
    let mut text = String::with_capacity(1 + cell.combining.len());
    text.push(cell.ch);
    text.extend(cell.combining.iter());
    text
}

/// The size of one cell in the terminal font: the advance of `M` and the terminal line height.
pub fn cell_metrics(cx: &App) -> Option<CellSize> {
    let text = cx.text_system();
    let font_id = text.resolve_font(&font(text_preferences::terminal_family()));
    let advance = text.advance(font_id, px(TYPE_TERMINAL.size), 'M').ok()?;
    Some(CellSize {
        width: f32::from(advance.width),
        height: TYPE_TERMINAL.line_height,
    })
}

/// Paints the frame in `bounds`. Without a frame (no font metrics yet) nothing is drawn.
pub fn paint_grid(bounds: Bounds<Pixels>, frame: Option<Frame>, window: &mut Window, cx: &mut App) {
    let Some(frame) = frame else {
        return;
    };
    let cell = frame.cell;
    let columns = frame.snapshot.columns;
    let block = frame
        .cursor
        .filter(|cursor| frame.focused && cursor.shape == CursorShape::Block);
    // The terminal shows each character in its own cell: no ligatures.
    let features = FontFeatures(Arc::new(vec![
        ("liga".to_owned(), 0),
        ("calt".to_owned(), 0),
    ]));
    for (line, row) in frame.snapshot.cells.chunks(columns).enumerate() {
        let y = bounds.origin.y + px(line as f32 * cell.height);
        let cursor_column = block
            .filter(|cursor| cursor.line == line)
            .map(|cursor| cursor.column);
        for rect in fills(row, cursor_column, frame.palette.cursor, frame.palette.bg) {
            let x = bounds.origin.x + px(rect.column as f32 * cell.width);
            let extent = size(px(rect.columns as f32 * cell.width), px(cell.height));
            window.paint_quad(fill(
                Bounds::new(point(x, y), extent),
                Hsla::from(rect.color),
            ));
        }
        for span in spans(row, cursor_column) {
            let x = bounds.origin.x + px(span.column as f32 * cell.width);
            paint_span(&span, point(x, y), cell, &features, window, cx);
        }
    }
    if let Some(cursor) = frame.cursor {
        paint_cursor(
            cursor,
            frame.focused,
            cell,
            bounds.origin,
            frame.palette.cursor,
            window,
        );
    }
}

fn paint_span(
    span: &Span,
    origin: Point<Pixels>,
    cell: CellSize,
    features: &FontFeatures,
    window: &mut Window,
    cx: &mut App,
) {
    let color = Hsla::from(span.style.fg);
    let text_font = Font {
        weight: if span.style.bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        style: if span.style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        features: features.clone(),
        ..font(text_preferences::terminal_family())
    };
    let run = TextRun {
        len: span.text.len(),
        font: text_font,
        color,
        background_color: None,
        underline: span.style.underline.then(|| UnderlineStyle {
            thickness: px(BORDER_WIDTH_DEFAULT),
            color: Some(color),
            wavy: false,
        }),
        strikethrough: span.style.strikeout.then(|| StrikethroughStyle {
            thickness: px(BORDER_WIDTH_DEFAULT),
            color: Some(color),
        }),
    };
    // Narrow runs are forced onto the cell grid; a wide glyph keeps its own advance.
    let force_width = (!span.wide).then(|| px(cell.width));
    let line = window.text_system().shape_line(
        span.text.clone().into(),
        px(TYPE_TERMINAL.size),
        &[run],
        force_width,
    );
    let _ = line.paint(origin, px(cell.height), TextAlign::Left, None, window, cx);
}

/// The cursor outside the block: a bar or underline when focused, else a hollow block.
/// A focused block is drawn with its cell instead.
fn paint_cursor(
    cursor: RenderCursor,
    focused: bool,
    cell: CellSize,
    origin: Point<Pixels>,
    colour: Color,
    window: &mut Window,
) {
    let x = origin.x + px(cursor.column as f32 * cell.width);
    let y = origin.y + px(cursor.line as f32 * cell.height);
    let color = Hsla::from(colour);
    match (focused, cursor.shape) {
        (_, CursorShape::Hidden) | (true, CursorShape::Block) => {}
        (true, CursorShape::Beam) => {
            let bar = Bounds::new(point(x, y), size(px(BORDER_WIDTH_THICK), px(cell.height)));
            window.paint_quad(fill(bar, color));
        }
        (true, CursorShape::Underline) => {
            let underline = Bounds::new(
                point(x, y + px(cell.height - BORDER_WIDTH_THICK)),
                size(px(cell.width), px(BORDER_WIDTH_THICK)),
            );
            window.paint_quad(fill(underline, color));
        }
        _ => {
            let block = Bounds::new(point(x, y), size(px(cell.width), px(cell.height)));
            window.paint_quad(outline(block, color, BorderStyle::Solid));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Palette;

    fn cell(ch: char) -> RenderCell {
        let palette = Palette::DARK.terminal;
        RenderCell {
            ch,
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

    fn row(text: &str) -> Vec<RenderCell> {
        text.chars().map(cell).collect()
    }

    #[test]
    fn plain_text_is_one_span() {
        let spans = spans(&row("abc"), None);
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].column, spans[0].columns), (0, 3));
        assert_eq!(spans[0].text, "abc");
    }

    #[test]
    fn a_style_change_starts_a_new_span() {
        let mut cells = row("ab");
        cells[0].bold = true;
        let spans = spans(&cells, None);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "a");
        assert!(spans[0].style.bold);
        assert_eq!((spans[1].column, spans[1].text.as_str()), (1, "b"));
    }

    #[test]
    fn a_wide_character_is_its_own_span_and_its_spacer_is_skipped() {
        let mut cells = row("日 a");
        cells[0].wide = true;
        cells[1].spacer = true;
        let spans = spans(&cells, None);
        assert_eq!(spans.len(), 2);
        assert!(spans[0].wide);
        assert_eq!((spans[0].column, spans[0].columns), (0, 2));
        assert_eq!(spans[0].text, "日");
        assert_eq!((spans[1].column, spans[1].text.as_str()), (2, "a"));
    }

    #[test]
    fn blank_cells_without_decoration_are_not_drawn() {
        assert!(spans(&row("   "), None).is_empty());
        let mut cells = row("  ");
        cells[1].underline = true;
        let spans = spans(&cells, None);
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].column, spans[0].text.as_str()), (1, " "));
    }

    #[test]
    fn the_cursor_cell_is_drawn_in_its_background_colour_in_its_own_span() {
        let palette = Palette::DARK.terminal;
        let mut cells = row("abc");
        cells[1].bg = Color::rgba8(1, 2, 3, 1.0);
        let spans = spans(&cells, Some(1));
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "b");
        assert_eq!(spans[1].style.fg, Color::rgba8(1, 2, 3, 1.0));
        assert_eq!(spans[0].style.fg, palette.fg);
    }

    #[test]
    fn combining_marks_stay_with_their_cell() {
        let mut cells = row("ea");
        cells[0].combining = vec!['\u{301}'];
        let spans = spans(&cells, None);
        assert_eq!(spans[0].text, "e\u{301}a");
        assert_eq!(spans[0].columns, 2);
    }

    #[test]
    fn backgrounds_merge_into_runs_and_skip_the_default() {
        let palette = Palette::DARK.terminal;
        let red = Color::rgba8(200, 0, 0, 1.0);
        let mut cells = row("abcde");
        cells[1].bg = red;
        cells[2].bg = red;
        let fills = fills(&cells, None, palette.cursor, palette.bg);
        assert_eq!(
            fills,
            vec![Fill {
                column: 1,
                columns: 2,
                color: red
            }]
        );
    }

    #[test]
    fn the_block_cursor_fill_replaces_the_cell_background() {
        let palette = Palette::DARK.terminal;
        let fills = fills(&row("abc"), Some(1), palette.cursor, palette.bg);
        assert_eq!(
            fills,
            vec![Fill {
                column: 1,
                columns: 1,
                color: palette.cursor
            }]
        );
    }
}
