//! The Git row of Settings > Sources and integrations draws the state the service answered (GIT-01).
//! Each answer goes through the store as the bridge sends it, and the row is checked through the
//! selector of its state; the words and sentences of each state are tested in the model.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::service::GitStatus as ServiceGitStatus;

use super::settings_tests::{Rig, click, draw, settings_rig};
use crate::service::{Refusal, StoreUpdate};

const STATE_SELECTORS: [&str; 5] = [
    "settings-git-not-checked",
    "settings-git-present",
    "settings-git-missing",
    "settings-git-too-old",
    "settings-git-unknown",
];

/// Sends one answer of the service through the store, and draws the page again.
fn answer(rig: &Rig, cx: &mut VisualTestContext, git: Result<Option<ServiceGitStatus>, Refusal>) {
    rig._updates
        .send(StoreUpdate::ServiceGit(git))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

/// The Git row draws the control of `expected` and no other state's control.
fn assert_only_drawn(cx: &mut VisualTestContext, expected: &str) {
    for selector in STATE_SELECTORS {
        assert_eq!(
            cx.debug_bounds(selector).is_some(),
            selector == expected,
            "{selector} is drawn only for {expected}"
        );
    }
}

#[gpui::test]
fn the_git_row_draws_the_state_the_service_answered(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    click(cx, "settings-nav-Sources");
    assert_only_drawn(cx, "settings-git-not-checked");

    answer(
        &rig,
        cx,
        Ok(Some(ServiceGitStatus::Present {
            version: "2.49.0".to_owned(),
        })),
    );
    assert_only_drawn(cx, "settings-git-present");

    answer(&rig, cx, Ok(Some(ServiceGitStatus::Missing)));
    assert_only_drawn(cx, "settings-git-missing");

    answer(
        &rig,
        cx,
        Ok(Some(ServiceGitStatus::TooOld {
            version: "2.20.1".to_owned(),
            required: "2.25.0".to_owned(),
        })),
    );
    assert_only_drawn(cx, "settings-git-too-old");

    answer(
        &rig,
        cx,
        Ok(Some(ServiceGitStatus::Unknown {
            reason: "git --version timed out".to_owned(),
        })),
    );
    assert_only_drawn(cx, "settings-git-unknown");

    answer(&rig, cx, Ok(None));
    assert_only_drawn(cx, "settings-git-unknown");

    answer(&rig, cx, Err(Refusal::unavailable("the service went away")));
    assert_only_drawn(cx, "settings-git-unknown");
}
