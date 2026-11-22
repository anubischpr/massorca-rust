//! The controls of settings whose layer is not built yet. Each is the spec's control type, drawn
//! disabled, with its status saying what is missing (SettingsRow.md). None holds or sends a value,
//! so nothing here claims a change the service has not confirmed.

use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::checkbox::Checkbox;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::settings_row::{self, RowStatus, SettingsRow};
use massorca_ui::components::switch::Switch;
use massorca_ui::tokens::{SPACE_2, TYPE_CAPTION};
use massorca_ui::{IconName, TextStyled, Theme};

use crate::model::settings_sections::{NOT_AVAILABLE, Planned};

/// The six accent presets: lavender (the default), blue, teal, rose, orange and graphite.
const ACCENT_PRESETS: usize = 6;

/// Adds the status of a planned setting to its row and returns its control. A `Status` row has
/// no control, only the status.
pub(super) fn planned_row(
    row: SettingsRow,
    planned: Planned,
    id: &SharedString,
    label: &'static str,
    theme: &Theme,
) -> (SettingsRow, Option<AnyElement>) {
    let status = NOT_AVAILABLE;
    let row = row.status(RowStatus::Unavailable(status.into()));
    let row = match planned {
        Planned::Checkboxes(_) => row.stacked(true),
        _ => row,
    };
    let control = match planned {
        Planned::Status => None,
        Planned::Switch => Some(
            Switch::new(id.clone(), false)
                .label(label)
                .disabled(true)
                .into_any_element(),
        ),
        Planned::Choice(options) => Some(choice(id, label, options)),
        Planned::Select(value) => Some(select(id, label, theme, value)),
        Planned::SelectAction(value, action) => Some(beside(
            select(id, label, theme, value),
            action_button(&action_id(id), action, ButtonVariant::Ghost),
        )),
        Planned::ChoiceAction(options, action) => Some(beside(
            choice(id, label, options),
            action_button(&action_id(id), action, ButtonVariant::Ghost),
        )),
        Planned::Text(value) => Some(settings_row::unavailable_input(
            id.clone(),
            label.into(),
            theme,
            value,
        )),
        Planned::Number { value, unit } => Some(settings_row::unavailable_stepper(
            id.clone(),
            label.into(),
            theme,
            value,
            unit,
        )),
        Planned::Slider { value, percent } => Some(settings_row::unavailable_slider(
            id.clone(),
            label.into(),
            theme,
            value,
            f32::from(percent) / 100.0,
        )),
        Planned::Swatches => Some(swatches(id, theme)),
        Planned::Checkboxes(options) => Some(checkboxes(id, options)),
        Planned::Action(text) => {
            Some(action_button(id, text, ButtonVariant::Secondary).into_any_element())
        }
        Planned::Link => Some(link_button(id.clone()).into_any_element()),
    };
    (row, control)
}

/// Segmented options that cannot be chosen yet. Short lists only (up to four).
fn choice(id: &SharedString, label: &'static str, options: &'static [&'static str]) -> AnyElement {
    SegmentedControl::new(
        id.clone(),
        label,
        options
            .iter()
            .map(|option| Segment::text(*option))
            .collect(),
    )
    .disabled(true)
    .into_any_element()
}

/// A dropdown that cannot be opened yet, showing `value` or "Choose".
fn select(
    id: &SharedString,
    label: &'static str,
    theme: &Theme,
    value: Option<&str>,
) -> AnyElement {
    settings_row::unavailable_select(
        SharedString::from(format!("{id}-select")),
        label.into(),
        theme,
        value,
    )
}

/// Two controls on one line, the second at the trailing edge.
fn beside(control: AnyElement, action: Button) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(SPACE_2))
        .w_full()
        .child(control)
        .child(action)
        .into_any_element()
}

/// The six accent presets that cannot be picked yet, a custom colour button, and the contrast
/// line that appears once a colour is chosen.
fn swatches(id: &SharedString, theme: &Theme) -> AnyElement {
    let presets = settings_row::unavailable_swatches(theme, ACCENT_PRESETS);
    div()
        .flex()
        .flex_col()
        .items_end()
        .gap(px(SPACE_2))
        .child(beside(
            presets,
            action_button(
                &SharedString::from(format!("{id}-custom")),
                "Custom colour",
                ButtonVariant::Ghost,
            ),
        ))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child("Contrast check appears when you choose a colour."),
        )
        .into_any_element()
}

/// One checkbox per option, stacked, each shown and disabled.
fn checkboxes(id: &SharedString, options: &'static [&'static str]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .w_full()
        .children(options.iter().enumerate().map(|(index, option)| {
            Checkbox::new(SharedString::from(format!("{id}-{index}")), true)
                .label(*option)
                .disabled(true)
        }))
        .into_any_element()
}

/// The disabled "Open" button of a link row.
fn link_button(id: SharedString) -> Button {
    Button::new(id, "Open")
        .variant(ButtonVariant::Ghost)
        .trailing_icon(IconName::ChevronRight)
        .disabled(true)
}

fn action_button(id: &SharedString, text: &'static str, variant: ButtonVariant) -> Button {
    Button::new(id.clone(), text)
        .variant(variant)
        .disabled(true)
}

fn action_id(id: &SharedString) -> SharedString {
    SharedString::from(format!("{id}-action"))
}
