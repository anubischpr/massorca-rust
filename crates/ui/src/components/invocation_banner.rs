//! Invocation banner: example prompts tagged with an item, then how to ask it directly.

use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{Palette, RADIUS_MD, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM};
use gpui::{
    AnyElement, App, Div, ElementId, Empty, HighlightStyle, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement, Styled,
    StyledText, Window, div, px,
};
use std::ops::Range;

/// The sentence that closes the banner and names the tag.
fn mention_line(tag: &str) -> String {
    format!("Type {tag} in any chat or room to ask it directly.")
}

/// Where the tag occurs in a prompt, as byte ranges; an empty tag matches nothing.
fn tag_ranges(prompt: &str, tag: &str) -> Vec<Range<usize>> {
    if tag.is_empty() {
        return Vec::new();
    }
    prompt
        .match_indices(tag)
        .map(|(start, _)| start..start + tag.len())
        .collect()
}

/// One prompt with every occurrence of the tag in the accent colour.
fn prompt_line(prompt: &SharedString, tag: &str, p: &Palette) -> AnyElement {
    let accent = HighlightStyle {
        color: Some(p.accent.fg.into()),
        ..Default::default()
    };
    let highlights = tag_ranges(prompt, tag)
        .into_iter()
        .map(move |range| (range, accent));
    div()
        .text_body()
        .text_color(p.text.primary)
        .child(StyledText::new(prompt.clone()).with_highlights(highlights))
        .into_any_element()
}

/// The banner element, or `None` when there is no example to show.
fn banner_element(
    id: ElementId,
    tag: &str,
    examples: &[SharedString],
    p: &Palette,
) -> Option<Stateful<Div>> {
    if examples.is_empty() {
        return None;
    }
    let mention = SharedString::from(mention_line(tag));
    let described = examples
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    let mut prompts = div().flex().flex_col().gap(px(SPACE_1));
    for prompt in examples {
        prompts = prompts.child(prompt_line(prompt, tag, p));
    }
    Some(
        div()
            .id(id)
            .role(Role::Status)
            .aria_label(mention.clone())
            .aria_description(SharedString::from(described))
            .flex()
            .items_start()
            .gap(px(SPACE_2))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(p.status.info_border)
            .bg(p.status.info_bg)
            .child(
                div().flex_none().pt(px(SPACE_1)).child(
                    Icon::new(IconName::Info)
                        .size(IconSize::Small)
                        .color(p.status.info_fg),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_2))
                    .child(prompts)
                    .child(
                        div()
                            .text_style(TYPE_BODY_SM)
                            .text_color(p.text.secondary)
                            .child(mention),
                    ),
            ),
    )
}

/// Example prompts for an item, each with its tag highlighted, then how to type the tag in a chat.
#[derive(Debug, IntoElement)]
pub struct InvocationBanner {
    id: ElementId,
    tag: SharedString,
    examples: Vec<SharedString>,
}

impl InvocationBanner {
    /// `tag` is the item's mention, such as "@GitHub".
    pub fn new(id: impl Into<ElementId>, tag: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
            examples: Vec::new(),
        }
    }

    /// One example prompt, such as "@GitHub review my open pull requests".
    pub fn example(mut self, prompt: impl Into<SharedString>) -> Self {
        self.examples.push(prompt.into());
        self
    }
}

impl RenderOnce for InvocationBanner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        match banner_element(self.id, &self.tag, &self.examples, &theme.palette) {
            Some(shown) => shown.into_any_element(),
            None => Empty.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[test]
    fn marketplace_invocation_banner_mention_line_names_the_tag() {
        assert_eq!(
            mention_line("@GitHub"),
            "Type @GitHub in any chat or room to ask it directly."
        );
    }

    #[test]
    fn marketplace_invocation_banner_highlights_every_tag_in_a_prompt() {
        assert_eq!(
            tag_ranges("@GitHub review my pull requests", "@GitHub"),
            vec![0..7]
        );
        assert_eq!(
            tag_ranges("ask @GitHub, then @GitHub again", "@GitHub"),
            vec![4..11, 18..25]
        );
        assert!(tag_ranges("no tag here", "@GitHub").is_empty());
        assert!(tag_ranges("anything", "").is_empty());
    }

    #[gpui::test]
    fn marketplace_invocation_banner_is_a_status_named_by_the_mention_line(
        cx: &mut TestAppContext,
    ) {
        let node = root(cx, |_, cx| {
            let p = cx.theme().palette;
            let examples = vec![SharedString::from("@GitHub review my pull requests")];
            banner_element(ElementId::from("banner"), "@GitHub", &examples, &p).and_then(reported)
        })
        .expect("the banner is reported");
        assert_eq!(node.role(), Role::Status);
        assert_eq!(
            node.label(),
            Some("Type @GitHub in any chat or room to ask it directly.")
        );
        assert_eq!(node.description(), Some("@GitHub review my pull requests"));
    }

    #[gpui::test]
    fn marketplace_invocation_banner_has_no_element_without_examples(cx: &mut TestAppContext) {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let absent = cx.update(|app| {
            let p = app.theme().palette;
            let none: Vec<SharedString> = Vec::new();
            banner_element(ElementId::from("banner"), "@GitHub", &none, &p).is_none()
        });
        assert!(absent, "a banner with no examples draws nothing");
    }
}
