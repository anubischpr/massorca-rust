//! The picker's columns: the agent rail, the model list with its search, the reasoning levels and
//! the Standard or Fast footer. Each draws the picker's state and reports the choice back.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ClickEvent, Context, Div, ElementId, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Role, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window, div, px, svg,
};

use super::ModelPicker;
use super::reasoning_slider::effects::{
    clock_seconds, effects_fade_seconds, fade_progress, halo_fade_seconds, move_progress,
};
use super::reasoning_slider::{
    ReasoningSlider, brand_colours, has_slider, is_top_level, name_colour,
};
use super::state::{AgentAvailability, PickerState, PickerView, RowRef};
use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind, NameTooltip};
use crate::components::button::ButtonSize;
use crate::components::icon_button::IconButton;
use crate::components::kbd::Kbd;
use crate::components::segmented_control::{
    Segment, SegmentedControl, SegmentedSize, SegmentedStyle,
};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    LAYOUT_MODEL_PICKER_MODEL_COLUMN, LAYOUT_MODEL_PICKER_PROVIDER_COLUMN,
    LAYOUT_MODEL_PICKER_REASONING_COLUMN, Palette, RADIUS_MD, SIZE_CONTROL_COMFORTABLE,
    SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_3, SPACE_HALF, TYPE_BODY, TYPE_BODY_STRONG,
    TYPE_CAPTION, TYPE_LABEL, TYPE_TITLE,
};

/// The reasoning column's height: the popover does not grow for the slider (216 by 304 px).
const REASONING_COLUMN_HEIGHT: f32 = 304.0;

/// The sparkles glyph that marks the top level, in px.
const SPARKLE_SIZE: f32 = 14.0;

/// Why Fast is off for a model that offers none, when the service gave no reason of its own.
const NO_FAST_MODEL: &str = "This model has no Fast mode.";

/// What one model row shows.
struct ModelRow {
    row: RowRef,
    position: usize,
    kind: AgentKind,
    id: SharedString,
    name: SharedString,
    current: bool,
    favourite: bool,
    highlighted: bool,
    shortcut: Option<String>,
}

impl ModelPicker {
    /// Chooses a reasoning level. The popover stays open: only Escape or an outside click closes it.
    pub(super) fn choose_effort(&mut self, index: usize, cx: &mut Context<Self>) {
        let before = self.state.choice().effort;
        if self.state.select_effort(index) {
            self.note_level_change(before);
            self.emit_change(cx);
        }
        cx.notify();
    }

    pub(super) fn set_speed(&mut self, fast: bool, cx: &mut Context<Self>) {
        if self.state.set_fast(fast) {
            self.emit_change(cx);
        }
        cx.notify();
    }

    pub(super) fn shortcut(&mut self, number: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.state.shortcut_row(number) {
            self.choose_row(row, cx);
        }
    }

    /// Favourites, then one mark per agent; the active view has a 2 px bar on the rail edge.
    pub(super) fn rail(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = cx.theme().palette;
        let view = self.state.view();
        let favourites_active = view == PickerView::Favourites;
        let mut rail = div()
            .id("model-picker-rail")
            .role(Role::TabList)
            .aria_label("Agents")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(LAYOUT_MODEL_PICKER_PROVIDER_COLUMN))
            .p(px(SPACE_1))
            .gap(px(SPACE_1))
            .border_r_1()
            .border_color(p.border.subtle)
            .child(
                self.rail_button(
                    p,
                    "rail-favourites",
                    favourites_active,
                    Icon::new(IconName::Star)
                        .size(IconSize::Medium)
                        .color(p.text.secondary),
                    "Favourites",
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.state.select_view(PickerView::Favourites);
                        cx.notify();
                    }),
                ),
            )
            .child(div().h(px(1.)).mx(px(SPACE_1)).bg(p.border.subtle));
        for (index, agent) in self.state.agents().iter().enumerate() {
            let label = agent.kind.label();
            let tip = match agent.availability {
                AgentAvailability::Ready => label.to_owned(),
                AgentAvailability::SignedOut => format!("{label}: sign in to use its models"),
                AgentAvailability::Missing => format!("{label} is not installed"),
            };
            rail = rail.child(self.rail_button(
                p,
                ("rail-agent", index),
                view == PickerView::Agent(index),
                AgentIcon::new(agent.kind).size(AgentIconSize::Medium),
                tip,
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.state.select_view(PickerView::Agent(index));
                    cx.notify();
                }),
            ));
        }
        rail
    }

    /// A 36 px rail button with its name as tooltip and accessible label.
    fn rail_button(
        &self,
        p: Palette,
        id: impl Into<ElementId>,
        active: bool,
        mark: impl IntoElement,
        name: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let name: SharedString = name.into();
        let tooltip = name.clone();
        let selector = format!("rail-{}", name.to_lowercase());
        div()
            .id(id)
            .debug_selector(move || selector.clone())
            .role(Role::Tab)
            .aria_label(name)
            .aria_selected(active)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(SIZE_CONTROL_COMFORTABLE + 4.))
            .rounded(px(RADIUS_MD))
            .cursor_pointer()
            .hover(move |s| s.bg(p.fill.hover_overlay))
            .when(active, |button| button.bg(p.fill.selected_overlay))
            .when(active, move |button| {
                button.child(
                    div()
                        .absolute()
                        .right(px(-SPACE_1))
                        .top(px(8.))
                        .w(px(2.))
                        .h(px(20.))
                        .bg(p.accent.fg),
                )
            })
            .tooltip(move |_, cx| cx.new(|_| NameTooltip(tooltip.clone())).into())
            .on_click(on_click)
            .child(mark)
    }

    /// Search on top, then the model rows; "Show more" under the featured four.
    pub(super) fn model_column(&self, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let p = theme.palette;
        let rows = self.state.rows();
        let highlight = self.state.highlight();
        let choice = self.state.choice().model.clone();
        let mut list = div()
            .id("model-picker-models")
            .role(Role::ListBox)
            .aria_label("Models")
            .flex()
            .flex_col()
            .p(px(SPACE_1))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        if rows.is_empty() {
            list = list.child(self.empty_line(&p, self.empty_copy()));
        }
        for (position, row) in rows.iter().copied().enumerate() {
            let Some(model) = self.state.model(row) else {
                continue;
            };
            let item = ModelRow {
                row,
                position,
                kind: self.state.agents()[row.agent].kind,
                id: model.id.clone(),
                name: model.name.clone(),
                current: model.id == choice,
                favourite: self.state.is_favourite(&model.id),
                highlighted: position == highlight,
                shortcut: (position < PickerState::FEATURED_ROWS)
                    .then(|| format!("Mod+{}", position + 1)),
            };
            list = list.child(self.model_row(item, cx));
        }
        // A search reads every agent, so "Show more" waits until the search is cleared.
        let more_available = self.state.query().is_empty()
            && matches!(self.state.view(), PickerView::Agent(agent)
                if self.state.agents().get(agent).is_some_and(|option| option.models.len() > PickerState::FEATURED_ROWS));
        let mut column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .w(px(LAYOUT_MODEL_PICKER_MODEL_COLUMN))
            .border_r_1()
            .border_color(p.border.subtle)
            .child(self.search_row(&p))
            .child(list);
        if more_available {
            let expanded = self.state.expanded();
            column = column.child(
                div()
                    .id("model-picker-show-more")
                    .debug_selector(|| "model-picker-show-more".to_owned())
                    .mx(px(SPACE_2))
                    .mb(px(SPACE_1))
                    .px(px(SPACE_2))
                    .h(px(SIZE_CONTROL_COMFORTABLE))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS_MD))
                    .text_style(TYPE_BODY)
                    .text_color(p.text.secondary)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.fill.hover_overlay))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if expanded {
                            this.state.collapse();
                        } else {
                            this.state.expand();
                        }
                        cx.notify();
                    }))
                    .child(if expanded { "Show fewer" } else { "Show more" }),
            );
        }
        column
    }

    fn empty_copy(&self) -> SharedString {
        if !self.state.query().is_empty() {
            SharedString::from(format!("No models match \"{}\"", self.state.query()))
        } else {
            match self.state.view() {
                PickerView::Favourites => {
                    SharedString::from("No favourites yet. Star a model to keep it here.")
                }
                PickerView::Agent(_) => SharedString::from("No models loaded."),
            }
        }
    }

    fn empty_line(&self, p: &crate::tokens::Palette, copy: SharedString) -> gpui::AnyElement {
        div()
            .p(px(SPACE_2))
            .text_style(TYPE_BODY)
            .text_color(p.text.secondary)
            .child(copy)
            .into_any_element()
    }

    /// The search field under the accent underline the artboard draws beneath it.
    fn search_row(&self, p: &crate::tokens::Palette) -> Div {
        div()
            .flex()
            .items_center()
            .px(px(SPACE_2))
            .border_b_2()
            .border_color(p.border.focus)
            .child(self.search.clone())
    }

    /// The quiet empty state: no agent is installed, so the popover says so and links to the
    /// Marketplace.
    pub(super) fn no_agents(&self, cx: &mut Context<Self>) -> Div {
        let p = cx.theme().palette;
        div()
            .debug_selector(|| "model-picker-empty".to_owned())
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .p(px(SPACE_3))
            .text_style(TYPE_BODY)
            .child(div().text_color(p.text.secondary).child(super::NO_AGENTS))
            .child(self.marketplace_link(cx))
    }

    /// "Open Marketplace" as a link-style action; Enter and Space press it as a click does.
    fn marketplace_link(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = *cx.theme();
        let p = theme.palette;
        div()
            .id("model-picker-marketplace")
            .debug_selector(|| "model-picker-marketplace".to_owned())
            .role(Role::Link)
            .aria_label("Open Marketplace")
            .track_focus(&self.marketplace_focus)
            .focus_ring(&theme)
            .flex()
            .items_center()
            .h(px(SIZE_CONTROL_COMFORTABLE))
            .rounded(px(RADIUS_MD))
            .text_color(p.accent.fg)
            .cursor_pointer()
            .hover(move |s| s.bg(p.fill.hover_overlay))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.open_marketplace(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.open_marketplace(window, cx);
                }
            }))
            .child("Open Marketplace")
    }

    /// One model row: provider mark, name, a star on hover and the `Mod+n` badge.
    fn model_row(&self, item: ModelRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let ModelRow {
            row,
            position,
            kind,
            id,
            name,
            current,
            favourite,
            highlighted,
            shortcut,
        } = item;
        let p = cx.theme().palette;
        let star_group = SharedString::from(format!("model-row-{position}"));
        let star_id = SharedString::from(format!("model-star-{position}"));
        let star_name = if favourite {
            "Remove from favourites"
        } else {
            "Add to favourites"
        };
        let toggle = id.clone();
        let picker = cx.entity();
        let star = div()
            .id(star_id)
            .debug_selector(move || format!("model-star-{position}"))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                IconButton::new(("model-star-button", position), IconName::Star, star_name)
                    .size(ButtonSize::Compact)
                    .toggle(favourite)
                    .on_click(move |_, cx| {
                        picker.update(cx, |this, cx| {
                            this.state.toggle_favourite(&toggle);
                            cx.notify();
                        });
                    }),
            );
        // The star shows on the highlighted row and on hover; a favourite keeps it visible.
        let star = div()
            .when(!favourite && !highlighted, |star_wrap| {
                star_wrap
                    .invisible()
                    .group_hover(star_group.clone(), |s| s.visible())
            })
            .child(star);
        div()
            .id(("model-row", position))
            .group(star_group)
            .role(Role::ListBoxOption)
            .aria_label(name.clone())
            .aria_selected(current)
            .relative()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .h(px(SIZE_CONTROL_COMFORTABLE))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .cursor_pointer()
            .text_style(if current { TYPE_BODY_STRONG } else { TYPE_BODY })
            .text_color(p.text.primary)
            .hover(move |s| s.bg(p.fill.hover_overlay))
            .when(highlighted, |row| row.bg(p.fill.selected_overlay))
            .when(current, |row| {
                row.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(6.))
                        .w(px(2.))
                        .h(px(20.))
                        .bg(p.accent.fg),
                )
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_row(row, cx)))
            .child(AgentIcon::new(kind).size(AgentIconSize::Small))
            .child(div().flex_1().min_w_0().truncate().child(name))
            .child(star)
            .when_some(shortcut, |row, shortcut| row.child(Kbd::new(shortcut)))
    }

    /// Whether the chosen model's selected level is its top level, which shows the top effects.
    pub(super) fn top_level_shown(&self) -> bool {
        let Some(model) = self.state.current_model() else {
            return false;
        };
        let count = model.efforts.len();
        let selected = self
            .state
            .choice()
            .effort
            .unwrap_or(model.default_effort)
            .min(count.saturating_sub(1));
        is_top_level(selected, count)
    }

    /// The reasoning column: the level's name, the agent and model, and a vertical slider with one stop per
    /// level the model declares. A model with fewer than two levels says it has none.
    pub(super) fn reasoning_column(&self, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let p = theme.palette;
        let model = self.state.current_model().cloned();
        let model_name = model
            .as_ref()
            .map(|model| model.name.clone())
            .unwrap_or_default();
        let column = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(LAYOUT_MODEL_PICKER_REASONING_COLUMN))
            .h(px(REASONING_COLUMN_HEIGHT))
            .px(px(SPACE_3))
            .pt(px(SPACE_2))
            .pb(px(SPACE_2 + SPACE_HALF))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(SPACE_2))
                    .h(px(SIZE_CONTROL_DEFAULT))
                    .border_b_1()
                    .border_color(p.border.subtle)
                    .child(
                        div()
                            .text_style(TYPE_LABEL)
                            .text_color(p.text.secondary)
                            .child("Reasoning"),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_style(TYPE_CAPTION)
                            .text_color(p.text.tertiary)
                            .child(model_name.clone()),
                    ),
            );
        let Some(model) = model else {
            return column;
        };
        if !has_slider(model.efforts.len()) {
            return column.child(
                div()
                    .pt(px(SPACE_2))
                    .text_style(TYPE_BODY)
                    .text_color(p.text.tertiary)
                    .child("This model has no reasoning levels"),
            );
        }
        let count = model.efforts.len();
        let selected = self
            .state
            .choice()
            .effort
            .unwrap_or(model.default_effort)
            .min(count - 1);
        // The agent's listing colours fill the bar; an agent that lists none fills with the accent.
        let brand = brand_colours(self.state.current_brand(), (p.accent.default, p.accent.fg));
        let name = name_colour(brand, p.surface.overlay);
        let agent = self.state.current_kind().label();
        let reduced = cx.reduce_motion();
        // The top effects fade in from when the top level was first shown with the popover open.
        let shown_for = self
            .top_since
            .map_or(0.0, |since| since.elapsed().as_secs_f32());
        let effects_fade = fade_progress(shown_for, effects_fade_seconds(), reduced);
        let picker = cx.entity();
        let slider = ReasoningSlider {
            id: "model-picker-reasoning-slider".into(),
            levels: model.efforts.clone(),
            selected,
            brand,
            label: format!("Reasoning for {model_name}").into(),
            focus: self.slider_focus.clone(),
            track: self.slider_track.clone(),
            on_change: Rc::new(move |index, _, cx| {
                picker.update(cx, |picker, cx| picker.choose_effort(index, cx));
            }),
            elapsed: clock_seconds(reduced),
            reduced,
            moving_from: self.effort_move.map(|moved| moved.from),
            progress: self.effort_move.map_or(1.0, |moved| {
                move_progress(moved.started.elapsed().as_secs_f32(), reduced)
            }),
            effects_fade,
            halo_fade: fade_progress(shown_for, halo_fade_seconds(), reduced),
        };
        let level = div()
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .mt(px(SPACE_2))
            .child(
                div()
                    .text_style(TYPE_TITLE)
                    .text_color(name)
                    .child(model.efforts[selected].clone()),
            )
            .when(is_top_level(selected, count), |row| {
                row.child(
                    svg()
                        .path(IconName::Sparkles.path())
                        .size(px(SPARKLE_SIZE))
                        .opacity(effects_fade)
                        .flex_none()
                        .text_color(name),
                )
            });
        column
            .child(level)
            .child(
                div()
                    .mt(px(SPACE_1))
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_CAPTION)
                    .text_color(p.text.tertiary)
                    .child(format!("{agent} · {model_name}")),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .mt(px(SPACE_2 + SPACE_HALF))
                    .child(slider),
            )
    }

    /// Standard or Fast, as a borderless sunken switch; Fast is off for models without it.
    pub(super) fn footer(&self, cx: &mut Context<Self>) -> Div {
        let p = cx.theme().palette;
        let fast = self.state.choice().fast;
        let supports_fast = self.state.current_model().is_some_and(|model| model.fast);
        let mut fast_segment = Segment::icon_text(IconName::Zap, "Fast").disabled(!supports_fast);
        if !supports_fast {
            let reason = self
                .state
                .fast_reason()
                .map_or_else(|| NO_FAST_MODEL.to_owned(), |reason| reason.to_string());
            fast_segment = fast_segment.tooltip(reason);
        }
        let picker = cx.entity();
        div()
            .flex()
            .items_center()
            .flex_none()
            .h(px(SIZE_CONTROL_DEFAULT + SPACE_2 * 2.))
            .px(px(SPACE_2))
            .border_t_1()
            .border_color(p.border.subtle)
            .child(
                SegmentedControl::new(
                    "model-picker-speed",
                    "Speed",
                    vec![Segment::text("Standard"), fast_segment],
                )
                .style(SegmentedStyle::Sunken)
                .size(SegmentedSize::Default)
                .selected(usize::from(fast))
                .on_change(move |index, _, cx| {
                    picker.update(cx, |picker, cx| picker.set_speed(index == 1, cx));
                }),
            )
    }
}
