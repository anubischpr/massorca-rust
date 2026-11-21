//! Design tokens. `generated.rs` is produced from `docs/design/tokens.json` by
//! the `tokens_are_generated` test; never edit it by hand (decision 0022).

#[rustfmt::skip]
mod generated;

pub use generated::*;

/// Light or dark: the two built-in themes of the design system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Appearance {
    Dark,
    Light,
}

/// An sRGB colour with alpha, as written in the token file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba8(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a,
        }
    }

    /// Relative luminance (WCAG 2.x), used by contrast checks.
    pub fn luminance(self) -> f32 {
        fn channel(c: f32) -> f32 {
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// WCAG contrast ratio between two opaque colours.
    pub fn contrast(self, other: Color) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

/// A length that is not in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Measure {
    Px(f32),
    /// Percent of the containing box.
    Percent(f32),
    /// Width of the `0` glyph of the current font.
    Ch(f32),
}

/// Which font family a text style uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFamily {
    Sans,
    Mono,
}

/// One type style of the scale (design-system 3.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyleToken {
    pub family: FontFamily,
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
    /// Letter spacing in em; 0 when the style has none.
    pub letter_spacing_em: f32,
}

/// One box shadow layer: offsets, blur and spread in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowToken {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

/// A motion curve (design-system 4.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    CubicBezier(f32, f32, f32, f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_palettes_resolve_every_name_the_same_way() {
        for name in PALETTE_NAMES {
            assert!(Palette::DARK.get(name).is_some(), "{name} missing in dark");
            assert!(
                Palette::LIGHT.get(name).is_some(),
                "{name} missing in light"
            );
        }
    }

    #[test]
    fn aliases_resolve_to_their_target() {
        let dark = Palette::DARK;
        assert_eq!(dark.agent.done, dark.status.success_fg);
        assert_eq!(dark.badge.neutral_fg, dark.text.primary);
    }

    #[test]
    fn frame_colours_match_agreements_a1_and_a14() {
        assert_eq!(
            Palette::DARK.surface.frame,
            Color::rgba8(0x1b, 0x1b, 0x1c, 1.0)
        );
        assert_eq!(
            Palette::DARK.surface.canvas,
            Color::rgba8(0x15, 0x15, 0x17, 1.0)
        );
        assert_eq!(
            Palette::LIGHT.surface.frame,
            Color::rgba8(0xee, 0xee, 0xf2, 1.0)
        );
        assert_eq!(
            Palette::LIGHT.surface.canvas,
            Color::rgba8(0xfb, 0xfb, 0xfc, 1.0)
        );
        assert_eq!(RADIUS_CANVAS, 12.0);
    }

    #[test]
    fn primary_text_meets_body_contrast_on_every_surface() {
        for p in [Palette::DARK, Palette::LIGHT] {
            for s in [
                p.surface.frame,
                p.surface.canvas,
                p.surface.raised,
                p.surface.overlay,
            ] {
                assert!(p.text.primary.contrast(s) >= 4.5);
            }
        }
    }
}
