//! Element test of the debug focus: the Search field takes the focus once the first frame is drawn,
//! as a background debug launch asks for, and the typed text reaches it.

use super::*;
use crate::data::SampleDataSource;
use crate::launch_options::DebugFocus;
use crate::model::teams::SampleTeamsSource;
use gpui::{TestAppContext, VisualTestContext};
use massorca_ui::{Appearance, Theme};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn the_debug_search_focus_takes_hold_on_the_first_frame(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx);
        shell.start_debug_input(DebugFocus::Search, window, cx);
        shell
    });
    draw(cx);
    let search = shell.read_with(cx, |shell, cx| shell.search.read(cx).focus_handle(cx));
    cx.update(|window, _| {
        assert!(
            !search.is_focused(window),
            "the focus waits for the first frame"
        );
    });

    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    draw(cx);
    cx.update(|window, _| assert!(search.is_focused(window)));

    cx.simulate_input("ab");
    let query = shell.read_with(cx, |shell, cx| shell.search.read(cx).text(cx).to_owned());
    assert_eq!(query, "ab");
}

#[gpui::test]
fn the_debug_composer_focus_takes_hold_on_the_first_frame(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let mut shell = Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx);
        shell.start_debug_input(DebugFocus::Composer, window, cx);
        shell
    });
    draw(cx);
    let workspace = shell
        .read_with(cx, |shell, _| {
            shell.model().selected_workspace().map(str::to_owned)
        })
        .expect("the debug launch opens the first workspace");
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    draw(cx);
    assert!(
        cx.debug_bounds("start-project-link").is_some(),
        "the workspace's start page is drawn"
    );
    let composer = shell.update(cx, |shell, cx| shell.start_composer(&workspace, cx));
    cx.simulate_input("ab");
    assert_eq!(
        composer.read_with(cx, |composer, cx| composer.draft(cx)),
        "ab"
    );
}
