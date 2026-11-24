//! The Sidebar's fold rows on a service store: "Show N more" unfolds a workspace's sessions and "Show less"
//! folds them again. A scripted handle answers nothing, so no window opens.

use super::service_tests::{click, draw, rig};
use crate::service::StoreUpdate;
use crate::service::fixtures::agent;
use gpui::TestAppContext;

#[gpui::test]
fn show_more_unfolds_a_workspace_and_show_less_folds_it_again(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let sessions: Vec<_> = (1..=7).map(|n| agent(&format!("a{n}"), "w1", n)).collect();
    rig.updates
        .send(StoreUpdate::AgentsResync(sessions))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-show-less-w1").is_none());

    click(cx, "sidebar-show-more-w1");
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-show-more-w1").is_none(),
        "every session is listed"
    );
    assert!(cx.debug_bounds("sidebar-show-less-w1").is_some());
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.sidebar_folds.is_unfolded("w1"))
    });

    click(cx, "sidebar-show-less-w1");
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-show-more-w1").is_some(),
        "the rest fold again"
    );
    rig.shell.read_with(cx, |shell, _| {
        assert!(!shell.sidebar_folds.is_unfolded("w1"))
    });
}
