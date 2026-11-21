//! Element tests of the shell in a test window (no visible window is opened).

use super::*;
use crate::data::SampleDataSource;
use crate::model::navigation::Page;
use gpui::TestAppContext;
use massorca_ui::{Appearance, Theme};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
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
