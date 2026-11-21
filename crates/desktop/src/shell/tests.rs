//! Element tests of the shell in a test window (no visible window is opened).

use super::*;
use crate::data::{DataSource, SampleDataSource};
use crate::model::navigation::Page;
use crate::model::regions::Region;
use crate::model::shell::ShellSnapshot;
use gpui::{TestAppContext, px, size};
use massorca_ui::{Appearance, Theme};

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

fn draw(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
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
