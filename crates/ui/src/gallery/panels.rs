//! Gallery stories for the desktop panels: notifications, extensions, secret requests, the
//! project and work panel tabs, and the browser chrome.

use super::{Story, variant};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::notification_card::{
    NotificationCard, NotificationKind, NotificationStack, UpdateState,
};
use crate::tokens::{SPACE_3, SPACE_4};
use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Notification card",
        spec: "NotificationCard.md",
        render: notification_story,
    }]
}

fn notification_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let attention = NotificationCard::new(
        "notification-attention",
        NotificationKind::Attention,
        "oppm: Run command? pnpm install",
    )
    .source("oppm")
    .age("2 min ago")
    .body("Runs in the project folder with workspace access.")
    .answer(primary("notification-allow", "Allow"))
    .answer(ghost("notification-deny", "Deny"))
    .on_dismiss(|_, _| {});

    let error = NotificationCard::new(
        "notification-error",
        NotificationKind::Error,
        "NexusCare: the run failed",
    )
    .source("NexusCare")
    .age("just now")
    .body("Exit code 1 after 42 seconds.")
    .action(primary("notification-retry", "Retry"))
    .on_dismiss(|_, _| {});

    let sign_in = NotificationCard::new(
        "notification-sign-in",
        NotificationKind::SignIn,
        "Claude was signed out",
    )
    .source("Claude")
    .age("4 min ago")
    .action(primary("notification-sign-in-button", "Sign in"))
    .on_dismiss(|_, _| {});

    let note = NotificationCard::new(
        "notification-note",
        NotificationKind::Info,
        "Acme Metrics: 2 commands added",
    )
    .source("Acme Metrics")
    .age("1 h ago")
    .on_dismiss(|_, _| {});

    let unreachable = NotificationCard::new(
        "notification-unreachable",
        NotificationKind::Attention,
        "Run command? pnpm test",
    )
    .source("oppm")
    .age("5 min ago")
    .unreachable(true)
    .action(ghost("notification-open", "Open"))
    .answer(primary("notification-unreachable-allow", "Allow"))
    .on_dismiss(|_, _| {});

    let updates = [
        UpdateState::Available {
            version: "1.4.2".into(),
        },
        UpdateState::Checking,
        UpdateState::Downloading { percent: 64 },
        UpdateState::Ready,
        UpdateState::Failed {
            reason: "offline".into(),
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, state)| {
        let mut card =
            NotificationCard::new(("notification-update", index), state.kind(), state.title())
                .source("MassOrca")
                .age("now")
                .spinner(state == UpdateState::Checking);
        if let Some(fraction) = state.progress() {
            card = card.progress(fraction);
        }
        for (action, label) in state.action_labels().iter().enumerate() {
            let id = format!("notification-update-{index}-{action}");
            let button = if action == 0 {
                primary(id, *label)
            } else {
                ghost(id, *label)
            };
            card = card.action(button);
        }
        card.on_dismiss(|_, _| {})
    })
    .collect::<Vec<_>>();
    let update_column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .children(updates.into_iter().map(IntoElement::into_any_element));

    let stack = NotificationStack::new("notification-stack")
        .cards(vec![
            NotificationCard::new("stack-1", NotificationKind::Info, "Acme: 2 commands added")
                .age("1 h ago"),
            NotificationCard::new("stack-2", NotificationKind::Update, "Update 1.4.2").age("now"),
            NotificationCard::new("stack-3", NotificationKind::SignIn, "Claude was signed out")
                .age("4 min ago"),
            NotificationCard::new(
                "stack-4",
                NotificationKind::Error,
                "NexusCare: the run failed",
            )
            .age("just now"),
            NotificationCard::new("stack-5", NotificationKind::Attention, "oppm: Run command?")
                .age("2 min ago"),
        ])
        .on_expand(|_, _| {});

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Attention: two answers and a dismiss",
            attention,
            cx,
        ))
        .child(variant("Error: a retry", error, cx))
        .child(variant("Sign-in needed: one action", sign_in, cx))
        .child(variant("Note: an extension card, name first", note, cx))
        .child(variant(
            "Update: available, checking, downloading, ready, failed",
            update_column,
            cx,
        ))
        .child(variant(
            "Host unreachable: last known state, answers disabled",
            unreachable,
            cx,
        ))
        .child(variant(
            "Stack: three in view, the rest behind +2 more",
            stack,
            cx,
        ))
        .into_any_element()
}

fn primary(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id, label)
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Compact)
}

fn ghost(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id, label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Compact)
}
