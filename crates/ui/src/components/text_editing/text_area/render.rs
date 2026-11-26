//! The text area's view: the key context, the actions its keys trigger, the pointer and wheel
//! handlers, and the element that paints the text.

use gpui::prelude::FluentBuilder;
use gpui::{
    Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    Role, StatefulInteractiveElement, Styled, Window, div,
};

use super::element::TextAreaElement;
use super::{
    Backspace, Cancel, CopySelection, CutSelection, Delete, InsertNewline, KEY_CONTEXT,
    MoveDocumentEnd, MoveDocumentStart, MoveDown, MoveLeft, MoveRight, MoveRowEnd, MoveRowStart,
    MoveUp, PasteText, SelectAll, SelectDocumentEnd, SelectDocumentStart, SelectDown, SelectLeft,
    SelectRight, SelectRowEnd, SelectRowStart, SelectUp, Submit, SubmitWithModifier, TextArea,
    TextAreaEvent, TextAreaKey,
};
use crate::components::text_editing::{
    DeleteWordBackward, DeleteWordForward, MoveWordLeft, MoveWordRight, Redo, SelectWordLeft,
    SelectWordRight, TextBuffer, Undo,
};

impl Render for TextArea {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(self.id.clone())
            .w_full()
            .key_context(KEY_CONTEXT)
            .role(Role::MultilineTextInput)
            .aria_label(self.label.clone())
            .aria_placeholder(self.placeholder.clone())
            .when(!self.disabled, |area| {
                area.track_focus(&self.focus_handle)
                    .cursor(CursorStyle::IBeam)
            })
            .on_action(cx.listener(|this, _: &Backspace, window, cx| {
                this.edit(window, cx, TextBuffer::backspace);
            }))
            .on_action(cx.listener(|this, _: &Delete, window, cx| {
                this.edit(window, cx, TextBuffer::delete);
            }))
            .on_action(cx.listener(|this, _: &Undo, window, cx| {
                this.edit(window, cx, TextBuffer::undo);
            }))
            .on_action(cx.listener(|this, _: &Redo, window, cx| {
                this.edit(window, cx, TextBuffer::redo);
            }))
            .on_action(cx.listener(|this, _: &MoveWordLeft, window, cx| {
                this.move_caret(TextBuffer::move_word_left, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveWordRight, window, cx| {
                this.move_caret(TextBuffer::move_word_right, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectWordLeft, window, cx| {
                this.move_caret(TextBuffer::select_word_left, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectWordRight, window, cx| {
                this.move_caret(TextBuffer::select_word_right, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &DeleteWordBackward, window, cx| {
                this.edit(window, cx, TextBuffer::delete_word_backward);
            }))
            .on_action(cx.listener(|this, _: &DeleteWordForward, window, cx| {
                this.edit(window, cx, TextBuffer::delete_word_forward);
            }))
            .on_action(cx.listener(|this, _: &MoveDocumentStart, window, cx| {
                this.move_caret(TextBuffer::move_home, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveDocumentEnd, window, cx| {
                this.move_caret(TextBuffer::move_end, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectDocumentStart, window, cx| {
                this.move_caret(TextBuffer::select_home, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectDocumentEnd, window, cx| {
                this.move_caret(TextBuffer::select_end, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveLeft, window, cx| {
                this.move_caret(TextBuffer::move_left, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveRight, window, cx| {
                this.move_caret(TextBuffer::move_right, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectLeft, window, cx| {
                this.move_caret(TextBuffer::select_left, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectRight, window, cx| {
                this.move_caret(TextBuffer::select_right, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveUp, window, cx| {
                this.move_vertical(false, false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveDown, window, cx| {
                this.move_vertical(true, false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectUp, window, cx| {
                this.move_vertical(false, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectDown, window, cx| {
                this.move_vertical(true, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveRowStart, window, cx| {
                this.move_row_edge(false, false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveRowEnd, window, cx| {
                this.move_row_edge(true, false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectRowStart, window, cx| {
                this.move_row_edge(false, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectRowEnd, window, cx| {
                this.move_row_edge(true, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.move_caret(TextBuffer::select_all, false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CopySelection, _, cx| {
                this.copy_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &CutSelection, window, cx| {
                this.cut_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &PasteText, window, cx| {
                this.paste_text(window, cx);
            }))
            .on_action(cx.listener(|this, _: &InsertNewline, window, cx| {
                this.edit(window, cx, |buffer| buffer.insert("\n"));
            }))
            .on_action(cx.listener(|_, _: &Submit, _, cx| {
                cx.emit(TextAreaEvent::Key(TextAreaKey::Enter));
            }))
            .on_action(cx.listener(|_, _: &SubmitWithModifier, _, cx| {
                cx.emit(TextAreaEvent::Key(TextAreaKey::ModifiedEnter));
            }))
            .on_action(cx.listener(|_, _: &Cancel, _, cx| {
                cx.emit(TextAreaEvent::Key(TextAreaKey::Escape));
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_scroll(cx.listener(Self::scroll_wheel))
            .child(TextAreaElement { input: cx.entity() })
    }
}
