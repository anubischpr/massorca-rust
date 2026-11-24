//! A package's listed images: a card's logo and an item page's banner, fetched through `extensions.asset`.

use std::io::Cursor;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use gpui::{App, Global, Image, ImageFormat, ImageSource, WeakEntity};
use massorca_protocol::methods::extensions::ExtensionAssetParams;

use super::MarketplaceFrame;
use crate::model::marketplace::{ImageKey, ImageSlots, MarketplaceItem};
use crate::service::ServiceHandle;
use crate::shell::Shell;

/// Every listed image asked for so far, by key; a global, so page draws read it directly.
#[derive(Default)]
struct ItemImages(ImageSlots<Arc<Image>>);

impl Global for ItemImages {}

/// The longest side a listed image may have; a larger one is refused before its pixels are decoded.
const MAX_IMAGE_SIDE: u32 = 4096;

/// The logo a card draws, once it has loaded.
pub(super) fn logo(
    frame: &MarketplaceFrame,
    item: &MarketplaceItem,
    cx: &mut App,
) -> Option<ImageSource> {
    listed(frame, item.logo_key()?, cx)
}

/// The banner image an item page draws above its header, once it has loaded.
pub(super) fn banner(
    frame: &MarketplaceFrame,
    item: &MarketplaceItem,
    cx: &mut App,
) -> Option<ImageSource> {
    listed(frame, item.banner_key()?, cx)
}

/// The loaded image of a key; the first ask for it starts its fetch, when a service is connected.
fn listed(frame: &MarketplaceFrame, key: ImageKey, cx: &mut App) -> Option<ImageSource> {
    let images = &mut cx.default_global::<ItemImages>().0;
    if let Some(image) = images.loaded(&key) {
        return Some(image.into());
    }
    let service = frame.service.clone()?;
    if images.begin(&key) {
        fetch(service, key, frame.shell.clone(), cx);
    }
    None
}

/// Asks the service for one listed image, decodes it off the UI thread and records the answer.
fn fetch(service: ServiceHandle, key: ImageKey, shell: WeakEntity<Shell>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let params = ExtensionAssetParams {
            id: key.package.clone(),
            path: key.path.clone(),
        };
        let image = match service.extension_asset(params).await {
            Ok(asset) => {
                let data = asset.data;
                cx.background_executor()
                    .spawn(async move { decode(&data) })
                    .await
            }
            Err(_) => None,
        };
        cx.update_global::<ItemImages, _>(|images, _| images.0.finish(&key, image));
        let _ = shell.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

/// The image a listed asset holds, when its bytes decode as that image (an SVG is checked as text).
fn decode(data: &str) -> Option<Arc<Image>> {
    let bytes = STANDARD.decode(data).ok()?;
    let Ok(format) = image::guess_format(&bytes) else {
        let svg = std::str::from_utf8(&bytes).is_ok_and(|text| text.contains("<svg"));
        return svg.then(|| Arc::new(Image::from_bytes(ImageFormat::Svg, bytes)));
    };
    let listed = match format {
        image::ImageFormat::Png => ImageFormat::Png,
        image::ImageFormat::Jpeg => ImageFormat::Jpeg,
        image::ImageFormat::WebP => ImageFormat::Webp,
        image::ImageFormat::Gif => ImageFormat::Gif,
        image::ImageFormat::Bmp => ImageFormat::Bmp,
        image::ImageFormat::Tiff => ImageFormat::Tiff,
        _ => return None,
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(&bytes), format)
        .into_dimensions()
        .ok()?;
    if width.max(height) > MAX_IMAGE_SIDE {
        return None;
    }
    image::load_from_memory_with_format(&bytes, format).ok()?;
    Some(Arc::new(Image::from_bytes(listed, bytes)))
}
