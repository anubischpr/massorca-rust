//! The wrapped layout of the multi-line text: which visual row a byte offset sits on, where the
//! caret and the selection are painted, and how far the text scrolls to keep the caret in view.
//! `shape_text` gives one wrapped line per hard line; this module maps offsets onto them.

use std::ops::Range;

use gpui::{Bounds, Hsla, PaintQuad, Pixels, Point, WrappedLine, fill, point, px};

/// The lines of the last shaping, with the byte offset and the top of each hard line.
#[derive(Debug)]
pub struct Shaped {
    pub lines: Vec<WrappedLine>,
    starts: Vec<usize>,
    pub tops: Vec<Pixels>,
    pub line_height: Pixels,
    wrap_width: Pixels,
    pub content_height: Pixels,
}

impl Shaped {
    pub fn new(lines: Vec<WrappedLine>, line_height: Pixels, wrap_width: Pixels) -> Self {
        let mut starts = Vec::with_capacity(lines.len());
        let mut tops = Vec::with_capacity(lines.len());
        let (mut start, mut top) = (0, px(0.));
        for line in &lines {
            starts.push(start);
            tops.push(top);
            start += line.len() + '\n'.len_utf8();
            top += line.size(line_height).height;
        }
        let content_height = px(f32::from(top).max(f32::from(line_height)));
        Self {
            lines,
            starts,
            tops,
            line_height,
            wrap_width,
            content_height,
        }
    }

    /// The hard line that holds `offset`; a newline belongs to the line before it.
    fn line_for_offset(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    /// The caret position of `offset`, relative to the top-left of the content.
    pub fn point_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        let line = self.line_for_offset(offset);
        let local = offset.checked_sub(*self.starts.get(line)?)?;
        let at = self
            .lines
            .get(line)?
            .position_for_index(local, self.line_height)?;
        Some(point(at.x, at.y + self.tops[line]))
    }

    /// The text offset under a content-relative point, before grapheme snapping.
    pub fn offset_for_point(&self, at: Point<Pixels>) -> usize {
        let Some(last) = self.lines.len().checked_sub(1) else {
            return 0;
        };
        let line = (0..=last)
            .find(|&i| at.y < self.tops[i] + self.lines[i].size(self.line_height).height)
            .unwrap_or(last);
        let y = px((f32::from(at.y) - f32::from(self.tops[line])).max(0.));
        let within =
            match self.lines[line].closest_index_for_position(point(at.x, y), self.line_height) {
                Ok(index) | Err(index) => index,
            };
        self.starts[line] + within.min(self.lines[line].len())
    }

    /// The visual row that holds `offset`, as absolute byte offsets.
    pub fn row_range(&self, offset: usize) -> Range<usize> {
        let line = self.line_for_offset(offset);
        let start = self.starts[line];
        let wrapped = &self.lines[line];
        let rows = row_starts(wrapped);
        let local = offset - start;
        let row = rows.partition_point(|&s| s <= local).saturating_sub(1);
        let end = rows.get(row + 1).copied().unwrap_or(wrapped.len());
        start + rows[row]..start + end
    }

    /// Selection rectangles in window coordinates, one per visual row. A row that wraps
    /// onward is selected to the wrap width, as editors draw a line break inside a selection.
    pub fn selection_quads(
        &self,
        range: Range<usize>,
        origin: Point<Pixels>,
        color: Hsla,
    ) -> Vec<PaintQuad> {
        let mut quads = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let start = self.starts[index];
            let end = start + line.len();
            let (from, to) = (range.start.max(start), range.end.min(end));
            if from >= to {
                continue;
            }
            let rows = row_starts(line);
            for (row, &row_start) in rows.iter().enumerate() {
                let row_end = rows.get(row + 1).copied().unwrap_or(line.len());
                let (a, b) = ((from - start).max(row_start), (to - start).min(row_end));
                if a >= b {
                    continue;
                }
                let Some(left) = line.position_for_index(a, self.line_height) else {
                    continue;
                };
                let wraps_on = b == row_end && row + 1 < rows.len();
                let right = if wraps_on {
                    self.wrap_width
                } else {
                    line.position_for_index(b, self.line_height)
                        .map_or(left.x, |p| p.x)
                };
                let top = origin.y + self.tops[index] + left.y;
                let bounds = Bounds::from_corners(
                    point(origin.x + left.x, top),
                    point(origin.x + right, top + self.line_height),
                );
                quads.push(fill(bounds, color));
            }
        }
        quads
    }
}

/// Byte offsets, within a hard line, where each visual row starts.
fn row_starts(line: &WrappedLine) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(line.wrap_boundaries().iter().map(|boundary| {
        line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index
    }));
    starts
}

/// The scroll offset that keeps the caret row in view when `caret` is given, and never
/// leaves empty space below the last row.
pub fn scroll_for_caret(
    scroll: f32,
    caret: Option<f32>,
    line_height: f32,
    viewport: f32,
    content: f32,
) -> f32 {
    let max = (content - viewport).max(0.);
    let mut scroll = scroll.clamp(0., max);
    if let Some(top) = caret {
        if top < scroll {
            scroll = top;
        } else if top + line_height > scroll + viewport {
            scroll = top + line_height - viewport;
        }
    }
    scroll.clamp(0., max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_brings_a_caret_below_the_viewport_into_view() {
        assert_eq!(scroll_for_caret(0., Some(300.), 22., 100., 400.), 222.);
    }

    #[test]
    fn scroll_brings_a_caret_above_the_viewport_into_view() {
        assert_eq!(scroll_for_caret(150., Some(40.), 22., 100., 400.), 40.);
    }

    #[test]
    fn scroll_stays_put_without_a_caret_request_and_never_passes_the_content() {
        assert_eq!(scroll_for_caret(50., None, 22., 100., 400.), 50.);
        assert_eq!(scroll_for_caret(500., None, 22., 100., 400.), 300.);
        assert_eq!(scroll_for_caret(30., Some(10.), 22., 100., 60.), 0.);
    }
}
