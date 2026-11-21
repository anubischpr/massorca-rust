//! Single-line text input: the editing model, the element that paints it, the
//! entity that owns it and its platform input handler (typing and IME).

use std::ops::Range;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, Div, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hsla, InteractiveElement, IntoElement, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Render,
    ShapedLine, SharedString, StatefulInteractiveElement, Style, Styled, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, actions, div, fill, point, px, relative, size,
};

use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, Palette, RADIUS_MD, RADIUS_SM, SIZE_CONTROL_COMFORTABLE, SIZE_CONTROL_DEFAULT,
    SIZE_HIT_POINTER, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY, TYPE_BODY_SM, TYPE_LABEL, TYPE_MONO,
};

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
    label: Option<SharedString>,
    helper: Option<SharedString>,
    error: Option<SharedString>,
    leading_icon: Option<IconName>,
    comfortable: bool,
    mono: bool,
    clearable: bool,
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
            label: None,
            helper: None,
            error: None,
            leading_icon: None,
            comfortable: false,
            mono: false,
            clearable: false,
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

    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    pub fn helper(mut self, text: impl Into<SharedString>) -> Self {
        self.helper = Some(text.into());
        self
    }

    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    pub fn leading_icon(mut self, icon: IconName) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    pub fn with_text(mut self, text: impl AsRef<str>) -> Self {
        self.buffer = TextBuffer::new(text.as_ref());
        self
    }

    /// The 32 px form size with 12 px padding.
    pub fn comfortable(mut self) -> Self {
        self.comfortable = true;
        self
    }

    /// Paths, IDs and values use the mono type style.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Shows a clear action while the field has a value and is focused or hovered.
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
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

    pub fn set_error(&mut self, error: Option<SharedString>, cx: &mut Context<Self>) {
        self.error = error;
        cx.notify();
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

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |buffer| buffer.set_text(""));
        self.focus(window, cx);
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
        } else if self.error.is_some() {
            (palette.status.danger_fg, if focused { 2. } else { 1. })
        } else if focused {
            (palette.border.focus, 2.)
        } else if self.read_only {
            (palette.border.subtle, 1.)
        } else {
            (palette.border.strong, 1.)
        }
    }

    fn clear_button(
        &self,
        focused: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let filled = !self.buffer.is_empty();
        div()
            .id("text-field-clear")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(SIZE_HIT_POINTER))
            .rounded(px(RADIUS_SM))
            .cursor_pointer()
            .invisible()
            .when(filled, |button| {
                button.group_hover(FIELD_GROUP, |style| style.visible())
            })
            .when(filled && focused, |button| button.visible())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.clear(window, cx)))
            .child(
                Icon::new(IconName::X)
                    .size(IconSize::Small)
                    .color(palette.text.secondary),
            )
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
        let inset = if self.comfortable { SPACE_3 } else { SPACE_2 };
        let height = if self.comfortable {
            SIZE_CONTROL_COMFORTABLE
        } else {
            SIZE_CONTROL_DEFAULT
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
            .h(px(height))
            .w_full()
            .px(px(inset - (width - 1.)))
            .rounded(px(RADIUS_MD))
            .bg(fill_color)
            .border_color(outline)
            .text_color(text_color)
            .text_style(if self.mono { TYPE_MONO } else { TYPE_BODY });
        let field = if width > 1. {
            field.border_2()
        } else {
            field.border_1()
        };
        let icon_color = if self.disabled {
            palette.text.disabled
        } else {
            palette.text.secondary
        };
        let field = field
            .when_some(self.leading_icon, |field, icon| {
                field.child(Icon::new(icon).size(IconSize::Small).color(icon_color))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(TextElement { input: cx.entity() }),
            )
            .when(self.clearable && enabled, |field| {
                field.child(self.clear_button(focused, &palette, cx))
            });

        let label = self.label.clone().map(|label| {
            div()
                .text_style(TYPE_LABEL)
                .text_color(palette.text.secondary)
                .child(label)
                .when(enabled, |row| {
                    row.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, window, cx| this.focus(window, cx)),
                    )
                })
        });
        let note = self
            .error
            .clone()
            .map(|error| {
                note_line(
                    error,
                    palette.status.danger_fg,
                    Some(IconName::OctagonAlert),
                )
            })
            .or_else(|| {
                self.helper
                    .clone()
                    .map(|helper| note_line(helper, palette.text.secondary, None))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .w_full()
            .when_some(label, |root, label| root.child(label))
            .child(field)
            .when_some(note, |root, note| root.child(note))
    }
}

/// A helper or error line under the field.
fn note_line(text: SharedString, color: Color, icon: Option<IconName>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .text_style(TYPE_BODY_SM)
        .text_color(color)
        .when_some(icon, |line, icon| {
            line.child(Icon::new(icon).size(IconSize::XSmall).color(color))
        })
        .child(text)
}

/// Paints the line, selection and caret, and registers the platform input handler.
struct TextElement {
    input: Entity<TextField>,
}

struct Prepaint {
    line: ShapedLine,
    scroll: Pixels,
    selection: Option<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl Element for TextElement {
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
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
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
        let field = self.input.read(cx);
        let focused = field.focus_handle.is_focused(window);
        let editable = field.editable();
        let empty = field.buffer.is_empty();
        let text = if empty {
            field.placeholder.clone()
        } else {
            SharedString::from(field.buffer.text().to_owned())
        };
        let selected = field.buffer.selected();
        let cursor = field.buffer.cursor();
        let marked = field.buffer.marked();
        let scroll_x = field.scroll_x;
        let color = Hsla::from(if empty {
            palette.text.tertiary
        } else if field.disabled {
            palette.text.disabled
        } else {
            palette.text.primary
        });

        let style = window.text_style();
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match marked {
            Some(range) if !empty => [
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
                    len: text.len() - range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            _ => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);

        let caret_x = line.x_for_index(cursor);
        let content_width = line.x_for_index(line.text.len());
        let scroll = px(visible_scroll(
            f32::from(scroll_x),
            f32::from(caret_x),
            f32::from(bounds.size.width),
            f32::from(content_width),
        ));
        let origin = bounds.left() - scroll;
        let (selection, caret) = if !selected.is_empty() {
            let span = Bounds::from_corners(
                point(origin + line.x_for_index(selected.start), bounds.top()),
                point(origin + line.x_for_index(selected.end), bounds.bottom()),
            );
            (Some(fill(span, Hsla::from(palette.selection.bg))), None)
        } else if focused && editable {
            let bar = Bounds::new(
                point(origin + caret_x, bounds.top()),
                size(px(1.), bounds.size.height),
            );
            (None, Some(fill(bar, Hsla::from(palette.text.primary))))
        } else {
            (None, None)
        };

        let layout = line.clone();
        self.input.update(cx, |field, _| {
            field.scroll_x = scroll;
            field.last_layout = Some(layout);
            field.last_bounds = Some(bounds);
        });
        Prepaint {
            line,
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
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let origin = point(bounds.left() - prepaint.scroll, bounds.top());
        prepaint
            .line
            .paint(
                origin,
                window.line_height(),
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .ok();
        if let Some(caret) = prepaint.caret.take() {
            window.paint_quad(caret);
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

/// Scroll offset that keeps the caret in view and leaves no empty space at the end.
fn visible_scroll(scroll: f32, caret: f32, viewport: f32, content: f32) -> f32 {
    let scroll = if caret < scroll {
        caret
    } else if caret > scroll + viewport {
        caret - viewport
    } else {
        scroll
    };
    scroll.clamp(0., (content - viewport).max(0.))
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
    use gpui::{TestAppContext, VisualTestContext};
    use std::{cell::RefCell, rc::Rc};

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

    #[test]
    fn scroll_keeps_the_caret_in_view_without_empty_space_at_the_end() {
        assert_eq!(visible_scroll(0., 150., 100., 200.), 50.);
        assert_eq!(visible_scroll(50., 20., 100., 200.), 20.);
        assert_eq!(visible_scroll(80., 300., 100., 200.), 100.);
        assert_eq!(visible_scroll(40., 10., 100., 50.), 0.);
    }

    /// Subscribes to a field and collects the events it emits.
    fn record_events(
        field: &Entity<TextField>,
        cx: &mut VisualTestContext,
    ) -> Rc<RefCell<Vec<TextFieldEvent>>> {
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(field, move |_, event: &TextFieldEvent, _| {
                sink.borrow_mut().push(*event)
            })
            .detach();
        });
        events
    }

    /// Focuses a field and redraws, so the platform input handler is installed.
    fn focus_and_draw(field: &Entity<TextField>, cx: &mut VisualTestContext) {
        field.update_in(cx, |field, window, cx| {
            field.focus(window, cx);
            window.refresh();
        });
        cx.run_until_parked();
    }

    fn text_of(field: &Entity<TextField>, cx: &VisualTestContext) -> String {
        field.read_with(cx, |field, _| field.text().to_owned())
    }

    #[gpui::test]
    fn typing_edits_the_focused_field(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx));
        focus_and_draw(&field, cx);
        cx.simulate_input("abc");
        assert_eq!(text_of(&field, cx), "abc");
    }

    #[gpui::test]
    fn enter_submits_without_inserting_a_line_break(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("oppm"));
        let events = record_events(&field, cx);
        focus_and_draw(&field, cx);
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![TextFieldEvent::Submitted]);
        assert_eq!(text_of(&field, cx), "oppm");
    }

    #[gpui::test]
    fn select_all_then_typing_replaces_the_text(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("old"));
        focus_and_draw(&field, cx);
        cx.simulate_keystrokes("secondary-a");
        cx.simulate_input("new");
        assert_eq!(text_of(&field, cx), "new");
    }

    #[gpui::test]
    fn read_only_field_keeps_its_text(cx: &mut TestAppContext) {
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).with_text("oppm").read_only(true));
        focus_and_draw(&field, cx);
        cx.simulate_input("x");
        assert_eq!(text_of(&field, cx), "oppm");
        field.update_in(cx, |field, window, cx| {
            assert!(!field.accepts_text_input(window, cx));
        });
    }

    #[gpui::test]
    fn disabled_field_never_takes_focus(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).disabled(true));
        field.update_in(cx, |field, window, cx| field.focus(window, cx));
        cx.run_until_parked();
        field.update_in(cx, |field, window, _| {
            assert!(!field.focus_handle.is_focused(window));
        });
    }

    #[gpui::test]
    fn composition_is_marked_then_committed_through_the_input_handler(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| {
            field.replace_and_mark_text_in_range(None, "ni", None, window, cx);
            assert_eq!(field.marked_text_range(window, cx), Some(0..2));
        });
        field.update_in(cx, |field, window, cx| {
            field.replace_text_in_range(None, "你", window, cx);
            assert_eq!(field.text(), "你");
            assert_eq!(field.marked_text_range(window, cx), None);
        });
    }

    #[gpui::test]
    fn edits_emit_changed_but_identical_text_does_not(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx));
        let events = record_events(&field, cx);
        focus_and_draw(&field, cx);
        cx.simulate_input("a");
        field.update_in(cx, |field, window, cx| field.set_text("a", window, cx));
        field.update_in(cx, |field, window, cx| field.set_text("ab", window, cx));
        assert_eq!(
            *events.borrow(),
            vec![TextFieldEvent::Changed, TextFieldEvent::Changed]
        );
    }

    #[gpui::test]
    fn copy_and_paste_round_trip_and_flatten_line_breaks(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("abc"));
        focus_and_draw(&field, cx);
        cx.simulate_keystrokes("secondary-a secondary-c end secondary-v");
        assert_eq!(text_of(&field, cx), "abcabc");
        cx.write_to_clipboard(ClipboardItem::new_string("x\ny".to_owned()));
        cx.simulate_keystrokes("secondary-v");
        assert_eq!(text_of(&field, cx), "abcabcx y");
    }

    #[gpui::test]
    fn error_outline_outranks_focus_and_thickens_when_focused(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).error("Name is taken."));
        let palette = cx.update(|_, cx| cx.theme().palette);
        assert_eq!(
            field.read_with(cx, |field, _| field.outline(false, &palette)),
            (palette.status.danger_fg, 1.)
        );
        focus_and_draw(&field, cx);
        assert_eq!(
            field.read_with(cx, |field, _| field.outline(true, &palette)),
            (palette.status.danger_fg, 2.)
        );
    }

    #[gpui::test]
    fn clear_empties_the_field_and_keeps_focus(cx: &mut TestAppContext) {
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).clearable().with_text("oppm"));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| field.clear(window, cx));
        assert_eq!(text_of(&field, cx), "");
        assert!(field.update_in(cx, |field, window, _| field.focus_handle.is_focused(window)));
    }
}
