//! Gallery stories for the feedback group.

use super::{Story, variant};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::card::{Card, CardPadding, CardVariant};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::icon::IconName;
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{SPACE_3, TYPE_BODY_SM};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Card",
        spec: "Card.md",
        render: card_story,
    }]
}

fn card_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.theme().palette;
    let note = |text: &'static str| {
        div()
            .text_style(TYPE_BODY_SM)
            .text_color(p.text.secondary)
            .child(text)
    };
    let overflow = || {
        IconButton::new("card-overflow", IconName::Ellipsis, "More actions")
            .variant(IconButtonVariant::Ghost)
            .size(ButtonSize::Compact)
            .surface(Surface::Raised)
    };
    let open = || {
        Button::new("card-open", "Open")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Compact)
            .surface(Surface::Raised)
    };

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(variant(
            "Static summary card: icon, value, overflow action and footer",
            Card::new("card-static")
                .icon(IconName::ListChecks)
                .title("Checklist")
                .value("2 of 5")
                .header_action(overflow())
                .child(note("Two items left"))
                .footer_action(open()),
            cx,
        ))
        .child(variant(
            "Interactive routine card, comfortable padding",
            Card::new("card-routine")
                .variant(CardVariant::Interactive)
                .padding(CardPadding::Comfortable)
                .icon(IconName::Workflow)
                .title("Nightly build")
                .child(note("Runs at 02:00 on weekdays")),
            cx,
        ))
        .child(variant(
            "Selected: accent bar on the leading edge",
            Card::new("card-selected")
                .variant(CardVariant::Interactive)
                .selected(true)
                .title("Selected workspace")
                .child(note("Two sessions running")),
            cx,
        ))
        .child(variant(
            "Disabled interactive card",
            Card::new("card-disabled")
                .variant(CardVariant::Interactive)
                .disabled(true)
                .title("Unavailable host")
                .child(note("Last known state, 5 min ago")),
            cx,
        ))
        .child(variant(
            "Loading: skeleton lines in the body",
            Card::new("card-loading").title("Plan").loading(true),
            cx,
        ))
        .child(variant(
            "No content: the card draws nothing (A35)",
            Card::new("card-empty").icon(IconName::Server),
            cx,
        ))
        .into_any_element()
}
