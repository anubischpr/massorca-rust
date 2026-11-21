//! The MassOrca desktop window. It talks to the service of its profile (decision 0028) and
//! starts it when it can; `--sample` opens the shell on sample data instead (decision 0023).
//!
//! Flags: `--sample`, `--profile <name>`, `--home <dir>`, `--no-autostart`. Like the CLI, it reads
//! `MASSORCA_HOME`, `MASSORCA_PROFILE` and `MASSORCA_NO_AUTOSTART`. `MASSORCA_DESKTOP_BACKGROUND=1`
//! opens the window hidden and unfocused for automated checks (see `launch_options`).

use gpui::{
    AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, point, px,
    size,
};
use massorca_desktop::data::SampleDataSource;
use massorca_desktop::launch_options::WindowLaunch;
use massorca_desktop::model::teams::SampleTeamsSource;
use massorca_desktop::service::LaunchOptions;
use massorca_desktop::shell::{self, Shell};
use massorca_desktop::store::ClientDataSource;
use massorca_ui::tokens::{LAYOUT_WINDOW_MIN_HEIGHT, LAYOUT_WINDOW_MIN_WIDTH};
use massorca_ui::{Appearance, Assets};
use std::io::Write;

fn main() {
    let options =
        match LaunchOptions::parse(std::env::args_os().skip(1), |key| std::env::var(key).ok()) {
            Ok(options) => options,
            Err(message) => {
                report(&message);
                std::process::exit(2);
            }
        };
    let launch = WindowLaunch::from_env(|key| std::env::var(key).ok());
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            if let Err(error) = massorca_ui::init(Appearance::Dark, cx) {
                report(&error.to_string());
            }
            shell::bind_keys(cx);
            // The bridge starts before the window so the first paint already shows "Connecting".
            let client = if options.sample {
                None
            } else {
                match start_client(&options) {
                    Ok(source) => Some(source),
                    Err(message) => {
                        report(&message);
                        cx.quit();
                        return;
                    }
                }
            };
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
                    // Linux draws its own frame; GPUI keeps the platform frame where the WM cannot.
                    window_decorations: cfg!(target_os = "linux")
                        .then_some(WindowDecorations::Client),
                    focus: launch.focus(),
                    show: launch.show(),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| match client {
                        Some(source) => Shell::with_client(source, window, cx),
                        None => Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx),
                    })
                },
            );
            match opened {
                Ok(_) if launch.background => {}
                Ok(_) => cx.activate(true),
                Err(error) => {
                    report(&format!("{error:#}"));
                    cx.quit();
                }
            }
        });
}

/// Resolves the profile the way the CLI does and starts the bridge. The service itself starts
/// from the `massorca` executable beside this one, unless autostart is off.
fn start_client(options: &LaunchOptions) -> Result<ClientDataSource, String> {
    let profile = options.resolve_profile()?;
    let exe = std::env::current_exe()
        .map_err(|error| format!("cannot locate the desktop executable: {error}"))?;
    ClientDataSource::start(profile, options.autostart(&exe))
        .map_err(|error| format!("cannot start the service link: {error}"))
}

fn report(message: &str) {
    // The window never opened, so stderr is the only place the reason can go.
    let _ = writeln!(std::io::stderr(), "massorca-desktop: {message}");
}
