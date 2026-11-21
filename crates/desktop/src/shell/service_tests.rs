//! Element tests of the shell on a service store: the notice and status item, the create form, and
//! the remove flow with its waivers. A scripted handle stands in for the service, so each call is
//! answered by hand and no window opens.

use super::*;
use crate::data::SampleDataSource;
use crate::model::workspace_flow::{ProjectChoice, RemoveStage};
use crate::service::fixtures::{project, workspace};
use crate::service::{Command, ConnectionState, Refusal, Script, StoreUpdate};
use gpui::{AppContext, Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::methods::workspaces::WorkspaceRemoved;
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

struct Rig {
    shell: Entity<Shell>,
    updates: UnboundedSender<StoreUpdate>,
    script: Script,
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn connected() -> ConnectionState {
    ConnectionState::Connected {
        server: "massorca".into(),
        version: "0.1.0".into(),
    }
}

/// A shell on a service store with these projects and one workspace, connected.
fn rig<'a>(
    cx: &'a mut TestAppContext,
    projects: &[(&str, &str)],
) -> (Rig, &'a mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(store, Some(handle), None, window, cx)
    });
    updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    updates
        .send(StoreUpdate::Resync {
            projects: projects
                .iter()
                .map(|(id, name)| project(id, name))
                .collect(),
            workspaces: projects
                .first()
                .map(|(id, _)| workspace("w1", id, "fix-login", 1))
                .into_iter()
                .collect(),
        })
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (
        Rig {
            shell,
            updates,
            script,
        },
        cx,
    )
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

fn removed() -> WorkspaceRemoved {
    WorkspaceRemoved {
        workspace: workspace("w1", "p1", "fix-login", 1),
        branch_deleted: true,
        branch_kept_reason: None,
        directory_kept: false,
    }
}

fn next_remove(
    script: &mut Script,
) -> (
    massorca_protocol::methods::workspaces::WorkspaceRemoveParams,
    tokio::sync::oneshot::Sender<Result<WorkspaceRemoved, Refusal>>,
) {
    match script.next() {
        Some(Command::Remove { params, reply }) => (params, reply),
        _ => panic!("a workspace.remove call was expected"),
    }
}

#[gpui::test]
fn the_service_fills_the_sidebar_and_shows_its_status_item(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell.read_with(cx, |shell, _| {
        let projects = &shell.model().snapshot().projects;
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].hosts[0].workspaces[0].name, "fix-login");
        assert_eq!(shell.model().snapshot().service_notice, None);
    });
    assert!(cx.debug_bounds("status-service").is_some());
    assert!(
        cx.debug_bounds("service-banner").is_none(),
        "connected shows no banner"
    );
}

#[gpui::test]
fn a_service_that_cannot_be_reached_shows_the_banner_and_retry_now_reaches_the_bridge(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::Connection(ConnectionState::Unreachable {
            reason: "The service is not running.".into(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("service-banner").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let notice = shell.model().snapshot().service_notice.clone();
        assert!(notice.is_some_and(|notice| notice.retry));
    });
    rig.shell.update(cx, |shell, _| shell.retry_service_now());
    assert!(matches!(rig.script.next(), Some(Command::RetryNow)));
}

#[gpui::test]
fn the_banner_goes_away_when_the_link_is_back(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::Connection(ConnectionState::Reconnecting {
            attempt: 2,
            retry_at: std::time::Instant::now(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("service-banner").is_some());
    rig.updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("service-banner").is_none());
}

#[gpui::test]
fn the_create_dialog_needs_a_name_then_sends_the_call_and_closes_on_the_answer(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_create(window, cx));
    draw(cx);
    assert!(cx.debug_bounds("dialog-panel").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell.dialogs.create.as_ref().expect("open").form;
        assert_eq!(form.project, Some(ProjectChoice::Project("p1".into())));
        assert!(!form.can_submit(), "no name yet");
    });

    cx.simulate_input("new-work");
    click(cx, "dialog-action");
    let Some(Command::Create { params, reply }) = rig.script.next() else {
        panic!("a workspace.create call was expected");
    };
    assert_eq!(
        (params.project.as_str(), params.name.as_str()),
        ("p1", "new-work")
    );
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.create.as_ref().expect("open").form.busy);
    });

    // A refusal keeps the dialog, free again, with the reason under the name.
    reply
        .send(Err(Refusal::NameProblem {
            message: "That name is taken.".into(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell.dialogs.create.as_ref().expect("still open").form;
        assert!(!form.busy);
        assert_eq!(form.name_error.as_deref(), Some("That name is taken."));
    });

    // Retrying with a new name sends again, and success closes the dialog.
    cx.simulate_input("-2");
    click(cx, "dialog-action");
    let Some(Command::Create { reply, .. }) = rig.script.next() else {
        panic!("a second workspace.create call was expected");
    };
    reply
        .send(Ok(workspace("w2", "p1", "new-work-2", 2)))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.dialogs.create.is_none()));
}

#[gpui::test]
fn the_project_menu_ends_with_no_project_which_cannot_create(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha"), ("p2", "beta")]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_create(window, cx));
    cx.simulate_input("w");
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_project_menu(window, cx);
    });
    draw(cx);
    assert!(cx.debug_bounds("menu-panel").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let menu = shell.dialogs.menu.as_ref().expect("the menu is open");
        let workspace_dialogs::MenuTarget::Projects(choices) = &menu.target else {
            panic!("a project menu was expected");
        };
        assert_eq!(
            choices.len(),
            5,
            "two projects, No project, a separator, Add a project"
        );
        assert_eq!(
            choices.last(),
            Some(&Some(workspace_dialogs::ProjectMenuChoice::AddProject))
        );
    });

    // Add a project is the last row, so one row up is "No project": nothing can be created from it.
    cx.simulate_keystrokes("end up enter");
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell.dialogs.create.as_ref().expect("open").form;
        assert_eq!(form.project, Some(ProjectChoice::NoProject));
        assert!(!form.can_submit());
        assert!(form.no_project_note().is_some());
        assert!(shell.dialogs.menu.is_none(), "the menu closed");
    });
}

#[gpui::test]
fn an_empty_project_menu_says_so_and_still_offers_add_a_project(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_create(window, cx));
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_project_menu(window, cx);
    });
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        let menu = shell.dialogs.menu.as_ref().expect("the menu is open");
        let workspace_dialogs::MenuTarget::Projects(rows) = &menu.target else {
            panic!("a project menu was expected");
        };
        // The caption, No project, a separator, then Add a project.
        assert_eq!(rows.len(), 4);
        assert_eq!(rows.first(), Some(&None), "the caption is not a choice");
        assert_eq!(
            rows.last(),
            Some(&Some(workspace_dialogs::ProjectMenuChoice::AddProject))
        );
    });
}

#[gpui::test]
fn add_a_project_is_refused_under_its_path_then_added_and_chosen_in_the_create_form(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_create(window, cx));
    cx.simulate_input("w");
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_project_menu(window, cx);
    });
    draw(cx);
    // Add a project is the last row, so End and Enter open its dialog.
    cx.simulate_keystrokes("end enter");
    draw(cx);
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.dialogs.add_project.is_some()));
    cx.simulate_input("/missing");
    cx.simulate_keystrokes("enter");
    let Some(Command::AddProject { params, reply }) = rig.script.next() else {
        panic!("a project.add call was expected");
    };
    assert_eq!(params.path, "/missing");
    assert_eq!(params.name, None);
    reply
        .send(Err(Refusal::PathProblem {
            message: "That folder does not exist.".into(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell.dialogs.add_project.as_ref().expect("still open");
        assert!(!dialog.form.busy);
        assert_eq!(
            dialog.form.path_error.as_deref(),
            Some("That folder does not exist.")
        );
    });

    // Typing clears the refusal; the next answer adds the project and chooses it.
    cx.simulate_input("2");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell.dialogs.add_project.as_ref().expect("still open");
        assert_eq!(dialog.form.path_error, None);
    });
    cx.simulate_keystrokes("enter");
    let Some(Command::AddProject { reply, .. }) = rig.script.next() else {
        panic!("a second project.add call was expected");
    };
    reply
        .send(Ok(project("p9", "gamma")))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.add_project.is_none());
        let form = &shell
            .dialogs
            .create
            .as_ref()
            .expect("the create form stays open")
            .form;
        assert_eq!(form.project, Some(ProjectChoice::Project("p9".into())));
    });
}

#[gpui::test]
fn removing_a_workspace_walks_dirty_then_ignored_files_each_with_one_more_flag(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update(cx, |shell, cx| shell.open_remove("w1", cx));
    click(cx, "dialog-action");
    let (params, reply) = next_remove(&mut rig.script);
    assert!(!params.force && !params.discard_ignored && !params.forget_orphan);
    reply.send(Err(Refusal::Dirty)).expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let flow = shell.dialogs.remove.as_ref().expect("open");
        assert_eq!(flow.stage(), &RemoveStage::Dirty);
        assert!(!flow.busy());
    });

    click(cx, "dialog-action");
    let (params, reply) = next_remove(&mut rig.script);
    assert!(params.force && !params.discard_ignored);
    reply
        .send(Err(Refusal::IgnoredFiles))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let stage = shell.dialogs.remove.as_ref().expect("open").stage().clone();
        assert_eq!(stage, RemoveStage::IgnoredFiles);
    });

    click(cx, "dialog-action");
    let (params, reply) = next_remove(&mut rig.script);
    assert!(params.force && params.discard_ignored && !params.forget_orphan);
    reply.send(Ok(removed())).expect("the shell waits");
    cx.run_until_parked();
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.dialogs.remove.is_none()));
    // The row stays until the service's event arrives; the answer alone does not drop it.
    rig.shell.read_with(cx, |shell, cx| {
        assert!(shell.store.read(cx).state().workspace("w1").is_some());
    });
}

#[gpui::test]
fn an_orphan_is_forgotten_with_its_own_flag_and_a_locked_one_only_closes(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update(cx, |shell, cx| shell.open_remove("w1", cx));
    click(cx, "dialog-action");
    let (_, reply) = next_remove(&mut rig.script);
    reply.send(Err(Refusal::Orphan)).expect("the shell waits");
    cx.run_until_parked();
    click(cx, "dialog-action");
    let (params, reply) = next_remove(&mut rig.script);
    assert!(params.forget_orphan && !params.force && !params.discard_ignored);
    reply
        .send(Err(Refusal::Locked {
            message: "Git has this worktree locked.".into(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let stage = shell.dialogs.remove.as_ref().expect("open").stage().clone();
        assert!(matches!(stage, RemoveStage::Refused { .. }));
    });
    click(cx, "dialog-action");
    assert!(rig.script.next().is_none(), "a plain refusal sends nothing");
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.dialogs.remove.is_none()));
}

#[gpui::test]
fn a_removed_event_drops_the_row_from_the_sidebar(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::WorkspaceGone("w1".into()))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let workspaces = &shell.model().snapshot().projects[0].hosts[0].workspaces;
        assert!(workspaces.is_empty());
    });
}

#[gpui::test]
fn sample_data_has_no_service_so_create_and_remove_do_nothing(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(&SampleDataSource, window, cx));
    shell.update_in(cx, |shell, window, cx| shell.open_create(window, cx));
    shell.update(cx, |shell, cx| shell.open_remove("w1", cx));
    shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.create.is_none() && shell.dialogs.remove.is_none());
        assert!(shell.service.is_none());
        assert!(shell.model().snapshot().service_status.is_none());
    });
}
