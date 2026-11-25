//! Model picker: the one selector for agent, model, reasoning effort and speed in every composer
//! (agreements A18, A27 and A28). The trigger chip opens a three-column popover above it: an
//! agent rail, a searchable model list and the reasoning levels, with a Standard or Fast footer.
//! Access is the composer's own chip, never part of this picker.

pub mod state;

mod columns;

pub use state::{
    AgentAvailability, AgentOption, Choice, ModelList, ModelOption, PickerState, PickerView, RowRef,
};

use std::fmt;

use gpui::prelude::FluentBuilder;
use gpui::{
    Anchor, App, AppContext, ClickEvent, Context, Div, Entity, EventEmitter, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyBinding, MouseDownEvent, ParentElement, Render,
    Role, StatefulInteractiveElement, Styled, Subscription, Window, actions, anchored, deferred,
    div, point, px,
};

use crate::components::agent_icon::{AgentIcon, AgentIconSize};
use crate::components::popover::{OverlayLayer, TriggerBounds, fade_in, overlay_surface};
use crate::components::text_field::{TextField, TextFieldEvent};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    LAYOUT_MODEL_PICKER_CHIP_MAX_WIDTH, LAYOUT_MODEL_PICKER_MAX_HEIGHT, LAYOUT_MODEL_PICKER_WIDTH,
    RADIUS_FULL, RADIUS_XL, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, TYPE_LABEL,
};

actions!(
    model_picker,
    [
        Close,
        MoveUp,
        MoveDown,
        ToggleFavourite,
        Shortcut1,
        Shortcut2,
        Shortcut3,
        Shortcut4,
    ]
);

const KEY_CONTEXT: &str = "ModelPicker";

/// Binds the picker's keys under its key context; apps call this once at startup.
pub fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    // `KeyBinding::new` cannot resolve `secondary-`, so Mod is spelled out per OS.
    let [one, two, three, four] = if cfg!(target_os = "macos") {
        ["cmd-1", "cmd-2", "cmd-3", "cmd-4"]
    } else {
        ["ctrl-1", "ctrl-2", "ctrl-3", "ctrl-4"]
    };
    cx.bind_keys([
        KeyBinding::new("escape", Close, context),
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("alt-enter", ToggleFavourite, context),
        KeyBinding::new(one, Shortcut1, context),
        KeyBinding::new(two, Shortcut2, context),
        KeyBinding::new(three, Shortcut3, context),
        KeyBinding::new(four, Shortcut4, context),
    ]);
}

/// What the picker reports to its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelPickerEvent {
    /// The model, reasoning level or speed changed.
    Changed(Choice),
}

/// The model picker: a trigger chip and its popover. The owner subscribes to its events.
pub struct ModelPicker {
    state: PickerState,
    open: bool,
    trigger_focus: FocusHandle,
    trigger_bounds: TriggerBounds,
    search: Entity<TextField>,
    _search_events: Subscription,
}

impl fmt::Debug for ModelPicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelPicker")
            .field("state", &self.state)
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<ModelPickerEvent> for ModelPicker {}
impl ModelPicker {
    pub fn new(state: PickerState, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("Search models")
                .comfortable()
                .leading_icon(IconName::Search)
        });
        let search_events =
            cx.subscribe(&search, |this, _, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed => {
                    let query = this.search.read(cx).text().to_owned();
                    this.state.set_query(&query);
                    cx.notify();
                }
                TextFieldEvent::Submitted => {
                    if let Some(row) = this.state.highlighted_row() {
                        this.choose_row(row, cx);
                    }
                    this.open = false;
                    cx.notify();
                }
            });
        Self {
            state,
            open: false,
            trigger_focus: cx.focus_handle().tab_stop(true),
            trigger_bounds: TriggerBounds::default(),
            search,
            _search_events: search_events,
        }
    }

    pub fn state(&self) -> &PickerState {
        &self.state
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the popover and moves focus into its search field.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
        cx.notify();
    }

    /// Closes the popover and returns focus to the trigger.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        window.focus(&self.trigger_focus, cx);
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn emit_change(&self, cx: &mut Context<Self>) {
        cx.emit(ModelPickerEvent::Changed(self.state.choice().clone()));
    }

    fn choose_row(&mut self, row: RowRef, cx: &mut Context<Self>) {
        if self.state.select_model(row) {
            self.emit_change(cx);
        }
        cx.notify();
    }
}

impl Render for ModelPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let (name, effort) = self.state.chip_labels();
        let kind = self.state.current_kind();
        let open = self.open;
        let aria = match &effort {
            Some(effort) => format!("Model: {name}, {effort}"),
            None => format!("Model: {name}"),
        };
        let chip = div()
            .id("model-picker-trigger")
            .debug_selector(|| "model-picker-trigger".to_owned())
            .role(Role::Button)
            .aria_label(aria)
            .aria_expanded(open)
            .track_focus(&self.trigger_focus)
            .focus_ring(&theme)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(SPACE_1))
            .h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_FULL))
            .text_style(TYPE_LABEL)
            .text_color(p.text.primary)
            .cursor_pointer()
            .hover(move |s| s.bg(p.fill.hover_raised))
            .when(open, |chip| chip.bg(p.fill.selected_raised))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle(window, cx)))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "down" && !this.open {
                    this.open(window, cx);
                }
            }))
            .child(AgentIcon::new(kind).size(AgentIconSize::Small))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .max_w(px(LAYOUT_MODEL_PICKER_CHIP_MAX_WIDTH))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .when_some(effort, |chip, effort| {
                chip.child(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(p.text.secondary)
                        .child(effort),
                )
            })
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(IconSize::XSmall)
                .color(p.text.tertiary),
            );

        // The tracked box hugs the chip, so the popover aligns to the chip's right edge even when
        // the host is stretched by a column parent.
        let anchor = self.trigger_bounds.track(div().flex_none()).child(chip);
        let host = div().relative().flex().child(anchor);
        let overlay = open.then(|| self.overlay(window, cx));
        host.when_some(overlay, |host, overlay| host.child(overlay))
    }
}

impl ModelPicker {
    /// The popover, anchored above the trigger and right-aligned to it. It is deferred so it
    /// paints over the composer and the transcript.
    fn overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let panel = self.panel(cx);
        let layer = match self.trigger_bounds.get() {
            Some(target) => anchored()
                .attach(target, Anchor::TopRight)
                .anchor(Anchor::BottomRight)
                .offset(point(px(0.), px(-SPACE_2))),
            None => {
                window.request_animation_frame();
                anchored().anchor(Anchor::BottomRight)
            }
        };
        // The dialog role sits on a stateful wrapper; the animated panel is its child.
        let dialog = div()
            .id("model-picker-dialog")
            .role(Role::Dialog)
            .aria_label("Choose model")
            .child(fade_in("model-picker-enter", panel, cx));
        deferred(layer.child(dialog))
            .priority(OverlayLayer::Page.priority())
            .into_any_element()
    }

    fn panel(&self, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let p = theme.palette;
        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .child(self.rail(cx))
            .child(self.model_column(cx))
            .child(self.reasoning_column(cx));
        let footer = self.footer(cx);
        let trigger = self.trigger_bounds.clone();
        overlay_surface(&theme, RADIUS_XL, theme.shadows.e2)
            .key_context(KEY_CONTEXT)
            .occlude()
            .w(px(LAYOUT_MODEL_PICKER_WIDTH))
            .max_h(px(LAYOUT_MODEL_PICKER_MAX_HEIGHT))
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_color(p.text.primary)
            .on_mouse_down_out(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                // A click on the trigger is the trigger's own toggle.
                let on_trigger = trigger
                    .get()
                    .is_some_and(|bounds| bounds.contains(&event.position));
                if !on_trigger && this.open {
                    this.open = false;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Close, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &MoveUp, _, cx| {
                this.state.move_highlight(-1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MoveDown, _, cx| {
                this.state.move_highlight(1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleFavourite, _, cx| {
                if let Some(row) = this.state.highlighted_row()
                    && let Some(id) = this.state.model(row).map(|model| model.id.clone())
                {
                    this.state.toggle_favourite(&id);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Shortcut1, _, cx| this.shortcut(1, cx)))
            .on_action(cx.listener(|this, _: &Shortcut2, _, cx| this.shortcut(2, cx)))
            .on_action(cx.listener(|this, _: &Shortcut3, _, cx| this.shortcut(3, cx)))
            .on_action(cx.listener(|this, _: &Shortcut4, _, cx| this.shortcut(4, cx)))
            .child(body)
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::state::fixtures::sample;
    use super::*;

    use gpui::{TestAppContext, VisualTestContext};
    use std::{cell::RefCell, rc::Rc};

    /// Subscribes to a picker and collects the choices it reports.
    fn record_choices(
        picker: &Entity<ModelPicker>,
        cx: &mut VisualTestContext,
    ) -> Rc<RefCell<Vec<Choice>>> {
        let choices = Rc::new(RefCell::new(Vec::new()));
        let sink = choices.clone();
        cx.update(|_, cx| {
            cx.subscribe(picker, move |_, event: &ModelPickerEvent, _| {
                let ModelPickerEvent::Changed(choice) = event;
                sink.borrow_mut().push(choice.clone());
            })
            .detach();
        });
        choices
    }

    fn picker_in(cx: &mut TestAppContext) -> (Entity<ModelPicker>, &mut VisualTestContext) {
        cx.update(|cx| {
            crate::components::text_field::bind_keys(cx);
            bind_keys(cx);
        });
        cx.add_window_view(|_, cx| ModelPicker::new(sample(), cx))
    }

    /// Opens the popover, focuses its search and redraws so its key bindings are live.
    fn open_and_draw(picker: &Entity<ModelPicker>, cx: &mut VisualTestContext) {
        picker.update_in(cx, |picker, window, cx| picker.open(window, cx));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn the_chip_is_as_tall_as_the_composer_row(cx: &mut TestAppContext) {
        let (_picker, cx) = picker_in(cx);
        cx.run_until_parked();
        let chip = cx
            .debug_bounds("model-picker-trigger")
            .expect("the chip is drawn");
        assert_eq!(chip.size.height, px(SIZE_CONTROL_DEFAULT));
    }

    #[gpui::test]
    fn the_trigger_opens_the_popover_and_escape_closes_it(cx: &mut TestAppContext) {
        let (picker, cx) = picker_in(cx);
        open_and_draw(&picker, cx);
        assert!(picker.read_with(cx, |picker, _| picker.is_open()));
        cx.simulate_keystrokes("escape");
        assert!(!picker.read_with(cx, |picker, _| picker.is_open()));
    }

    #[gpui::test]
    fn typing_in_the_search_filters_the_rows(cx: &mut TestAppContext) {
        let (picker, cx) = picker_in(cx);
        open_and_draw(&picker, cx);
        cx.simulate_input("gpt");
        let rows = picker.read_with(cx, |picker, _| picker.state().rows().len());
        assert_eq!(rows, 1);
    }

    #[gpui::test]
    fn enter_in_the_search_chooses_the_highlighted_row_and_closes(cx: &mut TestAppContext) {
        let (picker, cx) = picker_in(cx);
        let choices = record_choices(&picker, cx);
        open_and_draw(&picker, cx);
        cx.simulate_input("gpt");
        cx.simulate_keystrokes("enter");
        assert_eq!(
            choices.borrow().last().map(|c| c.model.to_string()),
            Some("gpt".into())
        );
        assert!(!picker.read_with(cx, |picker, _| picker.is_open()));
    }

    #[gpui::test]
    fn mod_digit_chooses_the_shortcut_row_while_open(cx: &mut TestAppContext) {
        let (picker, cx) = picker_in(cx);
        let choices = record_choices(&picker, cx);
        open_and_draw(&picker, cx);
        cx.simulate_keystrokes("secondary-2");
        assert_eq!(
            choices.borrow().last().map(|c| c.model.to_string()),
            Some("sonnet".into())
        );
        assert!(picker.read_with(cx, |picker, _| picker.is_open()));
    }
}
