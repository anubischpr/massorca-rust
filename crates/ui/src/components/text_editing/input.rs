//! The platform text-input contract that the single-line field and the multi-line area share:
//! typing, paste, IME composition and the UTF-16 ranges the platform speaks (decision 0021).
//! An entity implements [`InputHost`] and takes its `EntityInputHandler` from
//! [`impl_input_handler`], so the contract is written once for both.

use std::ops::Range;

use gpui::{Bounds, Context, FocusHandle, Pixels, Point, UTF16Selection, Window};

use crate::components::text_field::TextBuffer;

/// What an input entity provides to the shared platform glue.
pub(crate) trait InputHost: Sized + 'static {
    /// The text, selection and composing range the platform reads and writes.
    fn text_buffer(&self) -> &TextBuffer;
    fn text_buffer_mut(&mut self) -> &mut TextBuffer;
    /// Whether typing, paste and composition reach the buffer.
    fn input_accepts(&self) -> bool;
    /// Whether the input hides its text, so the platform is handed none of it.
    fn input_masked(&self) -> bool {
        false
    }
    /// The focus handle the platform input is bound to.
    fn input_focus(&self) -> &FocusHandle;
    /// Emits the owner-facing `Changed` event.
    fn input_changed(&mut self, cx: &mut Context<Self>);
    /// Runs before an edit changes the text; a multi-line area follows its caret here.
    fn input_will_edit(&mut self) {}
    /// What inserted text becomes; a single-line field drops line breaks.
    fn sanitize_insert(text: &str) -> String;
    /// The window box of a range of the text, from the last paint.
    fn layout_bounds(
        &self,
        range: Range<usize>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>>;
    /// The byte offset under a window position, from the last paint.
    fn offset_at(&self, position: Point<Pixels>) -> Option<usize>;
}

/// Commits `change` to the buffer when the input accepts text, then reports the result.
pub(crate) fn edit<H: InputHost>(
    host: &mut H,
    window: &Window,
    cx: &mut Context<H>,
    change: impl FnOnce(&mut TextBuffer) -> bool,
) {
    if host.input_accepts() {
        host.input_will_edit();
        let changed = change(host.text_buffer_mut());
        after_edit(host, changed, window, cx);
    }
}

/// Tells the platform and the owner what changed.
pub(crate) fn after_edit<H: InputHost>(
    host: &mut H,
    changed: bool,
    window: &Window,
    cx: &mut Context<H>,
) {
    if changed {
        window.notify_text_input_changed(host.input_focus());
        host.input_changed(cx);
    }
    window.notify_text_selection_changed(host.input_focus());
    cx.notify();
}

pub(crate) fn text_for_range<H: InputHost>(
    host: &H,
    range_utf16: Range<usize>,
    adjusted_range: &mut Option<Range<usize>>,
) -> Option<String> {
    if host.input_masked() {
        return None;
    }
    let buffer = host.text_buffer();
    let range = buffer.range_from_utf16(&range_utf16);
    adjusted_range.replace(buffer.range_to_utf16(&range));
    Some(buffer.text()[range].to_owned())
}

pub(crate) fn selected_text_range<H: InputHost>(host: &H) -> Option<UTF16Selection> {
    let buffer = host.text_buffer();
    Some(UTF16Selection {
        range: buffer.range_to_utf16(&buffer.selected()),
        reversed: buffer.reversed(),
    })
}

pub(crate) fn marked_text_range<H: InputHost>(host: &H) -> Option<Range<usize>> {
    let buffer = host.text_buffer();
    buffer.marked().map(|range| buffer.range_to_utf16(&range))
}

pub(crate) fn unmark_text<H: InputHost>(host: &mut H, window: &Window, cx: &mut Context<H>) {
    host.text_buffer_mut().unmark();
    after_edit(host, false, window, cx);
}

pub(crate) fn replace_text_in_range<H: InputHost>(
    host: &mut H,
    range_utf16: Option<Range<usize>>,
    text: &str,
    window: &Window,
    cx: &mut Context<H>,
) {
    let text = H::sanitize_insert(text);
    edit(host, window, cx, |buffer| {
        buffer.replace_text(range_utf16, &text)
    });
}

pub(crate) fn replace_and_mark_text_in_range<H: InputHost>(
    host: &mut H,
    range_utf16: Option<Range<usize>>,
    new_text: &str,
    new_selected_range: Option<Range<usize>>,
    window: &Window,
    cx: &mut Context<H>,
) {
    let text = H::sanitize_insert(new_text);
    edit(host, window, cx, |buffer| {
        buffer.replace_and_mark(range_utf16, &text, new_selected_range)
    });
}

pub(crate) fn bounds_for_range<H: InputHost>(
    host: &H,
    range_utf16: Range<usize>,
    element_bounds: Bounds<Pixels>,
) -> Option<Bounds<Pixels>> {
    let range = host.text_buffer().range_from_utf16(&range_utf16);
    host.layout_bounds(range, element_bounds)
}

pub(crate) fn character_index_for_point<H: InputHost>(
    host: &H,
    position: Point<Pixels>,
) -> Option<usize> {
    let index = host.offset_at(position)?;
    Some(host.text_buffer().offset_to_utf16(index))
}

pub(crate) fn set_selected_text_range<H: InputHost>(
    host: &mut H,
    range_utf16: Range<usize>,
    window: &Window,
    cx: &mut Context<H>,
) {
    host.text_buffer_mut().select_utf16(range_utf16);
    after_edit(host, false, window, cx);
}

pub(crate) fn text_length_utf16<H: InputHost>(host: &H) -> Option<usize> {
    Some(host.text_buffer().utf16_len())
}

pub(crate) fn accepts_text_input<H: InputHost>(host: &H) -> bool {
    host.input_accepts()
}

/// Gives an input entity its `EntityInputHandler`: each method forwards to the functions above,
/// so the entity implements only [`InputHost`].
macro_rules! impl_input_handler {
    ($entity:ty) => {
        impl ::gpui::EntityInputHandler for $entity {
            fn text_for_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                adjusted_range: &mut Option<::std::ops::Range<usize>>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<String> {
                $crate::components::text_editing::input::text_for_range(
                    self,
                    range_utf16,
                    adjusted_range,
                )
            }

            fn selected_text_range(
                &mut self,
                _: bool,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<::gpui::UTF16Selection> {
                $crate::components::text_editing::input::selected_text_range(self)
            }

            fn marked_text_range(
                &self,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<::std::ops::Range<usize>> {
                $crate::components::text_editing::input::marked_text_range(self)
            }

            fn unmark_text(&mut self, window: &mut ::gpui::Window, cx: &mut ::gpui::Context<Self>) {
                $crate::components::text_editing::input::unmark_text(self, window, cx)
            }

            fn replace_text_in_range(
                &mut self,
                range_utf16: Option<::std::ops::Range<usize>>,
                text: &str,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) {
                $crate::components::text_editing::input::replace_text_in_range(
                    self,
                    range_utf16,
                    text,
                    window,
                    cx,
                )
            }

            fn replace_and_mark_text_in_range(
                &mut self,
                range_utf16: Option<::std::ops::Range<usize>>,
                new_text: &str,
                new_selected_range: Option<::std::ops::Range<usize>>,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) {
                $crate::components::text_editing::input::replace_and_mark_text_in_range(
                    self,
                    range_utf16,
                    new_text,
                    new_selected_range,
                    window,
                    cx,
                )
            }

            fn bounds_for_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                element_bounds: ::gpui::Bounds<::gpui::Pixels>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<::gpui::Bounds<::gpui::Pixels>> {
                $crate::components::text_editing::input::bounds_for_range(
                    self,
                    range_utf16,
                    element_bounds,
                )
            }

            fn selection_bounds_for_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                element_bounds: ::gpui::Bounds<::gpui::Pixels>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Vec<::gpui::Bounds<::gpui::Pixels>> {
                $crate::components::text_editing::input::bounds_for_range(
                    self,
                    range_utf16,
                    element_bounds,
                )
                .into_iter()
                .collect()
            }

            fn character_index_for_point(
                &mut self,
                position: ::gpui::Point<::gpui::Pixels>,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<usize> {
                $crate::components::text_editing::input::character_index_for_point(self, position)
            }

            fn set_selected_text_range(
                &mut self,
                range_utf16: ::std::ops::Range<usize>,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) {
                $crate::components::text_editing::input::set_selected_text_range(
                    self,
                    range_utf16,
                    window,
                    cx,
                )
            }

            fn text_length_utf16(
                &mut self,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> Option<usize> {
                $crate::components::text_editing::input::text_length_utf16(self)
            }

            fn accepts_text_input(
                &self,
                _: &mut ::gpui::Window,
                _: &mut ::gpui::Context<Self>,
            ) -> bool {
                $crate::components::text_editing::input::accepts_text_input(self)
            }
        }
    };
}
pub(crate) use impl_input_handler;
