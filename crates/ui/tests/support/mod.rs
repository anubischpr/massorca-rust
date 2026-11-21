//! Helpers of the screenshot tests in `massorca-ui` and `massorca-desktop` (decision 0025).
//! The desktop test includes this file by path, so every item here must be used by both.

use gpui::{AnyWindowHandle, AssetSource, HeadlessAppContext, Platform};
use massorca_ui::assets::FONT_SANS;
use std::{
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

/// Opens a headless context with the real text system and the platform's headless renderer.
/// Returns `None`, after saying why, where there is no renderer (Windows) or it does not start.
pub fn headless_context(
    assets: Arc<dyn AssetSource>,
) -> Option<(HeadlessAppContext, Rc<dyn Platform>)> {
    match gpui_platform::current_headless_renderer() {
        Ok(Some(_)) => {}
        Ok(None) => {
            note("skipped: this platform has no headless renderer, so no screenshots");
            return None;
        }
        Err(error) => {
            note(&format!(
                "skipped: the headless renderer did not start: {error:#}"
            ));
            return None;
        }
    }
    // The caller keeps the platform alive until the test ends.
    let platform = gpui_platform::current_platform(true);
    let cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        assets,
        gpui_platform::current_headless_renderer,
    );
    Some((cx, platform))
}

/// Where the PNGs go: `$MASSORCA_SCREENSHOT_DIR`, else the workspace's `target/screenshots`.
pub fn output_dir() -> anyhow::Result<PathBuf> {
    let dir = std::env::var_os("MASSORCA_SCREENSHOT_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Fails unless the bundled Geist family is registered, so text cannot fall back silently.
pub fn assert_geist_loaded(cx: &HeadlessAppContext) -> anyhow::Result<()> {
    let names = cx.text_system().all_font_names();
    anyhow::ensure!(
        names.iter().any(|name| name.as_str() == FONT_SANS),
        "the bundled {FONT_SANS} font is not registered; families: {}",
        names.join(", ")
    );
    Ok(())
}

/// Lays out and paints the window's current state, so that its pixels can be captured.
pub fn draw(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
}

/// Paints the window and writes its pixels to `path` as a PNG.
pub fn write_png(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    path: &Path,
) -> anyhow::Result<()> {
    draw(cx, window)?;
    let image = cx.capture_screenshot(window)?;
    image.save(path)?;
    note(&format!(
        "wrote {} ({}x{})",
        path.display(),
        image.width(),
        image.height()
    ));
    Ok(())
}

/// Writes a progress line straight to stderr, which libtest does not capture.
pub fn note(message: &str) {
    // `writeln!` rather than `eprintln!`: the workspace denies `clippy::print_stderr`.
    let _ = writeln!(std::io::stderr(), "screenshots: {message}");
}
