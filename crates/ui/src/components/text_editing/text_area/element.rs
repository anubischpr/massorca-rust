//! The element that lays out, paints and registers the platform input handler of the multi-line
//! text area: the wrapped lines, the selection and caret rectangles, and the scroll.

use std::rc::Rc;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, Hsla, LayoutId, PaintQuad, Pixels, Style, TextAlign, Window, fill, point, px,
    relative, size,
};

use super::TextArea;
use crate::components::text_editing::layout::{Shaped, scroll_for_caret};
use crate::theme::ActiveTheme;

/// Lays out the text, paints it and registers the platform input handler.
pub(super) struct TextAreaElement {
    pub(super) input: Entity<TextArea>,
}

pub(super) struct Prepaint {
    shaped: Rc<Shaped>,
    scroll: Pixels,
    selection: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl Element for TextAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.clone();
        let layout = window.request_measured_layout(style, move |known, available, window, cx| {
            let width = known.width.unwrap_or(match available.width {
                AvailableSpace::Definite(width) => width,
                _ => px(0.),
            });
            let height = input.read(cx).height_for(width, window, cx);
            size(width, height)
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let palette = cx.theme().palette;
        let shaped = Rc::new(self.input.read(cx).shape(bounds.size.width, window, cx));
        let input = self.input.read(cx);
        let focused = input.focus_handle.is_focused(window);
        let editable = input.editable();
        let selected = input.buffer.selected();
        let cursor = input.buffer.cursor();
        let caret_top = if input.ensure_caret {
            shaped.point_for_offset(cursor).map(|at| f32::from(at.y))
        } else {
            None
        };
        let scroll = px(scroll_for_caret(
            f32::from(input.scroll),
            caret_top,
            f32::from(shaped.line_height),
            f32::from(bounds.size.height),
            f32::from(shaped.content_height),
        ));
        let origin = point(bounds.left(), bounds.top() - scroll);
        let (selection, caret) = if !selected.is_empty() {
            let quads = shaped.selection_quads(selected, origin, Hsla::from(palette.selection.bg));
            (quads, None)
        } else if focused && editable {
            let caret = shaped.point_for_offset(cursor).map(|at| {
                fill(
                    Bounds::new(
                        point(origin.x + at.x, origin.y + at.y),
                        size(px(1.), shaped.line_height),
                    ),
                    Hsla::from(palette.text.primary),
                )
            });
            (Vec::new(), caret)
        } else {
            (Vec::new(), None)
        };
        let stored = shaped.clone();
        self.input.update(cx, |input, _| {
            input.scroll = scroll;
            input.ensure_caret = false;
            input.bounds = Some(bounds);
            input.shaped = Some(stored);
        });
        Prepaint {
            shaped,
            scroll,
            selection,
            caret,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let top = bounds.top() - prepaint.scroll;
        let line_height = prepaint.shaped.line_height;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in prepaint.selection.drain(..) {
                window.paint_quad(quad);
            }
            for (line, line_top) in prepaint.shaped.lines.iter().zip(&prepaint.shaped.tops) {
                line.paint(
                    point(bounds.left(), top + *line_top),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
            if let Some(caret) = prepaint.caret.take() {
                window.paint_quad(caret);
            }
        });
    }
}
