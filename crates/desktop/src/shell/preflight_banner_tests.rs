//! The startup preflight notice in a test window: git missing or too old, dismissed, and gone once fixed on reconnect
//! (PLAT-08). The scripted store stands in for the service, so each `service.info` answer is sent by hand.

use super::service_tests::{Rig, connected, draw, rig};
use crate::service::{ConnectionState, StoreUpdate};
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::methods::service::GitStatus;
use std::time::Instant;

/// The service answers its git status, and the window draws the answer.
fn report_git(rig: &Rig, cx: &mut VisualTestContext, git: GitStatus) {
    rig.updates
        .send(StoreUpdate::ServiceGit(Some(git)))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

fn notice_title(rig: &Rig, cx: &mut VisualTestContext) -> Option<String> {
    rig.shell.read_with(cx, |shell, _| {
        shell.model().preflight().map(|notice| notice.title.clone())
    })
}

#[gpui::test]
fn a_missing_git_draws_a_notice_that_says_what_to_do(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, GitStatus::Missing);
    assert!(
        cx.debug_bounds("preflight-banner").is_some(),
        "the notice is drawn"
    );
    let message = rig.shell.read_with(cx, |shell, _| {
        shell
            .model()
            .preflight()
            .map(|notice| notice.message.clone())
    });
    let message = message.expect("a notice");
    assert!(message.contains("2.25.0 or newer"), "{message}");
    assert!(message.contains("MASSORCA_GIT"), "{message}");
}

#[gpui::test]
fn a_too_old_git_names_its_version_and_the_one_required(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(
        &rig,
        cx,
        GitStatus::TooOld {
            version: "2.20.1".to_owned(),
            required: "2.25.0".to_owned(),
        },
    );
    assert!(cx.debug_bounds("preflight-banner").is_some());
    assert_eq!(
        notice_title(&rig, cx).as_deref(),
        Some("Git 2.20.1 is too old")
    );
}

#[gpui::test]
fn a_present_or_unknown_git_draws_no_notice(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(
        &rig,
        cx,
        GitStatus::Present {
            version: "2.49.0".to_owned(),
        },
    );
    assert!(cx.debug_bounds("preflight-banner").is_none(), "present");
    report_git(
        &rig,
        cx,
        GitStatus::Unknown {
            reason: "timed out".to_owned(),
        },
    );
    assert!(cx.debug_bounds("preflight-banner").is_none(), "unknown");
}

#[gpui::test]
fn dismissing_the_notice_hides_it_until_the_problem_changes(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, GitStatus::Missing);
    let dismiss = cx
        .debug_bounds("banner-dismiss")
        .expect("the notice has a dismiss control");
    cx.simulate_click(dismiss.center(), Modifiers::default());
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("preflight-banner").is_none(), "dismissed");

    report_git(&rig, cx, GitStatus::Missing);
    assert!(
        cx.debug_bounds("preflight-banner").is_none(),
        "the same problem stays dismissed"
    );

    report_git(
        &rig,
        cx,
        GitStatus::TooOld {
            version: "2.20.1".to_owned(),
            required: "2.25.0".to_owned(),
        },
    );
    assert!(
        cx.debug_bounds("preflight-banner").is_some(),
        "a different problem shows again"
    );
}

#[gpui::test]
fn a_fixed_git_on_reconnect_clears_the_notice(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, GitStatus::Missing);
    assert!(cx.debug_bounds("preflight-banner").is_some(), "broken");

    rig.updates
        .send(StoreUpdate::Connection(ConnectionState::Reconnecting {
            attempt: 1,
            retry_at: Instant::now(),
        }))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    report_git(
        &rig,
        cx,
        GitStatus::Present {
            version: "2.49.0".to_owned(),
        },
    );
    assert!(
        cx.debug_bounds("preflight-banner").is_none(),
        "gone once fixed on reconnect"
    );
}
