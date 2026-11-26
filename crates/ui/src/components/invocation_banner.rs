//! Invocation banner: example prompts for an item, each a pill that opens a chat with it, then how to ask it directly.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::terminal_surface::colors::mix;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, Palette, RADIUS_2XL, RADIUS_FULL, RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, SPACE_4,
    SPACE_6, TYPE_BODY_SM,
};
use gpui::{
    AnyElement, App, Div, ElementId, Empty, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window, div, linear_color_stop, linear_gradient, px, svg,
};
use std::fmt;
use std::rc::Rc;

/// The contrast the footer keeps on each listed gradient stop (WCAG AA for body text).
const READABLE_CONTRAST: f32 = 4.5;
/// How many tints a stop tries, from its full colour down to the Canvas in steps of 5 percent.
const TINT_STEPS: u32 = 20;
/// The gradient runs from the top left corner to the bottom right, as CSS writes 135 degrees.
const GRADIENT_ANGLE: f32 = 135.0;
/// The widest a pill grows; the pills share one column, narrower in a narrow window.
const PILL_MAX_WIDTH: f32 = 560.0;
/// A pill is 44 px high. The measure comes from the approved screenshot, not a token.
const PILL_HEIGHT: f32 = 44.0;
/// The tag chip is 24 px high.
const TAG_HEIGHT: f32 = 24.0;
/// The tag's mark is 14 px, as the artboard draws it. No icon token is 14 px, so the mark is drawn at this size.
const TAG_MARK_PX: f32 = 14.0;

/// Runs when a pill is pressed, with the prompt's full text.
type OnSelect = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// A `#RRGGBB` colour as a listing writes it, or `None` when the text is not one.
pub fn hex_colour(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some(Color::rgba8(channel(0)?, channel(2)?, channel(4)?, 1.0))
}

/// The strongest tint of `colour` over the Canvas that keeps `text` at 4.5:1, or the Canvas when none does.
pub fn readable_stop(colour: Color, canvas: Color, text: Color) -> Color {
    (0..=TINT_STEPS)
        .rev()
        .map(|step| mix(canvas, colour, step as f32 / TINT_STEPS as f32))
        .find(|stop| text.contrast(*stop) >= READABLE_CONTRAST)
        .unwrap_or(canvas)
}

/// Where a banner's gradient starts: the item's listed colours, or one palette colour for an item with none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BannerFill {
    /// One or two listed colours: the gradient runs from the first to the second, or from the first to `banner.end`.
    Listed(Color, Option<Color>),
    /// A `banner.palette.N` colour: the gradient runs from it to `banner.end`.
    Palette(Color),
}

/// The footer's colour, `text.primary`: the text colour that every gradient stop keeps at 4.5:1.
pub fn footer_colour(palette: &Palette) -> Color {
    palette.text.primary
}

/// The two gradient stops for `fill`. A listed colour is tinted until the footer reads at 4.5:1 (`readable_stop`).
/// A palette colour already reads at 4.5:1, so it starts the gradient as it is. `banner.end` closes a gradient that
/// has one colour or none.
pub fn gradient_stops(fill: BannerFill, palette: &Palette) -> (Color, Color) {
    let (canvas, text) = (palette.surface.canvas, footer_colour(palette));
    match fill {
        BannerFill::Listed(first, Some(second)) => (
            readable_stop(first, canvas, text),
            readable_stop(second, canvas, text),
        ),
        BannerFill::Listed(first, None) => (readable_stop(first, canvas, text), palette.banner.end),
        BannerFill::Palette(colour) => (colour, palette.banner.end),
    }
}

/// The prompt without the tag that its chip shows; the whole prompt when nothing follows the tag.
pub fn prompt_text<'a>(prompt: &'a str, tag: &str) -> &'a str {
    match prompt.strip_prefix(tag).map(str::trim_start) {
        Some(rest) if !rest.is_empty() => rest,
        _ => prompt,
    }
}

/// The footer sentence that names the tag.
fn mention_line(tag: &str) -> String {
    format!("Type {tag} in any chat or room to ask it directly.")
}

/// The item's mark and name at the start of a pill.
#[derive(Clone, Debug)]
struct TagChip {
    name: SharedString,
    icon: IconName,
    agent: Option<AgentKind>,
}

impl TagChip {
    /// The chip's box and text style, without its contents.
    fn shape(&self, p: &Palette) -> Div {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .h(px(TAG_HEIGHT))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_FULL))
            .bg(p.badge.neutral_bg)
            .text_style(TYPE_BODY_SM)
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.badge.neutral_fg)
    }

    /// The chip with its mark and name, in the pill at `index`.
    fn element(&self, p: &Palette, index: usize) -> AnyElement {
        let mark = match self.agent {
            Some(kind) => AgentIcon::new(kind)
                .size(AgentIconSize::Name)
                .into_any_element(),
            None => svg()
                .path(self.icon.path())
                .size(px(TAG_MARK_PX))
                .flex_none()
                .text_color(p.badge.neutral_fg)
                .into_any_element(),
        };
        self.shape(p)
            .debug_selector(move || format!("invocation-chip-{index}"))
            .child(
                div()
                    .debug_selector(move || format!("invocation-mark-{index}"))
                    .flex_none()
                    .child(mark),
            )
            .child(self.name.clone())
            .into_any_element()
    }
}

/// One example prompt as a pill: a focusable button with the tag chip, the prompt on one line and an arrow.
#[derive(IntoElement)]
struct Pill {
    index: usize,
    chip: TagChip,
    prompt: SharedString,
    text: SharedString,
    on_select: Option<OnSelect>,
}

impl Pill {
    fn id(&self) -> ElementId {
        ElementId::NamedInteger("invocation-pill".into(), self.index as u64)
    }

    /// The pill's box, colours, chip, text and arrow. Focus and presses are added when it is drawn.
    fn shape(&self, p: &Palette) -> Stateful<Div> {
        let (index, hover, pressed) = (self.index, p.fill.hover_raised, p.fill.pressed_overlay);
        div()
            .id(self.id())
            .role(Role::Button)
            .aria_label(self.prompt.clone())
            .debug_selector(move || format!("invocation-pill-{index}"))
            .w_full()
            .max_w(px(PILL_MAX_WIDTH))
            .h(px(PILL_HEIGHT))
            .flex()
            .items_center()
            .gap(px(SPACE_3))
            .pl(px(SPACE_2))
            .pr(px(SPACE_4))
            .rounded(px(RADIUS_LG))
            .bg(p.surface.raised)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .active(move |style| style.bg(pressed))
            .child(self.chip.element(p, index))
            .child(
                div()
                    .debug_selector(move || format!("invocation-text-{index}"))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_body()
                    .text_color(p.text.primary)
                    .child(self.text.clone()),
            )
            .child(
                div()
                    .debug_selector(move || format!("invocation-arrow-{index}"))
                    .flex_none()
                    .child(
                        Icon::new(IconName::ArrowRight)
                            .size(IconSize::Small)
                            .color(p.text.secondary),
                    ),
            )
    }
}

impl RenderOnce for Pill {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let focus = window
            .use_keyed_state(self.id(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone()
            .tab_stop(true);
        let mut pill = self
            .shape(&theme.palette)
            .track_focus(&focus)
            .focus_ring(&theme);
        if let Some(on_select) = self.on_select {
            let prompt = self.prompt;
            pill = pill.on_click(move |_, window, cx| on_select(prompt.clone(), window, cx));
        }
        pill
    }
}

/// Example prompts for an item: a gradient panel of pills, each opening a chat with its prompt.
#[derive(IntoElement)]
pub struct InvocationBanner {
    id: ElementId,
    chip: TagChip,
    tag: SharedString,
    examples: Vec<SharedString>,
    fill: Option<BannerFill>,
    on_select: Option<OnSelect>,
}

impl InvocationBanner {
    /// `name` labels the tag chip and `tag` is the item's mention, such as "@GitHub".
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        tag: impl Into<SharedString>,
        icon: IconName,
    ) -> Self {
        Self {
            id: id.into(),
            chip: TagChip {
                name: name.into(),
                icon,
                agent: None,
            },
            tag: tag.into(),
            examples: Vec::new(),
            fill: None,
            on_select: None,
        }
    }

    /// Draws the agent's own mark in the tag chip instead of the icon.
    pub fn agent(mut self, kind: AgentKind) -> Self {
        self.chip.agent = Some(kind);
        self
    }

    /// Where the gradient starts: the item's listed colours, or one palette colour.
    pub fn fill(mut self, fill: BannerFill) -> Self {
        self.fill = Some(fill);
        self
    }

    /// One example prompt, such as "@GitHub review my open pull requests".
    pub fn example(mut self, prompt: impl Into<SharedString>) -> Self {
        self.examples.push(prompt.into());
        self
    }

    /// Runs `listener` with a pill's full prompt when the pill is pressed.
    pub fn on_select(
        mut self,
        listener: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(listener));
        self
    }

    /// The banner element, or `None` when there is no example to show.
    fn element(&self, p: &Palette) -> Option<Stateful<Div>> {
        if self.examples.is_empty() {
            return None;
        }
        let mention = SharedString::from(mention_line(&self.tag));
        let fill = self.fill.unwrap_or(BannerFill::Palette(p.banner.palette_1));
        let (start, end) = gradient_stops(fill, p);
        let mut pills = div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(SPACE_2));
        for (index, prompt) in self.examples.iter().enumerate() {
            pills = pills.child(Pill {
                index,
                chip: self.chip.clone(),
                prompt: prompt.clone(),
                text: SharedString::from(prompt_text(prompt, &self.tag).to_owned()),
                on_select: self.on_select.clone(),
            });
        }
        Some(
            div()
                .id(self.id.clone())
                .role(Role::Group)
                .aria_label(mention.clone())
                .debug_selector(|| "invocation-banner".to_owned())
                .flex()
                .flex_col()
                .items_center()
                .gap(px(SPACE_3))
                .p(px(SPACE_6))
                .rounded(px(RADIUS_2XL))
                .bg(linear_gradient(
                    GRADIENT_ANGLE,
                    linear_color_stop(start, 0.0),
                    linear_color_stop(end, 1.0),
                ))
                .child(pills)
                .child(
                    div()
                        .debug_selector(|| "invocation-footer".to_owned())
                        .w_full()
                        .text_style(TYPE_BODY_SM)
                        .text_color(footer_colour(p))
                        .text_center()
                        .child(mention),
                ),
        )
    }
}

impl fmt::Debug for InvocationBanner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InvocationBanner")
            .field("id", &self.id)
            .field("tag", &self.tag)
            .field("examples", &self.examples)
            .finish_non_exhaustive()
    }
}

impl RenderOnce for InvocationBanner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        match self.element(&theme.palette) {
            Some(banner) => banner.into_any_element(),
            None => Empty.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{
        AbsoluteLength, Bounds, Context, Modifiers, Pixels, TestAppContext, VisualTestContext,
        accesskit::Role,
    };
    use std::cell::RefCell;

    const CLAUDE: Color = Color::rgba8(0xD9, 0x77, 0x57, 1.0);
    const GREEN: Color = Color::rgba8(0x3F, 0x9B, 0x6E, 1.0);
    const GREY: Color = Color::rgba8(0x8E, 0x8E, 0x93, 1.0);
    const FOOTER: &str = "Type @Claude in any chat or room to ask it directly.";
    /// The pills' debug selectors, in order; the test API takes selectors that live for the whole program.
    const PILLS: [&str; 3] = [
        "invocation-pill-0",
        "invocation-pill-1",
        "invocation-pill-2",
    ];

    /// The palette's eight banner colours and its end stop.
    fn palette_tokens(palette: &Palette) -> [Color; 9] {
        let b = palette.banner;
        [
            b.palette_1,
            b.palette_2,
            b.palette_3,
            b.palette_4,
            b.palette_5,
            b.palette_6,
            b.palette_7,
            b.palette_8,
            b.end,
        ]
    }

    /// Every colour a banner can start from: listed brands, and the palette of both themes.
    fn brands() -> Vec<Color> {
        let mut colours = vec![CLAUDE, GREEN, GREY];
        for palette in [Palette::DARK, Palette::LIGHT] {
            colours.extend(palette_tokens(&palette));
        }
        colours
    }

    fn claude_chip() -> TagChip {
        TagChip {
            name: SharedString::from("Claude"),
            icon: IconName::Bot,
            agent: Some(AgentKind::Claude),
        }
    }

    fn claude_banner() -> InvocationBanner {
        InvocationBanner::new("banner", "Claude", "@Claude", IconName::Bot)
            .agent(AgentKind::Claude)
            .fill(BannerFill::Listed(CLAUDE, None))
            .example("@Claude review my changes")
            .example("@Claude explain the failing test in this project")
            .example("@Claude summarise today's sessions")
    }

    #[test]
    fn marketplace_invocation_banner_mention_line_names_the_tag() {
        assert_eq!(
            mention_line("@GitHub"),
            "Type @GitHub in any chat or room to ask it directly."
        );
    }

    #[test]
    fn marketplace_invocation_banner_reads_hex_colours() {
        assert_eq!(hex_colour("#D97757"), Some(CLAUDE));
        assert_eq!(hex_colour("D97757"), None);
        assert_eq!(hex_colour("#D9775"), None);
        assert_eq!(hex_colour("#GG7757"), None);
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
    fn marketplace_invocation_banner_two_listed_colours_run_from_the_first_to_the_second() {
        let p = Palette::DARK;
        let (canvas, text) = (p.surface.canvas, footer_colour(&p));
        let (start, end) = gradient_stops(BannerFill::Listed(GREEN, Some(GREY)), &p);
        assert_eq!(start, readable_stop(GREEN, canvas, text));
        assert_eq!(end, readable_stop(GREY, canvas, text));
        assert_ne!(start, end, "two listed colours make a visible gradient");
    }

    #[test]
    fn marketplace_invocation_banner_one_listed_colour_ends_on_banner_end() {
        for p in [Palette::DARK, Palette::LIGHT] {
            let (start, end) = gradient_stops(BannerFill::Listed(CLAUDE, None), &p);
            assert_eq!(
                start,
                readable_stop(CLAUDE, p.surface.canvas, footer_colour(&p))
            );
            assert_eq!(
                end, p.banner.end,
                "the end is banner.end, not a fade to the Canvas"
            );
        }
    }

    #[test]
    fn marketplace_invocation_banner_no_listed_colour_starts_on_its_palette_colour() {
        for p in [Palette::DARK, Palette::LIGHT] {
            let slot = p.banner.palette_4;
            assert_eq!(
                gradient_stops(BannerFill::Palette(slot), &p),
                (slot, p.banner.end)
            );
        }
    }

    #[test]
    fn marketplace_invocation_banner_footer_is_text_primary() {
        assert_eq!(footer_colour(&Palette::DARK), Palette::DARK.text.primary);
        assert_eq!(footer_colour(&Palette::LIGHT), Palette::LIGHT.text.primary);
    }

    #[test]
    fn marketplace_invocation_banner_footer_reads_at_4_5_on_every_stop_in_both_themes() {
        for p in [Palette::DARK, Palette::LIGHT] {
            let text = footer_colour(&p);
            for brand in brands() {
                for fill in [
                    BannerFill::Listed(brand, None),
                    BannerFill::Listed(brand, Some(brand)),
                ] {
                    let (start, end) = gradient_stops(fill, &p);
                    for stop in [start, end] {
                        assert!(
                            text.contrast(stop) >= READABLE_CONTRAST,
                            "{stop:?} is under {fill:?}"
                        );
                    }
                }
            }
            for token in palette_tokens(&p) {
                assert!(
                    text.contrast(token) >= READABLE_CONTRAST,
                    "the palette or end colour {token:?} is below 4.5:1"
                );
            }
        }
    }

    #[gpui::test]
    fn marketplace_invocation_banner_is_a_group_named_by_its_footer_line(cx: &mut TestAppContext) {
        let node = root(cx, |_, cx| {
            let p = cx.theme().palette;
            claude_banner().element(&p).and_then(reported)
        })
        .expect("the banner is reported");
        assert_eq!(node.role(), Role::Group);
        assert_eq!(node.label(), Some(FOOTER));
        assert_eq!(node.description(), None, "the pills name themselves");
    }

    #[gpui::test]
    fn marketplace_invocation_banner_has_no_element_without_examples(cx: &mut TestAppContext) {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let absent = cx.update(|app| {
            let p = app.theme().palette;
            InvocationBanner::new("banner", "Claude", "@Claude", IconName::Bot)
                .element(&p)
                .is_none()
        });
        assert!(absent, "a banner with no examples draws nothing");
    }

    #[test]
    fn marketplace_invocation_banner_panel_has_no_border_a_gradient_and_16_px_corners() {
        let p = Palette::DARK;
        let mut banner = claude_banner()
            .element(&p)
            .expect("the banner has examples");
        let style = banner.style();
        let edges = &style.border_widths;
        assert!(
            edges.top.is_none()
                && edges.right.is_none()
                && edges.bottom.is_none()
                && edges.left.is_none(),
            "the panel draws no border"
        );
        assert!(style.background.is_some(), "the panel has a background");
        assert_eq!(
            style.corner_radii.top_left,
            Some(AbsoluteLength::Pixels(px(RADIUS_2XL)))
        );
    }

    #[test]
    fn marketplace_invocation_banner_pill_has_8_px_corners_and_its_chip_stays_full() {
        let p = Palette::DARK;
        let pill = Pill {
            index: 0,
            chip: claude_chip(),
            prompt: SharedString::from("@Claude review my changes"),
            text: SharedString::from("review my changes"),
            on_select: None,
        };
        let mut shape = pill.shape(&p);
        assert_eq!(
            shape.style().corner_radii.top_left,
            Some(AbsoluteLength::Pixels(px(RADIUS_LG)))
        );
        let mut chip = claude_chip().shape(&p);
        assert_eq!(
            chip.style().corner_radii.top_left,
            Some(AbsoluteLength::Pixels(px(RADIUS_FULL)))
        );
    }

    /// Hosts one banner in a window and records the prompts its pills report.
    struct Host {
        presses: Rc<RefCell<Vec<String>>>,
    }

    impl gpui::Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let presses = self.presses.clone();
            // A column keeps the banner at its content height; as the window root it would fill the window.
            div()
                .flex()
                .flex_col()
                .child(claude_banner().on_select(move |prompt, _, _| {
                    presses.borrow_mut().push(prompt.to_string());
                }))
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// The drawn bounds of the element with this debug selector, after a fresh draw.
    fn bounds_of(selector: &'static str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
        draw(cx);
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("no element with selector {selector}"))
    }

    fn px_of(value: Pixels) -> f32 {
        f32::from(value)
    }

    /// Asserts the sizes, gaps and insets the approved spec gives, on the drawn banner.
    fn check_the_spec_layout(cx: &mut VisualTestContext) {
        let panel = bounds_of("invocation-banner", cx);
        let footer = bounds_of("invocation-footer", cx);
        let pills = PILLS.map(|selector| bounds_of(selector, cx));
        let chip = bounds_of("invocation-chip-0", cx);
        let mark = bounds_of("invocation-mark-0", cx);
        let text = bounds_of("invocation-text-0", cx);
        let arrow = bounds_of("invocation-arrow-0", cx);
        let first = pills[0];
        let close = |actual: f32, expected: f32, what: &str| {
            assert!(
                (actual - expected).abs() < 0.5,
                "{what}: {actual} is not {expected}"
            );
        };

        for (index, pill) in pills.iter().enumerate() {
            close(
                px_of(pill.size.height),
                PILL_HEIGHT,
                &format!("pill {index} height"),
            );
            close(
                px_of(pill.size.width),
                px_of(first.size.width),
                "equal pill widths",
            );
            close(
                px_of(pill.center().x),
                px_of(panel.center().x),
                &format!("pill {index} centred"),
            );
        }
        for pair in pills.windows(2) {
            let bottom = px_of(pair[0].origin.y) + px_of(pair[0].size.height);
            close(
                px_of(pair[1].origin.y) - bottom,
                SPACE_2_PX,
                "8 px between pills",
            );
        }
        let last = pills[2];
        close(
            px_of(footer.origin.y) - (px_of(last.origin.y) + px_of(last.size.height)),
            SPACE_3_PX,
            "12 px from the last pill to the footer",
        );
        close(
            px_of(footer.center().x),
            px_of(panel.center().x),
            "the footer is centred",
        );

        close(
            px_of(first.origin.y) - px_of(panel.origin.y),
            SPACE_6_PX,
            "24 px panel top",
        );
        close(
            px_of(panel.origin.y) + px_of(panel.size.height)
                - (px_of(footer.origin.y) + px_of(footer.size.height)),
            SPACE_6_PX,
            "24 px panel bottom",
        );
        close(
            px_of(footer.origin.x) - px_of(panel.origin.x),
            SPACE_6_PX,
            "24 px panel left",
        );
        close(
            px_of(panel.origin.x) + px_of(panel.size.width)
                - (px_of(footer.origin.x) + px_of(footer.size.width)),
            SPACE_6_PX,
            "24 px panel right",
        );

        close(
            px_of(chip.origin.x) - px_of(first.origin.x),
            8.0,
            "8 px left of the chip",
        );
        close(
            px_of(first.origin.x) + px_of(first.size.width)
                - (px_of(arrow.origin.x) + px_of(arrow.size.width)),
            16.0,
            "16 px right of the arrow",
        );
        close(
            px_of(chip.size.height),
            TAG_HEIGHT,
            "the chip is 24 px high",
        );
        close(
            px_of(mark.size.width),
            TAG_MARK_PX,
            "the mark is 14 px wide",
        );
        close(
            px_of(mark.size.height),
            TAG_MARK_PX,
            "the mark is 14 px high",
        );
        close(px_of(arrow.size.width), 16.0, "the arrow is 16 px");
        close(
            px_of(text.origin.x) - (px_of(chip.origin.x) + px_of(chip.size.width)),
            12.0,
            "12 px from the chip to the prompt",
        );
        close(
            px_of(arrow.origin.x) - (px_of(text.origin.x) + px_of(text.size.width)),
            12.0,
            "12 px from the prompt to the arrow",
        );
    }

    const SPACE_2_PX: f32 = 8.0;
    const SPACE_3_PX: f32 = 12.0;
    const SPACE_6_PX: f32 = 24.0;

    #[gpui::test]
    fn marketplace_invocation_banner_lays_out_the_spec_sizes_in_the_dark_theme(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let (_, cx) = cx.add_window_view(|_, _| Host {
            presses: Rc::default(),
        });
        check_the_spec_layout(cx);
    }

    #[gpui::test]
    fn marketplace_invocation_banner_lays_out_the_spec_sizes_in_the_light_theme(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| {
            crate::init(crate::Appearance::Light, app).expect("design system initialises")
        });
        let (_, cx) = cx.add_window_view(|_, _| Host {
            presses: Rc::default(),
        });
        check_the_spec_layout(cx);
    }

    #[gpui::test]
    fn marketplace_invocation_banner_pressing_a_pill_reports_its_full_prompt(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let presses = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| Host {
            presses: presses.clone(),
        });
        let bounds = bounds_of("invocation-pill-1", cx);
        cx.simulate_click(bounds.center(), Modifiers::default());
        assert_eq!(
            presses.borrow().as_slice(),
            ["@Claude explain the failing test in this project"]
        );
    }

    #[gpui::test]
    fn marketplace_invocation_banner_pill_takes_keyboard_focus_in_tab_order(
        cx: &mut TestAppContext,
    ) {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let (_, cx) = cx.add_window_view(|_, _| Host {
            presses: Rc::default(),
        });
        draw(cx);
        cx.update(|window, cx| window.focus_next(cx));
        let focused = cx.update(|window, cx| window.focused(cx).is_some());
        assert!(focused, "Tab moves focus onto a pill");
    }

    /// Presses and releases one key, as the keyboard does.
    fn press(cx: &mut VisualTestContext, key: &str) {
        let os = cx.update(|_, cx| cx.platform_info().os);
        let keystroke = gpui::Keystroke::parse_with_os(key, os).expect("a valid keystroke");
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
    }

    #[gpui::test]
    fn marketplace_invocation_banner_space_presses_the_focused_pill(cx: &mut TestAppContext) {
        cx.update(|app| {
            crate::init(crate::Appearance::Dark, app).expect("design system initialises")
        });
        let presses = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| Host {
            presses: presses.clone(),
        });
        draw(cx);
        cx.update(|window, cx| window.focus_next(cx));
        press(cx, "space");
        assert_eq!(presses.borrow().as_slice(), ["@Claude review my changes"]);
    }
}
