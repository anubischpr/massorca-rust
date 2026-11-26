//! Element tests of the Source control panel's Fetch, Pull and Push buttons (GIT-19) on a scripted
//! service: each button sends one call for the selected workspace, the panel waits for the answer, and
//! the note or the refusal shows under the panel. The buttons appear once the service offers git sync.
//! No window opens.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::git::remote::{GitFetched, GitPulled, PullMode, PushOutcome};
use massorca_protocol::methods::git::status::{BranchState, GitStatus};

use super::service_tests::{Rig, click, draw, rig};
use crate::model::source_control_sync::SyncTone;
use crate::service::fixtures::{base_branch_state, base_git_status, git_pushed};
use crate::service::{Command, GitUpdate, Refusal, StoreUpdate};

/// The status of `w1`: its branch is one commit ahead of its upstream.
fn ahead_status() -> GitStatus {
    GitStatus {
        branch: BranchState {
            head: Some("feat/scm".into()),
            oid: Some("a".repeat(40)),
            upstream: Some("origin/feat/scm".into()),
            ahead: Some(1),
            behind: Some(0),
            ..base_branch_state()
        },
        ..base_git_status()
    }
}

/// Selects workspace `w1`, opens its Source control tab and feeds its status. The service offers git
/// sync when `offered` is set.
fn open_panel(rig: &Rig, cx: &mut VisualTestContext, offered: bool) {
    rig.updates
        .send(StoreUpdate::GitSyncOffered(offered))
        .expect("the store listens");
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    rig.shell
        .update(cx, |shell, cx| shell.open_source_control(cx));
    rig.updates
        .send(StoreUpdate::Git(GitUpdate::Status {
            workspace: "w1".into(),
            result: Ok(ahead_status()),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    // The commit button is drawn only by a Ready panel, so the buttons below are looked for in a real one.
    assert!(
        cx.debug_bounds("button:source-control-commit").is_some(),
        "the Source control panel is drawn"
    );
}

fn busy(rig: &Rig, cx: &mut VisualTestContext) -> bool {
    rig.shell.update(cx, |shell, _| shell.source_control.busy)
}

fn notice(rig: &Rig, cx: &mut VisualTestContext) -> Option<(String, SyncTone)> {
    rig.shell.update(cx, |shell, _| {
        shell
            .source_control
            .notice
            .clone()
            .map(|note| (note.text, note.tone))
    })
}

fn error(rig: &Rig, cx: &mut VisualTestContext) -> Option<String> {
    rig.shell
        .update(cx, |shell, _| shell.source_control.error.clone())
}

#[gpui::test]
fn push_sends_one_git_push_call_and_its_answer_shows_as_a_note(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx, true);
    // The panel's watch on the selected workspace is the first call it sent.
    assert!(matches!(rig.script.next(), Some(Command::GitWatch { .. })));

    click(cx, "button:source-control-push");
    let Some(Command::GitPush { params, reply }) = rig.script.next() else {
        panic!("a git.push call was expected");
    };
    assert_eq!(params.workspace, "w1");
    assert_eq!((params.remote, params.force_with_lease), (None, None));
    assert!(busy(&rig, cx));

    // The buttons wait for the answer: a press while the call is out sends nothing.
    click(cx, "button:source-control-fetch");
    assert!(rig.script.next().is_none());

    let _ = reply.send(Ok(git_pushed(PushOutcome::FastForward)));
    cx.run_until_parked();
    draw(cx);
    assert!(!busy(&rig, cx));
    assert_eq!(
        notice(&rig, cx),
        Some(("Pushed feat/scm.".to_owned(), SyncTone::Info))
    );
}

#[gpui::test]
fn a_refused_push_shows_the_service_message_under_the_panel(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx, true);
    rig.script.next();

    click(cx, "button:source-control-push");
    let Some(Command::GitPush { reply, .. }) = rig.script.next() else {
        panic!("a git.push call was expected");
    };
    let _ = reply.send(Err(Refusal::Failed {
        code: codes::GIT_PUSH_REJECTED.into(),
        message: "The remote refused the push.".into(),
    }));
    cx.run_until_parked();
    draw(cx);
    assert!(!busy(&rig, cx));
    assert_eq!(
        error(&rig, cx),
        Some("The remote refused the push.".to_owned())
    );
    assert_eq!(notice(&rig, cx), None);
}

#[gpui::test]
fn pull_merges_and_a_divergence_is_a_warning_that_changes_nothing(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx, true);
    rig.script.next();

    click(cx, "button:source-control-pull");
    let Some(Command::GitPull { params, reply }) = rig.script.next() else {
        panic!("a git.pull call was expected");
    };
    assert_eq!(params.workspace, "w1");
    assert_eq!(params.mode, PullMode::Merge);
    let _ = reply.send(Ok(GitPulled::Diverged {
        ahead: 2,
        behind: 1,
    }));
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        notice(&rig, cx),
        Some((
            "Diverged: 2 ahead and 1 behind. Nothing changed.".to_owned(),
            SyncTone::Warning
        ))
    );
}

#[gpui::test]
fn fetch_sends_one_git_fetch_for_the_default_remote(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx, true);
    rig.script.next();

    click(cx, "button:source-control-fetch");
    let Some(Command::GitFetch { params, reply }) = rig.script.next() else {
        panic!("a git.fetch call was expected");
    };
    assert_eq!(params.workspace, "w1");
    assert_eq!((params.remote, params.all), (None, false));
    let _ = reply.send(Ok(GitFetched::default()));
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        notice(&rig, cx),
        Some(("Nothing new on the remote.".to_owned(), SyncTone::Info))
    );
}

#[gpui::test]
fn the_sync_buttons_stay_hidden_until_the_service_offers_git_sync(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx, false);
    for id in [
        "button:source-control-fetch",
        "button:source-control-pull",
        "button:source-control-push",
    ] {
        assert!(
            cx.debug_bounds(id).is_none(),
            "{id} is drawn without git sync"
        );
    }
    drop(rig);
}
