//! Pointer and wheel input for the multi-line text area: the caret follows the mouse, and the
//! wheel scrolls the text inside the area.

use gpui::{
    Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent, Window,
    point, px,
};

use super::TextArea;

impl TextArea {
    /// The text offset under a window position, snapped to a grapheme boundary.
    fn index_at(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(shaped)) = (self.bounds, self.shaped.as_ref()) else {
            return self.buffer.cursor();
        };
        let at = point(
            position.x - bounds.left(),
            position.y - bounds.top() + self.scroll,
        );
        self.buffer.snap_to_grapheme(shaped.offset_for_point(at))
    }

    pub(super) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.focus(window, cx);
        self.selecting = true;
        let index = self.index_at(event.position);
        let extend = event.modifiers.shift;
        let clicks = event.click_count;
        self.move_caret(
            move |buffer| match (clicks, extend) {
                (1, true) => buffer.select_to(index),
                (2, _) => buffer.select_word_at(index),
                (n, _) if n >= 3 => buffer.select_line_at(index),
                _ => buffer.move_to(index),
            },
            false,
            window,
            cx,
        );
    }

    pub(super) fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selecting {
            let index = self.index_at(event.position);
            self.move_caret(move |buffer| buffer.select_to(index), false, window, cx);
        }
    }

    pub(super) fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// Wheel scrolling moves the text inside the area, between the top and the last row.
    pub(super) fn scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let (Some(shaped), Some(bounds)) = (self.shaped.as_ref(), self.bounds) else {
            return;
        };
        let max = f32::from(shaped.content_height) - f32::from(bounds.size.height);
        let scroll = f32::from(self.scroll) - f32::from(delta);
        self.scroll = px(scroll.clamp(0., max.max(0.)));
        cx.notify();
    }
}
