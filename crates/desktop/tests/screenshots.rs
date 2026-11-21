//! Opt-in screenshots of the desktop shell at the 1440x900 artboard size, in both themes and
//! with the command palette open (decision 0025).
//!
//! Run `cargo test -p massorca-desktop --test screenshots -- --ignored`. PNGs go to
//! `$MASSORCA_SCREENSHOT_DIR`, else `target/screenshots/`.

#[path = "../../ui/tests/support/mod.rs"]
mod support;

use gpui::{AnyWindowHandle, AppContext, px, size};
use massorca_desktop::data::SampleDataSource;
use massorca_desktop::model::teams::SampleTeamsSource;
use massorca_desktop::shell::{self, Shell};
use massorca_ui::{Appearance, Assets, Theme};
use std::sync::Arc;

#[test]
#[ignore = "writes PNG screenshots; run with -- --ignored"]
fn shell_screenshots() -> anyhow::Result<()> {
    let Some((mut cx, _platform)) = support::headless_context(Arc::new(Assets)) else {
        return Ok(());
    };
    cx.update(|app| massorca_ui::init(Appearance::Dark, app))?;
    cx.update(shell::bind_keys);
    support::assert_geist_loaded(&cx)?;
    let dir = support::output_dir()?;

    for (theme, appearance) in [("dark", Appearance::Dark), ("light", Appearance::Light)] {
        cx.update(|app| Theme::set(appearance, app));
        let handle = cx.open_window(size(px(1440.), px(900.)), |window, cx| {
            cx.new(|cx| Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx))
        })?;
        let window: AnyWindowHandle = *handle;
        // The test platform paints at 2x by default; 1x makes each PNG the artboard's pixel size.
        cx.simulate_window_scale_factor_change(window, 1.0);

        support::write_png(&mut cx, window, &dir.join(format!("shell-{theme}.png")))?;
        handle.update(&mut cx, |shell, window, cx| {
            shell.toggle_palette(window, cx)
        })?;
        let palette = format!("shell-{theme}-palette.png");
        support::write_png(&mut cx, window, &dir.join(palette))?;

        cx.update_window(window, |_, window, _| window.remove_window())?;
    }
    Ok(())
}
