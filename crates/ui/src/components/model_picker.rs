//! Model picker: the one selector for agent, model, reasoning effort and speed in every composer
//! (agreements A18, A27 and A28). The trigger chip opens a three-column popover above it: an
//! agent rail, a searchable model list and the reasoning levels, with a Standard or Fast footer.
//! Access is the composer's own chip, never part of this picker.

pub mod state;

pub use state::{
    AgentAvailability, AgentOption, Choice, ModelList, ModelOption, PickerState, PickerView, RowRef,
};

use std::fmt;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Render, Role, StatefulInteractiveElement, Styled,
    Subscription, Window, actions, div, px,
};

use crate::components::agent_icon::{AgentIcon, AgentIconSize};
use crate::components::popover::TriggerBounds;
use crate::components::text_field::{TextField, TextFieldEvent};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    LAYOUT_MODEL_PICKER_CHIP_MAX_WIDTH, RADIUS_FULL, SIZE_CONTROL_COMPACT, SPACE_1, SPACE_2,
    TYPE_LABEL,
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .role(Role::Button)
            .aria_label(aria)
            .aria_expanded(open)
            .track_focus(&self.trigger_focus)
            .focus_ring(&theme)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(SPACE_1))
            .h(px(SIZE_CONTROL_COMPACT))
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
                    .child(name),
            )
            .when_some(effort, |chip, effort| {
                chip.child(div().flex_none().text_color(p.text.secondary).child(effort))
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

        let host = self.trigger_bounds.track(div().relative()).child(chip);
        host
    }
}
