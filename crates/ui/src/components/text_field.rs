//! Single-line text input: the editing model, and the entity that edits it through
//! the platform input handler (typing and IME).

use std::ops::Range;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, ShapedLine, SharedString,
    Styled, UTF16Selection, Window, actions, div, point, px,
};

use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{Color, Palette, RADIUS_MD, SIZE_CONTROL_DEFAULT, SPACE_2, TYPE_BODY};

const ZWJ: char = '\u{200D}';

/// Editing model of a single-line field: text, selection and the composing
/// (marked) range. Offsets are UTF-8 byte offsets on grapheme boundaries; the
/// platform input side speaks UTF-16 and converts at the edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextBuffer {
    text: String,
    selected: Range<usize>,
    /// The cursor sits at the start of the selection.
    reversed: bool,
    marked: Option<Range<usize>>,
}

impl TextBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self {
            text,
            selected: end..end,
            reversed: false,
            marked: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn selected(&self) -> Range<usize> {
        self.selected.clone()
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selected.clone()]
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    /// The cursor sits at the start of the selection.
    pub fn reversed(&self) -> bool {
        self.reversed
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// Replaces the whole text and puts the cursor at its end; `true` when it changed.
    pub fn set_text(&mut self, text: &str) -> bool {
        if self.text == text {
            return false;
        }
        self.text = text.to_owned();
        let end = self.text.len();
        self.selected = end..end;
        self.reversed = false;
        self.marked = None;
        true
    }

    pub fn move_to(&mut self, offset: usize) {
        let offset = self.snap_to_grapheme(offset);
        self.selected = offset..offset;
        self.reversed = false;
    }

    /// Moves the free end of the selection to `offset`, keeping the anchor.
    pub fn select_to(&mut self, offset: usize) {
        let anchor = if self.reversed {
            self.selected.end
        } else {
            self.selected.start
        };
        let offset = self.snap_to_grapheme(offset);
        self.selected = offset.min(anchor)..offset.max(anchor);
        self.reversed = offset < anchor;
    }

    pub fn select_all(&mut self) {
        self.selected = 0..self.text.len();
        self.reversed = false;
    }

    /// Collapses a selection to its start, or moves one grapheme left.
    pub fn move_left(&mut self) {
        let target = if self.selected.is_empty() {
            self.prev_boundary(self.cursor())
        } else {
            self.selected.start
        };
        self.move_to(target);
    }

    /// Collapses a selection to its end, or moves one grapheme right.
    pub fn move_right(&mut self) {
        let target = if self.selected.is_empty() {
            self.next_boundary(self.cursor())
        } else {
            self.selected.end
        };
        self.move_to(target);
    }

    pub fn select_left(&mut self) {
        self.select_to(self.prev_boundary(self.cursor()));
    }

    pub fn select_right(&mut self) {
        self.select_to(self.next_boundary(self.cursor()));
    }

    pub fn move_home(&mut self) {
        self.move_to(0);
    }

    pub fn move_end(&mut self) {
        self.move_to(self.text.len());
    }

    pub fn select_home(&mut self) {
        self.select_to(0);
    }

    pub fn select_end(&mut self) {
        self.select_to(self.text.len());
    }

    /// Writes `text` over the selection (typing, paste); `true` when the text changed.
    pub fn insert(&mut self, text: &str) -> bool {
        let range = self.selected.clone();
        self.replace(range, text)
    }

    /// Deletes the selection, or the grapheme before the cursor.
    pub fn backspace(&mut self) -> bool {
        let range = if self.selected.is_empty() {
            self.prev_boundary(self.cursor())..self.cursor()
        } else {
            self.selected.clone()
        };
        self.replace(range, "")
    }

    /// Deletes the selection, or the grapheme after the cursor.
    pub fn delete(&mut self) -> bool {
        let range = if self.selected.is_empty() {
            self.cursor()..self.next_boundary(self.cursor())
        } else {
            self.selected.clone()
        };
        self.replace(range, "")
    }

    /// Commits `text` over `range_utf16`, else over the composing range, else over the selection.
    pub fn replace_text(&mut self, range_utf16: Option<Range<usize>>, text: &str) -> bool {
        let range = self.target_range(range_utf16);
        self.replace(range, text)
    }

    /// Writes composing text and marks it. `selected_utf16` is the selection inside
    /// the new text, relative to its start, as the platform reports it.
    pub fn replace_and_mark(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
    ) -> bool {
        let range = self.clamp_range(self.target_range(range_utf16));
        let start = range.start;
        let changed = self.splice(range, text);
        self.marked = (!text.is_empty()).then(|| start..start + text.len());
        self.reversed = false;
        self.selected = match selected_utf16 {
            Some(selected) => {
                let base = self.offset_to_utf16(start);
                let inserted = text.encode_utf16().count();
                let from = base + selected.start.min(inserted);
                let to = base + selected.end.min(inserted);
                self.range_from_utf16(&(from..to))
            }
            None => start + text.len()..start + text.len(),
        };
        changed
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    pub fn select_utf16(&mut self, range_utf16: Range<usize>) {
        self.selected = self.range_from_utf16(&range_utf16);
        self.reversed = false;
    }

    pub fn utf16_len(&self) -> usize {
        self.text.encode_utf16().count()
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..self.clamp_offset(offset)]
            .encode_utf16()
            .count()
    }

    /// The byte offset for a UTF-16 offset, rounding up inside a surrogate pair.
    pub fn offset_from_utf16(&self, units: usize) -> usize {
        let mut seen = 0;
        for (index, ch) in self.text.char_indices() {
            if seen >= units {
                return index;
            }
            seen += ch.len_utf16();
        }
        self.text.len()
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let start = self.offset_from_utf16(range.start);
        start..self.offset_from_utf16(range.end).max(start)
    }

    /// The grapheme boundary before `offset`, or 0.
    pub fn prev_boundary(&self, offset: usize) -> usize {
        let offset = self.clamp_offset(offset);
        grapheme_starts(&self.text)
            .iter()
            .rev()
            .copied()
            .find(|&start| start < offset)
            .unwrap_or(0)
    }

    /// The grapheme boundary after `offset`, or the end of the text.
    pub fn next_boundary(&self, offset: usize) -> usize {
        let offset = self.clamp_offset(offset);
        grapheme_starts(&self.text)
            .into_iter()
            .find(|&start| start > offset)
            .unwrap_or(self.text.len())
    }

    /// The nearest grapheme boundary at or before `offset`.
    pub fn snap_to_grapheme(&self, offset: usize) -> usize {
        let offset = self.clamp_offset(offset);
        if offset == self.text.len() {
            return offset;
        }
        grapheme_starts(&self.text)
            .iter()
            .rev()
            .copied()
            .find(|&start| start <= offset)
            .unwrap_or(0)
    }

    fn target_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        match range_utf16 {
            Some(range) => self.range_from_utf16(&range),
            None => self.marked.clone().unwrap_or_else(|| self.selected.clone()),
        }
    }

    /// Replaces `range` with `text` and puts the cursor after it.
    fn replace(&mut self, range: Range<usize>, text: &str) -> bool {
        let range = self.clamp_range(range);
        let start = range.start;
        let changed = self.splice(range, text);
        self.selected = start + text.len()..start + text.len();
        self.reversed = false;
        self.marked = None;
        changed
    }

    fn splice(&mut self, range: Range<usize>, text: &str) -> bool {
        if self.text[range.clone()] == *text {
            return false;
        }
        self.text.replace_range(range, text);
        true
    }

    fn clamp_offset(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn clamp_range(&self, range: Range<usize>) -> Range<usize> {
        let start = self.clamp_offset(range.start);
        start..self.clamp_offset(range.end).max(start)
    }
}

actions!(
    text_field,
    [
        Backspace,
        Delete,
        MoveLeft,
        MoveRight,
        SelectLeft,
        SelectRight,
        MoveHome,
        MoveEnd,
        SelectHome,
        SelectEnd,
        SelectAll,
        CopySelection,
        CutSelection,
        PasteText,
        Submit,
    ]
);

const KEY_CONTEXT: &str = "TextField";
const FIELD_GROUP: &str = "text-field";

/// Binds the text field keys under its key context; apps call this once at startup.
pub fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    // `KeyBinding::new` cannot resolve `secondary-`, so Mod is spelled out per OS.
    let [select_all, copy, cut, paste] = if cfg!(target_os = "macos") {
        ["cmd-a", "cmd-c", "cmd-x", "cmd-v"]
    } else {
        ["ctrl-a", "ctrl-c", "ctrl-x", "ctrl-v"]
    };
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", MoveLeft, context),
        KeyBinding::new("right", MoveRight, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", MoveHome, context),
        KeyBinding::new("end", MoveEnd, context),
        KeyBinding::new("shift-home", SelectHome, context),
        KeyBinding::new("shift-end", SelectEnd, context),
        KeyBinding::new(select_all, SelectAll, context),
        KeyBinding::new(copy, CopySelection, context),
        KeyBinding::new(cut, CutSelection, context),
        KeyBinding::new(paste, PasteText, context),
        KeyBinding::new("enter", Submit, context),
    ]);
}

/// What a [`TextField`] emits; subscribe with `cx.subscribe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldEvent {
    /// The text changed by typing, paste, composition, clearing or `set_text`.
    Changed,
    /// Enter was pressed in the field.
    Submitted,
}

/// Single-line text input. It owns its text and focus; forms subscribe to its events.
#[derive(Debug)]
pub struct TextField {
    focus_handle: FocusHandle,
    buffer: TextBuffer,
    placeholder: SharedString,
    disabled: bool,
    read_only: bool,
    /// The last paint, for pointer hit tests and IME geometry.
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    selecting: bool,
}

impl EventEmitter<TextFieldEvent> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle().tab_stop(true),
            buffer: TextBuffer::new(""),
            placeholder: SharedString::default(),
            disabled: false,
            read_only: false,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.),
            selecting: false,
        }
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn with_text(mut self, text: impl AsRef<str>) -> Self {
        self.buffer = TextBuffer::new(text.as_ref());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    /// Replaces the text; emits `Changed` only when it differs.
    pub fn set_text(&mut self, text: impl AsRef<str>, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.buffer.set_text(text.as_ref());
        self.after_edit(changed, window, cx);
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    /// Moves focus into the field; a disabled field never takes focus.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if !self.disabled {
            window.focus(&self.focus_handle, cx);
        }
    }

    fn editable(&self) -> bool {
        !self.disabled && !self.read_only
    }

    fn after_edit(&mut self, changed: bool, window: &Window, cx: &mut Context<Self>) {
        if changed {
            window.notify_text_input_changed(&self.focus_handle);
            cx.emit(TextFieldEvent::Changed);
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    /// Applies a text change when the field accepts input.
    fn edit(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut TextBuffer) -> bool,
    ) {
        if self.editable() {
            let changed = change(&mut self.buffer);
            self.after_edit(changed, window, cx);
        }
    }

    /// Moves or selects the caret without changing the text.
    fn move_caret(
        &mut self,
        change: impl FnOnce(&mut TextBuffer),
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        change(&mut self.buffer);
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, TextBuffer::backspace);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, TextBuffer::delete);
    }

    fn move_left(&mut self, _: &MoveLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::move_left, window, cx);
    }

    fn move_right(&mut self, _: &MoveRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::move_right, window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::select_left, window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::select_right, window, cx);
    }

    fn move_home(&mut self, _: &MoveHome, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::move_home, window, cx);
    }

    fn move_end(&mut self, _: &MoveEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::move_end, window, cx);
    }

    fn select_home(&mut self, _: &SelectHome, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::select_home, window, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::select_end, window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::select_all, window, cx);
    }

    fn copy_selection(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        let selected = self.buffer.selected_text();
        if !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }

    fn cut_selection(&mut self, _: &CutSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.copy_selection(&CopySelection, window, cx);
        self.edit(window, cx, |buffer| buffer.insert(""));
    }

    fn paste_text(&mut self, _: &PasteText, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = single_line(&text);
            self.edit(window, cx, |buffer| buffer.insert(&text));
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Submitted);
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.focus(window, cx);
        window.request_virtual_keyboard();
        self.selecting = true;
        let index = self.index_for(event.position);
        let extend = event.modifiers.shift;
        self.move_caret(
            move |buffer| {
                if extend {
                    buffer.select_to(index)
                } else {
                    buffer.move_to(index)
                }
            },
            window,
            cx,
        );
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            let index = self.index_for(event.position);
            self.move_caret(move |buffer| buffer.select_to(index), window, cx);
        }
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// The last paint, while it still shows the current text.
    fn current_line(&self) -> Option<&ShapedLine> {
        let line = self.last_layout.as_ref()?;
        (self.buffer.is_empty() || line.text.as_ref() == self.buffer.text()).then_some(line)
    }

    /// The text offset under a window position, snapped to a grapheme boundary.
    fn index_for(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds, self.current_line()) else {
            return self.buffer.cursor();
        };
        let x = position.x - bounds.left() + self.scroll_x;
        self.buffer.snap_to_grapheme(line.closest_index_for_x(x))
    }

    /// Outline colour and width for the current state; focus draws a 2 px inside border.
    fn outline(&self, focused: bool, palette: &Palette) -> (Color, f32) {
        if self.disabled {
            (palette.border.disabled, 1.)
        } else if focused {
            (palette.border.focus, 2.)
        } else if self.read_only {
            (palette.border.subtle, 1.)
        } else {
            (palette.border.strong, 1.)
        }
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.theme().palette;
        let focused = self.focus_handle.is_focused(window);
        let enabled = !self.disabled;
        let (outline, width) = self.outline(focused, &palette);
        let (text_color, fill_color) = if self.disabled {
            (palette.text.disabled, palette.fill.disabled)
        } else {
            (palette.text.primary, palette.surface.sunken)
        };
        let field = div()
            .group(FIELD_GROUP)
            .key_context(KEY_CONTEXT)
            .when(enabled, |field| {
                field
                    .track_focus(&self.focus_handle)
                    .cursor(CursorStyle::IBeam)
            })
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::move_home))
            .on_action(cx.listener(Self::move_end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy_selection))
            .on_action(cx.listener(Self::cut_selection))
            .on_action(cx.listener(Self::paste_text))
            .on_action(cx.listener(Self::submit))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .h(px(SIZE_CONTROL_DEFAULT))
            .w_full()
            .px(px(SPACE_2 - (width - 1.)))
            .rounded(px(RADIUS_MD))
            .bg(fill_color)
            .border_color(outline)
            .text_color(text_color)
            .text_style(TYPE_BODY);
        if width > 1. {
            field.border_2()
        } else {
            field.border_1()
        }
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.buffer.range_to_utf16(&range));
        Some(self.buffer.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selected()),
            reversed: self.buffer.reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked()
            .map(|range| self.buffer.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        self.after_edit(false, window, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = strip_line_breaks(text);
        self.edit(window, cx, |buffer| buffer.replace_text(range_utf16, &text));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = strip_line_breaks(new_text);
        self.edit(window, cx, |buffer| {
            buffer.replace_and_mark(range_utf16, &text, new_selected_range)
        });
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.current_line()?;
        let range = self.buffer.range_from_utf16(&range_utf16);
        let left = element_bounds.left() - self.scroll_x;
        Some(Bounds::from_corners(
            point(left + line.x_for_index(range.start), element_bounds.top()),
            point(left + line.x_for_index(range.end), element_bounds.bottom()),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range_utf16, element_bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line = self.current_line()?;
        let index = line.closest_index_for_x(position.x - bounds.left() + self.scroll_x);
        Some(self.buffer.offset_to_utf16(index))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.select_utf16(range_utf16);
        self.after_edit(false, window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buffer.utf16_len())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.editable()
    }
}

/// Typed line breaks never enter a single-line field.
fn strip_line_breaks(text: &str) -> String {
    text.replace(['\r', '\n'], "")
}

/// Pasted line breaks become single spaces.
fn single_line(text: &str) -> String {
    text.trim_end_matches(['\r', '\n'])
        .replace("\r\n", "\n")
        .replace(['\r', '\n'], " ")
}

/// Byte offsets where extended grapheme clusters start. Approximates UAX #29 for
/// combining marks, variation selectors, emoji modifiers, ZWJ sequences and flags.
fn grapheme_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut previous: Option<char> = None;
    let mut regional_run = 0;
    for (index, ch) in text.char_indices() {
        let joined = previous.is_some_and(|prev| joins(prev, ch, regional_run));
        if !joined {
            starts.push(index);
        }
        regional_run = if is_regional_indicator(ch) {
            regional_run + 1
        } else {
            0
        };
        previous = Some(ch);
    }
    starts
}

/// Whether `next` continues the cluster that ends with `prev`; `regional_run`
/// counts the regional indicators ending at `prev`.
fn joins(prev: char, next: char, regional_run: usize) -> bool {
    match (prev, next) {
        ('\r', '\n') => true,
        _ if prev.is_control() || next.is_control() => false,
        _ if is_extend(next) || next == ZWJ || prev == ZWJ => true,
        _ => is_regional_indicator(prev) && is_regional_indicator(next) && regional_run % 2 == 1,
    }
}

fn is_regional_indicator(ch: char) -> bool {
    matches!(ch as u32, 0x1F1E6..=0x1F1FF)
}

fn is_extend(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0900..=0x0903
            | 0x093A..=0x094F
            | 0x0E31
            | 0x0E34..=0x0E3A
            | 0x0E47..=0x0E4E
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200C
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0x1F3FB..=0x1F3FF
            | 0xE0020..=0xE007F
            | 0xE0100..=0xE01EF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_puts_the_cursor_at_the_end() {
        let buffer = TextBuffer::new("abc");
        assert_eq!(buffer.selected(), 3..3);
        assert_eq!(buffer.cursor(), 3);
    }

    #[test]
    fn grapheme_boundaries_keep_marks_flags_and_sequences_together() {
        // "e" + combining acute (3 bytes), a flag (8), a skin-toned thumb (8), a ZWJ family (11).
        let buffer = TextBuffer::new("e\u{301}🇫🇷👍🏽👨\u{200D}👩");
        assert_eq!(buffer.text().len(), 30);
        assert_eq!(buffer.next_boundary(0), 3);
        assert_eq!(buffer.next_boundary(3), 11);
        assert_eq!(buffer.next_boundary(11), 19);
        assert_eq!(buffer.prev_boundary(30), 19);
        assert_eq!(buffer.prev_boundary(19), 11);
        assert_eq!(buffer.snap_to_grapheme(5), 3);
        assert_eq!(buffer.snap_to_grapheme(30), 30);
    }

    #[test]
    fn crlf_is_one_grapheme() {
        let buffer = TextBuffer::new("a\r\nb");
        assert_eq!(buffer.next_boundary(1), 3);
        assert_eq!(buffer.prev_boundary(3), 1);
    }

    #[test]
    fn arrow_keys_step_over_whole_multibyte_graphemes() {
        let mut buffer = TextBuffer::new("a漢🙂b");
        buffer.move_left();
        assert_eq!(buffer.cursor(), 8);
        buffer.move_left();
        assert_eq!(buffer.cursor(), 4);
        buffer.move_right();
        assert_eq!(buffer.cursor(), 8);
    }

    #[test]
    fn backspace_removes_a_whole_cluster() {
        let mut buffer = TextBuffer::new("e\u{301}x");
        assert!(buffer.backspace());
        assert_eq!(buffer.text(), "e\u{301}");
        assert!(buffer.backspace());
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn backspace_at_start_and_delete_at_end_are_no_ops() {
        let mut buffer = TextBuffer::new("abc");
        buffer.move_home();
        assert!(!buffer.backspace());
        buffer.move_end();
        assert!(!buffer.delete());
        assert_eq!(buffer.text(), "abc");
    }

    #[test]
    fn insert_replaces_the_selection_and_reports_changes() {
        let mut buffer = TextBuffer::new("hello world");
        buffer.move_home();
        buffer.select_to(5);
        assert!(buffer.insert("X"));
        assert_eq!(buffer.text(), "X world");
        assert_eq!(buffer.cursor(), 1);
        assert!(!buffer.insert(""));
    }

    #[test]
    fn move_collapses_a_selection_to_its_edge() {
        let mut buffer = TextBuffer::new("abc");
        buffer.select_all();
        buffer.move_left();
        assert_eq!(buffer.selected(), 0..0);
        buffer.select_all();
        buffer.move_right();
        assert_eq!(buffer.selected(), 3..3);
    }

    #[test]
    fn shift_selection_flips_direction_around_the_anchor() {
        let mut buffer = TextBuffer::new("abcdef");
        buffer.select_left();
        assert_eq!((buffer.selected(), buffer.cursor()), (5..6, 5));
        buffer.select_left();
        assert_eq!((buffer.selected(), buffer.cursor()), (4..6, 4));
        buffer.select_right();
        buffer.select_right();
        assert_eq!((buffer.selected(), buffer.cursor()), (6..6, 6));
    }

    #[test]
    fn utf16_offsets_follow_the_platform_units() {
        // "e" (1 unit), "🙂" (2 units, a surrogate pair), "漢" (1 unit).
        let buffer = TextBuffer::new("e🙂漢");
        assert_eq!(buffer.utf16_len(), 4);
        assert_eq!(buffer.offset_to_utf16(5), 3);
        assert_eq!(buffer.offset_from_utf16(3), 5);
        assert_eq!(buffer.range_to_utf16(&(1..5)), 1..3);
        assert_eq!(buffer.range_from_utf16(&(1..3)), 1..5);
    }

    #[test]
    fn replace_text_takes_a_utf16_range() {
        let mut buffer = TextBuffer::new("e🙂漢");
        assert!(buffer.replace_text(Some(1..3), "x"));
        assert_eq!(buffer.text(), "ex漢");
        assert_eq!(buffer.cursor(), 2);
    }

    #[test]
    fn composing_text_is_marked_then_committed() {
        let mut buffer = TextBuffer::new("ab");
        buffer.move_to(1);
        assert!(buffer.replace_and_mark(None, "ni", None));
        assert_eq!(buffer.text(), "anib");
        assert_eq!(buffer.marked(), Some(1..3));
        assert_eq!(buffer.selected(), 3..3);

        assert!(buffer.replace_text(None, "你"));
        assert_eq!(buffer.text(), "a你b");
        assert_eq!(buffer.marked(), None);
        assert_eq!(buffer.cursor(), 4);
    }

    #[test]
    fn marked_selection_is_relative_to_the_composing_text() {
        let mut buffer = TextBuffer::new("A🙂Z");
        buffer.replace_and_mark(Some(1..3), "é🙂", Some(1..3));
        assert_eq!(buffer.text(), "Aé🙂Z");
        assert_eq!(buffer.marked(), Some(1..7));
        assert_eq!(buffer.selected(), 3..7);

        buffer.replace_and_mark(None, "漢", Some(1..1));
        assert_eq!(buffer.text(), "A漢Z");
        assert_eq!(buffer.marked(), Some(1..4));
        assert_eq!(buffer.selected(), 4..4);

        buffer.replace_and_mark(None, "", Some(0..0));
        assert_eq!(buffer.text(), "AZ");
        assert_eq!(buffer.marked(), None);
        assert_eq!(buffer.selected(), 1..1);
    }

    #[test]
    fn set_text_resets_the_cursor_only_when_the_text_changes() {
        let mut buffer = TextBuffer::new("abc");
        buffer.move_home();
        assert!(!buffer.set_text("abc"));
        assert_eq!(buffer.cursor(), 0);
        assert!(buffer.set_text("xy"));
        assert_eq!(buffer.cursor(), 2);
    }

    #[test]
    fn select_all_then_typing_replaces_everything() {
        let mut buffer = TextBuffer::new("old 漢字");
        buffer.select_all();
        assert_eq!(buffer.selected_text(), "old 漢字");
        assert!(buffer.insert("new"));
        assert_eq!(buffer.text(), "new");
    }

    #[test]
    fn line_breaks_follow_the_single_line_rules() {
        assert_eq!(single_line("a\r\nb\n"), "a b");
        assert_eq!(strip_line_breaks("a\nb"), "ab");
    }
}
