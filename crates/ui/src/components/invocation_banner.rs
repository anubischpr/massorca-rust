//! Invocation banner: example prompts tagged with an item, then how to ask it directly.

use crate::components::terminal_surface::colors::mix;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{Color, Palette, RADIUS_MD, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM};
use gpui::{
    AnyElement, App, Div, ElementId, Empty, HighlightStyle, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement, Styled,
    StyledText, Window, div, px,
};
use std::ops::Range;

/// How much of the first listing colour tints a banner's background.
pub const TINT_ALPHA: f32 = 0.14;

/// A `#RRGGBB` colour as a listing writes it, or `None` when the text is not one.
pub fn hex_colour(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some(Color::rgba8(channel(0)?, channel(2)?, channel(4)?, 1.0))
}

/// The background of a tinted banner: the colour's hue at a low alpha, so the prompts stay readable in both themes.
pub fn tint_fill(colour: Color) -> Color {
    Color {
        a: TINT_ALPHA,
        ..colour
    }
}

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

/// How many tints a stop tries, from its full colour down to the Canvas.
const TINT_STEPS: u32 = 20;
/// The contrast the footer sentence keeps on each gradient stop (WCAG AA for body text).
const READABLE_CONTRAST: f32 = 4.5;

/// The strongest tint of `colour` over the Canvas that keeps `text` readable, or the Canvas when none does.
pub fn readable_stop(colour: Color, canvas: Color, text: Color) -> Color {
    (0..=TINT_STEPS)
        .rev()
        .map(|step| mix(canvas, colour, step as f32 / TINT_STEPS as f32))
        .find(|stop| text.contrast(*stop) >= READABLE_CONTRAST)
        .unwrap_or(canvas)
}

/// The two gradient stops: each listed colour as readable as it can be; with one colour the end fades halfway
/// into the Canvas.
pub fn gradient_stops(
    first: Color,
    second: Option<Color>,
    canvas: Color,
    text: Color,
) -> (Color, Color) {
    let start = readable_stop(first, canvas, text);
    let end = match second {
        Some(colour) => readable_stop(colour, canvas, text),
        None => mix(start, canvas, 0.5),
    };
    (start, end)
}

/// The prompt without the tag that its chip shows; the whole prompt when nothing follows the tag.
pub fn prompt_text<'a>(prompt: &'a str, tag: &str) -> &'a str {
    match prompt.strip_prefix(tag).map(str::trim_start) {
        Some(rest) if !rest.is_empty() => rest,
        _ => prompt,
    }
}

/// Example prompts for an item, each with its tag highlighted, then how to type the tag in a chat.
#[derive(Debug, IntoElement)]
pub struct InvocationBanner {
    id: ElementId,
    tag: SharedString,
    examples: Vec<SharedString>,
    /// The item's colours: the first tints the background, the second, if any, the border.
    tint: Option<(Color, Option<Color>)>,
}

impl InvocationBanner {
    /// `tag` is the item's mention, such as "@GitHub".
    pub fn new(id: impl Into<ElementId>, tag: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
            examples: Vec::new(),
            tint: None,
        }
    }

    /// One example prompt, such as "@GitHub review my open pull requests".
    pub fn example(mut self, prompt: impl Into<SharedString>) -> Self {
        self.examples.push(prompt.into());
        self
    }

    /// Tints the banner with the item's colours: the first softly behind the prompts, the second on the border.
    pub fn tint(mut self, first: Color, accent: Option<Color>) -> Self {
        self.tint = Some((first, accent));
        self
    }
}

impl RenderOnce for InvocationBanner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        match banner_element(self.id, &self.tag, &self.examples, &theme.palette) {
            Some(shown) => match self.tint {
                Some((first, accent)) => shown
                    .bg(tint_fill(first))
                    .border_color(accent.unwrap_or(first))
                    .into_any_element(),
                None => shown.into_any_element(),
            },
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
    fn marketplace_invocation_banner_reads_hex_colours_and_tints_them_softly() {
        let claude = Color::rgba8(0xD9, 0x77, 0x57, 1.0);
        assert_eq!(hex_colour("#D97757"), Some(claude));
        assert_eq!(hex_colour("D97757"), None);
        assert_eq!(hex_colour("#D9775"), None);
        assert_eq!(hex_colour("#GG7757"), None);
        assert_eq!(
            tint_fill(claude),
            Color {
                a: TINT_ALPHA,
                ..claude
            }
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

    const CLAUDE_BRAND: Color = Color::rgba8(0xD9, 0x77, 0x57, 1.0);
    const GREEN: Color = Color::rgba8(0x3F, 0x9B, 0x6E, 1.0);
    const GREY: Color = Color::rgba8(0x8E, 0x8E, 0x93, 1.0);

    /// Every colour an item can be tinted with: listed brands, and the six slots of both themes.
    fn brands() -> Vec<Color> {
        let mut colours = vec![CLAUDE_BRAND, GREEN, GREY];
        for palette in [Palette::DARK, Palette::LIGHT] {
            let usage = palette.usage;
            colours.extend([
                usage.ring_1,
                usage.ring_2,
                usage.ring_3,
                usage.ring_4,
                usage.ring_5,
                usage.ring_6,
            ]);
        }
        colours
    }

    #[test]
    fn marketplace_invocation_banner_pill_text_drops_only_the_leading_tag() {
        assert_eq!(
            prompt_text("@Claude review my changes", "@Claude"),
            "review my changes"
        );
        assert_eq!(
            prompt_text("ask @Claude later", "@Claude"),
            "ask @Claude later"
        );
        assert_eq!(prompt_text("@Claude", "@Claude"), "@Claude");
    }

    #[test]
    fn marketplace_invocation_banner_gradient_takes_each_listed_colour_into_its_stop() {
        let p = Palette::DARK;
        let (start, end) = gradient_stops(GREEN, Some(GREY), p.surface.canvas, p.text.secondary);
        assert_eq!(
            start,
            readable_stop(GREEN, p.surface.canvas, p.text.secondary)
        );
        assert_eq!(end, readable_stop(GREY, p.surface.canvas, p.text.secondary));
        assert_ne!(start, end, "two listed colours make a visible gradient");
    }

    #[test]
    fn marketplace_invocation_banner_with_one_colour_fades_its_end_into_the_canvas() {
        let p = Palette::DARK;
        let (start, end) = gradient_stops(CLAUDE_BRAND, None, p.surface.canvas, p.text.secondary);
        assert_eq!(end, mix(start, p.surface.canvas, 0.5));
        assert_ne!(start, end);
    }

    #[test]
    fn marketplace_invocation_banner_footer_stays_readable_on_both_stops_in_both_themes() {
        for palette in [Palette::DARK, Palette::LIGHT] {
            let (canvas, text) = (palette.surface.canvas, palette.text.secondary);
            for brand in brands() {
                let (start, end) = gradient_stops(brand, Some(brand), canvas, text);
                let (_, faded) = gradient_stops(brand, None, canvas, text);
                for stop in [start, end, faded] {
                    assert!(
                        text.contrast(stop) >= 4.5,
                        "the footer on {stop:?} is below 4.5:1 (brand {brand:?})"
                    );
                }
            }
        }
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
