//! Gallery stories for the feedback group.

use super::{Story, variant};
use crate::components::banner::{Banner, BannerVariant};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::card::{Card, CardPadding, CardVariant};
use crate::components::empty_state::{EmptyState, EmptyStateVariant};
use crate::components::error_state::{ErrorState, ErrorStateVariant};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::toast::{Toast, ToastKind, ToastSpec, Toasts};
use crate::icon::IconName;
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{SPACE_2, SPACE_3, TYPE_BODY_SM};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Card",
            spec: "Card.md",
            render: card_story,
        },
        Story {
            name: "Empty state",
            spec: "EmptyState.md",
            render: empty_state_story,
        },
        Story {
            name: "Error state",
            spec: "ErrorState.md",
            render: error_state_story,
        },
        Story {
            name: "Banner",
            spec: "Banner.md",
            render: banner_story,
        },
        Story {
            name: "Toast",
            spec: "Toast.md",
            render: toast_story,
        },
    ]
}

fn toast_story(_: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(variant(
            "Kinds: info, success and warning clear after eight seconds; an error stays",
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .child(Toast::new("toast-info", ToastKind::Info, "Copied"))
                .child(Toast::new(
                    "toast-success",
                    ToastKind::Success,
                    "Session renamed",
                ))
                .child(Toast::new(
                    "toast-warning",
                    ToastKind::Warning,
                    "Runs on a host that is slow to answer",
                ))
                .child(Toast::new(
                    "toast-error",
                    ToastKind::Error,
                    "Could not rename the session",
                )),
            cx,
        ))
        .child(variant(
            "Undo: times out after eight seconds",
            Toast::new("toast-undo", ToastKind::Success, "Removed from queue")
                .action("Undo", |_, _| {}),
            cx,
        ))
        .child(variant(
            "Repeated within two seconds: a counter chip instead of a second toast",
            Toast::new("toast-repeat", ToastKind::Info, "Copied").repeats(2),
            cx,
        ))
        .child(variant(
            "Action running: the button shows a spinner and cannot be pressed",
            Toast::new("toast-busy", ToastKind::Error, "Setup script failed")
                .action("Open log", |_, _| {})
                .action_loading(true),
            cx,
        ))
        .child(variant(
            "Live stack: bottom right of the canvas, newest at the bottom, three visible",
            div()
                .relative()
                .h(px(240.))
                .child(
                    div()
                        .flex()
                        .gap(px(SPACE_2))
                        .child(
                            Button::new("toast-demo-info", "Show toast")
                                .variant(ButtonVariant::Secondary)
                                .on_click(|_, cx| {
                                    Toasts::show(cx, ToastSpec::new(ToastKind::Info, "Copied"));
                                }),
                        )
                        .child(
                            Button::new("toast-demo-undo", "Show with Undo")
                                .variant(ButtonVariant::Secondary)
                                .on_click(|_, cx| {
                                    let spec =
                                        ToastSpec::new(ToastKind::Success, "Removed from queue")
                                            .with_action("Undo", true);
                                    Toasts::show_with_action(cx, spec, |_, _| {});
                                }),
                        )
                        .child(
                            Button::new("toast-demo-error", "Show error")
                                .variant(ButtonVariant::Secondary)
                                .on_click(|_, cx| {
                                    Toasts::show(
                                        cx,
                                        ToastSpec::new(
                                            ToastKind::Error,
                                            "Could not rename the session",
                                        ),
                                    );
                                }),
                        ),
                )
                .child(Toasts::stack(cx)),
            cx,
        ))
        .into_any_element()
}

fn banner_story(_: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(variant(
            "Info: a note about the content, dismissible",
            Banner::new(
                "banner-info",
                BannerVariant::Info,
                "This diff is large (12,431 lines). Show it anyway?",
            )
            .action(Button::new("banner-info-show", "Show diff"))
            .on_dismiss(|_, _| {}),
            cx,
        ))
        .child(variant(
            "Success: restored, dismissible",
            Banner::new(
                "banner-success",
                BannerVariant::Success,
                "Reconnected to VPS",
            )
            .on_dismiss(|_, _| {}),
            cx,
        ))
        .child(variant(
            "Warning: sign-in needed, one action",
            Banner::new(
                "banner-warning",
                BannerVariant::Warning,
                "VPS needs you to sign in",
            )
            .action(Button::new("banner-warning-sign-in", "Sign in")),
            cx,
        ))
        .child(variant(
            "Danger: a failure with a Details disclosure, two actions and a dismiss control",
            Banner::new(
                "banner-danger",
                BannerVariant::Danger,
                "Setup script failed.",
            )
            .detail("Exit code 1. Open the log for the full output.")
            .action(Button::new("banner-danger-log", "Open log"))
            .action(Button::new("banner-danger-retry", "Retry"))
            .on_dismiss(|_, _| {}),
            cx,
        ))
        .child(variant(
            "Connecting: a spinner in place of the glyph",
            Banner::new(
                "banner-connecting",
                BannerVariant::Info,
                "Connecting to VPS",
            )
            .loading(true),
            cx,
        ))
        .child(variant(
            "Unreachable: the last known state, never worded as stopped",
            Banner::new(
                "banner-unreachable",
                BannerVariant::Unreachable,
                "VPS can't be reached. Showing the last known state from 5 min ago. Work that was running there may still be running.",
            )
            .detail("Last contact 5 min ago.")
            .action(Button::new("banner-unreachable-retry", "Retry")),
            cx,
        ))
        .child(variant(
            "Edge to edge, with a +2 more count for hidden notices",
            Banner::new(
                "banner-more",
                BannerVariant::Warning,
                "Two more notices are waiting.",
            )
            .edge_to_edge(true)
            .more(2, |_, _| {}),
            cx,
        ))
        .into_any_element()
}

fn error_state_story(_: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(variant(
            "Page: title, explanation, Retry and Show details",
            ErrorState::new("error-page", "Could not load routines")
                .variant(ErrorStateVariant::Page)
                .explanation("Your changes are kept.")
                .details("E_TIMEOUT at 10:14:03"),
            cx,
        ))
        .child(variant(
            "Section: a danger panel with Retry",
            ErrorState::new("error-section", "Could not load sessions")
                .explanation("The list shows the last loaded sessions."),
            cx,
        ))
        .child(variant(
            "Retrying: a spinner and Retry cannot be activated",
            ErrorState::new("error-retrying", "Could not load sessions").retrying(true),
            cx,
        ))
        .child(variant(
            "Inline: no panel, Retry as a text button",
            ErrorState::new("error-inline", "Could not delete")
                .variant(ErrorStateVariant::Inline)
                .explanation("The secret is still there."),
            cx,
        ))
        .into_any_element()
}

fn empty_state_story(_: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(variant(
            "Page: display title, centred, one primary action",
            EmptyState::new("empty-page", "No routines yet")
                .variant(EmptyStateVariant::Page)
                .icon(IconName::Workflow)
                .explanation("Routines run a task on a schedule. Create one to start.")
                .action(
                    Button::new("empty-page-new", "New routine")
                        .variant(ButtonVariant::Primary)
                        .leading_icon(IconName::Plus),
                ),
            cx,
        ))
        .child(variant(
            "Section: title-sm and one secondary action",
            EmptyState::new("empty-section", "No files found")
                .icon(IconName::Folder)
                .explanation("This folder has no files that match the view.")
                .action(
                    Button::new("empty-section-add", "Add web address")
                        .leading_icon(IconName::Globe),
                ),
            cx,
        ))
        .child(variant(
            "Filtered: the title names the filter and Clear search clears it",
            EmptyState::filtered("empty-filtered", "docs", |_, _| {}),
            cx,
        ))
        .child(variant(
            "Compact: a label and one line inside a list",
            EmptyState::new("empty-compact", "Nothing open")
                .variant(EmptyStateVariant::Compact)
                .explanation("Open a file to see it here.")
                .action(
                    Button::new("empty-compact-open", "Open file")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Compact),
                ),
            cx,
        ))
        .child(variant(
            "Work panel (A43): a centred column of four actions, no panel or title",
            EmptyState::work_panel(
                "empty-work",
                [
                    ("New terminal", IconName::SquareTerminal),
                    ("New browser tab", IconName::Globe),
                    ("New Markdown", IconName::FileText),
                    ("New mobile emulator", IconName::Smartphone),
                ]
                .map(|(label, icon)| {
                    Button::new(label, label)
                        .variant(ButtonVariant::Secondary)
                        .leading_icon(icon)
                }),
            ),
            cx,
        ))
        .child(variant(
            "Host unreachable: say what is unknown, with no create action",
            EmptyState::new("empty-host", "Host unreachable")
                .explanation("Last known state, 5 min ago")
                .note("Runs on the VPS host."),
            cx,
        ))
        .into_any_element()
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
