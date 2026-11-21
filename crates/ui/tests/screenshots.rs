//! Opt-in screenshots of every component-gallery story in both themes (decision 0025).
//!
//! Run `cargo test -p massorca-ui --test screenshots -- --ignored`. Each story is shot in a
//! window only as tall as the story and the navigation list need. PNGs go to
//! `$MASSORCA_SCREENSHOT_DIR`, else `target/screenshots/`.

#[path = "support/mod.rs"]
mod support;

use gpui::{AnyWindowHandle, AppContext, HeadlessAppContext, px, size};
use massorca_ui::gallery::{Gallery, stories};
use massorca_ui::tokens::SPACE_6;
use massorca_ui::{Appearance, Assets, Theme};
use std::ops::Range;
use std::sync::Arc;

/// Width of every gallery window, the same as the desktop shell's artboard.
const WIDTH: f32 = 1440.0;
/// Height of the probe window that measures a story before its real window opens.
const PROBE_HEIGHT: f32 = 2400.0;
/// Width of the navigation column in `Gallery::render`.
const NAV_WIDTH: u32 = 220;

#[test]
#[ignore = "writes PNG screenshots; run with -- --ignored"]
fn gallery_screenshots() -> anyhow::Result<()> {
    let Some((mut cx, _platform)) = support::headless_context(Arc::new(Assets)) else {
        return Ok(());
    };
    cx.update(|app| massorca_ui::init(Appearance::Dark, app))?;
    support::assert_geist_loaded(&cx)?;
    let dir = support::output_dir()?;

    let names: Vec<String> = stories().iter().map(|story| slug(story.name)).collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    anyhow::ensure!(unique.len() == names.len(), "two stories share a file name");

    for (theme, appearance) in [("dark", Appearance::Dark), ("light", Appearance::Light)] {
        cx.update(|app| Theme::set(appearance, app));
        for (index, name) in names.iter().enumerate() {
            let height = fitted_height(&mut cx, index)?;
            let window = open_gallery(&mut cx, index, height)?;
            let path = dir.join(format!("gallery-{theme}-{name}.png"));
            support::write_png(&mut cx, window, &path)?;
            cx.update_window(window, |_, window, _| window.remove_window())?;
        }
    }
    Ok(())
}

/// The window height that shows story `index` whole, with the body's bottom padding. The story
/// is first painted in a tall probe window, and its lowest painted row sets the height.
fn fitted_height(cx: &mut HeadlessAppContext, index: usize) -> anyhow::Result<f32> {
    let probe = open_gallery(cx, index, PROBE_HEIGHT)?;
    support::draw(cx, probe)?;
    let image = cx.capture_screenshot(probe)?;
    cx.update_window(probe, |_, window, _| window.remove_window())?;

    let (width, height) = (image.width(), image.height());
    let pixel = |x: u32, y: u32| image.get_pixel(x, y).0;
    // The body is painted over the canvas colour, the navigation over the frame colour.
    let body = lowest_ink(height, NAV_WIDTH..width, (width - 1, height - 1), pixel);
    let nav = lowest_ink(height, 0..NAV_WIDTH, (0, height - 1), pixel);
    let body = body.unwrap_or(0);
    anyhow::ensure!(
        body + 2 < height,
        "story {index} does not fit the {PROBE_HEIGHT}px probe window"
    );
    Ok((body.max(nav.unwrap_or(0)) + 1) as f32 + SPACE_6)
}

/// The lowest row, within `columns`, with a pixel unlike the one at `backdrop`.
fn lowest_ink(
    height: u32,
    columns: Range<u32>,
    backdrop: (u32, u32),
    pixel: impl Fn(u32, u32) -> [u8; 4],
) -> Option<u32> {
    let backdrop = pixel(backdrop.0, backdrop.1);
    (0..height)
        .rev()
        .find(|&y| columns.clone().any(|x| pixel(x, y) != backdrop))
}

/// Opens a gallery window of `height` with story `index` selected, at 1x.
fn open_gallery(
    cx: &mut HeadlessAppContext,
    index: usize,
    height: f32,
) -> anyhow::Result<AnyWindowHandle> {
    let window = cx.open_window(size(px(WIDTH), px(height)), |_, cx| {
        cx.new(|_| Gallery::default())
    })?;
    window.update(cx, |gallery, _, cx| gallery.select(index, cx))?;
    let window: AnyWindowHandle = window.into();
    // The test platform paints at 2x by default; 1x makes each PNG the design's pixel size.
    cx.simulate_window_scale_factor_change(window, 1.0);
    Ok(window)
}

/// A file-name slug: `Icon button` becomes `icon-button`.
fn slug(name: &str) -> String {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}
