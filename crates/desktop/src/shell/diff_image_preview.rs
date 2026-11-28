//! The before and after images of an image file's diff, side by side (GIT-06). A side that cannot be drawn says why in a
//! sentence: there is no file on it, it is over the size cap, or its image type cannot be drawn here.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Image, ImageFormat, ImageSource, IntoElement, ObjectFit, ParentElement,
    Styled, StyledImage, div, img, prelude::*, px,
};
use massorca_protocol::methods::git::diff::MAX_CONTENT_BYTES;
use massorca_ui::tokens::{SPACE_2, SPACE_3, TYPE_BODY, TYPE_CAPTION};
use massorca_ui::{ActiveTheme, TextStyled};

use crate::model::image_preview::{ImagePreview, ImageSide};
use crate::model::source_control_diff::size_text;

/// The tallest a preview draws, in pixels; a taller image is scaled down to this height.
const PREVIEW_MAX_HEIGHT: f32 = 320.0;

/// The two sides of the diff side by side, each under its caption: the image, or the sentence that says why not.
pub(super) fn image_preview_panel(preview: &ImagePreview, cx: &App) -> AnyElement {
    div()
        .debug_selector(|| "image-preview".to_owned())
        .w_full()
        .flex()
        .flex_row()
        .gap(px(SPACE_3))
        .child(side_panel(
            "Before",
            "image-preview-before",
            preview.mime,
            preview.before.as_ref(),
            cx,
        ))
        .child(side_panel(
            "After",
            "image-preview-after",
            preview.mime,
            preview.after.as_ref(),
            cx,
        ))
        .into_any_element()
}

fn side_panel(
    title: &str,
    selector: &'static str,
    mime: &str,
    side: Option<&ImageSide>,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let note_selector = format!("{selector}-note");
    let body = match side {
        None => note(&note_selector, "Loading the preview…", cx),
        Some(ImageSide::Missing) => note(&note_selector, "There is no file on this side.", cx),
        Some(ImageSide::TooLarge { size }) => note(
            &note_selector,
            &format!(
                "{} is over the {} MiB preview limit, so it is not drawn.",
                size_text(Some(*size)),
                MAX_CONTENT_BYTES >> 20
            ),
            cx,
        ),
        Some(ImageSide::Unreadable) => note(&note_selector, "The preview could not be read.", cx),
        Some(ImageSide::Loaded { bytes }) => match image_format(mime) {
            Some(format) => {
                let picture: ImageSource =
                    Arc::new(Image::from_bytes(format, bytes.clone())).into();
                img(picture)
                    .w_full()
                    .max_h(px(PREVIEW_MAX_HEIGHT))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            }
            None => note(&note_selector, &format!("{mime} cannot be drawn here."), cx),
        },
    };
    div()
        .debug_selector(move || selector.to_owned())
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.secondary)
                .child(format!("{title} · {mime}")),
        )
        .child(body)
        .into_any_element()
}

/// A sentence in place of an image, found by its selector.
fn note(selector: &str, text: &str, cx: &App) -> AnyElement {
    let selector = selector.to_owned();
    div()
        .debug_selector(move || selector)
        .text_style(TYPE_BODY)
        .text_color(cx.theme().palette.text.secondary)
        .child(text.to_owned())
        .into_any_element()
}

/// The GPUI image type a previewable mime type decodes as; `None` for a type this build cannot draw.
fn image_format(mime: &str) -> Option<ImageFormat> {
    Some(match mime {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        "image/bmp" => ImageFormat::Bmp,
        "image/svg+xml" => ImageFormat::Svg,
        _ => return None,
    })
}
