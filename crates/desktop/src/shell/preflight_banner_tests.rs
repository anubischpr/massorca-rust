//! The startup preflight notice in a test window: git missing or too old, dismissed, and gone once fixed on reconnect
//! (PLAT-08). The scripted store stands in for the service, so each `service.info` answer is sent by hand, and the
//! words are the Git row's (GIT-01).

use super::service_tests::{Rig, connected, draw, rig};
use crate::service::{ConnectionState, Refusal, StoreUpdate};
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::methods::service::GitStatus;
use std::time::Instant;

/// The service answers this computer's git, and the window draws the answer.
fn report_git(rig: &Rig, cx: &mut VisualTestContext, answer: Result<Option<GitStatus>, Refusal>) {
    rig.updates
        .send(StoreUpdate::ServiceGit(answer))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

fn missing() -> Result<Option<GitStatus>, Refusal> {
    Ok(Some(GitStatus::Missing))
}

fn too_old() -> Result<Option<GitStatus>, Refusal> {
    Ok(Some(GitStatus::TooOld {
        version: "2.20.1".to_owned(),
        required: "2.25.0".to_owned(),
    }))
}

fn present() -> Result<Option<GitStatus>, Refusal> {
    Ok(Some(GitStatus::Present {
        version: "2.49.0".to_owned(),
    }))
}

fn notice(rig: &Rig, cx: &mut VisualTestContext) -> Option<(String, String)> {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .model()
            .preflight()
            .map(|notice| (notice.title.clone(), notice.message.clone()))
    })
}

#[gpui::test]
fn a_missing_git_draws_a_notice_that_says_what_to_do(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, missing());
    assert!(
        cx.debug_bounds("preflight-banner").is_some(),
        "the notice is drawn"
    );
    let (title, message) = notice(&rig, cx).expect("a notice");
    assert_eq!(title, "Not installed");
    assert!(message.contains("Install Git"), "{message}");
    assert!(message.contains("restart MassOrca"), "{message}");
}

#[gpui::test]
fn a_too_old_git_names_its_version_and_the_one_required(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, too_old());
    assert!(cx.debug_bounds("preflight-banner").is_some());
    let (title, message) = notice(&rig, cx).expect("a notice");
    assert_eq!(title, "Too old");
    assert!(message.contains("2.20.1"), "{message}");
    assert!(message.contains("2.25.0"), "{message}");
    assert!(message.contains("restart MassOrca"), "{message}");
}

#[gpui::test]
fn a_present_unknown_or_refused_git_draws_no_notice(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, present());
    assert!(cx.debug_bounds("preflight-banner").is_none(), "present");
    report_git(
        &rig,
        cx,
        Ok(Some(GitStatus::Unknown {
            reason: "timed out".to_owned(),
        })),
    );
    assert!(cx.debug_bounds("preflight-banner").is_none(), "unknown");
    report_git(&rig, cx, Err(Refusal::unavailable("the service went away")));
    assert!(cx.debug_bounds("preflight-banner").is_none(), "refused");
}

#[gpui::test]
fn dismissing_the_notice_hides_it_until_the_problem_changes(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, missing());
    let dismiss = cx
        .debug_bounds("banner-dismiss")
        .expect("the notice has a dismiss control");
    cx.simulate_click(dismiss.center(), Modifiers::default());
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("preflight-banner").is_none(), "dismissed");

    report_git(&rig, cx, missing());
    assert!(
        cx.debug_bounds("preflight-banner").is_none(),
        "the same problem stays dismissed"
    );

    report_git(&rig, cx, too_old());
    assert!(
        cx.debug_bounds("preflight-banner").is_some(),
        "a different problem shows again"
    );
}

#[gpui::test]
fn a_fixed_git_on_reconnect_clears_the_notice(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    report_git(&rig, cx, missing());
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
    report_git(&rig, cx, present());
    assert!(
        cx.debug_bounds("preflight-banner").is_none(),
        "gone once fixed on reconnect"
    );
}
