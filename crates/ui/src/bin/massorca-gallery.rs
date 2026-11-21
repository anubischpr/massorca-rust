//! Component gallery: every component of the design system, dark and light.
//! Run it by hand: `cargo run -p massorca-ui --bin massorca-gallery`.

use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use massorca_ui::{Appearance, Assets, gallery::Gallery};

fn main() {
    let light = std::env::args().any(|a| a == "--light");
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            let appearance = if light {
                Appearance::Light
            } else {
                Appearance::Dark
            };
            if let Err(error) = massorca_ui::init(appearance, cx) {
                log_error(&error);
            }
            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(
                        px(massorca_ui::tokens::LAYOUT_WINDOW_MIN_WIDTH),
                        px(massorca_ui::tokens::LAYOUT_WINDOW_MIN_HEIGHT),
                    )),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Gallery::default()),
            );
            if let Err(error) = opened {
                log_error(&error);
                cx.quit();
            }
        });
}

#[allow(clippy::print_stderr)]
fn log_error(error: &anyhow::Error) {
    eprintln!("massorca-gallery: {error:#}");
}
