//! Lucide icons as an enum, so a missing icon is a compile error (0024).

use crate::tokens::{SIZE_ICON_LG, SIZE_ICON_MD, SIZE_ICON_SM, SIZE_ICON_XS};
use gpui::{App, Hsla, IntoElement, RenderOnce, Styled, Window, px, svg};

include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));

/// The four icon sizes of the design system (12, 16, 20 and 24 px).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconSize {
    XSmall,
    #[default]
    Small,
    Medium,
    Large,
}

impl IconSize {
    pub fn px(self) -> f32 {
        match self {
            IconSize::XSmall => SIZE_ICON_XS,
            IconSize::Small => SIZE_ICON_SM,
            IconSize::Medium => SIZE_ICON_MD,
            IconSize::Large => SIZE_ICON_LG,
        }
    }
}

/// A single-colour Lucide icon tinted with the given colour.
#[derive(Debug, IntoElement)]
pub struct Icon {
    name: IconName,
    size: IconSize,
    color: Option<Hsla>,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: IconSize::default(),
            color: None,
        }
    }

    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        use crate::theme::ActiveTheme;
        let color = self
            .color
            .unwrap_or_else(|| cx.theme().palette.text.secondary.into());
        let s = px(self.size.px());
        svg()
            .path(self.name.path())
            .size(s)
            .flex_none()
            .text_color(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Assets;

    #[test]
    fn every_icon_name_points_at_a_bundled_svg() {
        assert!(IconName::ALL.len() > 50);
        for icon in IconName::ALL {
            assert!(Assets::get(icon.path()).is_some(), "{icon:?}");
        }
    }

    #[test]
    fn sizes_follow_the_token_scale() {
        let sizes: Vec<f32> = [
            IconSize::XSmall,
            IconSize::Small,
            IconSize::Medium,
            IconSize::Large,
        ]
        .map(IconSize::px)
        .into();
        assert_eq!(sizes, vec![12.0, 16.0, 20.0, 24.0]);
    }
}
