//! The window's identity on Linux: the app id that X11 and Wayland shells read, the desktop entry
//! that matches it, and the window icon that X11 shows.

use image::RgbaImage;
use std::sync::Arc;

/// The app id: the X11 WM_CLASS and the Wayland app_id. It names the desktop entry `massorca.desktop`.
pub const APP_ID: &str = "massorca";

/// The X11 window icon. GPUI writes one image, so this is the 128 pixel size of the hicolor set:
/// a 256 pixel image is about 256 KiB of property data, at the classic X11 request-size limit.
#[cfg(target_os = "linux")]
pub fn window_icon() -> Option<Arc<RgbaImage>> {
    const ICON_128: &[u8] = include_bytes!("../linux/icons/hicolor/128x128/apps/massorca.png");
    let image = image::load_from_memory_with_format(ICON_128, image::ImageFormat::Png).ok()?;
    Some(Arc::new(image.into_rgba8()))
}

/// Only X11 reads the window icon from the app; the other platforms keep their own.
#[cfg(not(target_os = "linux"))]
pub fn window_icon() -> Option<Arc<RgbaImage>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESKTOP_ENTRY: &str = include_str!("../linux/massorca.desktop");

    #[test]
    fn the_desktop_entry_matches_the_app_id_and_names_the_icon_after_it() {
        assert!(
            DESKTOP_ENTRY
                .lines()
                .any(|line| line == format!("StartupWMClass={APP_ID}"))
        );
        assert!(
            DESKTOP_ENTRY
                .lines()
                .any(|line| line == format!("Icon={APP_ID}"))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_window_icon_is_the_128_pixel_logo() {
        let icon = window_icon().expect("the bundled icon decodes");
        assert_eq!(icon.dimensions(), (128, 128));
    }
}
