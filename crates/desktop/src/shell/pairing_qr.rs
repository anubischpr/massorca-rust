//! The offer's QR code on the Pairing page (D-PR-01, SRV-06): an SVG drawn by GPUI's image element. The SVG carries
//! its own white tile, so the code reads dark on light in both themes.

use std::fmt::Write as _;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, Image, ImageFormat, ImageSource, div, img, px};
use massorca_ui::tokens::{RADIUS_MD, SPACE_4};
use qrcode::{Color, QrCode};

/// The code's side on screen (D-PR-01: about 192 px).
const CODE_SIDE: f32 = 192.0;

/// The QR code of `text` as an SVG with a white tile around it, `SPACE_4` wide at `CODE_SIDE`; `None` when the text is
/// too long for a QR code.
pub(super) fn svg(text: &str) -> Option<String> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    let width = code.width();
    let colors = code.to_colors();
    let modules = f32::from(u16::try_from(width).ok()?);
    let margin = modules * SPACE_4 / CODE_SIDE;
    let side = modules + 2.0 * margin;
    let mut path = String::new();
    for (index, color) in colors.iter().enumerate() {
        if *color == Color::Dark {
            let _ = write!(path, "M{} {}h1v1h-1z", index % width, index / width);
        }
    }
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x} {x} {side} {side}\" shape-rendering=\"crispEdges\">\
         <rect x=\"{x}\" y=\"{x}\" width=\"{side}\" height=\"{side}\" fill=\"#fff\"/>\
         <path d=\"{path}\" fill=\"#000\"/></svg>",
        x = -margin,
    ))
}

/// The tile with the code, or nothing when the offer is too long to draw.
pub(super) fn tile(text: &str) -> Option<AnyElement> {
    let svg = svg(text)?;
    let image = Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()));
    let side = px(CODE_SIDE + 2.0 * SPACE_4);
    Some(
        div()
            .debug_selector(|| "pairing-offer-qr".to_owned())
            .size(side)
            .flex_none()
            .rounded(px(RADIUS_MD))
            .overflow_hidden()
            .child(img(ImageSource::Image(image)).size(side))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_svg_draws_dark_modules_on_a_white_tile() {
        let svg = svg("mop1.placeholder-offer-text").expect("a code");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("fill=\"#fff\""));
        assert!(svg.contains("h1v1h-1z"));
    }

    #[test]
    fn text_too_long_for_a_code_draws_nothing() {
        assert!(svg(&"x".repeat(8000)).is_none());
    }
}
