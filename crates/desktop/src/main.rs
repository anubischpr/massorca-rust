//! The MassOrca desktop window: opens the shell on sample data (decision 0023).

use gpui::{AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};
use massorca_desktop::data::SampleDataSource;
use massorca_desktop::shell::{self, Shell};
use massorca_ui::tokens::{LAYOUT_WINDOW_MIN_HEIGHT, LAYOUT_WINDOW_MIN_WIDTH};
use massorca_ui::{Appearance, Assets};
use std::io::Write;

fn main() {
    gpui_platform::application().with_assets(Assets).run(|cx| {
        if let Err(error) = massorca_ui::init(Appearance::Dark, cx) {
            report(&error.to_string());
        }
        shell::bind_keys(cx);
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(
                    px(LAYOUT_WINDOW_MIN_WIDTH),
                    px(LAYOUT_WINDOW_MIN_HEIGHT),
                )),
                titlebar: Some(TitlebarOptions {
                    title: Some("MassOrca".into()),
                    appears_transparent: true,
                    traffic_light_position: cfg!(target_os = "macos")
                        .then(|| point(px(16.), px(14.))),
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Shell::new(&SampleDataSource, window, cx)),
        );
        match opened {
            Ok(_) => cx.activate(true),
            Err(error) => {
                report(&format!("{error:#}"));
                cx.quit();
            }
        }
    });
}

fn report(message: &str) {
    // The window never opened, so stderr is the only place the reason can go.
    let _ = writeln!(std::io::stderr(), "massorca-desktop: {message}");
}
