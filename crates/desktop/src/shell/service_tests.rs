//! Element tests of the shell on a service store: the notice and status item, the add-project
//! dialog, and the remove flow with its waivers. A scripted handle stands in for the service, so
//! each call is answered by hand and no window opens.

use super::*;
use crate::data::SampleDataSource;
use crate::model::marketplace::{ItemKind, ServiceMarketplace, installed_agent_links};
use crate::model::teams::{SampleTeamsSource, TeamsSource, UnavailableTeams};
use crate::model::workspace_flow::RemoveStage;
use crate::service::fixtures::{agent_kind, project, workspace};
use crate::service::{Command, ConnectionState, Refusal, Script, StoreUpdate};
use gpui::{AppContext, Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::PackageState;
use massorca_protocol::methods::projects::{ProjectInspectParams, ProjectInspection, ProjectKind};
use massorca_protocol::methods::workspaces::WorkspaceRemoved;
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

pub(super) struct Rig {
    pub(super) shell: Entity<Shell>,
    pub(super) updates: UnboundedSender<StoreUpdate>,
    pub(super) script: Script,
}

pub(super) fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

pub(super) fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

pub(super) fn connected() -> ConnectionState {
    ConnectionState::Connected {
        server: "massorca".into(),
        version: "0.1.0".into(),
    }
}

/// A shell on a service store with these projects and one workspace, connected.
pub(super) fn rig<'a>(
    cx: &'a mut TestAppContext,
    projects: &[(&str, &str)],
) -> (Rig, &'a mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    script.set_connection(connected());
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(
            store,
            Some(handle),
            None,
            UnavailableTeams.teams(),
            Box::new(ServiceMarketplace::default()),
            window,
            cx,
        )
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

pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
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
        removed_children: Vec::new(),
        archive: None,
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
fn a_live_status_bar_has_no_memory_or_ports_item_because_no_service_reports_them(
    cx: &mut TestAppContext,
) {
    let (_rig, cx) = rig(cx, &[("p1", "alpha")]);
    assert!(
        cx.debug_bounds("status-memory").is_none(),
        "no source for memory"
    );
    assert!(
        cx.debug_bounds("status-ports").is_none(),
        "no source for ports"
    );
    assert!(cx.debug_bounds("status-terminals").is_some());
    assert!(cx.debug_bounds("status-hosts").is_some());
}

#[gpui::test]
fn the_service_fills_the_sidebar_and_the_hosts_item_counts_its_hosts(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell.read_with(cx, |shell, _| {
        let projects = &shell.model().snapshot().projects;
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].hosts[0].workspaces[0].name, "fix-login");
        assert_eq!(shell.model().snapshot().service_notice, None);
    });
    assert!(cx.debug_bounds("status-hosts").is_some());
    assert!(cx.debug_bounds("status-service").is_none());
    assert!(
        cx.debug_bounds("service-banner").is_none(),
        "connected shows no banner"
    );
}

#[gpui::test]
fn a_service_that_cannot_be_reached_takes_the_canvas_and_retry_now_reaches_the_bridge(
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
    assert!(
        cx.debug_bounds("service-banner").is_none(),
        "the page replaces the banner"
    );
    assert!(cx.debug_bounds("service-down").is_some());
    rig.shell.read_with(cx, |shell, _| {
        let notice = shell.model().snapshot().service_notice.clone();
        assert!(notice.is_some_and(|notice| notice.retry));
    });
    rig.shell.update(cx, |shell, _| shell.retry_service_now());
    assert!(matches!(rig.script.next(), Some(Command::RetryNow)));
}

#[gpui::test]
fn the_kinds_the_service_lists_are_its_marketplace_agents_and_settings_agents(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Disabled),
            agent_kind("cursor", "Cursor", PackageState::Available),
        ]))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let marketplace = shell.model().marketplace();
        let agents: Vec<&str> = marketplace
            .of_kind(ItemKind::Agent)
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(agents, ["Claude", "Codex", "Cursor"]);
        let installed: Vec<String> = installed_agent_links(marketplace)
            .into_iter()
            .map(|link| link.name)
            .collect();
        assert_eq!(
            installed,
            ["Claude", "Codex"],
            "Settings lists installed kinds only"
        );
    });
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
fn add_a_project_is_refused_under_its_path_then_added_and_chosen_on_the_start_page(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    draw(cx);
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.dialogs.add_project.is_some()));
    cx.simulate_input("/missing");
    cx.simulate_keystrokes("enter");
    // The dialog asks the service what the path is first. It refuses a missing folder there too, and the
    // add gets the same refusal, so the refusal shows under the field.
    let (inspect, inspect_reply) = next_inspect(&mut rig.script);
    assert_eq!(inspect.path, "/missing");
    inspect_reply
        .send(Err(Refusal::PathProblem {
            message: "That folder does not exist.".into(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
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
    let (_, inspect_reply) = next_inspect(&mut rig.script);
    inspect_reply
        .send(Ok(inspection(ProjectKind::Git, "/missing2", None)))
        .expect("the shell waits");
    cx.run_until_parked();
    let Some(Command::AddProject { reply, .. }) = rig.script.next() else {
        panic!("a second project.add call was expected");
    };
    reply
        .send(Ok(project("p9", "gamma")))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.add_project.is_none());
        assert_eq!(
            shell.home_project,
            Some(ProjectChoice::Project("p9".to_owned()))
        );
    });
}

/// The `project.inspect` a submitted path sends first, for the test to answer.
fn next_inspect(
    script: &mut Script,
) -> (
    ProjectInspectParams,
    tokio::sync::oneshot::Sender<Result<ProjectInspection, Refusal>>,
) {
    match script.next() {
        Some(Command::InspectProject { params, reply }) => (params, reply),
        _ => panic!("a project.inspect call was expected"),
    }
}

pub(super) fn inspection(
    kind: ProjectKind,
    path: &str,
    registered: Option<&str>,
) -> ProjectInspection {
    ProjectInspection {
        path: path.to_owned(),
        kind,
        registered: registered.map(str::to_owned),
    }
}

/// Opens "Add a project" and sends `path` with Enter; the dialog then asks the service what the path is.
fn send_path(rig: &Rig, cx: &mut VisualTestContext, path: &str) {
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    draw(cx);
    cx.simulate_input(path);
    cx.simulate_keystrokes("enter");
}

#[gpui::test]
fn a_git_folder_is_added_without_a_warning(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    send_path(&rig, cx, "/repos/gamma");
    let (inspect, reply) = next_inspect(&mut rig.script);
    assert_eq!(inspect.path, "/repos/gamma");
    reply
        .send(Ok(inspection(ProjectKind::Git, "/repos/gamma", None)))
        .expect("the shell waits");
    cx.run_until_parked();
    let Some(Command::AddProject { params, reply }) = rig.script.next() else {
        panic!("a project.add call was expected");
    };
    assert_eq!(
        params.kind, None,
        "the service picks the kind of a repository"
    );
    reply
        .send(Ok(project("p9", "gamma")))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.add_project.is_none());
        assert_eq!(
            shell.home_project,
            Some(ProjectChoice::Project("p9".to_owned()))
        );
    });
}

#[gpui::test]
fn a_plain_folder_is_held_for_the_warning_and_nothing_is_added_yet(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    send_path(&rig, cx, "/repos/notes");
    let (_, reply) = next_inspect(&mut rig.script);
    reply
        .send(Ok(inspection(ProjectKind::Folder, "/repos/notes", None)))
        .expect("the shell waits");
    cx.run_until_parked();
    draw(cx);
    assert!(
        rig.script.next().is_none(),
        "no project.add before the person decides"
    );
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog stays open");
        assert_eq!(dialog.form.plain_folder.as_deref(), Some("/repos/notes"));
        assert!(!dialog.form.busy);
    });
    assert!(
        cx.debug_bounds("dialog-action").is_some(),
        "the warning offers Continue without Git"
    );
    assert!(cx.debug_bounds("dialog-cancel").is_some(), "and Cancel");
}

#[gpui::test]
fn continue_without_git_adds_the_folder_as_a_plain_folder(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    send_path(&rig, cx, "/repos/notes");
    let (_, reply) = next_inspect(&mut rig.script);
    reply
        .send(Ok(inspection(ProjectKind::Folder, "/repos/notes", None)))
        .expect("the shell waits");
    cx.run_until_parked();
    click(cx, "dialog-action");
    let Some(Command::AddProject { params, reply }) = rig.script.next() else {
        panic!("a project.add call was expected");
    };
    assert_eq!(params.path, "/repos/notes");
    assert_eq!(
        params.kind,
        Some(ProjectKind::Folder),
        "the person chose to add it without Git"
    );
    let mut added = project("p9", "notes");
    added.kind = ProjectKind::Folder;
    reply.send(Ok(added)).expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.add_project.is_none());
        assert_eq!(
            shell.home_project,
            Some(ProjectChoice::Project("p9".to_owned()))
        );
    });
}

#[gpui::test]
fn cancel_at_the_warning_adds_nothing_and_returns_to_the_path(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    send_path(&rig, cx, "/repos/notes");
    let (_, reply) = next_inspect(&mut rig.script);
    reply
        .send(Ok(inspection(ProjectKind::Folder, "/repos/notes", None)))
        .expect("the shell waits");
    cx.run_until_parked();
    click(cx, "dialog-cancel");
    assert!(rig.script.next().is_none(), "nothing is added");
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is back on the path");
        assert_eq!(dialog.form.plain_folder, None);
        assert_eq!(dialog.form.path, "/repos/notes");
    });
}

#[gpui::test]
fn a_folder_already_added_is_not_warned_about_and_the_add_refuses_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    send_path(&rig, cx, "/repos/alpha");
    let (_, reply) = next_inspect(&mut rig.script);
    reply
        .send(Ok(inspection(
            ProjectKind::Folder,
            "/repos/alpha",
            Some("p1"),
        )))
        .expect("the shell waits");
    cx.run_until_parked();
    let Some(Command::AddProject { params, reply }) = rig.script.next() else {
        panic!("a project.add call was expected");
    };
    assert_eq!(params.kind, None);
    reply
        .send(Err(Refusal::PathProblem {
            message: "That folder is already a project.".into(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell.dialogs.add_project.as_ref().expect("still open");
        assert_eq!(dialog.form.plain_folder, None);
        assert_eq!(
            dialog.form.path_error.as_deref(),
            Some("That folder is already a project.")
        );
    });
}

#[gpui::test]
fn a_plain_folder_project_shows_the_no_git_marker_in_the_sidebar(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-no-git-p1").is_none(),
        "a repository has no marker"
    );
    let mut notes = project("p2", "notes");
    notes.kind = ProjectKind::Folder;
    rig.updates
        .send(StoreUpdate::Resync {
            projects: vec![project("p1", "alpha"), notes],
            workspaces: Vec::new(),
        })
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-no-git-p2").is_some(),
        "the plain folder shows No Git"
    );
    assert!(cx.debug_bounds("sidebar-no-git-p1").is_none());
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
fn sample_data_has_no_service_so_remove_does_nothing(cx: &mut TestAppContext) {
    setup(cx);
    let (shell, cx) = cx.add_window_view(|window, cx| {
        Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
    });
    shell.update(cx, |shell, cx| shell.open_remove("w1", cx));
    shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.remove.is_none());
        assert!(shell.service.is_none());
        assert!(shell.model().snapshot().service_status.is_none());
    });
}
