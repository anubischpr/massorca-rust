//! Element tests of the shell in a test window (no visible window is opened).

use super::workspace_dialogs::MenuTarget;
use super::*;
use crate::data::{DataSource, SampleDataSource};
use crate::model::navigation::{Mode, Page};
use crate::model::palette::{PaletteGroup, PaletteMode, PaletteModel};
use crate::model::region_cycle::Zone;
use crate::model::regions::Region;
use crate::model::shell::{Agent, ShellSnapshot, UsageModel, UsageStatus};
use crate::model::status_items::StatusItem;
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size};
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
    cx.simulate_input("t");
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

/// Sample data whose rings show the reading states other than a percentage.
struct UsageStates;

impl DataSource for UsageStates {
    fn snapshot(&self) -> ShellSnapshot {
        let mut snapshot = SampleDataSource.snapshot();
        let states = [
            UsageStatus::Loading,
            UsageStatus::NotReported,
            UsageStatus::Unavailable,
            UsageStatus::Failed { last: Some(62) },
        ];
        snapshot.usage = states
            .into_iter()
            .enumerate()
            .map(|(index, status)| UsageModel {
                agent: Agent::Claude,
                slot: u8::try_from(index + 1).unwrap_or(1),
                status,
                resets_in: String::new(),
            })
            .collect();
        snapshot
    }
}

fn usage_popover(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<usize> {
    shell.read_with(cx, |shell, _| shell.model().usage_popover())
}

#[gpui::test]
fn a_usage_ring_opens_its_popover_and_a_second_press_closes_it(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    click(cx, "rail-usage-0");
    assert_eq!(usage_popover(&shell, cx), Some(0));
    draw(cx);
    assert!(
        cx.debug_bounds("popover-panel").is_some(),
        "the popover is drawn"
    );
    click(cx, "rail-usage-0");
    assert_eq!(usage_popover(&shell, cx), None);
}

#[gpui::test]
fn escape_closes_an_open_usage_popover(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    click(cx, "rail-usage-1");
    assert_eq!(usage_popover(&shell, cx), Some(1));
    cx.simulate_keystrokes("escape");
    assert_eq!(usage_popover(&shell, cx), None);
}

#[gpui::test]
fn a_ring_without_a_percentage_still_opens_its_popover(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&UsageStates, window, cx));
    click(cx, "rail-usage-2");
    draw(cx);
    assert_eq!(usage_popover(&shell, cx), Some(2));
    assert!(cx.debug_bounds("popover-panel").is_some());
}

/// The rows of the open Status bar menu: `None` for the heading, the item for each other row.
fn status_menu_rows(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<Option<StatusItem>> {
    shell.read_with(cx, |shell, _| {
        match shell.dialogs.menu.as_ref().map(|open| &open.target) {
            Some(MenuTarget::StatusItems(rows)) => rows.clone(),
            _ => Vec::new(),
        }
    })
}

#[gpui::test]
fn right_clicking_the_status_bar_opens_a_checkable_row_per_item(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    let hosts = cx
        .debug_bounds("status-hosts")
        .expect("the hosts item is drawn");
    cx.simulate_mouse_down(hosts.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(hosts.center(), MouseButton::Right, Modifiers::default());
    draw(cx);
    let rows = status_menu_rows(&shell, cx);
    // A heading, then the five items of sample data; the service item needs a service.
    assert_eq!(rows.len(), 6);
    assert_eq!(rows.iter().flatten().count(), 5);
    assert!(cx.debug_bounds("menu-panel").is_some());
}

#[gpui::test]
fn choosing_a_row_hides_that_item_and_closes_the_menu(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    shell.update_in(cx, |shell, window, cx| {
        shell.open_status_menu(point(px(40.), px(40.)), window, cx);
    });
    draw(cx);
    // The first item is the active row, so Enter chooses it.
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(!shell.read_with(cx, |shell, _| {
        shell.model().status_items().is_shown(StatusItem::Agents)
    }));
    assert!(
        cx.debug_bounds("status-agents").is_none(),
        "the hidden item leaves the bar"
    );
    assert!(cx.debug_bounds("status-memory").is_some());
    assert!(shell.read_with(cx, |shell, _| shell.dialogs.menu.is_none()));
}

#[gpui::test]
fn the_menu_key_on_the_status_bar_opens_the_visibility_menu(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    cx.update(|window, cx| {
        let focus = shell.read(cx).zones.get(Zone::StatusBar).clone();
        window.focus(&focus, cx);
    });
    draw(cx);
    cx.simulate_keystrokes("menu");
    draw(cx);
    assert!(!status_menu_rows(&shell, cx).is_empty());
}

#[gpui::test]
fn a_command_run_from_the_palette_comes_back_under_recent(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    cx.simulate_keystrokes(palette_key());
    draw(cx);
    cx.simulate_input("theme");
    draw(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    cx.simulate_keystrokes(palette_key());
    draw(cx);
    let first = shell.read_with(cx, |shell, _| {
        let palette = shell.model().palette().expect("the palette is open");
        let found = palette.matches().first().expect("a recent row is listed");
        let item = &palette.items()[found.item];
        (item.group, item.title.clone())
    });
    assert_eq!(first, (PaletteGroup::Recent, "Toggle theme".to_owned()));
}

fn quick_open_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-p"
    } else {
        "ctrl-p"
    }
}

#[gpui::test]
fn mod_p_opens_the_palette_in_go_to_file_mode_and_escape_closes_it(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    draw(cx);
    cx.simulate_keystrokes(quick_open_key());
    draw(cx);
    let mode = shell.read_with(cx, |shell, _| {
        shell.model().palette().map(PaletteModel::mode)
    });
    assert_eq!(mode, Some(PaletteMode::GoToFile));
    assert!(
        cx.debug_bounds("palette-mode").is_some(),
        "the mode label is drawn"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(palette_query(&shell, cx), None);
}
