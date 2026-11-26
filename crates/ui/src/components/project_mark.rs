//! A project's icon where a row or a picker draws it: an emoji, a Lucide icon or an image (decision 0091).

use std::sync::Arc;

use gpui::{Image, ImageSource, IntoElement, ObjectFit, SharedString, div, img, prelude::*, px};

use crate::icon::IconName;
use crate::tokens::{RADIUS_SM, SIZE_ICON_SM};

/// The bundled Lucide icon with this name, or `None` when the set has no icon of that name.
pub fn lucide_icon(name: &str) -> Option<IconName> {
    IconName::ALL
        .iter()
        .copied()
        .find(|icon| file_stem(icon.path()) == name)
}

/// The file name of `path` without its `.svg` extension: the Lucide name the icon is bundled under.
fn file_stem(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".svg")
}

/// An emoji drawn as text in a 16 px box.
pub fn emoji_mark(emoji: SharedString) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(SIZE_ICON_SM))
        .child(emoji)
}

/// An image drawn in a 16 px rounded square, cropped to fill it.
pub fn image_mark(image: Arc<Image>) -> impl IntoElement {
    img(ImageSource::Image(image))
        .size(px(SIZE_ICON_SM))
        .flex_none()
        .rounded(px(RADIUS_SM))
        .object_fit(ObjectFit::Cover)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lucide_name_finds_its_bundled_icon() {
        assert_eq!(lucide_icon("folder"), Some(IconName::Folder));
        assert_eq!(lucide_icon("git-branch"), Some(IconName::GitBranch));
        assert_eq!(lucide_icon("no-such-icon"), None);
    }

    #[test]
    fn every_bundled_icon_is_found_by_its_own_name() {
        for icon in IconName::ALL {
            assert_eq!(lucide_icon(file_stem(icon.path())), Some(*icon), "{icon:?}");
        }
    }
}
