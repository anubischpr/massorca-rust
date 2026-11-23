//! The Status bar in a test window: which items it draws in each mode, and the copy they read (D-SH-05).

use super::Shell;
use crate::data::SampleDataSource;
use crate::model::teams::SampleTeamsSource;
use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_ui::{Appearance, Theme as UiTheme};

fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| {
        UiTheme::set(Appearance::Dark, cx);
        crate::shell::bind_keys(cx);
    });
    cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx))
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn the_workspaces_status_bar_draws_its_agents_and_labelled_memory(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx);
    draw(cx);
    assert!(cx.debug_bounds("status-agents").is_some());
    assert!(cx.debug_bounds("status-memory").is_some());
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.model().snapshot().status.memory_text(),
            Some("Memory 1.22 GB".into())
        );
    });
}

#[gpui::test]
fn the_bot_teams_status_bar_counts_terminals_and_ports_and_names_unreachable_hosts(
    cx: &mut TestAppContext,
) {
    let (shell, cx) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.model.press_mode_switch();
        cx.notify();
    });
    draw(cx);
    for selector in ["status-terminals", "status-ports", "status-hosts"] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} is drawn in Bot Teams"
        );
    }
    shell.read_with(cx, |shell, _| {
        let status = &shell.model().snapshot().status;
        assert_eq!(status.bot_teams_terminals_text(), "Terminals 2");
        assert_eq!(status.bot_teams_ports_text(), Some("Ports 0".into()));
        assert_eq!(status.bot_teams_hosts_text(), "1 host unreachable");
    });
}
