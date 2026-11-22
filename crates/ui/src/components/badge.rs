//! Badge: a small label, count or dot on another element
//! (docs/design/components/Badge.md).
//!
//! A status badge always carries a glyph and a word; only a count or an unread
//! dot may stand alone, beside a labelled item.

use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, Palette, RADIUS_FULL, RADIUS_SM, SIZE_BADGE_HEIGHT, SIZE_INDICATOR_DOT, SPACE_1,
    TYPE_CAPTION, TYPE_LABEL, TextStyleToken,
};
use gpui::{
    AnyElement, App, Empty, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, px,
};

/// Longest text a badge shows before its ellipsis (Badge.md, long content).
pub const LABEL_LIMIT: usize = 16;

/// Largest count shown as a number; larger counts read "99+".
const COUNT_LIMIT: u32 = 99;

/// Transparent: a chip without a border or fill.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// What a badge means. Sets its colours and, for the status tones, its glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeTone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

impl BadgeTone {
    /// The glyph of a status tone; neutral and accent badges have none.
    pub fn glyph(self) -> Option<IconName> {
        match self {
            Self::Success => Some(IconName::CircleCheck),
            Self::Warning => Some(IconName::TriangleAlert),
            Self::Danger => Some(IconName::OctagonAlert),
            Self::Info => Some(IconName::Info),
            Self::Neutral | Self::Accent => None,
        }
    }
}

/// What a badge shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Content {
    Text(SharedString),
    Count(u32),
    Dot,
}

/// A small label, count or dot.
#[derive(Debug, IntoElement)]
pub struct Badge {
    content: Content,
    tone: BadgeTone,
    solid: bool,
    category: bool,
    glyph: Option<IconName>,
    disabled: bool,
}

impl Badge {
    /// A text badge. A status tone adds its glyph; `category` switches to the label style.
    pub fn text(label: impl Into<SharedString>) -> Self {
        Self::with(Content::Text(label.into()))
    }

    /// A count. Zero draws nothing: the badge is removed.
    pub fn count(count: u32) -> Self {
        Self::with(Content::Count(count))
    }

    /// A dot with no text, beside a labelled item.
    pub fn dot() -> Self {
        Self::with(Content::Dot)
    }

    fn with(content: Content) -> Self {
        Self {
            content,
            tone: BadgeTone::default(),
            solid: false,
            category: false,
            glyph: None,
            disabled: false,
        }
    }

    pub fn tone(mut self, tone: BadgeTone) -> Self {
        self.tone = tone;
        self
    }

    /// High-emphasis status: the solid fill with its `on` text. Status tones only, on a dark row.
    pub fn solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    /// Category chip: the label style instead of the caption style.
    pub fn category(mut self, category: bool) -> Self {
        self.category = category;
        self
    }

    /// Replaces the tone's glyph, for example the dashed ring of an unreachable host.
    pub fn glyph(mut self, glyph: IconName) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The number a count badge shows: `None` at zero, "99+" above 99.
pub fn count_label(count: u32) -> Option<String> {
    match count {
        0 => None,
        1..=COUNT_LIMIT => Some(count.to_string()),
        _ => Some(format!("{COUNT_LIMIT}+")),
    }
}

/// The text a badge shows: at most `LABEL_LIMIT` characters, then an ellipsis.
pub fn clip_label(text: &str) -> String {
    if text.chars().count() <= LABEL_LIMIT {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(LABEL_LIMIT).collect();
    clipped.push('…');
    clipped
}

/// The status palette of one status tone.
struct Status {
    solid: Color,
    on: Color,
    bg: Color,
    fg: Color,
    border: Color,
}

fn status(tone: BadgeTone, p: &Palette) -> Option<Status> {
    let s = &p.status;
    let (solid, on, bg, fg, border) = match tone {
        BadgeTone::Success => (
            s.success_solid,
            s.success_on,
            s.success_bg,
            s.success_fg,
            s.success_border,
        ),
        BadgeTone::Warning => (
            s.warning_solid,
            s.warning_on,
            s.warning_bg,
            s.warning_fg,
            s.warning_border,
        ),
        BadgeTone::Danger => (
            s.danger_solid,
            s.danger_on,
            s.danger_bg,
            s.danger_fg,
            s.danger_border,
        ),
        BadgeTone::Info => (s.info_solid, s.info_on, s.info_bg, s.info_fg, s.info_border),
        BadgeTone::Neutral | BadgeTone::Accent => return None,
    };
    Some(Status {
        solid,
        on,
        bg,
        fg,
        border,
    })
}

/// Background, border and text of a chip (Badge.md, Variants and states).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Chip {
    bg: Color,
    border: Color,
    text: Color,
}

fn chip(tone: BadgeTone, solid: bool, count: bool, disabled: bool, p: &Palette) -> Chip {
    if disabled {
        return Chip {
            bg: NONE,
            border: NONE,
            text: p.text.disabled,
        };
    }
    match status(tone, p) {
        Some(s) if solid && !count => Chip {
            bg: s.solid,
            border: NONE,
            text: s.on,
        },
        Some(s) if count => Chip {
            bg: s.bg,
            border: s.border,
            text: p.badge.neutral_fg,
        },
        Some(s) => Chip {
            bg: s.bg,
            border: s.border,
            text: s.fg,
        },
        None if tone == BadgeTone::Accent => Chip {
            bg: p.accent.subtle_raised,
            border: NONE,
            text: if count {
                p.badge.neutral_fg
            } else {
                p.accent.fg
            },
        },
        None => Chip {
            bg: p.badge.neutral_bg,
            border: NONE,
            text: p.badge.neutral_fg,
        },
    }
}

/// The solid colour of a dot in one tone.
fn dot_colour(tone: BadgeTone, p: &Palette) -> Color {
    match (status(tone, p), tone) {
        (Some(s), _) => s.solid,
        (None, BadgeTone::Accent) => p.accent.r#default,
        (None, _) => p.badge.neutral_fg,
    }
}

fn pill(
    label: String,
    colours: Chip,
    radius: f32,
    style: TextStyleToken,
    glyph: Option<IconName>,
    tabular: bool,
) -> AnyElement {
    let mut pill = div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(SIZE_BADGE_HEIGHT))
        .px(px(SPACE_1))
        .rounded(px(radius))
        .border_1()
        .border_color(colours.border)
        .bg(colours.bg)
        .text_style(style)
        .text_color(colours.text);
    if tabular {
        pill = pill.tabular_figures();
    }
    if let Some(name) = glyph {
        pill = pill.child(Icon::new(name).size(IconSize::XSmall).color(colours.text));
    }
    pill.child(div().min_w_0().truncate().child(label))
        .into_any_element()
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        match self.content {
            Content::Dot => {
                let colour = if self.disabled {
                    p.text.disabled
                } else {
                    dot_colour(self.tone, p)
                };
                div()
                    .flex_none()
                    .size(px(SIZE_INDICATOR_DOT))
                    .rounded(px(RADIUS_FULL))
                    .bg(colour)
                    .into_any_element()
            }
            Content::Count(count) => match count_label(count) {
                None => Empty.into_any_element(),
                Some(label) => pill(
                    label,
                    chip(self.tone, self.solid, true, self.disabled, p),
                    RADIUS_FULL,
                    TYPE_CAPTION,
                    None,
                    true,
                ),
            },
            Content::Text(text) => {
                let style = if self.category {
                    TYPE_LABEL
                } else {
                    TYPE_CAPTION
                };
                pill(
                    clip_label(&text),
                    chip(self.tone, self.solid, false, self.disabled, p),
                    RADIUS_SM,
                    style,
                    self.glyph.or_else(|| self.tone.glyph()),
                    false,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_shows_the_number_up_to_99_then_99_plus_and_nothing_at_zero() {
        assert_eq!(count_label(0), None);
        assert_eq!(count_label(1).as_deref(), Some("1"));
        assert_eq!(count_label(99).as_deref(), Some("99"));
        assert_eq!(count_label(100).as_deref(), Some("99+"));
        assert_eq!(count_label(u32::MAX).as_deref(), Some("99+"));
    }

    #[test]
    fn text_is_clipped_after_sixteen_characters() {
        assert_eq!(clip_label("Failed"), "Failed");
        let sixteen = "abcdefghijklmnop";
        assert_eq!(clip_label(sixteen), sixteen);
        assert_eq!(clip_label("abcdefghijklmnopq"), "abcdefghijklmnop…");
    }

    #[test]
    fn status_tones_carry_a_glyph_and_neutral_and_accent_do_not() {
        assert_eq!(BadgeTone::Success.glyph(), Some(IconName::CircleCheck));
        assert_eq!(BadgeTone::Warning.glyph(), Some(IconName::TriangleAlert));
        assert_eq!(BadgeTone::Danger.glyph(), Some(IconName::OctagonAlert));
        assert_eq!(BadgeTone::Info.glyph(), Some(IconName::Info));
        assert_eq!(BadgeTone::Neutral.glyph(), None);
        assert_eq!(BadgeTone::Accent.glyph(), None);
    }

    #[test]
    fn status_chip_uses_its_soft_background_border_and_text() {
        let t = crate::theme::Theme::new(crate::tokens::Appearance::Dark);
        let p = &t.palette;
        let warning = chip(BadgeTone::Warning, false, false, false, p);
        assert_eq!(warning.bg, p.status.warning_bg);
        assert_eq!(warning.border, p.status.warning_border);
        assert_eq!(warning.text, p.status.warning_fg);
    }

    #[test]
    fn solid_status_fills_with_its_solid_colour_and_on_text() {
        let t = crate::theme::Theme::new(crate::tokens::Appearance::Dark);
        let p = &t.palette;
        let solid = chip(BadgeTone::Danger, true, false, false, p);
        assert_eq!(solid.bg, p.status.danger_solid);
        assert_eq!(solid.text, p.status.danger_on);
        assert_eq!(solid.border.a, 0.0);
        // Neutral has no solid form: it stays on the neutral background.
        let neutral = chip(BadgeTone::Neutral, true, false, false, p);
        assert_eq!(neutral.bg, p.badge.neutral_bg);
    }

    #[test]
    fn count_keeps_the_kind_background_and_the_neutral_text() {
        let t = crate::theme::Theme::new(crate::tokens::Appearance::Dark);
        let p = &t.palette;
        let danger = chip(BadgeTone::Danger, false, true, false, p);
        assert_eq!(danger.bg, p.status.danger_bg);
        assert_eq!(danger.text, p.badge.neutral_fg);
        let neutral = chip(BadgeTone::Neutral, false, true, false, p);
        assert_eq!(neutral.bg, p.badge.neutral_bg);
        let accent = chip(BadgeTone::Accent, false, true, false, p);
        assert_eq!(accent.bg, p.accent.subtle_raised);
    }

    #[test]
    fn disabled_chip_has_no_background_and_disabled_text() {
        let t = crate::theme::Theme::new(crate::tokens::Appearance::Dark);
        let p = &t.palette;
        let disabled = chip(BadgeTone::Danger, true, false, true, p);
        assert_eq!(disabled.bg.a, 0.0);
        assert_eq!(disabled.text, p.text.disabled);
    }

    #[test]
    fn dot_takes_the_solid_colour_of_its_tone() {
        let p = Palette::DARK;
        assert_eq!(dot_colour(BadgeTone::Accent, &p), p.accent.r#default);
        assert_eq!(dot_colour(BadgeTone::Danger, &p), p.status.danger_solid);
        assert_eq!(dot_colour(BadgeTone::Neutral, &p), p.badge.neutral_fg);
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::TestAppContext;

    #[gpui::test]
    fn a_badge_adds_no_node_because_its_text_belongs_to_its_row(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(Badge::count(3).render(window, cx))
        });
        assert!(node.is_none());
    }
}
