//! Shaping and measuring the multi-line text: the wrapped lines at a width, the runs that carry
//! the text colour and the IME underline, and the height the text needs.

use gpui::{App, Font, Hsla, Pixels, SharedString, TextRun, UnderlineStyle, Window, px};

use super::TextArea;
use crate::components::text_editing::layout::Shaped;
use crate::theme::ActiveTheme;

impl TextArea {
    /// The text as lines wrapped to `width`; empty text shows the placeholder.
    pub(super) fn shape(&self, width: Pixels, window: &Window, cx: &App) -> Shaped {
        let palette = cx.theme().palette;
        let empty = self.buffer.is_empty();
        let text = if empty {
            self.placeholder.clone()
        } else {
            SharedString::from(self.buffer.text().to_owned())
        };
        let color = Hsla::from(if empty {
            palette.text.tertiary
        } else if self.disabled {
            palette.text.disabled
        } else {
            palette.text.primary
        });
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let runs = self.runs(text.len(), color, style.font());
        let lines = window
            .text_system()
            .shape_text(text, font_size, &runs, Some(width), None)
            .map(|lines| lines.into_vec())
            .unwrap_or_default();
        Shaped::new(lines, line_height, width)
    }

    /// One run for the text; the composing range gets an underline (IME).
    fn runs(&self, len: usize, color: Hsla, font: Font) -> Vec<TextRun> {
        let run = TextRun {
            len,
            font,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        match self.buffer.marked() {
            Some(range) if !self.buffer.is_empty() => [
                TextRun {
                    len: range.start,
                    ..run.clone()
                },
                TextRun {
                    len: range.end - range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: len - range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            _ => vec![run],
        }
    }

    /// The height the text needs at `width`, held between the minimum and the maximum.
    pub(super) fn height_for(&self, width: Pixels, window: &Window, cx: &App) -> Pixels {
        let content = f32::from(self.shape(width, window, cx).content_height);
        px(content.clamp(self.min_height, self.max_height))
    }
}
