//! The MassOrca rose (A26) on the start pages: the logo's vector paths under a root of its drawn size.

use gpui::{IntoElement, Styled, img, px};

/// The rose's drawn size in logical px on the start pages.
pub const ROSE_LOGO_EDGE: f32 = 48.0;

/// The logo with a root `ROSE_LOGO_EDGE` px wide and high; its paths are the logo's own.
pub const ROSE_LOGO_PATH: &str = "brand/massorca-rose-48.svg";

/// The rose on a start page. GPUI rasterises an SVG image at twice its root size and samples it
/// without mipmaps (gpui `svg_renderer.rs`, `SMOOTH_SVG_SCALE_FACTOR`), so this root gives a 96 px
/// raster for 48 px on screen. The 512 px logo drew at 1024 px and was minified about 21 times.
pub fn rose_logo() -> impl IntoElement {
    img(ROSE_LOGO_PATH).size(px(ROSE_LOGO_EDGE)).flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Assets;
    use std::collections::BTreeSet;

    fn bundled(path: &str) -> String {
        let bytes = Assets::get(path).unwrap_or_else(|| panic!("{path} is not bundled"));
        String::from_utf8(bytes.to_vec()).expect("the logo is UTF-8")
    }

    #[test]
    fn the_rose_root_is_the_size_it_is_drawn_at() {
        let svg = bundled(ROSE_LOGO_PATH);
        let root = svg.split('>').next().expect("a root tag");
        let edge = ROSE_LOGO_EDGE as u32;
        assert!(
            root.contains(&format!("width=\"{edge}\" height=\"{edge}\"")),
            "{root}"
        );
    }

    #[test]
    fn the_rose_is_the_logo_with_only_its_root_size_changed() {
        let edge = ROSE_LOGO_EDGE as u32;
        let rose = bundled(ROSE_LOGO_PATH).replace(
            &format!("width=\"{edge}\" height=\"{edge}\""),
            "width=\"512\" height=\"512\"",
        );
        assert_eq!(rose, bundled("brand/massorca-logo.svg"));
    }

    #[test]
    fn the_logo_is_one_flat_pink_fill() {
        let svg = bundled(ROSE_LOGO_PATH);
        let fills: BTreeSet<&str> = svg
            .split("fill=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect();
        assert_eq!(fills, BTreeSet::from(["#FF2E88"]));
    }
}
