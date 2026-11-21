//! Multi-line text input for the composer and the queued-message editor. It edits the
//! [`TextBuffer`] of the single-line field and wraps it to the width it is given; the wrapping
//! lives in [`super::layout`] and the element that paints it in `text_area::element`.

mod editing;
mod element;
mod pointer;
mod render;
mod shaping;

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, ElementId, EventEmitter, FocusHandle, Focusable, KeyBinding, Pixels,
    Point, SharedString, Window, actions, point, px,
};

use super::input::{self, InputHost, impl_input_handler};
use super::layout::Shaped;
use super::{TextBuffer, normalize_line_breaks};
use crate::tokens::{SIZE_COMPOSER_MAX_HEIGHT, SIZE_COMPOSER_MIN_HEIGHT};

actions!(
    text_area,
    [
        Backspace,
        Delete,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        MoveRowStart,
        MoveRowEnd,
        SelectRowStart,
        SelectRowEnd,
        SelectAll,
        CopySelection,
        CutSelection,
        PasteText,
        Submit,
        SubmitWithModifier,
        InsertNewline,
        Cancel,
    ]
);

const KEY_CONTEXT: &str = "TextArea";

/// Binds the text area keys under its key context; apps call this once at startup.
pub fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    // `KeyBinding::new` cannot resolve `secondary-`, so Mod is spelled out per OS.
    let [select_all, copy, cut, paste, submit_with_modifier] = if cfg!(target_os = "macos") {
        ["cmd-a", "cmd-c", "cmd-x", "cmd-v", "cmd-enter"]
    } else {
        ["ctrl-a", "ctrl-c", "ctrl-x", "ctrl-v", "ctrl-enter"]
    };
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", MoveLeft, context),
        KeyBinding::new("right", MoveRight, context),
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("home", MoveRowStart, context),
        KeyBinding::new("end", MoveRowEnd, context),
        KeyBinding::new("shift-home", SelectRowStart, context),
        KeyBinding::new("shift-end", SelectRowEnd, context),
        KeyBinding::new(select_all, SelectAll, context),
        KeyBinding::new(copy, CopySelection, context),
        KeyBinding::new(cut, CutSelection, context),
        KeyBinding::new(paste, PasteText, context),
        KeyBinding::new("enter", Submit, context),
        KeyBinding::new(submit_with_modifier, SubmitWithModifier, context),
        KeyBinding::new("shift-enter", InsertNewline, context),
        KeyBinding::new("escape", Cancel, context),
    ]);
}

/// A key the owner decides about: sending, queueing or leaving the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAreaKey {
    /// Plain Enter.
    Enter,
    /// Enter with the primary modifier (Ctrl on Windows and Linux, Command on macOS).
    ModifiedEnter,
    /// Escape.
    Escape,
}

/// What a [`TextArea`] emits; subscribe with `cx.subscribe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAreaEvent {
    /// The text changed by typing, paste, composition, clearing or `set_text`.
    Changed,
    /// A key the owner decides about; the area inserts no text for it.
    Key(TextAreaKey),
    /// Up in an empty area: the owner may restore an earlier draft.
    HistoryPrevious,
    /// Down in an empty area.
    HistoryNext,
}

/// Multi-line text input. It owns its text and focus; the owner subscribes to its events.
#[derive(Debug)]
pub struct TextArea {
    id: ElementId,
    focus_handle: FocusHandle,
    buffer: TextBuffer,
    placeholder: SharedString,
    label: SharedString,
    min_height: f32,
    max_height: f32,
    disabled: bool,
    /// The last paint, for hit tests, the caret and IME geometry.
    shaped: Option<Rc<Shaped>>,
    bounds: Option<Bounds<Pixels>>,
    scroll: Pixels,
    /// Scroll so the caret row is in view on the next paint; set by edits and caret moves.
    ensure_caret: bool,
    selecting: bool,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextArea {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            id: "text-area".into(),
            focus_handle: cx.focus_handle().tab_stop(true),
            buffer: TextBuffer::new(""),
            placeholder: SharedString::default(),
            label: SharedString::default(),
            min_height: SIZE_COMPOSER_MIN_HEIGHT,
            max_height: SIZE_COMPOSER_MAX_HEIGHT,
            disabled: false,
            shaped: None,
            bounds: None,
            scroll: px(0.),
            ensure_caret: false,
            selecting: false,
        }
    }

    /// The element id; an owner with several areas gives each its own.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// The accessible name, for example "Message to Claude".
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = text.into();
        self
    }

    /// Height limits in logical pixels: the area grows from `min` to `max`, then scrolls.
    pub fn heights(mut self, min: f32, max: f32) -> Self {
        self.min_height = min;
        self.max_height = max.max(min);
        self
    }

    pub fn with_text(mut self, text: impl AsRef<str>) -> Self {
        self.buffer = TextBuffer::new(normalize_line_breaks(text.as_ref()));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn cursor(&self) -> usize {
        self.buffer.cursor()
    }

    /// Whether an IME composition is in progress; Enter then confirms it.
    pub fn is_composing(&self) -> bool {
        self.buffer.marked().is_some()
    }

    /// Replaces the whole text without a window; emits `Changed` when the text differed.
    pub fn set_text_in_place(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = normalize_line_breaks(text);
        self.edit_in_place(|buffer| buffer.set_text(&text), cx);
    }

    /// Empties the area without a window, as after a send.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text_in_place("", cx);
    }

    /// Writes a line break at the caret without a window (Enter in the newline setting).
    pub fn insert_newline(&mut self, cx: &mut Context<Self>) {
        self.edit_in_place(|buffer| buffer.insert("\n"), cx);
    }

    /// Applies a change without a window, as the owner's clear and line break do.
    fn edit_in_place(
        &mut self,
        change: impl FnOnce(&mut TextBuffer) -> bool,
        cx: &mut Context<Self>,
    ) {
        if self.editable() {
            self.input_will_edit();
            if change(&mut self.buffer) {
                cx.emit(TextAreaEvent::Changed);
            }
            cx.notify();
        }
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    /// Replaces the text and puts the caret at its end; emits `Changed` only when it differs.
    pub fn set_text(&mut self, text: impl AsRef<str>, window: &mut Window, cx: &mut Context<Self>) {
        let text = normalize_line_breaks(text.as_ref());
        self.input_will_edit();
        let changed = self.buffer.set_text(&text);
        input::after_edit(self, changed, window, cx);
    }

    /// Writes `text` over the selection, as typing does.
    pub fn insert_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = normalize_line_breaks(text);
        self.edit(window, cx, |buffer| buffer.insert(&text));
    }

    /// Moves focus into the area; a disabled area never takes focus.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if !self.disabled {
            window.focus(&self.focus_handle, cx);
        }
    }

    fn editable(&self) -> bool {
        !self.disabled
    }

    /// Applies a text change when the area accepts input.
    fn edit(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut TextBuffer) -> bool,
    ) {
        input::edit(self, window, cx, change);
    }
}

impl InputHost for TextArea {
    fn text_buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    fn text_buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    fn input_accepts(&self) -> bool {
        self.editable()
    }

    fn input_focus(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn input_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Changed);
    }

    fn sanitize_insert(text: &str) -> String {
        normalize_line_breaks(text)
    }

    fn input_will_edit(&mut self) {
        self.ensure_caret = true;
    }

    fn layout_bounds(
        &self,
        range: Range<usize>,
        element_bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let shaped = self.shaped.as_ref()?;
        let start = shaped.point_for_offset(range.start)?;
        let end = shaped.point_for_offset(range.end)?;
        let top = element_bounds.top() - self.scroll;
        Some(Bounds::from_corners(
            point(element_bounds.left() + start.x, top + start.y),
            point(element_bounds.left() + end.x, top + end.y + shaped.line_height),
        ))
    }

    fn offset_at(&self, position: Point<Pixels>) -> Option<usize> {
        let bounds = self.bounds?;
        let shaped = self.shaped.as_ref()?;
        let at = point(
            position.x - bounds.left(),
            position.y - bounds.top() + self.scroll,
        );
        Some(self.buffer.snap_to_grapheme(shaped.offset_for_point(at)))
    }
}

impl_input_handler!(TextArea);
