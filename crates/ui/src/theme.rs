//! The run-time theme: a palette and shadows per appearance, read by every
//! component through [`ActiveTheme::theme`] (decision 0022).

use crate::assets::{FONT_MONO, FONT_SANS};
use crate::tokens::{Appearance, Color, Palette, ShadowToken, Shadows, TextStyleToken};
use crate::tokens::{FontFamily, TYPE_BODY};
use gpui::{App, BoxShadow, FontFeatures, FontWeight, Global, Hsla, Rgba, Styled, point, px};
use std::sync::Arc;

/// Theme of the whole app; switching is instant and never animates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub appearance: Appearance,
    pub palette: Palette,
    pub shadows: Shadows,
}

impl Global for Theme {}

impl Theme {
    pub const fn new(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self {
                appearance,
                palette: Palette::DARK,
                shadows: Shadows::DARK,
            },
            Appearance::Light => Self {
                appearance,
                palette: Palette::LIGHT,
                shadows: Shadows::LIGHT,
            },
        }
    }

    /// Installs the theme for `appearance` and refreshes every window.
    pub fn set(appearance: Appearance, cx: &mut App) {
        cx.set_global(Self::new(appearance));
        cx.refresh_windows();
    }

    /// Swaps dark and light (the rail theme toggle).
    pub fn toggle(cx: &mut App) {
        let next = match cx.theme().appearance {
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::Dark,
        };
        Self::set(next, cx);
    }

    pub fn is_dark(&self) -> bool {
        self.appearance == Appearance::Dark
    }
}

/// Access to the current theme from any context that derefs to `App`.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.try_global::<Theme>().unwrap_or(&DEFAULT_THEME)
    }
}

static DEFAULT_THEME: Theme = Theme::new(Appearance::Dark);

impl From<Color> for Rgba {
    fn from(c: Color) -> Self {
        Rgba {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

impl From<Color> for gpui::Fill {
    fn from(c: Color) -> Self {
        Hsla::from(c).into()
    }
}

impl From<Color> for Hsla {
    fn from(c: Color) -> Self {
        Rgba::from(c).into()
    }
}

/// Turns a shadow token into GPUI box shadows (spike S3: blur maps one to one).
pub fn box_shadows(layers: &[ShadowToken]) -> Vec<BoxShadow> {
    layers
        .iter()
        .map(|l| BoxShadow {
            color: l.color.into(),
            offset: point(px(l.x), px(l.y)),
            blur_radius: px(l.blur),
            spread_radius: px(l.spread),
            inset: false,
        })
        .collect()
}

/// Applies a type style token. Letter spacing is not supported by GPUI and is
/// dropped (recorded in decision 0027).
pub trait TextStyled: Styled + Sized {
    fn text_style(self, token: TextStyleToken) -> Self {
        let family = match token.family {
            FontFamily::Sans => FONT_SANS,
            FontFamily::Mono => FONT_MONO,
        };
        self.font_family(family)
            .text_size(px(token.size))
            .line_height(px(token.line_height))
            .font_weight(FontWeight(f32::from(token.weight)))
    }

    /// Default UI text (`body`).
    fn text_body(self) -> Self {
        self.text_style(TYPE_BODY)
    }

    /// Tabular figures for numbers that change in place (design-system 3.3).
    fn tabular_figures(self) -> Self {
        self.font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
    }
}

impl<T: Styled> TextStyled for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_keeps_channels_and_alpha() {
        let c = Color::rgba8(0x74, 0x61, 0xd8, 0.5);
        let rgba = Rgba::from(c);
        assert!((rgba.r - 0x74 as f32 / 255.0).abs() < 1e-6);
        assert!((rgba.a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn shadow_e2_has_one_layer_and_focus_has_two_rings() {
        let t = Theme::new(Appearance::Dark);
        assert_eq!(box_shadows(t.shadows.e2).len(), 1);
        assert_eq!(box_shadows(t.shadows.focus).len(), 2);
        assert!(box_shadows(t.shadows.e0).is_empty());
    }

    #[gpui::test]
    fn toggle_swaps_appearance(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            Theme::toggle(cx);
            assert_eq!(cx.theme().appearance, Appearance::Light);
            assert_eq!(
                cx.theme().palette.surface.frame,
                Palette::LIGHT.surface.frame
            );
        });
    }
}
