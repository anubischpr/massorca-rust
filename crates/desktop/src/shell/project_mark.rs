//! A project's icon as the shell draws it: the mark a row, a heading or a picker shows (decision 0091).

use std::sync::Arc;

use gpui::{AnyElement, Image, ImageFormat, IntoElement};
use massorca_ui::components::project_mark::{emoji_mark, image_mark, lucide_icon};
use massorca_ui::{Icon, IconName, IconSize};

use crate::model::project_icon::{ProjectGlyph, ProjectImage, ProjectImageFormat};

/// The mark a project's icon draws at the heading's size: the folder glyph by default.
pub(super) fn project_mark(glyph: &ProjectGlyph) -> AnyElement {
    match glyph {
        ProjectGlyph::Folder => Icon::new(IconName::Folder)
            .size(IconSize::Medium)
            .into_any_element(),
        ProjectGlyph::Lucide(name) => Icon::new(lucide_icon(name).unwrap_or(IconName::Folder))
            .size(IconSize::Medium)
            .into_any_element(),
        ProjectGlyph::Emoji(emoji) => emoji_mark(emoji.clone().into()).into_any_element(),
        ProjectGlyph::Image(image) => image_mark(decoded_image(image)).into_any_element(),
    }
}

/// The image a project icon's bytes draw as, built for this frame.
pub(super) fn decoded_image(image: &ProjectImage) -> Arc<Image> {
    let format = match image.format {
        ProjectImageFormat::Png => ImageFormat::Png,
        ProjectImageFormat::Webp => ImageFormat::Webp,
    };
    Arc::new(Image::from_bytes(format, image.bytes.to_vec()))
}
