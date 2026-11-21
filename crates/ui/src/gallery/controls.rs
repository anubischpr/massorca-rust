//! Gallery stories for the controls group.

use super::{Story, variant};
use crate::components::badge::{Badge, BadgeTone};
use crate::components::button::{Button, ButtonSize, ButtonVariant, StatePreview};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::kbd::{Kbd, ShortcutOs};
use crate::icon::IconName;
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{LAYOUT_MENU_MIN_WIDTH, SPACE_2, SPACE_3, SPACE_4};
use gpui::{AnyElement, App, Div, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Button",
            spec: "Button.md",
            render: button_story,
        },
        Story {
            name: "IconButton",
            spec: "IconButton.md",
            render: icon_button_story,
        },
        Story {
            name: "Kbd",
            spec: "Kbd.md",
            render: kbd_story,
        },
        Story {
            name: "Badge",
            spec: "Badge.md",
            render: badge_story,
        },
    ]
}

/// Every button variant, in gallery order.
const VARIANTS: [(&str, ButtonVariant); 4] = [
    ("Primary", ButtonVariant::Primary),
    ("Secondary", ButtonVariant::Secondary),
    ("Ghost", ButtonVariant::Ghost),
    ("Danger", ButtonVariant::Danger),
];

/// Icon buttons in gallery order, each with its accessible name.
const ICONS: [(IconName, &str); 4] = [
    (IconName::Plus, "New terminal"),
    (IconName::Copy, "Copy path"),
    (IconName::Ellipsis, "More actions"),
    (IconName::X, "Close tab"),
];

/// One row of icon buttons, each configured by `configure`.
fn icon_row(prefix: &'static str, configure: impl Fn(IconButton) -> IconButton) -> Div {
    ICONS
        .iter()
        .enumerate()
        .fold(row(), |r, (i, (icon, name))| {
            r.child(configure(IconButton::new((prefix, i), *icon, *name)))
        })
}

fn icon_button_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let frame = cx.theme().palette.surface.frame;
    column()
        .child(variant("Ghost at rest", icon_row("ghost-rest", |b| b), cx))
        .child(variant(
            "Ghost, hover",
            icon_row("ghost-hover", |b| b.preview(StatePreview::Hover)),
            cx,
        ))
        .child(variant(
            "Ghost, pressed",
            icon_row("ghost-pressed", |b| b.preview(StatePreview::Pressed)),
            cx,
        ))
        .child(variant(
            "Focus ring (keyboard focus)",
            icon_row("ghost-focus", |b| b.preview(StatePreview::Focus)),
            cx,
        ))
        .child(variant(
            "Outline",
            icon_row("outline", |b| b.variant(IconButtonVariant::Outline)),
            cx,
        ))
        .child(variant(
            "Outline, hover",
            icon_row("outline-hover", |b| {
                b.variant(IconButtonVariant::Outline)
                    .preview(StatePreview::Hover)
            }),
            cx,
        ))
        .child(variant(
            "Toggle, off and on (aria-pressed)",
            row()
                .child(
                    IconButton::new("toggle-off", IconName::PanelLeft, "Show project sidebar")
                        .toggle(false),
                )
                .child(
                    IconButton::new("toggle-on", IconName::PanelLeft, "Hide project sidebar")
                        .toggle(true),
                ),
            cx,
        ))
        .child(variant(
            "Disabled (the reason replaces the name in the tooltip)",
            icon_row("disabled", |b| {
                b.disabled(true)
                    .disabled_reason("Host unreachable. Last known state, 5 min ago.")
            }),
            cx,
        ))
        .child(variant(
            "Loading (spinner after 200 ms)",
            row().child(IconButton::new("loading", IconName::RefreshCw, "Retry").loading(true)),
            cx,
        ))
        .child(variant(
            "Sizes",
            row()
                .child(
                    IconButton::new("size-compact", IconName::Plus, "Compact")
                        .size(ButtonSize::Compact),
                )
                .child(
                    IconButton::new("size-default", IconName::Plus, "Default")
                        .size(ButtonSize::Default),
                )
                .child(
                    IconButton::new("size-comfortable", IconName::Plus, "Comfortable")
                        .size(ButtonSize::Comfortable),
                ),
            cx,
        ))
        .child(variant(
            "Title bar (frame surface, no fill of its own)",
            div().flex().p(px(SPACE_3)).bg(frame).child(
                row()
                    .child(
                        IconButton::new(
                            "titlebar-sidebar",
                            IconName::PanelLeft,
                            "Show project sidebar",
                        )
                        .surface(Surface::Frame),
                    )
                    .child(
                        IconButton::new("titlebar-settings", IconName::Settings, "Settings")
                            .surface(Surface::Frame),
                    ),
            ),
            cx,
        ))
        .into_any_element()
}

fn kbd_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.theme().palette;
    column()
        .child(variant(
            "Single keys",
            row()
                .child(Kbd::new("Esc"))
                .child(Kbd::new("F6"))
                .child(Kbd::new("Enter")),
            cx,
        ))
        .child(variant(
            "Chord on Windows and Linux (Ctrl+K, a + between caps)",
            row()
                .child(Kbd::new("Mod+K").os(ShortcutOs::WindowsLinux))
                .child(Kbd::new("Mod+Shift+F").os(ShortcutOs::WindowsLinux))
                .child(Kbd::new("Mod+Alt+Shift+K").os(ShortcutOs::WindowsLinux)),
            cx,
        ))
        .child(variant(
            "Chord on macOS (⌘K, glyphs with no separator)",
            row()
                .child(Kbd::new("Mod+K").os(ShortcutOs::MacOs))
                .child(Kbd::new("Mod+Shift+F").os(ShortcutOs::MacOs))
                .child(Kbd::new("Ctrl+K").os(ShortcutOs::MacOs)),
            cx,
        ))
        .child(variant(
            "Inline in a menu row, for the running OS",
            row()
                .child(
                    div()
                        .text_body()
                        .text_color(p.text.primary)
                        .child("Open command palette"),
                )
                .child(Kbd::new("Mod+K")),
            cx,
        ))
        .child(variant(
            "Selected row (primary text)",
            row().child(Kbd::new("Mod+P").selected(true)),
            cx,
        ))
        .child(variant(
            "Disabled (no fill)",
            row().child(Kbd::new("Mod+K").disabled(true)),
            cx,
        ))
        .into_any_element()
}

fn badge_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.theme().palette;
    column()
        .child(variant(
            "Text, subtle (a status tone carries its glyph and word)",
            row()
                .child(Badge::text("Category"))
                .child(Badge::text("New").tone(BadgeTone::Accent))
                .child(Badge::text("Connected").tone(BadgeTone::Success))
                .child(Badge::text("Waiting for you").tone(BadgeTone::Warning))
                .child(Badge::text("Failed").tone(BadgeTone::Danger))
                .child(Badge::text("Notice").tone(BadgeTone::Info)),
            cx,
        ))
        .child(variant(
            "Solid (high-emphasis status on a dark row)",
            row()
                .child(Badge::text("Passed").tone(BadgeTone::Success).solid(true))
                .child(Badge::text("Failed").tone(BadgeTone::Danger).solid(true))
                .child(Badge::text("Waiting").tone(BadgeTone::Warning).solid(true))
                .child(Badge::text("Notice").tone(BadgeTone::Info).solid(true)),
            cx,
        ))
        .child(variant(
            "Category chips (label style)",
            row().child(Badge::text("Extension").category(true)).child(
                Badge::text("Selected")
                    .tone(BadgeTone::Accent)
                    .category(true),
            ),
            cx,
        ))
        .child(variant(
            "Counts (99+ above 99; a zero count draws nothing)",
            row()
                .child(Badge::count(1))
                .child(Badge::count(7).tone(BadgeTone::Accent))
                .child(Badge::count(99).tone(BadgeTone::Danger))
                .child(Badge::count(100).tone(BadgeTone::Warning)),
            cx,
        ))
        .child(variant(
            "Dots, beside a labelled item",
            row()
                .child(Badge::dot().tone(BadgeTone::Accent))
                .child(div().text_body().text_color(p.text.primary).child("Checks"))
                .child(Badge::dot().tone(BadgeTone::Success))
                .child(div().text_body().text_color(p.text.primary).child("Build"))
                .child(Badge::dot().tone(BadgeTone::Danger))
                .child(div().text_body().text_color(p.text.primary).child("Deploy")),
            cx,
        ))
        .child(variant(
            "Disabled (text disabled, no background)",
            row()
                .child(Badge::text("Failed").tone(BadgeTone::Danger).disabled(true))
                .child(Badge::count(3).tone(BadgeTone::Danger).disabled(true)),
            cx,
        ))
        .child(variant(
            "Long text (clipped after 16 characters)",
            row().child(Badge::text("Waiting for your approval on the checks")),
            cx,
        ))
        .child(variant(
            "Disconnected host (dashed ring and Unreachable, never Stopped)",
            row().child(Badge::text("Unreachable").glyph(IconName::CircleDashed)),
            cx,
        ))
        .into_any_element()
}

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
