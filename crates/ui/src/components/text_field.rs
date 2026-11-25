//! Single-line text input: the editing model, the element that paints it, the
//! entity that owns it and its platform input handler (typing and IME).

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use gpui::accesskit::Role;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, Div, Element, ElementId,
    ElementInputHandler, Entity, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla,
    InteractiveElement, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Render, ShapedLine,
    SharedString, StatefulInteractiveElement, Style, Styled, Subscription, TextAlign, TextRun,
    UnderlineStyle, Window, actions, div, fill, point, px, relative, size,
};

use crate::a11y;
use crate::components::button::ButtonSize;
use crate::components::icon_button::IconButton;
use crate::components::text_editing::input::{self, InputHost, impl_input_handler};
use crate::components::text_editing::keymap::{self, EditShortcut};
use crate::components::text_editing::masking::{masked_offset, masked_text, text_offset};
use crate::components::text_editing::undo_history::{EditKind, Snapshot, UndoHistory};
use crate::components::text_editing::{
    DeleteWordBackward, DeleteWordForward, MoveWordLeft, MoveWordRight, Redo, SelectWordLeft,
    SelectWordRight, Undo,
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
    history: UndoHistory,
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
            history: UndoHistory::new(true),
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

    /// Overwrites the text in place before it is released, so a masked value leaves no readable copy behind.
    pub fn wipe(&mut self) {
        let len = self.text.len();
        self.text.clear();
        self.text.extend((0..len).map(|_| '\0'));
        self.text.clear();
        self.selected = 0..0;
        self.reversed = false;
        self.marked = None;
        self.history.clear();
    }

    /// Replaces the whole text and puts the cursor at its end; `true` when it changed.
    /// Replaces the whole text and puts the cursor at its end; `true` when it changed. A change drops
    /// the undo history; an identical text keeps it, so a draft synced on every frame keeps its undo.
    pub fn set_text(&mut self, text: &str) -> bool {
        if self.text == text {
            return false;
        }
        self.text = text.to_owned();
        let end = self.text.len();
        self.selected = end..end;
        self.reversed = false;
        self.marked = None;
        self.history.clear();
        true
    }

    /// Turns undo recording on or off; a masked value turns it off so no copy of it is kept.
    pub(crate) fn set_history(&mut self, enabled: bool) {
        self.history.set_enabled(enabled);
    }

    pub fn move_to(&mut self, offset: usize) {
        self.history.break_group();
        let offset = self.snap_to_grapheme(offset);
        self.selected = offset..offset;
        self.reversed = false;
    }

    /// Moves the free end of the selection to `offset`, keeping the anchor.
    pub fn select_to(&mut self, offset: usize) {
        self.history.break_group();
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
        self.history.break_group();
        self.selected = 0..self.text.len();
        self.reversed = false;
    }

    /// Steps back to the state before the last edit; `false` when there is no step to undo.
    pub fn undo(&mut self) -> bool {
        if !self.history.can_undo() {
            return false;
        }
        let current = self.snapshot();
        let Some(previous) = self.history.take_undo(current) else {
            return false;
        };
        self.restore(previous);
        true
    }

    /// Steps forward again after an undo; `false` when there is no step to redo.
    pub fn redo(&mut self) -> bool {
        if !self.history.can_redo() {
            return false;
        }
        let current = self.snapshot();
        let Some(next) = self.history.take_redo(current) else {
            return false;
        };
        self.restore(next);
        true
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            selected: self.selected.clone(),
            reversed: self.reversed,
        }
    }

    fn restore(&mut self, state: Snapshot) {
        self.text = state.text;
        self.selected = state.selected;
        self.reversed = state.reversed;
        self.marked = None;
    }

    /// Runs `change` as one edit of `kind`, recording the step it starts or extends.
    fn recorded(&mut self, kind: EditKind, change: impl FnOnce(&mut Self) -> bool) -> bool {
        if !self.history.enabled() {
            return change(self);
        }
        let settled = self.marked.is_some() || self.selected.is_empty();
        let before = (!self.history.joins(kind, settled)).then(|| self.snapshot());
        let changed = change(self);
        if changed {
            self.history.record(before, kind);
        }
        changed
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

    /// Collapses a selection to its start, or moves the caret to the start of the word before it.
    pub fn move_word_left(&mut self) {
        let target = if self.selected.is_empty() {
            self.prev_word_boundary(self.cursor())
        } else {
            self.selected.start
        };
        self.move_to(target);
    }

    /// Collapses a selection to its end, or moves the caret past the word after it.
    pub fn move_word_right(&mut self) {
        let target = if self.selected.is_empty() {
            self.next_word_boundary(self.cursor())
        } else {
            self.selected.end
        };
        self.move_to(target);
    }

    pub fn select_word_left(&mut self) {
        self.select_to(self.prev_word_boundary(self.cursor()));
    }

    pub fn select_word_right(&mut self) {
        self.select_to(self.next_word_boundary(self.cursor()));
    }

    /// Deletes the selection, or the text back to the start of the word before the cursor.
    pub fn delete_word_backward(&mut self) -> bool {
        self.recorded(EditKind::Other, |buffer| {
            let range = if buffer.selected.is_empty() {
                buffer.prev_word_boundary(buffer.cursor())..buffer.cursor()
            } else {
                buffer.selected.clone()
            };
            buffer.replace(range, "")
        })
    }

    /// Deletes the selection, or the text up to the end of the word after the cursor.
    pub fn delete_word_forward(&mut self) -> bool {
        self.recorded(EditKind::Other, |buffer| {
            let range = if buffer.selected.is_empty() {
                buffer.cursor()..buffer.next_word_boundary(buffer.cursor())
            } else {
                buffer.selected.clone()
            };
            buffer.replace(range, "")
        })
    }

    /// The start of the word before `offset`, past the separators between them.
    pub fn prev_word_boundary(&self, offset: usize) -> usize {
        let clusters = self.word_clusters();
        let offset = self.clamp_offset(offset);
        let mut at = clusters.partition_point(|(range, _)| range.start < offset);
        while at > 0 && !clusters[at - 1].1 {
            at -= 1;
        }
        while at > 0 && clusters[at - 1].1 {
            at -= 1;
        }
        clusters
            .get(at)
            .map_or(self.text.len(), |(range, _)| range.start)
    }

    /// The end of the word after `offset`, past the separators before it.
    pub fn next_word_boundary(&self, offset: usize) -> usize {
        let clusters = self.word_clusters();
        let offset = self.clamp_offset(offset);
        let mut at = clusters.partition_point(|(range, _)| range.start < offset);
        while at < clusters.len() && !clusters[at].1 {
            at += 1;
        }
        while at < clusters.len() && clusters[at].1 {
            at += 1;
        }
        clusters
            .get(at)
            .map_or(self.text.len(), |(range, _)| range.start)
    }

    /// Selects the run of word characters, or of separators, under `offset`: a double click.
    pub fn select_word_at(&mut self, offset: usize) {
        let clusters = self.word_clusters();
        let offset = self.clamp_offset(offset);
        let Some(index) = clusters
            .partition_point(|(range, _)| range.start <= offset)
            .checked_sub(1)
        else {
            self.select_range(0..0);
            return;
        };
        let class = clusters[index].1;
        let mut first = index;
        while first > 0 && clusters[first - 1].1 == class {
            first -= 1;
        }
        let mut last = index;
        while last + 1 < clusters.len() && clusters[last + 1].1 == class {
            last += 1;
        }
        self.select_range(clusters[first].0.start..clusters[last].0.end);
    }

    /// Selects the line under `offset`, without its line break: a triple click.
    pub fn select_line_at(&mut self, offset: usize) {
        let offset = self.clamp_offset(offset);
        let start = self.text[..offset].rfind('\n').map_or(0, |at| at + 1);
        let end = self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |at| offset + at);
        self.select_range(start..end);
    }

    /// Writes `text` over the selection (paste, cut, a line break); `true` when the text changed.
    pub fn insert(&mut self, text: &str) -> bool {
        self.recorded(EditKind::Other, |buffer| {
            let range = buffer.selected.clone();
            buffer.replace(range, text)
        })
    }

    /// Deletes the selection, or the grapheme before the cursor.
    pub fn backspace(&mut self) -> bool {
        let kind = if self.selected.is_empty() {
            EditKind::Backspace
        } else {
            EditKind::Other
        };
        self.recorded(kind, |buffer| {
            let range = if buffer.selected.is_empty() {
                buffer.prev_boundary(buffer.cursor())..buffer.cursor()
            } else {
                buffer.selected.clone()
            };
            buffer.replace(range, "")
        })
    }

    /// Deletes the selection, or the grapheme after the cursor.
    pub fn delete(&mut self) -> bool {
        let kind = if self.selected.is_empty() {
            EditKind::Delete
        } else {
            EditKind::Other
        };
        self.recorded(kind, |buffer| {
            let range = if buffer.selected.is_empty() {
                buffer.cursor()..buffer.next_boundary(buffer.cursor())
            } else {
                buffer.selected.clone()
            };
            buffer.replace(range, "")
        })
    }

    /// Commits `text` over `range_utf16`, else over the composing range, else over the selection.
    pub fn replace_text(&mut self, range_utf16: Option<Range<usize>>, text: &str) -> bool {
        self.recorded(typing_kind(text), |buffer| {
            let range = buffer.target_range(range_utf16);
            buffer.replace(range, text)
        })
    }

    /// Writes composing text and marks it. `selected_utf16` is the selection inside
    /// the new text, relative to its start, as the platform reports it.
    pub fn replace_and_mark(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
    ) -> bool {
        self.recorded(EditKind::Typing, |buffer| {
            buffer.compose(range_utf16, text, selected_utf16)
        })
    }

    fn compose(
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
        self.history.break_group();
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

    /// Selects `range` with the caret at its end, ending any open undo step.
    fn select_range(&mut self, range: Range<usize>) {
        self.history.break_group();
        self.selected = range;
        self.reversed = false;
    }

    /// The grapheme clusters, each with whether it is a word: its first character is a letter, digit or underscore.
    fn word_clusters(&self) -> Vec<(Range<usize>, bool)> {
        let starts = grapheme_starts(&self.text);
        starts
            .iter()
            .enumerate()
            .map(|(index, &start)| {
                let end = starts.get(index + 1).copied().unwrap_or(self.text.len());
                let word = self.text[start..].chars().next().is_some_and(is_word_char);
                (start..end, word)
            })
            .collect()
    }
}

/// One typed character joins the typing before it; a longer insert or a line break is a step of its own.
fn typing_kind(text: &str) -> EditKind {
    if text != "\n" && grapheme_starts(text).len() == 1 {
        EditKind::Typing
    } else {
        EditKind::Other
    }
}

/// Letters, digits and underscores make up words; everything else separates them.
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
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
    let mut bindings = vec![
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
    ];
    bindings.extend(keymap::shortcuts(cfg!(target_os = "macos")).iter().map(
        |&(keys, shortcut)| match shortcut {
            EditShortcut::Undo => KeyBinding::new(keys, Undo, context),
            EditShortcut::Redo => KeyBinding::new(keys, Redo, context),
            EditShortcut::WordLeft => KeyBinding::new(keys, MoveWordLeft, context),
            EditShortcut::WordRight => KeyBinding::new(keys, MoveWordRight, context),
            EditShortcut::SelectWordLeft => KeyBinding::new(keys, SelectWordLeft, context),
            EditShortcut::SelectWordRight => KeyBinding::new(keys, SelectWordRight, context),
            EditShortcut::DeleteWordBackward => KeyBinding::new(keys, DeleteWordBackward, context),
            EditShortcut::DeleteWordForward => KeyBinding::new(keys, DeleteWordForward, context),
            EditShortcut::LineStart | EditShortcut::DocumentStart => {
                KeyBinding::new(keys, MoveHome, context)
            }
            EditShortcut::LineEnd | EditShortcut::DocumentEnd => {
                KeyBinding::new(keys, MoveEnd, context)
            }
            EditShortcut::SelectLineStart | EditShortcut::SelectDocumentStart => {
                KeyBinding::new(keys, SelectHome, context)
            }
            EditShortcut::SelectLineEnd | EditShortcut::SelectDocumentEnd => {
                KeyBinding::new(keys, SelectEnd, context)
            }
        },
    ));
    cx.bind_keys(bindings);
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
    clear_when_filled: bool,
    disabled: bool,
    read_only: bool,
    /// A search box: its root carries the searchbox role instead of the textbox role.
    search: bool,
    /// Paints the text as bullets and gives it to no clipboard, platform or accessibility reader.
    masked: bool,
    /// Adds a Show button that reveals a masked value while the field has focus.
    revealable: bool,
    /// The Show button is pressed, so the masked value paints as text.
    revealed: bool,
    /// Hides the value again when focus leaves the field or the window loses activation.
    _reveal_watch: Vec<Subscription>,
    /// The last paint, for pointer hit tests and IME geometry.
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    selecting: bool,
}

impl fmt::Debug for TextField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A masked field's text stays out of debug output, as it stays out of the accessibility tree.
        let buffer: &dyn fmt::Debug = if self.masked {
            &"[masked]"
        } else {
            &self.buffer
        };
        f.debug_struct("TextField")
            .field("focus_handle", &self.focus_handle)
            .field("buffer", buffer)
            .field("placeholder", &self.placeholder)
            .field("label", &self.label)
            .field("helper", &self.helper)
            .field("error", &self.error)
            .field("leading_icon", &self.leading_icon)
            .field("comfortable", &self.comfortable)
            .field("mono", &self.mono)
            .field("clearable", &self.clearable)
            .field("clear_when_filled", &self.clear_when_filled)
            .field("disabled", &self.disabled)
            .field("read_only", &self.read_only)
            .field("search", &self.search)
            .field("masked", &self.masked)
            .field("revealable", &self.revealable)
            .field("revealed", &self.revealed)
            .field("last_layout", &self.last_layout)
            .field("last_bounds", &self.last_bounds)
            .field("scroll_x", &self.scroll_x)
            .field("selecting", &self.selecting)
            .finish_non_exhaustive()
    }
}

impl Drop for TextField {
    /// Backstop for owners that clear a masked value on close: whatever is left is overwritten as the field goes.
    fn drop(&mut self) {
        if self.masked {
            self.buffer.wipe();
        }
    }
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
            clear_when_filled: false,
            disabled: false,
            read_only: false,
            search: false,
            masked: false,
            revealable: false,
            revealed: false,
            _reveal_watch: Vec::new(),
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
        let mut buffer = TextBuffer::new(text.as_ref());
        buffer.set_history(!self.masked);
        self.buffer = buffer;
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

    /// Keeps the clear action shown whenever the field has a value (SearchField.md), not only on hover or focus.
    pub fn clear_when_filled(mut self) -> Self {
        self.clear_when_filled = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes the field a search box: its root carries the searchbox role.
    pub fn search(mut self) -> Self {
        self.search = true;
        self
    }

    /// Hides the text as bullets, one per character, for a secret value (decision 0029).
    pub fn masked(mut self) -> Self {
        self.masked = true;
        self.buffer.set_history(false);
        self
    }

    /// Adds a Show button that reveals a masked value while the field has focus. Use it only for a
    /// value the user types: the field shows whatever it holds.
    pub fn revealable(mut self) -> Self {
        self.revealable = true;
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

    /// Inserts clipboard text at the caret, as Paste does: line breaks become spaces.
    pub fn insert_pasted(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = single_line(text);
        self.edit(window, cx, |buffer| buffer.insert(&text));
    }

    /// Empties the field without moving focus.
    pub fn clear_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.buffer.set_text("");
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
        input::after_edit(self, changed, window, cx);
    }

    /// Applies a text change when the field accepts input.
    fn edit(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut TextBuffer) -> bool,
    ) {
        input::edit(self, window, cx, change);
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

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, TextBuffer::undo);
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, TextBuffer::redo);
    }

    fn move_word_left(&mut self, _: &MoveWordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::move_word_left, window, cx);
    }

    fn move_word_right(&mut self, _: &MoveWordRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_caret(TextBuffer::move_word_right, window, cx);
    }

    fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_caret(TextBuffer::select_word_left, window, cx);
    }

    fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_caret(TextBuffer::select_word_right, window, cx);
    }

    fn delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(window, cx, TextBuffer::delete_word_backward);
    }

    fn delete_word_forward(
        &mut self,
        _: &DeleteWordForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(window, cx, TextBuffer::delete_word_forward);
    }

    /// A masked field gives nothing to the clipboard, so copy and cut do nothing there.
    fn copy_selection(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        let selected = self.buffer.selected_text();
        if !self.masked && !selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }

    fn cut_selection(&mut self, _: &CutSelection, window: &mut Window, cx: &mut Context<Self>) {
        if self.masked {
            return;
        }
        self.copy_selection(&CopySelection, window, cx);
        self.edit(window, cx, |buffer| buffer.insert(""));
    }

    fn paste_text(&mut self, _: &PasteText, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert_pasted(&text, window, cx);
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Submitted);
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Selecting and deleting keeps the clear undoable, unlike a programmatic `set_text`.
        self.edit(window, cx, |buffer| {
            buffer.select_all();
            buffer.insert("")
        });
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
        let clicks = event.click_count;
        self.move_caret(
            move |buffer| match (clicks, extend) {
                (1, true) => buffer.select_to(index),
                (2, _) => buffer.select_word_at(index),
                (n, _) if n >= 3 => buffer.select_line_at(index),
                _ => buffer.move_to(index),
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

    /// Whether the text paints as bullets: a masked field whose Show button has not revealed it.
    fn hides_text(&self) -> bool {
        self.masked && !self.revealed
    }

    /// The text as the line paints it: one bullet per character while the text is hidden.
    fn shown_text(&self) -> Cow<'_, str> {
        if self.hides_text() {
            Cow::Owned(masked_text(self.buffer.text()))
        } else {
            Cow::Borrowed(self.buffer.text())
        }
    }

    /// The painted offset of a text offset: a bullet stands for one whole character.
    fn painted_offset(&self, offset: usize) -> usize {
        if self.hides_text() {
            masked_offset(self.buffer.text(), offset)
        } else {
            offset
        }
    }

    /// The text offset of a painted offset.
    fn offset_from_painted(&self, painted: usize) -> usize {
        if self.hides_text() {
            text_offset(self.buffer.text(), painted)
        } else {
            painted
        }
    }

    /// The last paint, while it still shows the current text.
    fn current_line(&self) -> Option<&ShapedLine> {
        let line = self.last_layout.as_ref()?;
        let shown = self.shown_text();
        (self.buffer.is_empty() || line.text.as_ref() == shown.as_ref()).then_some(line)
    }

    /// The text offset under a window position, snapped to a grapheme boundary.
    fn index_for(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds, self.current_line()) else {
            return self.buffer.cursor();
        };
        let x = position.x - bounds.left() + self.scroll_x;
        let painted = line.closest_index_for_x(x);
        self.buffer
            .snap_to_grapheme(self.offset_from_painted(painted))
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
            .when(filled && self.clear_when_filled, |button| button.visible())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.clear(window, cx)))
            .child(
                Icon::new(IconName::X)
                    .size(IconSize::Small)
                    .color(palette.text.secondary),
            )
    }

    /// The Show button, which reveals the value while the field has focus. Its wrapper keeps a click
    /// from moving the caret, and the field takes focus back so that typing carries on.
    fn reveal_button(&self, cx: &mut Context<Self>) -> Div {
        let field = cx.entity();
        div()
            .debug_selector(|| SHOW_SELECTOR.to_owned())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(show_button(
                ElementId::from(format!("text-field-show-{}", cx.entity_id())),
                self.revealed,
                move |window, cx| field.update(cx, |field, cx| field.toggle_reveal(window, cx)),
            ))
    }

    /// Flips the reveal and returns focus to the field.
    fn toggle_reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revealed = !self.revealed;
        self.focus(window, cx);
        cx.notify();
    }
}

/// Where the test window finds a masked field's Show button.
const SHOW_SELECTOR: &str = "text-field-show";

/// The Show button of a masked field: a toggle named "Show value", pressed while the value shows.
fn show_button(
    id: ElementId,
    revealed: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> IconButton {
    IconButton::new(id, IconName::Eye, "Show value")
        .size(ButtonSize::Compact)
        .toggle(revealed)
        .on_click(on_click)
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
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::move_word_left))
            .on_action(cx.listener(Self::move_word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::delete_word_backward))
            .on_action(cx.listener(Self::delete_word_forward))
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
        if self.revealable && self._reveal_watch.is_empty() {
            let blur = cx.on_focus_out(&self.focus_handle, window, |field, _, _, cx| {
                field.revealed = false;
                cx.notify();
            });
            let activation = cx.observe_window_activation(window, |field, window, cx| {
                if !window.is_window_active() {
                    field.revealed = false;
                    cx.notify();
                }
            });
            self._reveal_watch = vec![blur, activation];
        }
        let reveal = (self.revealable && self.masked && enabled).then(|| self.reveal_button(cx));
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
            .when_some(reveal, |field, button| field.child(button))
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
        let role = if self.masked {
            Role::PasswordInput
        } else if self.search {
            Role::SearchInput
        } else {
            Role::TextInput
        };
        // A search box has no visible label: its placeholder names it.
        let name = self.label.clone().or_else(|| {
            (self.search && !self.placeholder.is_empty()).then(|| self.placeholder.clone())
        });
        let description = self.error.clone().or_else(|| self.helper.clone());
        let placeholder = self.placeholder.clone();
        // A masked field reports no value, so the accessibility tree never holds its text.
        let value = (!self.masked).then(|| SharedString::from(self.text().to_owned()));
        let (disabled, read_only, invalid) = (self.disabled, self.read_only, self.error.is_some());
        let root = div()
            .id(ElementId::from(format!("text-field-{}", cx.entity_id())))
            .role(role)
            .when_some(value, |root, value| root.aria_value(value))
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .w_full()
            .when_some(label, |root, label| root.child(label))
            .child(field)
            .when_some(note, |root, note| root.child(note));
        let root = match name {
            Some(name) => root.aria_label(name),
            None => root,
        };
        let root = match description {
            Some(text) => root.aria_description(text),
            None => root,
        };
        let root = if placeholder.is_empty() {
            root
        } else {
            root.aria_placeholder(placeholder)
        };
        a11y::annotate(root, move |node| {
            if disabled {
                node.set_disabled();
            }
            if read_only {
                node.set_read_only();
            }
            if invalid {
                node.set_invalid(gpui::accesskit::Invalid::True);
            }
        })
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
            SharedString::from(field.shown_text().into_owned())
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
            Some(range) if !empty => {
                let start = field.painted_offset(range.start);
                let end = field.painted_offset(range.end);
                [
                    TextRun {
                        len: start,
                        ..run.clone()
                    },
                    TextRun {
                        len: end - start,
                        underline: Some(UnderlineStyle {
                            color: Some(color),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: text.len() - end,
                        ..run
                    },
                ]
                .into_iter()
                .filter(|run| run.len > 0)
                .collect()
            }
            _ => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);

        let caret_x = line.x_for_index(field.painted_offset(cursor));
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
                point(
                    origin + line.x_for_index(field.painted_offset(selected.start)),
                    bounds.top(),
                ),
                point(
                    origin + line.x_for_index(field.painted_offset(selected.end)),
                    bounds.bottom(),
                ),
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

impl InputHost for TextField {
    fn text_buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    fn text_buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    fn input_accepts(&self) -> bool {
        self.editable()
    }

    fn input_masked(&self) -> bool {
        self.masked
    }

    fn input_focus(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn input_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Changed);
    }

    fn sanitize_insert(text: &str) -> String {
        strip_line_breaks(text)
    }

    fn layout_bounds(
        &self,
        range: Range<usize>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.current_line()?;
        let left = element_bounds.left() - self.scroll_x;
        Some(Bounds::from_corners(
            point(
                left + line.x_for_index(self.painted_offset(range.start)),
                element_bounds.top(),
            ),
            point(
                left + line.x_for_index(self.painted_offset(range.end)),
                element_bounds.bottom(),
            ),
        ))
    }

    fn offset_at(&self, position: Point<Pixels>) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line = self.current_line()?;
        let painted = line.closest_index_for_x(position.x - bounds.left() + self.scroll_x);
        Some(self.offset_from_painted(painted))
    }
}

impl_input_handler!(TextField);

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
    use crate::components::text_editing::keymap::{REDO_KEYS, UNDO_KEYS, platform_keys};
    use gpui::{EntityInputHandler, Modifiers, TestAppContext, VisualTestContext};
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
    fn a_dead_accent_then_a_letter_commits_the_composed_letter(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| {
            field.replace_and_mark_text_in_range(None, "´", None, window, cx);
            assert_eq!(field.marked_text_range(window, cx), Some(0..1));
        });
        field.update_in(cx, |field, window, cx| {
            field.replace_text_in_range(None, "á", window, cx);
            assert_eq!(field.text(), "á");
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

    /// A masked test value: ASCII, a two-byte letter, a three-byte ideograph and a four-byte emoji.
    const SECRET: &str = "placeholder-ñ漢🙂";

    /// The line the field painted last: its bullets when masked, its text otherwise.
    fn painted_text(field: &Entity<TextField>, cx: &VisualTestContext) -> String {
        field.read_with(cx, |field, _| {
            field
                .last_layout
                .as_ref()
                .map(|line| line.text.to_string())
                .unwrap_or_default()
        })
    }

    /// Redraws the window, so the field paints its latest text.
    fn redraw(cx: &mut VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[gpui::test]
    fn a_masked_field_paints_one_bullet_per_character(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text(SECRET));
        focus_and_draw(&field, cx);
        assert!(
            painted_text(&field, cx) == "•".repeat(SECRET.chars().count()),
            "the field paints one bullet per character"
        );
        assert!(text_of(&field, cx) == SECRET, "the field keeps its text");
    }

    #[gpui::test]
    fn a_masked_field_lays_each_character_over_its_own_bullet(cx: &mut TestAppContext) {
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text("a漢🙂b"));
        focus_and_draw(&field, cx);
        let element = field.read_with(cx, |field, _| {
            field.last_bounds.expect("the field was painted")
        });
        // UTF-16 units: "a" 0..1, "漢" 1..2, "🙂" 2..4 (a surrogate pair), "b" 4..5.
        let ranges: [Range<usize>; 4] = [0..1, 1..2, 2..4, 4..5];
        let spans: Vec<Bounds<Pixels>> = ranges
            .into_iter()
            .map(|range| {
                field
                    .update_in(cx, |field, window, cx| {
                        field.bounds_for_range(range, element, window, cx)
                    })
                    .expect("each character has a box")
            })
            .collect();
        for pair in spans.windows(2) {
            assert!(
                pair[0].right() == pair[1].left(),
                "the boxes sit side by side"
            );
        }
        let first_width = f32::from(spans[0].size.width);
        assert!(
            spans
                .iter()
                .all(|span| (f32::from(span.size.width) - first_width).abs() < 0.01),
            "every bullet has the same width"
        );
        let middle = element.center().y;
        let inside_emoji = point(spans[2].left() + px(1.), middle);
        let before_b = point(spans[2].right() - px(1.), middle);
        let (start, end) = field.update_in(cx, |field, window, cx| {
            (
                field.character_index_for_point(inside_emoji, window, cx),
                field.character_index_for_point(before_b, window, cx),
            )
        });
        assert_eq!(
            start,
            Some(2),
            "a point on the emoji's bullet hits its first unit"
        );
        assert_eq!(
            end,
            Some(4),
            "a point at its right edge hits the end of the emoji"
        );
    }

    #[gpui::test]
    fn a_selection_in_a_masked_field_covers_whole_bullets(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text("a漢🙂b"));
        focus_and_draw(&field, cx);
        cx.simulate_keystrokes("home shift-right shift-right");
        redraw(cx);
        assert!(
            field.read_with(cx, |field, _| field.buffer.selected()) == (0..4),
            "two characters are selected: 'a' and '漢'"
        );
        assert!(
            painted_text(&field, cx) == "••••",
            "the four characters paint four bullets"
        );
        let element = field.read_with(cx, |field, _| {
            field.last_bounds.expect("the field was painted")
        });
        let selection = field
            .update_in(cx, |field, window, cx| {
                field.bounds_for_range(0..2, element, window, cx)
            })
            .expect("the selection has a box");
        let second = field
            .update_in(cx, |field, window, cx| {
                field.bounds_for_range(1..2, element, window, cx)
            })
            .expect("the second character has a box");
        assert!(
            selection.left() == element.left(),
            "the selection starts at the first bullet"
        );
        assert!(
            selection.right() == second.right(),
            "the selection ends where the second bullet ends"
        );
    }

    #[gpui::test]
    fn copy_and_cut_put_nothing_on_the_clipboard_and_leave_a_masked_field_alone(
        cx: &mut TestAppContext,
    ) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text(SECRET));
        focus_and_draw(&field, cx);
        cx.write_to_clipboard(ClipboardItem::new_string("before".to_owned()));
        cx.simulate_keystrokes("secondary-a secondary-c secondary-x");
        let clipboard = cx.read_from_clipboard().and_then(|item| item.text());
        assert!(
            clipboard.as_deref() == Some("before"),
            "copy and cut write nothing to the clipboard"
        );
        assert!(
            text_of(&field, cx) == SECRET,
            "cut removes nothing from a masked field"
        );
    }

    #[gpui::test]
    fn paste_still_fills_a_masked_field_on_one_line(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked());
        focus_and_draw(&field, cx);
        cx.write_to_clipboard(ClipboardItem::new_string("ab\ncd".to_owned()));
        cx.simulate_keystrokes("secondary-v");
        redraw(cx);
        assert_eq!(text_of(&field, cx), "ab cd");
        assert_eq!(painted_text(&field, cx), "•••••");
    }

    #[gpui::test]
    fn typing_and_backspace_in_a_masked_field_move_whole_characters(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked());
        focus_and_draw(&field, cx);
        cx.simulate_input("ñ漢");
        redraw(cx);
        assert_eq!(painted_text(&field, cx), "••");
        cx.simulate_keystrokes("backspace");
        redraw(cx);
        assert!(
            text_of(&field, cx) == "ñ",
            "backspace removes one whole character"
        );
        assert_eq!(painted_text(&field, cx), "•");
    }

    #[gpui::test]
    fn an_unmasked_field_paints_its_text(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("oppm"));
        focus_and_draw(&field, cx);
        assert_eq!(painted_text(&field, cx), "oppm");
    }

    #[gpui::test]
    fn the_platform_is_given_no_text_from_a_masked_field(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text(SECRET));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| {
            let mut adjusted = None;
            assert!(
                field
                    .text_for_range(0..3, &mut adjusted, window, cx)
                    .is_none(),
                "no text is handed to the platform"
            );
        });
    }

    #[gpui::test]
    fn a_masked_field_prints_no_text_in_debug_output(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text(SECRET));
        let printed = field.read_with(cx, |field, _| format!("{field:?}"));
        assert!(!printed.contains(SECRET), "the debug output shows no text");
    }

    /// Clicks the Show button where the test window draws it.
    fn click_show(cx: &mut VisualTestContext) {
        redraw(cx);
        let bounds = cx
            .debug_bounds(SHOW_SELECTOR)
            .expect("the field draws its Show button");
        cx.simulate_click(bounds.center(), Modifiers::default());
    }

    #[gpui::test]
    fn a_revealed_value_shows_while_the_field_has_focus_and_hides_when_focus_leaves(
        cx: &mut TestAppContext,
    ) {
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).masked().revealable().with_text(SECRET));
        focus_and_draw(&field, cx);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let hidden = "•".repeat(SECRET.chars().count());
        assert!(
            painted_text(&field, cx) == hidden,
            "the value starts hidden"
        );

        click_show(cx);
        redraw(cx);
        assert!(
            field.read_with(cx, |field, _| field.revealed),
            "Show reveals the value"
        );
        assert!(
            painted_text(&field, cx) == SECRET,
            "the value shows while the field has focus"
        );

        cx.update(|window, cx| window.blur(cx));
        cx.run_until_parked();
        redraw(cx);
        assert!(
            !field.read_with(cx, |field, _| field.revealed),
            "leaving the field hides the value again"
        );
        assert!(
            painted_text(&field, cx) == hidden,
            "the value is bullets again"
        );
    }

    #[gpui::test]
    fn the_show_button_keeps_focus_in_the_field_and_toggles_the_value(cx: &mut TestAppContext) {
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).masked().revealable().with_text(SECRET));
        focus_and_draw(&field, cx);
        click_show(cx);
        assert!(
            field.update_in(cx, |field, window, _| field.focus_handle.is_focused(window)),
            "the field keeps focus after Show"
        );
        click_show(cx);
        assert!(
            !field.read_with(cx, |field, _| field.revealed),
            "a second click hides the value"
        );
        assert!(
            field.update_in(cx, |field, window, _| field.focus_handle.is_focused(window)),
            "the field keeps focus after the second click"
        );
    }

    #[gpui::test]
    fn a_revealed_value_hides_when_the_window_loses_activation(cx: &mut TestAppContext) {
        let (field, cx) =
            cx.add_window_view(|_, cx| TextField::new(cx).masked().revealable().with_text(SECRET));
        focus_and_draw(&field, cx);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        click_show(cx);
        assert!(
            field.read_with(cx, |field, _| field.revealed),
            "Show reveals the value"
        );
        cx.deactivate_window();
        cx.run_until_parked();
        assert!(
            !field.read_with(cx, |field, _| field.revealed),
            "a window in the background shows no revealed value"
        );
    }

    #[gpui::test]
    fn a_masked_field_that_is_not_revealable_draws_no_show_button(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text(SECRET));
        focus_and_draw(&field, cx);
        redraw(cx);
        assert!(
            cx.debug_bounds(SHOW_SELECTOR).is_none(),
            "no Show button unless the field is revealable"
        );
    }

    #[gpui::test]
    fn insert_pasted_puts_the_clipboard_text_at_the_caret_on_one_line(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("ab"));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| {
            field.insert_pasted("x\ny", window, cx)
        });
        assert_eq!(text_of(&field, cx), "abx y");
    }

    #[gpui::test]
    fn insert_pasted_into_a_masked_field_paints_one_bullet_per_character(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked());
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| {
            field.insert_pasted("ab\ncd", window, cx)
        });
        redraw(cx);
        assert_eq!(text_of(&field, cx), "ab cd");
        assert_eq!(painted_text(&field, cx), "•••••");
    }

    #[gpui::test]
    fn clear_text_empties_the_field_and_reports_one_change(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("oppm"));
        let events = record_events(&field, cx);
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| field.clear_text(window, cx));
        field.update_in(cx, |field, window, cx| field.clear_text(window, cx));
        assert_eq!(text_of(&field, cx), "");
        assert_eq!(
            *events.borrow(),
            vec![TextFieldEvent::Changed],
            "only the first clear changes the text"
        );
    }

    #[gpui::test]
    fn clear_text_empties_a_masked_field(cx: &mut TestAppContext) {
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked().with_text(SECRET));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| field.clear_text(window, cx));
        redraw(cx);
        assert!(text_of(&field, cx).is_empty(), "the masked text is cleared");
        assert!(painted_text(&field, cx).is_empty(), "nothing is painted");
    }

    /// Types `text` one character at a time, as the keyboard does.
    fn typed_buffer(text: &str) -> TextBuffer {
        let mut buffer = TextBuffer::new("");
        for ch in text.chars() {
            buffer.replace_text(None, &ch.to_string());
        }
        buffer
    }

    #[test]
    fn a_run_of_typed_characters_undoes_as_one_step() {
        let mut buffer = typed_buffer("hello");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "");
        assert!(!buffer.undo(), "nothing is left to undo");
    }

    #[test]
    fn redo_replays_the_undone_step_and_a_new_edit_clears_it() {
        let mut buffer = typed_buffer("ab");
        assert!(buffer.undo());
        assert!(buffer.redo());
        assert_eq!(buffer.text(), "ab");
        assert!(!buffer.redo(), "nothing is left to redo");
        assert!(buffer.undo());
        buffer.insert("x");
        assert!(!buffer.redo(), "a new edit drops the redo steps");
    }

    #[test]
    fn paste_and_cut_are_steps_of_their_own() {
        let mut buffer = typed_buffer("ab");
        buffer.insert("xyz");
        buffer.select_all();
        buffer.insert("");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "abxyz", "the cut comes back alone");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "ab", "then the paste");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn a_backspace_run_undoes_as_one_step_and_restores_the_caret() {
        let mut buffer = TextBuffer::new("abc");
        buffer.backspace();
        buffer.backspace();
        assert_eq!(buffer.text(), "a");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "abc");
        assert_eq!(buffer.cursor(), 3);
    }

    #[test]
    fn moving_the_caret_starts_a_new_typing_step() {
        let mut buffer = typed_buffer("ab");
        buffer.move_left();
        buffer.replace_text(None, "x");
        assert_eq!(buffer.text(), "axb");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "ab");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn a_composition_undoes_as_one_step() {
        let mut buffer = TextBuffer::new("");
        buffer.replace_and_mark(None, "n", None);
        buffer.replace_and_mark(None, "ni", None);
        buffer.replace_text(None, "你");
        assert_eq!(buffer.text(), "你");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "");
        assert!(!buffer.undo());
    }

    #[test]
    fn set_text_drops_the_history_only_when_the_text_changes() {
        let mut buffer = typed_buffer("ab");
        assert!(!buffer.set_text("ab"), "an identical text is no change");
        assert!(buffer.undo(), "the history survives an identical set");
        assert_eq!(buffer.text(), "");
        let mut buffer = typed_buffer("ab");
        assert!(buffer.set_text("zz"));
        assert!(!buffer.undo(), "a changed text starts with no history");
        assert_eq!(buffer.text(), "zz");
    }

    #[test]
    fn a_buffer_without_history_records_nothing() {
        let mut buffer = TextBuffer::new("");
        buffer.set_history(false);
        buffer.replace_text(None, "s");
        buffer.insert("ecret");
        assert!(!buffer.undo(), "a masked value keeps no copy of itself");
        assert_eq!(buffer.text(), "secret");
    }

    #[test]
    fn history_keeps_at_most_one_hundred_steps() {
        let mut buffer = TextBuffer::new("");
        for _ in 0..150 {
            buffer.insert("x");
        }
        let undone = std::iter::from_fn(|| buffer.undo().then_some(())).count();
        assert_eq!(undone, 100);
        assert_eq!(buffer.text(), "x".repeat(50));
    }

    #[test]
    fn history_keeps_the_newest_step_even_past_four_mebibytes() {
        let mut buffer = TextBuffer::new("");
        let block = "a".repeat(1 << 20);
        for _ in 0..6 {
            buffer.insert(&block);
        }
        let undone = std::iter::from_fn(|| buffer.undo().then_some(())).count();
        assert_eq!(undone, 1, "only the newest step fits the budget");
        assert_eq!(buffer.text(), "a".repeat(5 << 20));
    }

    #[gpui::test]
    fn undo_and_redo_keys_step_through_typing(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx));
        focus_and_draw(&field, cx);
        cx.simulate_input("ab");
        cx.simulate_keystrokes(UNDO_KEYS);
        assert_eq!(text_of(&field, cx), "");
        cx.simulate_keystrokes(REDO_KEYS);
        assert_eq!(text_of(&field, cx), "ab");
    }

    #[gpui::test]
    fn the_clear_button_is_undoable(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("oppm"));
        focus_and_draw(&field, cx);
        field.update_in(cx, |field, window, cx| field.clear(window, cx));
        assert_eq!(text_of(&field, cx), "");
        cx.simulate_keystrokes(UNDO_KEYS);
        assert_eq!(text_of(&field, cx), "oppm");
    }

    #[gpui::test]
    fn a_masked_field_keeps_no_undo_steps(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).masked());
        focus_and_draw(&field, cx);
        cx.simulate_input("secret");
        cx.simulate_keystrokes(UNDO_KEYS);
        assert_eq!(text_of(&field, cx), "secret");
    }

    #[test]
    fn word_motion_skips_separators_both_ways() {
        let buffer = TextBuffer::new("foo, bar");
        assert_eq!(buffer.next_word_boundary(0), 3);
        assert_eq!(buffer.next_word_boundary(3), 8);
        assert_eq!(buffer.prev_word_boundary(8), 5);
        assert_eq!(buffer.prev_word_boundary(5), 0);
    }

    #[test]
    fn word_motion_keeps_a_combining_mark_with_its_letter() {
        let buffer = TextBuffer::new("e\u{301}x y");
        assert_eq!(buffer.next_word_boundary(0), 4, "é and x are one word");
        assert_eq!(buffer.prev_word_boundary(4), 0);
    }

    #[test]
    fn word_deletes_remove_the_word_before_or_after_the_caret() {
        let mut buffer = TextBuffer::new("one two three");
        assert!(buffer.delete_word_backward());
        assert_eq!(buffer.text(), "one two ");
        assert!(buffer.delete_word_backward());
        assert_eq!(buffer.text(), "one ");
        buffer.move_home();
        assert!(buffer.delete_word_forward());
        assert_eq!(buffer.text(), " ");
    }

    #[test]
    fn a_word_delete_is_a_step_of_its_own_in_the_history() {
        let mut buffer = typed_buffer("one two");
        assert!(buffer.delete_word_backward());
        assert_eq!(buffer.text(), "one ");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "one two");
    }

    #[test]
    fn a_double_click_selects_a_word_or_a_run_of_separators() {
        let mut buffer = TextBuffer::new("hello world");
        buffer.select_word_at(7);
        assert_eq!(buffer.selected(), 6..11);
        buffer.select_word_at(5);
        assert_eq!(buffer.selected(), 5..6, "a single space is its own run");
        buffer.select_word_at(11);
        assert_eq!(
            buffer.selected(),
            6..11,
            "the end of the text picks the last word"
        );
    }

    #[test]
    fn a_triple_click_selects_the_line_without_its_break() {
        let mut buffer = TextBuffer::new("ab\ncd ef");
        buffer.select_line_at(1);
        assert_eq!(buffer.selected(), 0..2);
        buffer.select_line_at(4);
        assert_eq!(buffer.selected(), 3..8);
    }

    fn caret_of(field: &Entity<TextField>, cx: &VisualTestContext) -> usize {
        field.read_with(cx, |field, _| field.buffer.cursor())
    }

    fn selection_of(field: &Entity<TextField>, cx: &VisualTestContext) -> Range<usize> {
        field.read_with(cx, |field, _| field.buffer.selected())
    }

    /// A left-button press at `position` that is the `clicks`th click in a row.
    fn press_at(position: Point<Pixels>, clicks: usize) -> MouseDownEvent {
        MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            click_count: clicks,
            first_mouse: false,
        }
    }

    #[gpui::test]
    fn word_keys_move_select_and_delete_by_the_word(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("one two three"));
        focus_and_draw(&field, cx);
        cx.simulate_keystrokes(platform_keys("alt-left", "ctrl-left"));
        assert_eq!(caret_of(&field, cx), 8);
        cx.simulate_keystrokes(platform_keys("alt-right", "ctrl-right"));
        assert_eq!(caret_of(&field, cx), 13);
        cx.simulate_keystrokes(platform_keys("alt-shift-left", "ctrl-shift-left"));
        assert_eq!(selection_of(&field, cx), 8..13);
        cx.simulate_keystrokes(platform_keys("alt-backspace", "ctrl-backspace"));
        assert_eq!(text_of(&field, cx), "one two ");
        cx.simulate_keystrokes(platform_keys("alt-backspace", "ctrl-backspace"));
        assert_eq!(text_of(&field, cx), "one ");
    }

    #[gpui::test]
    fn home_end_select_all_and_document_keys_span_the_field(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("abc"));
        focus_and_draw(&field, cx);
        cx.simulate_keystrokes("home");
        assert_eq!(caret_of(&field, cx), 0);
        cx.simulate_keystrokes("end");
        assert_eq!(caret_of(&field, cx), 3);
        cx.simulate_keystrokes("shift-home");
        assert_eq!(selection_of(&field, cx), 0..3);
        cx.simulate_keystrokes(platform_keys("cmd-a", "ctrl-a"));
        assert_eq!(selection_of(&field, cx), 0..3);
        cx.simulate_keystrokes(platform_keys("cmd-up", "ctrl-home"));
        assert_eq!(caret_of(&field, cx), 0);
        cx.simulate_keystrokes(platform_keys("cmd-down", "ctrl-end"));
        assert_eq!(caret_of(&field, cx), 3);
    }

    #[gpui::test]
    fn a_double_click_selects_a_word_and_a_triple_click_the_field(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new(cx).with_text("one two three"));
        focus_and_draw(&field, cx);
        let bounds = field.read_with(cx, |field, _| {
            field.last_bounds.expect("the field is painted")
        });
        let position = point(bounds.left() + px(1.), bounds.center().y);
        field.update_in(cx, |field, window, cx| {
            field.mouse_down(&press_at(position, 2), window, cx)
        });
        assert_eq!(selection_of(&field, cx), 0..3);
        field.update_in(cx, |field, window, cx| {
            field.mouse_down(&press_at(position, 3), window, cx)
        });
        assert_eq!(selection_of(&field, cx), 0..13);
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root, view};
    use gpui::{
        RenderOnce, TestAppContext,
        accesskit::{Role, Toggled},
    };

    #[gpui::test]
    fn the_show_button_is_a_toggle_named_show_value(cx: &mut TestAppContext) {
        let hidden = root(cx, |window, cx| {
            reported(show_button(ElementId::from("show"), false, |_, _| {}).render(window, cx))
        })
        .expect("the Show button is reported");
        assert_eq!(hidden.role(), Role::Button);
        assert_eq!(hidden.label(), Some("Show value"));
        assert_eq!(hidden.toggled(), Some(Toggled::False));

        let shown = root(cx, |window, cx| {
            reported(show_button(ElementId::from("show"), true, |_, _| {}).render(window, cx))
        })
        .expect("the Show button is reported");
        assert_eq!(shown.toggled(), Some(Toggled::True));
    }

    #[gpui::test]
    fn a_text_field_is_a_textbox_named_by_its_label_with_its_value(cx: &mut TestAppContext) {
        let node = view(cx, |cx| TextField::new(cx).label("Name").with_text("oppm"))
            .expect("the field is reported");
        assert_eq!(node.role(), Role::TextInput);
        assert_eq!(node.label(), Some("Name"));
        assert_eq!(node.value(), Some("oppm"));
    }

    #[gpui::test]
    fn a_read_only_field_with_an_error_reports_both(cx: &mut TestAppContext) {
        let node = view(cx, |cx| {
            TextField::new(cx)
                .label("Name")
                .read_only(true)
                .error("Required")
        })
        .expect("the field is reported");
        assert!(node.is_read_only());
        assert!(node.invalid().is_some());
        assert_eq!(node.description(), Some("Required"));
    }

    #[gpui::test]
    fn a_search_field_is_a_searchbox(cx: &mut TestAppContext) {
        let node = view(cx, |cx| TextField::new(cx).search()).expect("the field is reported");
        assert_eq!(node.role(), Role::SearchInput);
    }

    #[gpui::test]
    fn a_masked_field_is_a_password_input_with_no_value(cx: &mut TestAppContext) {
        const SECRET: &str = "placeholder-ñ漢🙂";
        let node = view(cx, |cx| {
            TextField::new(cx).label("Value").masked().with_text(SECRET)
        })
        .expect("the field is reported");
        assert_eq!(node.role(), Role::PasswordInput);
        assert_eq!(node.label(), Some("Value"));
        assert!(
            node.value().is_none(),
            "the accessibility tree gets no value"
        );
    }
}

#[cfg(test)]
mod search_name_tests {
    use super::*;
    use crate::a11y::probe::view;
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn a_search_field_is_named_by_its_placeholder(cx: &mut TestAppContext) {
        let node = view(cx, |cx| {
            TextField::new(cx).placeholder("Search files").search()
        })
        .expect("the field is reported");
        assert_eq!(node.role(), Role::SearchInput);
        assert_eq!(node.label(), Some("Search files"));
    }
}
