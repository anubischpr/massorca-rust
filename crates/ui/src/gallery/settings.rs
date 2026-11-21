//! Gallery stories for the settings group: the radio group and the settings row.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Story, variant};
use crate::components::radio::{RadioGroup, RadioOption};
use crate::components::settings_row::{RowStatus, SettingsRow, SettingsSection};
use crate::components::switch::Switch;
use crate::tokens::{SPACE_4, SPACE_8};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Radio group",
            spec: "Radio.md",
            render: radio_story,
        },
        Story {
            name: "Settings row",
            spec: "SettingsRow.md",
            render: settings_row_story,
        },
    ]
}

fn settings_row_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let section = SettingsSection::new("gallery-section", "Notifications")
        .description("Choose what reaches you while you are away from the app.")
        .row(
            SettingsRow::new("row-plain", "Agent finished")
                .helper("When an agent ends a run that you started.")
                .control(Switch::new("row-plain-switch", true))
                .reset(|_, _| {}),
        )
        .row(
            SettingsRow::new("row-saving", "Waiting for you")
                .control(Switch::new("row-saving-switch", false))
                .status(RowStatus::Saving),
        )
        .row(
            SettingsRow::new("row-saved", "Errors")
                .control(Switch::new("row-saved-switch", true))
                .status(RowStatus::Saved),
        )
        .row(
            SettingsRow::new("row-error", "App updates")
                .helper("Ask before installing.")
                .control(Switch::new("row-error-switch", false))
                .status(RowStatus::Error(
                    "Could not save. The previous value is kept.".into(),
                )),
        )
        .row(
            SettingsRow::new("row-disabled", "Usage data")
                .disabled_reason("Not available yet")
                .control(Switch::new("row-disabled-switch", false).disabled(true)),
        );
    let stacked = SettingsSection::new("gallery-stacked", "Narrow page").row(
        SettingsRow::new("row-stacked", "Conversations folder")
            .helper("Saved in ~/MassOrca/Conversations on this PC.")
            .control(Switch::new("row-stacked-switch", true))
            .stacked(true),
    );
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_8))
        .child(variant("Section with each status", section, cx))
        .child(variant("Stacked below 560 px", stacked, cx))
        .into_any_element()
}

fn theme_options() -> Vec<RadioOption> {
    vec![
        RadioOption::new("System"),
        RadioOption::new("Dark"),
        RadioOption::new("Light"),
    ]
}

fn radio_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let with_helpers = vec![
        RadioOption::new("This PC").helper("Runs on this computer."),
        RadioOption::new("Remote host").helper("Runs on the server while it is reachable."),
    ];
    let one_disabled = vec![
        RadioOption::new("This PC"),
        RadioOption::new("Remote host").disabled(true),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Chosen",
            RadioGroup::new("radio-chosen", theme_options()).selected(Some(1)),
            cx,
        ))
        .child(variant(
            "With descriptions",
            RadioGroup::new("radio-helpers", with_helpers).selected(Some(0)),
            cx,
        ))
        .child(variant(
            "One option disabled",
            RadioGroup::new("radio-one-disabled", one_disabled).selected(Some(0)),
            cx,
        ))
        .child(variant(
            "Disabled group",
            RadioGroup::new("radio-disabled", theme_options())
                .selected(Some(2))
                .disabled(true),
            cx,
        ))
        .child(variant(
            "Error",
            RadioGroup::new("radio-error", theme_options()).error("Choose one option to continue."),
            cx,
        ))
        .into_any_element()
}
