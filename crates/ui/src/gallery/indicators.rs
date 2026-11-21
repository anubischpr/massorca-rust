//! Gallery stories for the indicators group.

use super::{Story, variant};
use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::theme::TextStyled;
use crate::tokens::{SPACE_2, SPACE_4};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Agent icon",
        spec: "AgentIcon.md",
        render: agent_icon,
    }]
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
