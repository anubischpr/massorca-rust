//! Gallery stories for the controls group.

use super::{Story, variant};
use crate::components::button::{Button, ButtonSize, ButtonVariant, StatePreview};
use crate::icon::IconName;
use crate::style::Surface;
use crate::theme::ActiveTheme;
use crate::tokens::{LAYOUT_MENU_MIN_WIDTH, SPACE_2, SPACE_3, SPACE_4};
use gpui::{AnyElement, App, Div, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Button",
        spec: "Button.md",
        render: button_story,
    }]
}

/// Every button variant, in gallery order.
const VARIANTS: [(&str, ButtonVariant); 4] = [
    ("Primary", ButtonVariant::Primary),
    ("Secondary", ButtonVariant::Secondary),
    ("Ghost", ButtonVariant::Ghost),
    ("Danger", ButtonVariant::Danger),
];

fn column() -> Div {
    div().flex().flex_col().gap(px(SPACE_4))
}

fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(SPACE_2))
}

/// One row with every variant, each configured by `configure`.
fn variant_row(prefix: &'static str, configure: impl Fn(Button) -> Button) -> Div {
    VARIANTS
        .iter()
        .enumerate()
        .fold(row(), |r, (i, (name, kind))| {
            r.child(configure(Button::new((prefix, i), *name).variant(*kind)))
        })
}

fn button_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let frame = cx.theme().palette.surface.frame;
    column()
        .child(variant("Variants at rest", variant_row("rest", |b| b), cx))
        .child(variant(
            "Hover",
            variant_row("hover", |b| b.preview(StatePreview::Hover)),
            cx,
        ))
        .child(variant(
            "Pressed",
            variant_row("pressed", |b| b.preview(StatePreview::Pressed)),
            cx,
        ))
        .child(variant(
            "Focus ring (keyboard focus)",
            variant_row("focus", |b| b.preview(StatePreview::Focus)),
            cx,
        ))
        .child(variant(
            "Disabled (the reason is the tooltip text)",
            variant_row("disabled", |b| {
                b.disabled(true)
                    .tooltip("Host unreachable. Last known state, 5 min ago.")
            }),
            cx,
        ))
        .child(variant(
            "Loading (spinner after 200 ms, width fixed)",
            row()
                .child(
                    Button::new("loading-primary", "Saving")
                        .variant(ButtonVariant::Primary)
                        .loading(true),
                )
                .child(Button::new("loading-secondary", "Saving").loading(true)),
            cx,
        ))
        .child(variant(
            "Sizes",
            row()
                .child(Button::new("size-compact", "Compact").size(ButtonSize::Compact))
                .child(Button::new("size-default", "Default").size(ButtonSize::Default))
                .child(
                    Button::new("size-comfortable", "Comfortable").size(ButtonSize::Comfortable),
                ),
            cx,
        ))
        .child(variant(
            "Icons",
            row()
                .child(
                    Button::new("icon-leading", "New routine")
                        .variant(ButtonVariant::Primary)
                        .leading_icon(IconName::Plus),
                )
                .child(
                    Button::new("icon-trailing", "Open in editor")
                        .trailing_icon(IconName::ExternalLink),
                )
                .child(
                    Button::new("icon-danger", "Delete secret")
                        .variant(ButtonVariant::Danger)
                        .leading_icon(IconName::Trash2),
                ),
            cx,
        ))
        .child(variant(
            "Ghost, open menu (selected fill) and on the Title bar frame",
            row()
                .child(
                    Button::new("selected-canvas", "Project")
                        .variant(ButtonVariant::Ghost)
                        .selected(true),
                )
                .child(
                    div().flex().p(px(SPACE_3)).bg(frame).child(
                        Button::new("selected-frame", "Terminal")
                            .variant(ButtonVariant::Ghost)
                            .surface(Surface::Frame)
                            .leading_icon(IconName::SquareTerminal),
                    ),
                ),
            cx,
        ))
        .child(variant(
            "Long label (truncates, never wraps)",
            div().w(px(LAYOUT_MENU_MIN_WIDTH)).child(
                Button::new("long", "Review the pull request description and checks")
                    .variant(ButtonVariant::Ghost)
                    .leading_icon(IconName::GitBranch),
            ),
            cx,
        ))
        .into_any_element()
}
