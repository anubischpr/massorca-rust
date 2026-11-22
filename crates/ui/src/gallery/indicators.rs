//! Gallery stories for the indicators group.

use super::{Story, variant};
use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::progress::{ProgressBar, ProgressState, Spinner};
use crate::components::skeleton::{LineWidth, Skeleton, SkeletonShape};
use crate::components::usage_ring::{UsageReading, UsageRing, UsageSlot};
use crate::icon::IconSize;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{RADIUS_MD, SPACE_2, SPACE_4};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Agent icon",
            spec: "AgentIcon.md",
            render: agent_icon,
        },
        Story {
            name: "Agent status",
            spec: "AgentStatus.md",
            render: agent_status,
        },
        Story {
            name: "Progress",
            spec: "Progress.md",
            render: progress,
        },
        Story {
            name: "Skeleton",
            spec: "Skeleton.md",
            render: skeleton,
        },
        Story {
            name: "Usage ring",
            spec: "UsageRing.md",
            render: usage_ring,
        },
    ]
}

fn agent_icon(_: &mut Window, cx: &mut App) -> AnyElement {
    let marks = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_4))
        .children(AgentKind::ALL.map(|kind| AgentIcon::new(kind).size(AgentIconSize::Large)));
    let sizes = div().flex().items_center().gap(px(SPACE_4)).children(
        [
            AgentIconSize::XSmall,
            AgentIconSize::Name,
            AgentIconSize::Small,
            AgentIconSize::Medium,
            AgentIconSize::Large,
        ]
        .map(|size| AgentIcon::new(AgentKind::Gemini).size(size)),
    );
    let beside_name = div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .text_body()
        .text_color(cx.theme().palette.text.primary)
        .child(AgentIcon::new(AgentKind::GentleShell).size(AgentIconSize::Name))
        .child("Gentle Shell session");
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Every agent", marks, cx))
        .child(variant("Sizes, 12 to 24 px", sizes, cx))
        .child(variant("Icon beside a name", beside_name, cx))
        .into_any_element()
}

fn agent_status(_: &mut Window, cx: &mut App) -> AnyElement {
    let palette = cx.theme().palette;
    let states_at = |size: AgentStatusSize| {
        div()
            .flex()
            .items_center()
            .gap(px(SPACE_4))
            .children(AgentState::ALL.map(|state| AgentStatus::new(state).size(size)))
    };
    let row = |state: AgentState, name: &'static str| {
        let text = if state == AgentState::Unreachable {
            palette.text.tertiary
        } else {
            palette.text.primary
        };
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .p(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .text_body()
            .text_color(text)
            .child(AgentStatus::new(state))
            .child(name);
        if let Some(tint) = state.row_tint(&palette) {
            row = row.bg(tint);
        }
        row
    };
    let tinted = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(row(AgentState::Waiting, "Session needs an answer"))
        .child(row(AgentState::Error, "Build failed"));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Six states, 20 px",
            states_at(AgentStatusSize::Medium),
            cx,
        ))
        .child(variant(
            "Six states, 16 px",
            states_at(AgentStatusSize::Small),
            cx,
        ))
        .child(variant(
            "Six states, 12 px",
            states_at(AgentStatusSize::XSmall),
            cx,
        ))
        .child(variant("Waiting and error tint the row", tinted, cx))
        .child(variant(
            "Unreachable dims the row by text colour",
            row(AgentState::Unreachable, "Last known: running, 5 min ago"),
            cx,
        ))
        .into_any_element()
}

fn progress(_: &mut Window, cx: &mut App) -> AnyElement {
    let bar_width = px(320.);
    let bar = |bar: ProgressBar| div().w(bar_width).child(bar);
    let spinners = div()
        .flex()
        .items_center()
        .gap(px(SPACE_4))
        .child(Spinner::new().size(IconSize::XSmall))
        .child(Spinner::new())
        .child(Spinner::new().size(IconSize::Medium))
        .child(Spinner::new().size(IconSize::Large));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Determinate",
            bar(ProgressBar::new("Creating worktree oppm-api").determinate(0.62)),
            cx,
        ))
        .child(variant(
            "Indeterminate, travels",
            bar(ProgressBar::new("Uploading 3 files").indeterminate()),
            cx,
        ))
        .child(variant(
            "Paused",
            bar(ProgressBar::new("Copying repository")
                .determinate(0.4)
                .state(ProgressState::Paused)),
            cx,
        ))
        .child(variant(
            "Complete",
            bar(ProgressBar::new("Creating worktree oppm-api")
                .determinate(1.0)
                .state(ProgressState::Complete)),
            cx,
        ))
        .child(variant(
            "Failed",
            bar(ProgressBar::new("Creating worktree oppm-api")
                .determinate(0.62)
                .state(ProgressState::Failed)),
            cx,
        ))
        .child(variant(
            "Connection lost",
            bar(ProgressBar::new("Uploading 3 files")
                .determinate(0.62)
                .state(ProgressState::Disconnected {
                    age: "5 min ago".into(),
                })),
            cx,
        ))
        .child(variant("Spinner, 12, 16, 20 and 24 px", spinners, cx))
        .child(variant(
            "Spinner, shown after 200 ms",
            Spinner::new().delayed("gallery-progress-spinner"),
            cx,
        ))
        .into_any_element()
}

fn skeleton(_: &mut Window, cx: &mut App) -> AnyElement {
    let width = px(320.);
    let lines = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .w(width)
        .child(Skeleton::line(LineWidth::Full))
        .child(Skeleton::line(LineWidth::Most))
        .child(Skeleton::line(LineWidth::Partial));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Text lines, 100, 80 and 60 percent", lines, cx))
        .child(variant(
            "Row",
            div().w(width).child(Skeleton::new(SkeletonShape::Row)),
            cx,
        ))
        .child(variant(
            "Card",
            div().w(width).child(Skeleton::new(SkeletonShape::Card)),
            cx,
        ))
        .child(variant("Avatar", Skeleton::new(SkeletonShape::Avatar), cx))
        .child(variant(
            "Block",
            div()
                .w(width)
                .h(px(120.))
                .child(Skeleton::new(SkeletonShape::Block)),
            cx,
        ))
        .child(variant(
            "Shown after 100 ms",
            div()
                .w(width)
                .child(Skeleton::new(SkeletonShape::Row).delayed("gallery-skeleton-row")),
            cx,
        ))
        .into_any_element()
}

fn usage_ring(_: &mut Window, cx: &mut App) -> AnyElement {
    let slot = |index: u8| UsageSlot::new(index).expect("slots run from 1 to 6");
    let slots = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_4))
        .child(UsageRing::new(AgentKind::Claude, UsageReading::Percent(20.0)).slot(slot(1)))
        .child(UsageRing::new(AgentKind::Codex, UsageReading::Percent(35.0)).slot(slot(2)))
        .child(UsageRing::new(AgentKind::Antigravity, UsageReading::Percent(50.0)).slot(slot(3)))
        .child(UsageRing::new(AgentKind::GentleShell, UsageReading::Percent(62.0)).slot(slot(4)))
        .child(UsageRing::new(AgentKind::Gemini, UsageReading::Percent(40.0)).slot(slot(5)))
        .child(UsageRing::new(AgentKind::OpenCode, UsageReading::Percent(10.0)).slot(slot(6)));
    let thresholds = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_4))
        .child(UsageRing::new(AgentKind::Claude, UsageReading::Percent(62.0)).resets_in("2 days"))
        .child(UsageRing::new(
            AgentKind::Claude,
            UsageReading::Percent(84.0),
        ))
        .child(UsageRing::new(
            AgentKind::Claude,
            UsageReading::Percent(97.0),
        ))
        .child(UsageRing::new(
            AgentKind::Claude,
            UsageReading::Percent(120.0),
        ));
    let states = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_4))
        .child(UsageRing::new(AgentKind::Cursor, UsageReading::Loading))
        .child(UsageRing::new(AgentKind::Cursor, UsageReading::NotReported))
        .child(UsageRing::new(AgentKind::Cursor, UsageReading::Unavailable))
        .child(UsageRing::new(
            AgentKind::Claude,
            UsageReading::Failed { last: Some(62.0) },
        ))
        .child(
            UsageRing::new(AgentKind::Claude, UsageReading::Percent(62.0))
                .disconnected("5 min ago"),
        );
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Provider slots 1 to 6", slots, cx))
        .child(variant(
            "Warn from 80 percent, limit from 95",
            thresholds,
            cx,
        ))
        .child(variant(
            "Loading, empty, unavailable, error and unreachable",
            states,
            cx,
        ))
        .into_any_element()
}
