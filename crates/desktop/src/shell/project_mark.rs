//! A project's icon as the shell draws it: the mark a row, a heading or a picker shows (decision 0091).

use std::cell::RefCell;
use std::collections::HashMap;
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

thread_local! {
    /// The images built so far, by the key of their bytes, so a frame reuses an image it built before.
    static IMAGES: RefCell<HashMap<u64, Arc<Image>>> = RefCell::new(HashMap::new());
}

/// How many built images the cache keeps before it starts again.
const MAX_CACHED_IMAGES: usize = 32;

/// The image a project icon's bytes draw as, built once for each distinct image.
pub(super) fn decoded_image(image: &ProjectImage) -> Arc<Image> {
    IMAGES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(built) = cache.get(&image.key) {
            return Arc::clone(built);
        }
        if cache.len() >= MAX_CACHED_IMAGES {
            cache.clear();
        }
        let format = match image.format {
            ProjectImageFormat::Png => ImageFormat::Png,
            ProjectImageFormat::Webp => ImageFormat::Webp,
        };
        let built = Arc::new(Image::from_bytes(format, image.bytes.to_vec()));
        cache.insert(image.key, Arc::clone(&built));
        built
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_reuses_the_image_built_for_the_same_bytes() {
        let bytes = b"\x89PNG\r\n\x1a\nreused".to_vec();
        let image = ProjectImage::new(ProjectImageFormat::Png, bytes, None);
        let first = decoded_image(&image);
        let again = decoded_image(&image);
        assert!(Arc::ptr_eq(&first, &again));
    }
}
