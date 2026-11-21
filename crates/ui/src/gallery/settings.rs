//! Gallery stories for the settings group: the radio group and the settings row.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Story, variant};
use crate::components::radio::{RadioGroup, RadioOption};
use crate::tokens::SPACE_4;

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Radio group",
        spec: "Radio.md",
        render: radio_story,
    }]
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
        RadioOption::new("VPS").helper("Runs on the server while it is reachable."),
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
