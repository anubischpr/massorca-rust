//! Opt-in screenshots of every shell page and state the sample data reaches, in both themes, at
//! the 1440x900 artboard size (decision 0025). Each shot opens a fresh shell and drives it with
//! the keys and clicks a user sends, so no shot inherits another shot's state. The service-backed
//! states run a real service on a temporary profile.
//!
//! Run `cargo test -p massorca-desktop --test screenshots -- --ignored --test-threads=1`. PNGs go
//! to `$MASSORCA_SCREENSHOT_DIR`, else `target/screenshots/`.

#[path = "support/drive.rs"]
mod drive;
#[path = "support/service.rs"]
mod service;
#[path = "../../ui/tests/support/mod.rs"]
mod support;

use drive::{click, press, right_click, type_text};
use gpui::{AnyWindowHandle, AppContext, HeadlessAppContext, WindowHandle, px, size};
use massorca_desktop::data::{DataSource, SampleDataSource};
use massorca_desktop::model::shell::ShellSnapshot;
use massorca_desktop::model::teams::SampleTeamsSource;
use massorca_desktop::service::{ConnectionState, ServiceHandle};
use massorca_desktop::shell::{self, Shell};
use massorca_desktop::store::ClientDataSource;
use massorca_protocol::methods::workspaces::Workspace;
use massorca_ui::{Appearance, Assets, Theme};
use service::{LocalService, add_project_and_workspace, block_on, profile, repo};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// The artboard height; every shot is this tall.
const HEIGHT: f32 = 900.0;
/// The artboard width, and the default width of every shot.
const WIDTH: f32 = 1440.0;
/// How long a service-backed shot waits for the bridge before it gives up.
const LIMIT: Duration = Duration::from_secs(15);

/// The palette, quick-open and settings keys: Mod is Cmd on macOS and Ctrl elsewhere.
const PALETTE_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-k"
} else {
    "ctrl-k"
};
const QUICK_OPEN_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-p"
} else {
    "ctrl-p"
};
const SETTINGS_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-,"
} else {
    "ctrl-,"
};

// Where the artboard draws its controls, in window pixels at 1440x900 (see the baseline PNGs).
const MODE_BUTTON: (f32, f32) = (24.0, 24.0);
const TASKS_BUTTON: (f32, f32) = (24.0, 66.0);
const CALENDAR_BUTTON: (f32, f32) = (24.0, 106.0);
const MARKETPLACE_BUTTON: (f32, f32) = (24.0, 146.0);
const USAGE_RING: (f32, f32) = (24.0, 752.0);
const CREATE_BUTTON: (f32, f32) = (176.0, 53.0);
const PROTOTYPES_ROW: (f32, f32) = (120.0, 114.0);
const AUTOMATIONS_ROW: (f32, f32) = (127.0, 174.0);
const WORKSPACE_ROW: (f32, f32) = (150.0, 334.0);
const UNREACHABLE_SESSION_ROW: (f32, f32) = (200.0, 574.0);
const STATUS_HOSTS: (f32, f32) = (1357.0, 888.0);
const SESSION_SUMMARY_TOGGLE: (f32, f32) = (1220.0, 18.0);
const WORK_PANEL_TOGGLE: (f32, f32) = (1252.0, 18.0);
const PROJECT_SIDEBAR_TOGGLE: (f32, f32) = (1284.0, 18.0);
const TEAM_CONFERENCE_ROW: (f32, f32) = (152.0, 142.0);
const TEAM_BOARD_ROW: (f32, f32) = (120.0, 170.0);
const NEW_TEAM_BUTTON: (f32, f32) = (1363.0, 345.0);
const TEAM_ADD_BOT_BUTTON: (f32, f32) = (556.0, 548.0);
/// The workspace-remove item of the Sidebar row menu, under the workspace row.
const REMOVE_ITEM: (f32, f32) = (170.0, 352.0);
/// The Remove button of the remove dialog that opens from the row menu.
const CONFIRM_REMOVE: (f32, f32) = (854.0, 483.0);
/// The "Choose a project" button of the New workspace dialog.
const CHOOSE_PROJECT: (f32, f32) = (720.0, 350.0);

/// The centre of Settings navigation item `index`, in the spec's order (232 px column, 32 px rows).
fn settings_nav(index: usize) -> (f32, f32) {
    (110.0, 62.0 + 32.0 * index as f32)
}

/// Which data a shot's shell starts from.
#[derive(Clone, Copy)]
enum Data {
    /// The sample projects with the sample session open, as the app starts on sample data.
    Sample,
    /// The sample projects with no session open, so the start pages show.
    NoSession,
    /// No projects and no session: the empty start page.
    Empty,
    /// A client to a service that is not running, so the shell says it cannot reach it.
    Unreachable,
}

impl Data {
    /// The snapshot of the sample-backed variants; `Unreachable` takes its data from the service.
    fn snapshot(self) -> ShellSnapshot {
        let mut snapshot = SampleDataSource.snapshot();
        match self {
            Data::Sample | Data::Unreachable => {}
            Data::NoSession => {
                snapshot.session_title = None;
                snapshot.active_session = None;
            }
            Data::Empty => {
                snapshot.projects.clear();
                snapshot.session_title = None;
                snapshot.active_session = None;
            }
        }
        snapshot
    }
}

/// Steps that bring a fresh shell to the state a shot shows.
type Reach = fn(&mut HeadlessAppContext, AnyWindowHandle) -> anyhow::Result<()>;

/// One screenshot: its file stem after `shell-{theme}-`, its window width, its data and its steps.
struct Shot {
    name: &'static str,
    width: f32,
    data: Data,
    reach: Reach,
}

/// A shot of the sample session at the artboard width.
const fn sample(name: &'static str, reach: Reach) -> Shot {
    Shot {
        name,
        width: WIDTH,
        data: Data::Sample,
        reach,
    }
}

/// A shot of the sample session at a narrower window width.
const fn narrow(width: f32, name: &'static str) -> Shot {
    Shot {
        name,
        width,
        data: Data::Sample,
        reach: nothing,
    }
}

const SHOTS: &[Shot] = &[
    sample("", nothing),
    sample("palette", palette),
    sample("palette-recent", palette_recent),
    sample("palette-no-match", palette_no_match),
    sample("palette-sessions", palette_sessions),
    sample("quick-open", quick_open),
    sample("usage-popover", usage_popover),
    sample("status-menu", status_menu),
    sample("focus-zone", focus_zone),
    sample("session-summary", session_summary),
    sample("work-panel", work_panel),
    sample("project-sidebar", project_sidebar),
    sample("session-host-unreachable", unreachable_session),
    sample("tasks-page", tasks_page),
    sample("calendar-page", calendar_page),
    sample("marketplace-page", marketplace_page),
    sample("prototypes-page", prototypes_page),
    sample("automations-page", automations_page),
    sample("settings-general", settings_section::<0>),
    sample("settings-agents", settings_section::<1>),
    sample("settings-appearance", settings_section::<2>),
    sample("settings-input", settings_section::<3>),
    sample("settings-terminal", settings_section::<4>),
    sample("settings-sources", settings_section::<5>),
    sample("settings-hosts", settings_section::<6>),
    sample("settings-notifications", settings_section::<7>),
    sample("settings-extensions", settings_section::<8>),
    sample("settings-vault", settings_section::<9>),
    sample("settings-privacy", settings_section::<10>),
    sample("settings-routing", settings_section::<11>),
    Shot {
        name: "start-empty",
        width: WIDTH,
        data: Data::Empty,
        reach: nothing,
    },
    Shot {
        name: "start-workspace",
        width: WIDTH,
        data: Data::NoSession,
        reach: start_workspace,
    },
    Shot {
        name: "create-workspace",
        width: WIDTH,
        data: Data::Unreachable,
        reach: create_workspace,
    },
    Shot {
        name: "notice-unreachable",
        width: WIDTH,
        data: Data::Unreachable,
        reach: wait_for_service,
    },
    sample("bot-teams-home", bot_teams_home),
    sample("bot-teams-conference", bot_teams_conference),
    sample("bot-teams-board", bot_teams_board),
    sample("bot-teams-new-team", bot_teams_new_team),
    sample("bot-teams-add-bot", bot_teams_add_bot),
    narrow(1280.0, "narrow-1280"),
    narrow(1040.0, "narrow-1040"),
    narrow(840.0, "narrow-840"),
];

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
    // The profile of the unreachable shots: an empty home, so no real service is ever reached.
    let home = TempDir::new()?;

    for (theme, appearance) in [("dark", Appearance::Dark), ("light", Appearance::Light)] {
        cx.update(|app| Theme::set(appearance, app));
        for shot in SHOTS {
            let window = open(&mut cx, shot, home.path())?;
            (shot.reach)(&mut cx, window)?;
            let stem = match shot.name {
                "" => format!("shell-{theme}"),
                name => format!("shell-{theme}-{name}"),
            };
            support::write_png(&mut cx, window, &dir.join(format!("{stem}.png")))?;
            cx.update_window(window, |_, window, _| window.remove_window())?;
        }
    }
    Ok(())
}

/// Opens a shell of `shot`'s width and data, drawn at 1x so each PNG is the artboard's pixel size.
fn open(cx: &mut HeadlessAppContext, shot: &Shot, home: &Path) -> anyhow::Result<AnyWindowHandle> {
    let window_size = size(px(shot.width), px(HEIGHT));
    let window: AnyWindowHandle = match shot.data {
        Data::Unreachable => {
            let source = ClientDataSource::start(profile(home)?, None)?;
            cx.open_window(window_size, |window, cx| {
                cx.new(|cx| Shell::with_client(source, window, cx))
            })?
            .into()
        }
        data => {
            let fixed = Fixed(data.snapshot());
            cx.open_window(window_size, |window, cx| {
                cx.new(|cx| Shell::new(&fixed, &SampleTeamsSource, window, cx))
            })?
            .into()
        }
    };
    // The test platform paints at 2x by default; 1x makes each PNG the artboard's pixel size.
    cx.simulate_window_scale_factor_change(window, 1.0);
    Ok(window)
}

/// A fixed snapshot in place of the sample data, for the variants of `Data`.
struct Fixed(ShellSnapshot);

impl DataSource for Fixed {
    fn snapshot(&self) -> ShellSnapshot {
        self.0.clone()
    }
}

fn nothing(_: &mut HeadlessAppContext, _: AnyWindowHandle) -> anyhow::Result<()> {
    Ok(())
}

fn palette(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    press(cx, window, PALETTE_KEY)
}

/// Runs "Toggle sidebar" from the palette, so it comes back under Recent when the palette reopens.
fn palette_recent(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    press(cx, window, PALETTE_KEY)?;
    type_text(cx, window, "sidebar")?;
    press(cx, window, "enter")?;
    press(cx, window, PALETTE_KEY)
}

fn palette_no_match(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    press(cx, window, PALETTE_KEY)?;
    type_text(cx, window, "zzqx")
}

fn palette_sessions(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    press(cx, window, PALETTE_KEY)?;
    type_text(cx, window, "feed")
}

fn quick_open(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    press(cx, window, QUICK_OPEN_KEY)
}

fn usage_popover(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, USAGE_RING)
}

fn status_menu(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    right_click(cx, window, STATUS_HOSTS)
}

fn focus_zone(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    press(cx, window, "f6")
}

fn session_summary(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, SESSION_SUMMARY_TOGGLE)
}

fn work_panel(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, WORK_PANEL_TOGGLE)
}

fn project_sidebar(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, PROJECT_SIDEBAR_TOGGLE)
}

fn unreachable_session(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, UNREACHABLE_SESSION_ROW)
}

fn tasks_page(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, TASKS_BUTTON)
}

fn calendar_page(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, CALENDAR_BUTTON)
}

fn marketplace_page(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, MARKETPLACE_BUTTON)
}

fn prototypes_page(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, PROTOTYPES_ROW)
}

fn automations_page(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, AUTOMATIONS_ROW)
}

/// Opens Settings and chooses section `N`, in the spec's order.
fn settings_section<const N: usize>(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
) -> anyhow::Result<()> {
    press(cx, window, SETTINGS_KEY)?;
    click(cx, window, settings_nav(N))
}

fn start_workspace(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, WORKSPACE_ROW)
}

fn create_workspace(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, CREATE_BUTTON)
}

fn bot_teams_home(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    click(cx, window, MODE_BUTTON)
}

fn bot_teams_conference(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
) -> anyhow::Result<()> {
    bot_teams_home(cx, window)?;
    click(cx, window, TEAM_CONFERENCE_ROW)
}

fn bot_teams_board(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    bot_teams_home(cx, window)?;
    click(cx, window, TEAM_BOARD_ROW)
}

fn bot_teams_new_team(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    bot_teams_home(cx, window)?;
    click(cx, window, NEW_TEAM_BUTTON)
}

fn bot_teams_add_bot(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    bot_teams_home(cx, window)?;
    click(cx, window, TEAM_ADD_BOT_BUTTON)
}

/// Lets the bridge's first attempt fail and its update reach the store, which runs on its own
/// thread; a refused local endpoint answers at once, so a second is plenty.
fn wait_for_service(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    for _ in 0..40 {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(25));
    }
    cx.run_until_parked();
    support::draw(cx, window)
}

/// One service-backed screenshot: writes its PNG for `theme` into `dir`.
type Scenario = fn(&mut HeadlessAppContext, &Path, &str) -> anyhow::Result<()>;

const SCENARIOS: &[Scenario] = &[
    connecting,
    reconnecting,
    demo_start,
    demo_menu,
    demo_remove,
    demo_unsaved,
    demo_ignored,
    create_project_menu,
];

#[test]
#[ignore = "writes PNG screenshots; run with -- --ignored"]
fn service_screenshots() -> anyhow::Result<()> {
    let Some((mut cx, _platform)) = support::headless_context(Arc::new(Assets)) else {
        return Ok(());
    };
    cx.update(|app| massorca_ui::init(Appearance::Dark, app))?;
    cx.update(shell::bind_keys);
    support::assert_geist_loaded(&cx)?;
    let dir = support::output_dir()?;
    for (theme, appearance) in [("dark", Appearance::Dark), ("light", Appearance::Light)] {
        cx.update(|app| Theme::set(appearance, app));
        for scenario in SCENARIOS {
            scenario(&mut cx, &dir, theme)?;
        }
    }
    Ok(())
}

/// A temporary home served by a running service, with the project "demo" and its workspace
/// "fix-login", a worktree of a repository with one commit.
struct Demo {
    home: TempDir,
    service: LocalService,
    workspace: Workspace,
}

fn start_demo() -> anyhow::Result<Demo> {
    let home = TempDir::new()?;
    let repo_dir = home.path().join("demo");
    repo(&repo_dir)?;
    let service = LocalService::start(profile(home.path())?)?;
    let workspace = block_on(add_project_and_workspace(
        &profile(home.path())?,
        &repo_dir,
        "fix-login",
    ))??;
    Ok(Demo {
        home,
        service,
        workspace,
    })
}

/// Opens a shell that follows the service of `home`, and the handle that reports its link.
fn open_following(
    cx: &mut HeadlessAppContext,
    home: &Path,
) -> anyhow::Result<(WindowHandle<Shell>, ServiceHandle)> {
    let source = ClientDataSource::start(profile(home)?, None)?;
    let link = source.handle();
    let window = cx.open_window(size(px(WIDTH), px(HEIGHT)), |window, cx| {
        cx.new(|cx| Shell::with_client(source, window, cx))
    })?;
    cx.simulate_window_scale_factor_change(window.into(), 1.0);
    Ok((window, link))
}

/// Runs the background work until `done` holds or `LIMIT` passes; returns whether it held.
fn wait_until(cx: &mut HeadlessAppContext, done: impl Fn(&mut HeadlessAppContext) -> bool) -> bool {
    let started = Instant::now();
    loop {
        cx.run_until_parked();
        if done(cx) {
            return true;
        }
        if started.elapsed() > LIMIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Lets the background work run for `span`, as a service answer needs a moment to reach the shell.
fn let_work_run(cx: &mut HeadlessAppContext, span: Duration) {
    let started = Instant::now();
    while started.elapsed() < span {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(25));
    }
    cx.run_until_parked();
}

/// Whether the shell lists a workspace called `name`.
fn lists_workspace(cx: &mut HeadlessAppContext, window: WindowHandle<Shell>, name: &str) -> bool {
    window
        .update(cx, |shell, _, _| {
            shell
                .model()
                .snapshot()
                .projects
                .iter()
                .flat_map(|project| &project.hosts)
                .flat_map(|host| &host.workspaces)
                .any(|workspace| workspace.name == name)
        })
        .unwrap_or(false)
}

fn shoot(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<Shell>,
    dir: &Path,
    theme: &str,
    name: &str,
) -> anyhow::Result<()> {
    support::write_png(
        cx,
        window.into(),
        &dir.join(format!("shell-{theme}-{name}.png")),
    )
}

fn close(cx: &mut HeadlessAppContext, window: WindowHandle<Shell>) -> anyhow::Result<()> {
    cx.update_window(window.into(), |_, window, _| window.remove_window())
}

/// A shell following the demo service, with its workspace listed.
fn demo_shell(cx: &mut HeadlessAppContext, demo: &Demo) -> anyhow::Result<WindowHandle<Shell>> {
    let (window, _link) = open_following(cx, demo.home.path())?;
    anyhow::ensure!(
        wait_until(cx, |cx| lists_workspace(cx, window, "fix-login")),
        "the workspace never reached the shell"
    );
    Ok(window)
}

#[cfg(unix)]
fn connecting(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let profile = profile(home.path())?;
    std::fs::create_dir_all(profile.dir())?;
    // A listener that never answers: the bridge's first attempt connects and waits for a welcome.
    let _listener = std::os::unix::net::UnixListener::bind(profile.socket_path())?;
    let (window, _link) = open_following(cx, home.path())?;
    let_work_run(cx, Duration::from_millis(300));
    shoot(cx, window, dir, theme, "notice-connecting")?;
    close(cx, window)
}

#[cfg(not(unix))]
fn connecting(_: &mut HeadlessAppContext, _: &Path, _: &str) -> anyhow::Result<()> {
    Ok(())
}

fn reconnecting(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let service = LocalService::start(profile(home.path())?)?;
    let (window, link) = open_following(cx, home.path())?;
    anyhow::ensure!(
        wait_until(cx, |_| link.connection().is_connected()),
        "the shell never connected"
    );
    service.stop();
    anyhow::ensure!(
        wait_until(cx, |_| matches!(
            link.connection(),
            ConnectionState::Reconnecting { .. }
        )),
        "the shell never noticed the loss"
    );
    shoot(cx, window, dir, theme, "notice-reconnecting")?;
    close(cx, window)
}

fn demo_start(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let demo = start_demo()?;
    let window = demo_shell(cx, &demo)?;
    click(cx, window.into(), WORKSPACE_ROW)?;
    shoot(cx, window, dir, theme, "start-workspace-demo")?;
    close(cx, window)?;
    demo.service.stop();
    Ok(())
}

fn demo_menu(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let demo = start_demo()?;
    let window = demo_shell(cx, &demo)?;
    right_click(cx, window.into(), WORKSPACE_ROW)?;
    shoot(cx, window, dir, theme, "sidebar-row-menu")?;
    close(cx, window)?;
    demo.service.stop();
    Ok(())
}

fn demo_remove(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let demo = start_demo()?;
    let window = demo_shell(cx, &demo)?;
    right_click(cx, window.into(), WORKSPACE_ROW)?;
    click(cx, window.into(), REMOVE_ITEM)?;
    shoot(cx, window, dir, theme, "remove-clean")?;
    close(cx, window)?;
    demo.service.stop();
    Ok(())
}

fn demo_unsaved(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let demo = start_demo()?;
    std::fs::write(
        Path::new(&demo.workspace.path).join("notes.txt"),
        "unsaved\n",
    )?;
    let window = demo_shell(cx, &demo)?;
    right_click(cx, window.into(), WORKSPACE_ROW)?;
    click(cx, window.into(), REMOVE_ITEM)?;
    click(cx, window.into(), CONFIRM_REMOVE)?;
    let_work_run(cx, Duration::from_millis(1500));
    shoot(cx, window, dir, theme, "remove-unsaved")?;
    close(cx, window)?;
    demo.service.stop();
    Ok(())
}

fn demo_ignored(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let demo = start_demo()?;
    std::fs::write(
        Path::new(&demo.workspace.path).join("debug.log"),
        "ignored\n",
    )?;
    let window = demo_shell(cx, &demo)?;
    right_click(cx, window.into(), WORKSPACE_ROW)?;
    click(cx, window.into(), REMOVE_ITEM)?;
    click(cx, window.into(), CONFIRM_REMOVE)?;
    let_work_run(cx, Duration::from_millis(1500));
    shoot(cx, window, dir, theme, "remove-ignored")?;
    close(cx, window)?;
    demo.service.stop();
    Ok(())
}

fn create_project_menu(cx: &mut HeadlessAppContext, dir: &Path, theme: &str) -> anyhow::Result<()> {
    let demo = start_demo()?;
    let window = demo_shell(cx, &demo)?;
    click(cx, window.into(), CREATE_BUTTON)?;
    click(cx, window.into(), CHOOSE_PROJECT)?;
    shoot(cx, window, dir, theme, "create-project-menu")?;
    close(cx, window)?;
    demo.service.stop();
    Ok(())
}
