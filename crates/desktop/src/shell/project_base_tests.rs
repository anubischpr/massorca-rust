//! Element tests of the base-branch dialog (WS-04) on a scripted service: Save sends the branch in the
//! field once, a refused branch keeps the dialog open with its reason, and the button is absent without
//! `projects/1`. No window opens.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{Project, ProjectSetBaseParams};
use tokio::sync::oneshot;

use super::service_tests::{Rig, click, draw, rig};
use crate::service::fixtures::{base_git_status, project};
use crate::service::{Command, GitUpdate, Refusal, StoreUpdate};

/// `p1` with the base `main`.
fn based_project() -> Project {
    let mut based = project("p1", "alpha");
    based.base_ref = Some("main".into());
    based
}

/// Offers project edits when `offered` is set, gives `p1` its base, opens the Project sidebar on `w1`
/// and draws it.
fn open_project(rig: &Rig, cx: &mut VisualTestContext, offered: bool) {
    rig.updates
        .send(StoreUpdate::ProjectEditsOffered(offered))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::ProjectChanged(based_project()))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::Git(GitUpdate::Status {
            workspace: "w1".into(),
            result: Ok(base_git_status()),
        }))
        .expect("the store listens");
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    rig.shell
        .update(cx, |shell, cx| shell.open_source_control(cx));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("button:source-control-commit").is_some(),
        "the Project sidebar is drawn"
    );
}

/// The next `project.set_base` call. The calls the panel sent before it are skipped.
fn next_set_base(
    rig: &mut Rig,
) -> (
    ProjectSetBaseParams,
    oneshot::Sender<Result<Project, Refusal>>,
) {
    loop {
        match rig.script.next() {
            Some(Command::SetProjectBase { params, reply }) => return (params, reply),
            Some(_) => continue,
            None => panic!("a project.set_base call was expected"),
        }
    }
}

#[gpui::test]
fn saving_sends_the_branch_in_the_field_once(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_project(&rig, cx, true);

    click(cx, "button:project-base");
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-action").is_some(),
        "the base dialog is shown"
    );
    click(cx, "dialog-action");
    let (params, reply) = next_set_base(&mut rig);
    assert_eq!(
        (params.project.as_str(), params.base.as_deref()),
        ("p1", Some("main"))
    );
    let _ = reply.send(Ok(based_project()));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-action").is_none(),
        "the dialog closes once the base is set"
    );
}

#[gpui::test]
fn a_refused_branch_keeps_the_dialog_open_with_the_reason(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_project(&rig, cx, true);
    click(cx, "button:project-base");
    click(cx, "dialog-action");

    let (_, reply) = next_set_base(&mut rig);
    let _ = reply.send(Err(Refusal::Failed {
        code: codes::GIT_REVISION_NOT_FOUND.into(),
        message: "There is no branch named nope.".into(),
    }));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-action").is_some(),
        "the dialog stays open"
    );
    let error = rig.shell.update(cx, |shell, _| {
        shell
            .project_base
            .as_ref()
            .and_then(|base| base.flow.error().map(str::to_owned))
    });
    assert_eq!(error, Some("There is no branch named nope.".to_owned()));
}

#[gpui::test]
fn the_base_action_is_absent_without_the_projects_capability(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_project(&rig, cx, false);
    assert!(
        cx.debug_bounds("button:project-base").is_none(),
        "the base action is drawn without projects/1"
    );
}
