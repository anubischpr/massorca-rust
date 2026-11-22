//! Danger zone: the last section of an item page, where an installed item is uninstalled.

use crate::components::button::Button;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{Palette, RADIUS_LG, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_TITLE_SM};
use gpui::{
    App, Div, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

/// The fixed title of the zone.
const TITLE: &str = "Danger Zone";

/// The zone's heading: a warning glyph in the danger colour and the title.
fn zone_title(id: impl Into<ElementId>, p: &Palette) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Heading)
        .aria_label(TITLE)
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(IconSize::Small)
                .color(p.status.danger_fg),
        )
        .child(
            div()
                .text_style(TYPE_TITLE_SM)
                .text_color(p.text.primary)
                .child(TITLE),
        )
}

/// A bordered zone at the end of a page, with what the action does and the destructive button.
#[derive(IntoElement)]
pub struct DangerZone {
    id: ElementId,
    description: Option<SharedString>,
    action: Option<Button>,
}

impl std::fmt::Debug for DangerZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DangerZone")
            .field("id", &self.id)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

impl DangerZone {
    /// A zone titled "Danger Zone".
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            description: None,
            action: None,
        }
    }

    /// What the action does, in one or two sentences.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// The destructive button, for example "Uninstall".
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button);
        self
    }
}

impl RenderOnce for DangerZone {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let mut zone = div()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label(TITLE)
            .flex()
            .flex_col()
            .gap(px(SPACE_3))
            .p(px(SPACE_4))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.status.danger_border)
            .bg(p.surface.raised)
            .child(zone_title((self.id.clone(), "title"), p));
        if let Some(text) = self.description {
            zone = zone.aria_description(text.clone()).child(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(text),
            );
        }
        zone.children(self.action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn marketplace_danger_zone_is_a_group_named_danger_zone_and_described_by_its_text(
        cx: &mut TestAppContext,
    ) {
        let node = root(cx, |window, cx| {
            reported(
                DangerZone::new("danger")
                    .description("Removes the extension and its settings.")
                    .render(window, cx),
            )
        })
        .expect("the zone is reported");
        assert_eq!(node.role(), Role::Group);
        assert_eq!(node.label(), Some("Danger Zone"));
        assert_eq!(
            node.description(),
            Some("Removes the extension and its settings.")
        );
    }

    #[gpui::test]
    fn marketplace_danger_zone_title_is_a_heading(cx: &mut TestAppContext) {
        let node = root(cx, |_, cx| {
            let p = cx.theme().palette;
            reported(zone_title("title", &p))
        })
        .expect("the title is reported");
        assert_eq!(node.role(), Role::Heading);
        assert_eq!(node.label(), Some("Danger Zone"));
    }
}
