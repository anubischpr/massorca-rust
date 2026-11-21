//! Caret movement, the clipboard and the owner's history keys of the multi-line text area.

use gpui::{ClipboardItem, Context, Window, point};

use super::{TextArea, TextAreaEvent};
use crate::components::text_editing::{TextBuffer, normalize_line_breaks};

impl TextArea {
    /// Moves or selects the caret without changing the text.
    pub(super) fn move_caret(
        &mut self,
        change: impl FnOnce(&mut TextBuffer),
        ensure_caret: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_caret = ensure_caret;
        change(&mut self.buffer);
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    /// Up and Down move between visual rows; with nothing typed they ask the owner for history.
    pub(super) fn move_vertical(&mut self, down: bool, extend: bool, window: &Window, cx: &mut Context<Self>) {
        if self.buffer.is_empty() {
            let event = if down {
                TextAreaEvent::HistoryNext
            } else {
                TextAreaEvent::HistoryPrevious
            };
            cx.emit(event);
            return;
        }
        let Some(shaped) = self.shaped.clone() else {
            return;
        };
        let Some(at) = shaped.point_for_offset(self.buffer.cursor()) else {
            return;
        };
        let line = shaped.line_height;
        let target = if down {
            let below = at.y + line * 1.5;
            if below >= shaped.content_height {
                self.buffer.text().len()
            } else {
                shaped.offset_for_point(point(at.x, below))
            }
        } else if at.y < line {
            0
        } else {
            shaped.offset_for_point(point(at.x, at.y - line * 0.5))
        };
        self.move_caret(
            move |buffer| {
                if extend {
                    buffer.select_to(target)
                } else {
                    buffer.move_to(target)
                }
            },
            true,
            window,
            cx,
        );
    }

    /// Home and End go to the visual row the caret is on.
    pub(super) fn move_row_edge(&mut self, end: bool, extend: bool, window: &Window, cx: &mut Context<Self>) {
        let target = match self.shaped.as_ref() {
            Some(shaped) => {
                let row = shaped.row_range(self.buffer.cursor());
                if end { row.end } else { row.start }
            }
            None if end => self.buffer.text().len(),
            None => 0,
        };
        self.move_caret(
            move |buffer| {
                if extend {
                    buffer.select_to(target)
                } else {
                    buffer.move_to(target)
                }
            },
            true,
            window,
            cx,
        );
    }

    pub(super) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let selected = self.buffer.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }

    pub(super) fn cut_selection(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.copy_selection(cx);
        self.edit(window, cx, |buffer| buffer.insert(""));
    }

    pub(super) fn paste_text(&mut self, window: &Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = normalize_line_breaks(&text);
            self.edit(window, cx, |buffer| buffer.insert(&text));
        }
    }
}
