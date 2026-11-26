//! Element tests of the Remove project action (WS-17) on a scripted service: the confirm comes first, its
//! one call carries no waiver, a project that still has workspaces is refused until the waiver forgets
//! them, and the action is absent without `projects/1`. No window opens.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{Project, ProjectRemoveParams};
use tokio::sync::oneshot;

use super::service_tests::{Rig, click, draw, rig};
use crate::service::fixtures::{base_git_status, project};
use crate::service::{Command, GitUpdate, Refusal, StoreUpdate};

/// Offers project edits when `offered` is set, opens the Project sidebar on `w1` and draws it.
fn open_project(rig: &Rig, cx: &mut VisualTestContext, offered: bool) {
    rig.updates
        .send(StoreUpdate::ProjectEditsOffered(offered))
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

/// The next `project.remove` call. The calls the panel sent before it are skipped.
fn next_remove(
    rig: &mut Rig,
) -> (
    ProjectRemoveParams,
    oneshot::Sender<Result<Project, Refusal>>,
) {
    loop {
        match rig.script.next() {
            Some(Command::RemoveProject { params, reply }) => return (params, reply),
            Some(_) => continue,
            None => panic!("a project.remove call was expected"),
        }
    }
}

#[gpui::test]
fn removing_a_project_asks_first_and_sends_one_call_without_a_waiver(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_project(&rig, cx, true);

    click(cx, "button:project-remove");
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-action").is_some(),
        "the confirm is shown first"
    );
    click(cx, "dialog-action");
    let (params, reply) = next_remove(&mut rig);
    assert_eq!(
        (params.project.as_str(), params.forget_workspaces),
        ("p1", false)
    );
    let _ = reply.send(Ok(project("p1", "alpha")));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-action").is_none(),
        "the dialog closes once the project is removed"
    );
}

#[gpui::test]
fn a_project_with_workspaces_waits_for_the_forget_waiver(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_project(&rig, cx, true);
    click(cx, "button:project-remove");
    click(cx, "dialog-action");
    let (_, reply) = next_remove(&mut rig);
    let _ = reply.send(Err(Refusal::Failed {
        code: codes::PROJECT_HAS_WORKSPACES.into(),
        message: "The project still has workspaces.".into(),
    }));
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-action").is_some(),
        "the dialog waits for the waiver"
    );

    click(cx, "dialog-action");
    let (params, reply) = next_remove(&mut rig);
    assert!(params.forget_workspaces, "the waiver forgets the records");
    let _ = reply.send(Ok(project("p1", "alpha")));
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("dialog-action").is_none());
}

#[gpui::test]
fn the_remove_action_is_absent_without_the_projects_capability(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_project(&rig, cx, false);
    assert!(
        cx.debug_bounds("button:project-remove").is_none(),
        "Remove project is drawn without projects/1"
    );
}
