//! Gallery stories for the indicators group.

use super::{Story, variant};
use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::progress::{ProgressBar, ProgressState, Spinner};
use crate::icon::IconSize;
use crate::theme::TextStyled;
use crate::tokens::{SPACE_2, SPACE_4};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Agent icon",
            spec: "AgentIcon.md",
            render: agent_icon,
        },
        Story {
            name: "Progress",
            spec: "Progress.md",
            render: progress,
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
