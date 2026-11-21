//! Queued messages: follow-up messages waiting for the agent or the host, shown as a dock above
//! the composer (QueuedMessage.md, X-CH-03). Send now asks inline first, Edit swaps the text for
//! an inline editor, and Remove reports upward so the owner can offer Undo. The queue itself
//! belongs to the session; the dock only keeps the transient inline steps.

pub mod state;

pub use state::{
    QueueStatus, QueueView, QueuedItem, QueuedMessagesEvent, RowStep, WaitReason, caption,
    focus_after_removal, header_label,
};

use std::fmt;

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, Role, SharedString, Stateful, StatefulInteractiveElement, Styled, Subscription, Window,
    div, px,
};

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::icon_button::IconButton;
use crate::components::text_editing::text_area::{TextArea, TextAreaEvent, TextAreaKey};
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    RADIUS_LG, RADIUS_MD, SIZE_COMPOSER_MAX_HEIGHT, SIZE_CONTROL_COMFORTABLE, SPACE_1, SPACE_2,
    SPACE_3, TYPE_BODY, TYPE_BODY_SM, TYPE_CAPTION, TYPE_LABEL,
};

/// The rows scroll inside the dock past this height (QueuedMessage.md, Several).
const LIST_MAX_HEIGHT: f32 = 160.0;

/// The queued-message dock: a list of waiting messages above the composer.
pub struct QueuedMessages {
    items: Vec<QueuedItem>,
    view: QueueView,
    send_now_enabled: bool,
    editor: Option<Entity<TextArea>>,
    _editor_events: Option<Subscription>,
}

impl fmt::Debug for QueuedMessages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueuedMessages")
            .field("items", &self.items)
            .field("view", &self.view)
            .field("send_now_enabled", &self.send_now_enabled)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<QueuedMessagesEvent> for QueuedMessages {}

impl Default for QueuedMessages {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuedMessages {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            view: QueueView::default(),
            send_now_enabled: true,
            editor: None,
            _editor_events: None,
        }
    }

    pub fn items(&self) -> &[QueuedItem] {
        &self.items
    }

    pub fn view(&self) -> &QueueView {
        &self.view
    }

    /// Replaces the queue with the session's; a step on a row that left the queue is dropped.
    pub fn set_items(&mut self, items: Vec<QueuedItem>, cx: &mut Context<Self>) {
        let ids: Vec<u64> = items.iter().map(|item| item.id).collect();
        self.items = items;
        self.view.retain(&ids);
        if self.view.step().is_none() {
            self.editor = None;
            self._editor_events = None;
        }
        cx.notify();
    }

    /// Whether Send now is offered at all (the session is not archived and can run).
    pub fn set_send_now_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.send_now_enabled = enabled;
        cx.notify();
    }

    fn send_now_allowed(&self, item: &QueuedItem) -> bool {
        self.send_now_enabled && matches!(item.status, QueueStatus::Waiting(WaitReason::AgentBusy))
    }

    /// Opens the inline "Send now?" question on a row.
    pub fn begin_confirm(&mut self, id: u64, cx: &mut Context<Self>) {
        self.editor = None;
        self._editor_events = None;
        self.view.begin(id, RowStep::Confirm);
        cx.notify();
    }

    fn cancel_step(&mut self, cx: &mut Context<Self>) {
        self.view.end_step();
        self.editor = None;
        self._editor_events = None;
        cx.notify();
    }

    fn confirm_send_now(&mut self, id: u64, cx: &mut Context<Self>) {
        self.view.end_step();
        cx.emit(QueuedMessagesEvent::SendNow(id));
        cx.notify();
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.view.step().is_some_and(|(open, _)| open == id) {
            self.cancel_step(cx);
        }
        cx.emit(QueuedMessagesEvent::Removed(id));
    }

    /// Starts editing a row: its text becomes an inline editor with focus.
    pub fn begin_edit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.text.clone())
        else {
            return;
        };
        let editor = cx.new(|cx| {
            TextArea::new(cx)
                .id(("queued-editor", id as usize))
                .label("Edit queued message")
                .with_text(text.as_ref())
                .heights(SIZE_CONTROL_COMFORTABLE, SIZE_COMPOSER_MAX_HEIGHT)
        });
        self._editor_events =
            Some(
                cx.subscribe(&editor, |this, _, event: &TextAreaEvent, cx| match event {
                    TextAreaEvent::Key(TextAreaKey::Enter) => this.save_edit(cx),
                    TextAreaEvent::Key(TextAreaKey::Escape) => this.cancel_step(cx),
                    _ => {}
                }),
            );
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        self.editor = Some(editor);
        self.view.begin(id, RowStep::Edit);
        cx.notify();
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some((id, RowStep::Edit)) = self.view.step() else {
            return;
        };
        let text = self
            .editor
            .as_ref()
            .map(|editor| SharedString::from(editor.read(cx).text().to_owned()));
        self.view.end_step();
        self.editor = None;
        self._editor_events = None;
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            cx.emit(QueuedMessagesEvent::Edited { id, text });
        }
        cx.notify();
    }

    fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        self.view.toggle();
        cx.notify();
    }

    fn row(&self, index: usize, item: &QueuedItem, cx: &mut Context<Self>) -> Stateful<gpui::Div> {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = item.id;
        let dock = cx.entity();
        let reason = caption(&item.status);
        let step = self
            .view
            .step()
            .filter(|(open, _)| *open == id)
            .map(|(_, step)| step);
        let row = div()
            .id(SharedString::from(format!("queued-row-{id}")))
            .role(Role::ListItem)
            .aria_label(item.text.clone())
            .aria_description(reason.clone())
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .min_h(px(SIZE_CONTROL_COMFORTABLE))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .hover(move |s| s.bg(p.fill.hover_raised))
            .when(index > 0, |row| {
                row.border_t_1().border_color(p.border.subtle)
            });
        match step {
            Some(RowStep::Confirm) => row.child(self.confirm(id, dock, cx)),
            Some(RowStep::Edit) => row.child(self.editor_view(id, dock, cx)),
            None => {
                let (leading, reason_color) = match &item.status {
                    QueueStatus::Waiting(WaitReason::AgentBusy) => (
                        Icon::new(IconName::Clock).color(p.text.tertiary),
                        p.text.tertiary,
                    ),
                    QueueStatus::Waiting(WaitReason::HostUnconfirmed) => (
                        Icon::new(IconName::CloudOff).color(p.agent.unreachable),
                        p.text.tertiary,
                    ),
                    QueueStatus::Sending => (
                        Icon::new(IconName::LoaderCircle).color(p.agent.running),
                        p.text.tertiary,
                    ),
                    QueueStatus::Failed(_) => (
                        Icon::new(IconName::OctagonAlert).color(p.status.danger_fg),
                        p.status.danger_fg,
                    ),
                };
                let text_block = div()
                    .flex()
                    .flex_1()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_style(TYPE_BODY)
                            .text_color(p.text.primary)
                            .child(item.text.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_style(TYPE_CAPTION)
                            .text_color(reason_color)
                            .child(reason),
                    );
                row.child(leading.size(IconSize::Small))
                    .child(text_block)
                    .child(self.actions(item, dock))
            }
        }
    }

    /// Send now, Edit and Remove; a failed message offers Retry and Remove instead.
    fn actions(&self, item: &QueuedItem, dock: Entity<Self>) -> gpui::Div {
        let id = item.id;
        let sending = matches!(item.status, QueueStatus::Sending);
        let remove_dock = dock.clone();
        let remove = IconButton::new(("queued-remove", id as usize), IconName::X, "Remove")
            .size(ButtonSize::Compact)
            .on_click(move |_, cx| {
                remove_dock.update(cx, |this, cx| this.remove(id, cx));
            });
        let mut actions = div().flex().flex_none().items_center().gap(px(SPACE_1));
        if let QueueStatus::Failed(_) = item.status {
            let retry_dock = dock.clone();
            actions = actions.child(
                Button::new(("queued-retry", id as usize), "Retry")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Compact)
                    .on_click(move |_, cx| {
                        retry_dock.update(cx, |_, cx| cx.emit(QueuedMessagesEvent::Retry(id)));
                    }),
            );
            return actions.child(remove);
        }
        let send_dock = dock.clone();
        let edit_dock = dock;
        let send = IconButton::new(("queued-send", id as usize), IconName::ArrowUp, "Send now")
            .size(ButtonSize::Compact)
            .disabled(!self.send_now_allowed(item) || sending)
            .disabled_reason(if self.send_now_enabled {
                "Waits for VPS to confirm"
            } else {
                "The session cannot run right now"
            })
            .on_click(move |_, cx| {
                send_dock.update(cx, |this, cx| this.begin_confirm(id, cx));
            });
        let edit = IconButton::new(("queued-edit", id as usize), IconName::Pencil, "Edit")
            .size(ButtonSize::Compact)
            .disabled(sending)
            .on_click(move |window, cx| {
                edit_dock.update(cx, |this, cx| this.begin_edit(id, window, cx));
            });
        actions.child(send).child(edit).child(remove)
    }

    /// "Send now?" with Cancel (focused first) and Send now.
    fn confirm(&self, id: u64, dock: Entity<Self>, cx: &mut Context<Self>) -> gpui::Div {
        let p = cx.theme().palette;
        let cancel_dock = dock.clone();
        let send_dock = dock;
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(SPACE_2))
            .child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child("Send now? The agent will stop what it is doing and read this."),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(SPACE_2))
                    .child(
                        Button::new(("queued-confirm-cancel", id as usize), "Cancel")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Compact)
                            .on_click(move |_, cx| {
                                cancel_dock.update(cx, |this, cx| this.cancel_step(cx));
                            }),
                    )
                    .child(
                        Button::new(("queued-confirm-send", id as usize), "Send now")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Compact)
                            .on_click(move |_, cx| {
                                send_dock.update(cx, |this, cx| this.confirm_send_now(id, cx));
                            }),
                    ),
            )
    }

    /// The inline editor on a sunken well, with Cancel and Save.
    fn editor_view(&self, id: u64, dock: Entity<Self>, cx: &mut Context<Self>) -> gpui::Div {
        let p = cx.theme().palette;
        let cancel_dock = dock.clone();
        let save_dock = dock;
        let mut column = div().flex_1().flex().flex_col().gap(px(SPACE_2));
        if let Some(editor) = &self.editor {
            column = column.child(
                div()
                    .rounded(px(RADIUS_MD))
                    .bg(p.surface.sunken)
                    .border_1()
                    .border_color(p.border.strong)
                    .px(px(SPACE_2))
                    .py(px(SPACE_1))
                    .child(editor.clone()),
            );
        }
        column.child(
            div()
                .flex()
                .justify_end()
                .gap(px(SPACE_2))
                .child(
                    Button::new(("queued-edit-cancel", id as usize), "Cancel")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Compact)
                        .on_click(move |_, cx| {
                            cancel_dock.update(cx, |this, cx| this.cancel_step(cx));
                        }),
                )
                .child(
                    Button::new(("queued-edit-save", id as usize), "Save")
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Compact)
                        .on_click(move |_, cx| {
                            save_dock.update(cx, |this, cx| this.save_edit(cx));
                        }),
                ),
        )
    }
}

impl Render for QueuedMessages {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let len = self.items.len();
        let mut dock = div()
            .id("queued-messages")
            .role(Role::List)
            .aria_label("Queued messages")
            .flex()
            .flex_col()
            .w_full()
            .rounded(px(RADIUS_LG))
            .bg(p.surface.raised)
            .border_1()
            .border_color(p.border.default)
            .shadow(box_shadows(theme.shadows.e1));
        if len == 0 {
            return dock.hidden();
        }
        if let Some(label) = header_label(len) {
            let expanded = self.view.expanded();
            let toggle_dock = cx.entity();
            dock = dock.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(SPACE_3))
                    .py(px(SPACE_1))
                    .text_style(TYPE_LABEL)
                    .text_color(p.text.secondary)
                    .child(label)
                    .child(
                        IconButton::new(
                            "queued-toggle",
                            if expanded {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            },
                            if expanded {
                                "Collapse queue"
                            } else {
                                "Expand queue"
                            },
                        )
                        .size(ButtonSize::Compact)
                        .on_click(move |_, cx| {
                            toggle_dock.update(cx, |this, cx| this.toggle_expanded(cx));
                        }),
                    ),
            );
        }
        let visible = self.view.visible(len);
        let mut list = div()
            .id("queued-list")
            .flex()
            .flex_col()
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll();
        for (index, item) in self.items.iter().take(visible).enumerate() {
            list = list.child(self.row(index, item, cx));
        }
        dock = dock.child(list);
        let hidden = self.view.hidden(len);
        if hidden > 0 {
            let expand_dock = cx.entity();
            dock = dock.child(
                div()
                    .id("queued-more")
                    .px(px(SPACE_3))
                    .py(px(SPACE_2))
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.fill.hover_raised))
                    .on_click(move |_, _, cx| {
                        expand_dock.update(cx, |this, cx| {
                            this.view.expand();
                            cx.notify();
                        });
                    })
                    .child(format!("+{hidden} more")),
            );
        }
        dock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};
    use std::{cell::RefCell, rc::Rc};

    fn item(id: u64, status: QueueStatus) -> QueuedItem {
        QueuedItem {
            id,
            text: format!("message {id}").into(),
            status,
        }
    }

    fn busy(id: u64) -> QueuedItem {
        item(id, QueueStatus::Waiting(WaitReason::AgentBusy))
    }

    #[test]
    fn a_failed_message_is_not_offered_send_now_while_waiting() {
        let dock = QueuedMessages {
            send_now_enabled: true,
            ..QueuedMessages::new()
        };
        assert!(dock.send_now_allowed(&busy(1)));
        assert!(!dock.send_now_allowed(&item(2, QueueStatus::Sending)));
        assert!(
            !dock.send_now_allowed(&item(3, QueueStatus::Waiting(WaitReason::HostUnconfirmed)))
        );
    }

    /// Subscribes to a dock and collects its events.
    fn record(
        dock: &Entity<QueuedMessages>,
        cx: &mut VisualTestContext,
    ) -> Rc<RefCell<Vec<QueuedMessagesEvent>>> {
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(dock, move |_, event: &QueuedMessagesEvent, _| {
                sink.borrow_mut().push(event.clone())
            })
            .detach();
        });
        events
    }

    #[gpui::test]
    fn send_now_asks_first_and_sends_only_on_confirm(cx: &mut TestAppContext) {
        let (dock, cx) = cx.add_window_view(|_, _| QueuedMessages::new());
        dock.update_in(cx, |dock, _, cx| dock.set_items(vec![busy(7)], cx));
        let events = record(&dock, cx);
        dock.update(cx, |dock, cx| dock.begin_confirm(7, cx));
        assert!(events.borrow().is_empty(), "the first click only asks");
        dock.update(cx, |dock, cx| dock.confirm_send_now(7, cx));
        assert_eq!(*events.borrow(), vec![QueuedMessagesEvent::SendNow(7)]);
    }

    #[gpui::test]
    fn editing_saves_on_enter_and_emits_the_new_text(cx: &mut TestAppContext) {
        cx.update(crate::components::text_editing::text_area::bind_keys);
        let (dock, cx) = cx.add_window_view(|_, _| QueuedMessages::new());
        dock.update_in(cx, |dock, _, cx| dock.set_items(vec![busy(3)], cx));
        let events = record(&dock, cx);
        dock.update_in(cx, |dock, window, cx| dock.begin_edit(3, window, cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("end");
        cx.simulate_input("!");
        cx.simulate_keystrokes("enter");
        assert_eq!(
            *events.borrow(),
            vec![QueuedMessagesEvent::Edited {
                id: 3,
                text: "message 3!".into()
            }]
        );
        assert_eq!(dock.read_with(cx, |dock, _| dock.view().step()), None);
    }

    #[gpui::test]
    fn escape_cancels_an_edit_without_emitting(cx: &mut TestAppContext) {
        cx.update(crate::components::text_editing::text_area::bind_keys);
        let (dock, cx) = cx.add_window_view(|_, _| QueuedMessages::new());
        dock.update_in(cx, |dock, _, cx| dock.set_items(vec![busy(5)], cx));
        let events = record(&dock, cx);
        dock.update_in(cx, |dock, window, cx| dock.begin_edit(5, window, cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        assert!(events.borrow().is_empty());
        assert_eq!(dock.read_with(cx, |dock, _| dock.view().step()), None);
    }
}
