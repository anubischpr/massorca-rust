//! The MassOrca desktop window. It talks to the service of its profile (decision 0028) and
//! starts it when it can; `--sample` opens the shell on sample data instead (decision 0023).
//!
//! Flags: `--sample`, `--profile <name>`, `--home <dir>`, `--no-autostart`, `--page <name>` (a debug
//! page, see `launch_options::DebugPage`). Like the CLI, it reads `MASSORCA_HOME`, `MASSORCA_PROFILE` and
//! `MASSORCA_NO_AUTOSTART`. `MASSORCA_DESKTOP_BACKGROUND=1` opens the window hidden and unfocused for
//! automated checks (see `launch_options`). A second launch for the same profile raises the running
//! window and exits (PLAT-07).

use gpui::{
    AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, point, px,
    size,
};
use massorca_desktop::data::SampleDataSource;
use massorca_desktop::launch_options::WindowLaunch;
use massorca_desktop::model::teams::SampleTeamsSource;
use massorca_desktop::service::LaunchOptions;
use massorca_desktop::shell::{self, Shell};
use massorca_desktop::single_instance::{self, LaunchPlan};
use massorca_desktop::store::ClientDataSource;
use massorca_desktop::ui_state;
use massorca_desktop::window_identity;
use massorca_profile::claim_desktop;
use massorca_ui::Assets;
use massorca_ui::tokens::{LAYOUT_WINDOW_MIN_HEIGHT, LAYOUT_WINDOW_MIN_WIDTH};
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
    // An automated launch is never handed off to the user's window, so it claims nothing.
    let profile = if launch.background {
        None
    } else {
        match options.resolve_profile() {
            Ok(profile) => Some(profile),
            Err(message) => {
                report(&message);
                std::process::exit(2);
            }
        }
    };
    // A claim that fails must not stop the person from opening the app, so it only costs single-instance.
    let (plan, claim_error) = single_instance::plan_launch(profile.as_ref().map(claim_desktop));
    if let Some(message) = claim_error {
        report(&message);
    }
    let owner = match plan {
        LaunchPlan::Open(owner) => owner,
        // The running window was raised for this launch, so there is nothing left to open.
        LaunchPlan::HandedOff => return,
    };
    // Read before the window opens, so the first frame already paints the theme saved last.
    let saved = profile.as_ref().map(ui_state::load).unwrap_or_default();
    let debug_page = options.page;
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            if let Err(error) = massorca_ui::init(saved.theme, cx) {
                report(&error.to_string());
            }
            if let Some(profile) = profile {
                ui_state::install(cx, profile, saved.clone());
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
            let window_size = size(px(1440.), px(900.));
            let bounds = Bounds::centered(None, window_size, cx);
            let open_regions = saved.open_regions.clone();
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
                    // The window identity Linux shells read; other platforms keep their defaults.
                    app_id: cfg!(target_os = "linux").then(|| window_identity::APP_ID.to_string()),
                    icon: window_identity::window_icon(),
                    focus: launch.focus(),
                    show: launch.show(),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let mut shell = match client {
                            Some(source) => Shell::with_client(source, window, cx),
                            None => Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx),
                        };
                        shell.restore_open_regions(open_regions);
                        if let Some(field) = launch.debug_focus {
                            shell.start_debug_input(field, window, cx);
                        }
                        if let Some(page) = debug_page {
                            shell.start_debug_page(page);
                        }
                        shell
                    })
                },
            );
            // Only an opened window can be raised, so later launches are answered from here on.
            if let (Some(owner), Ok(window)) = (owner, opened.as_ref()) {
                single_instance::raise_on_handoff(owner, *window, cx).unwrap_or_else(|error| {
                    report(&format!("cannot answer later launches: {error}"))
                });
            }
            match opened {
                // GPUI applies the requested size only when it activates a window; a hidden one never is.
                Ok(window) if launch.background => {
                    let _ = window.update(cx, |_, window, _| window.resize(window_size));
                }
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
