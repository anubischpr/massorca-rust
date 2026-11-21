//! Element tests of the shell in a test window (no visible window is opened).

use super::*;
use crate::data::{DataSource, SampleDataSource};
use crate::model::navigation::{Mode, Page};
use crate::model::region_cycle::Zone;
use crate::model::regions::Region;
use crate::model::shell::ShellSnapshot;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, px, size};
use massorca_ui::{ActiveTheme, Appearance, Theme};

/// The sample data without an open session: the landing view.
struct Landing;

impl DataSource for Landing {
    fn snapshot(&self) -> ShellSnapshot {
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.session_title = None;
        snapshot
    }
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Clicks the centre of the element with this debug selector, after a fresh draw.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

fn navigation(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Mode, Page) {
    shell.read_with(cx, |shell, _| {
        (
            shell.model().navigation().mode,
            shell.model().navigation().page,
        )
    })
}

#[gpui::test]
fn the_shell_opens_on_home_with_sample_data(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model().snapshot().projects.len(), 2);
        assert_eq!(shell.model().navigation().page, Page::Home);
    });
}

/// Draws the current state once in each theme, so a broken paint in either shows up here.
fn draw_in_both_themes(cx: &mut VisualTestContext) {
    for appearance in [Appearance::Dark, Appearance::Light] {
        cx.update(|_, cx| Theme::set(appearance, cx));
        draw(cx);
    }
}

#[gpui::test]
fn every_shell_surface_renders_in_both_themes(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw_in_both_themes(cx);
    cx.simulate_keystrokes(palette_key());
    draw_in_both_themes(cx);
    cx.simulate_keystrokes("escape");
    click(cx, "rail-tasks");
    draw_in_both_themes(cx);
    click(cx, "rail-mode");
    click(cx, "rail-mode");
    draw_in_both_themes(cx);
    cx.simulate_resize(size(px(900.), px(600.)));
    shell.update(cx, |shell, cx| {
        shell.model.toggle_sidebar();
        cx.notify();
    });
    draw_in_both_themes(cx);
}

#[gpui::test]
fn the_landing_view_with_a_region_open_renders_in_both_themes(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&Landing, window, cx));
    shell.update(cx, |shell, _| {
        shell.model.toggle_region(Region::WorkPanel);
    });
    for appearance in [Appearance::Dark, Appearance::Light] {
        cx.update(|_, cx| Theme::set(appearance, cx));
        draw(cx);
    }
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.model().inline_regions()),
        vec![Region::WorkPanel]
    );
}

#[gpui::test]
fn the_mode_button_returns_home_first_and_then_switches_twice(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    click(cx, "rail-tasks");
    assert_eq!(navigation(&shell, cx), (Mode::Workspaces, Page::Tasks));
    click(cx, "rail-mode");
    assert_eq!(navigation(&shell, cx), (Mode::Workspaces, Page::Home));
    click(cx, "rail-mode");
    assert_eq!(navigation(&shell, cx), (Mode::BotTeams, Page::Home));
}

#[gpui::test]
fn the_rail_theme_toggle_switches_the_appearance(cx: &mut TestAppContext) {
    setup(cx);
    let (_shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    click(cx, "rail-theme");
    let appearance = cx.update(|_, cx| cx.theme().appearance);
    assert_eq!(appearance, Appearance::Light);
}

#[gpui::test]
fn a_title_bar_toggle_opens_its_region_inline(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    click(cx, "tip:Work panel");
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.model().inline_regions()),
        vec![Region::WorkPanel]
    );
}

#[gpui::test]
fn the_sidebar_leaves_full_pages_and_returns_with_the_mode_home(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    assert!(shell.read_with(cx, |shell, _| shell.model().sidebar_inline()));
    click(cx, "rail-tasks");
    assert!(!shell.read_with(cx, |shell, _| shell.model().sidebar_inline()));
    click(cx, "rail-mode");
    assert!(shell.read_with(cx, |shell, _| shell.model().sidebar_inline()));
}

#[gpui::test]
fn a_click_on_the_canvas_closes_the_sidebar_overlay(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    shell.update(cx, |shell, cx| {
        shell.model.toggle_sidebar();
        cx.notify();
    });
    draw(cx);
    assert!(shell.read_with(cx, |shell, _| shell.model().sidebar_overlay()));
    click(cx, "canvas");
    assert!(!shell.read_with(cx, |shell, _| shell.model().sidebar_overlay()));
}

#[gpui::test]
fn the_status_bar_sits_under_the_canvas_and_names_the_hosts(cx: &mut TestAppContext) {
    setup(cx);
    let (_shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    let canvas = cx.debug_bounds("canvas").expect("the canvas is drawn");
    let hosts = cx
        .debug_bounds("status-hosts")
        .expect("the hosts item is drawn");
    assert!(f32::from(hosts.top()) >= f32::from(canvas.bottom()) - 0.5);
    assert!(f32::from(hosts.left()) > f32::from(canvas.left()));
}

fn palette_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    }
}

fn palette_query(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<String> {
    shell.read_with(cx, |shell, _| {
        shell
            .model()
            .palette()
            .map(|palette| palette.query().to_owned())
    })
}

#[gpui::test]
fn mod_k_opens_the_palette_and_a_command_typed_into_it_runs_on_enter(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    cx.simulate_keystrokes(palette_key());
    draw(cx);
    assert_eq!(palette_query(&shell, cx).as_deref(), Some(""));
    cx.simulate_input("theme");
    draw(cx);
    assert_eq!(palette_query(&shell, cx).as_deref(), Some("theme"));
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(palette_query(&shell, cx), None);
    let appearance = cx.update(|_, cx| cx.theme().appearance);
    assert_eq!(appearance, Appearance::Light);
}

#[gpui::test]
fn escape_closes_the_palette_and_returns_focus_to_the_shell(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    cx.simulate_keystrokes(palette_key());
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(palette_query(&shell, cx), None);
    let focused = cx.update(|window, cx| {
        let focus = shell.read(cx).focus.clone();
        focus.is_focused(window)
    });
    assert!(focused);
}

#[gpui::test]
fn the_arrow_keys_move_the_active_palette_row(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    cx.simulate_keystrokes(palette_key());
    draw(cx);
    cx.simulate_keystrokes("down");
    let active = shell.read_with(cx, |shell, _| {
        shell
            .model()
            .palette()
            .map(|palette| palette.active_index())
    });
    assert_eq!(active, Some(1));
}

fn zone_has_focus(shell: &Entity<Shell>, cx: &mut VisualTestContext, zone: Zone) -> bool {
    cx.update(|window, cx| shell.read(cx).zones.get(zone).is_focused(window))
}

#[gpui::test]
fn f6_moves_focus_through_the_frame_and_shift_f6_steps_back(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    cx.simulate_keystrokes("f6");
    draw(cx);
    assert!(zone_has_focus(&shell, cx, Zone::TitleBar));
    cx.simulate_keystrokes("f6");
    draw(cx);
    let rail_item_focused =
        cx.update(|window, cx| shell.read(cx).rail.focus_target().is_focused(window));
    assert!(rail_item_focused);
    cx.simulate_keystrokes("shift-f6");
    draw(cx);
    assert!(zone_has_focus(&shell, cx, Zone::TitleBar));
}

#[gpui::test]
fn escape_closes_a_region_overlay_opened_on_a_narrow_window(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    // A resize refreshes the real window; the test window needs the view told to re-render.
    shell.update(cx, |_, cx| cx.notify());
    draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.model().window_width()),
        900.0
    );
    shell.update(cx, |shell, _| {
        shell.model.toggle_region(Region::SessionSummary);
    });
    draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.model().overlay()),
        Some(Region::SessionSummary)
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.model().overlay()),
        None
    );
}
