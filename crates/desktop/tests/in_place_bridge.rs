//! The project's own checkout registered in place, and the project's local branches, against a real service in a
//! temporary profile, driven headlessly: no window opens (desktop track A, "New worktree" off). A repeat in-place
//! create returns the same row, and the branch list reads the checkout with no workspace.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

const WAIT: Duration = Duration::from_secs(30);

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop_service(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

fn git(dir: &Path, args: &[&str]) {
    let output = massorca_process::command("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository with one commit.
fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Folds the bridge's updates into a store state until `done` holds.
async fn until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(WAIT, async {
        while !done(state) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}: {state:?}");
}

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
}

fn base_project_add_params() -> ProjectAddParams {
    ProjectAddParams {
        path: String::new(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    }
}

/// `workspace.create` with `in_place` set: no branch, base, path or name of its own, as the service requires.
fn in_place_params(project: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        in_place: true,
        project: project.to_owned(),
        name: String::new(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
        unique_name: false,
        name_generated: false,
    }
}

#[tokio::test]
async fn the_in_place_checkout_is_one_row_and_its_branches_need_no_workspace() {
    let home = temp_home();
    let repo_dir = home.path().join("demo");
    repo(&repo_dir);
    git(&repo_dir, &["branch", "release"]);
    // A remote-tracking ref is not a local branch, so the list leaves it out.
    git(
        &repo_dir,
        &["update-ref", "refs/remotes/origin/remote-only", "HEAD"],
    );
    let service = start_service(home.path()).await;

    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the service is connected and offers in-place workspaces and project branches",
        |s| {
            s.connection.is_connected()
                && s.loaded
                && s.workspace_in_place
                && s.git_project_branches
        },
    )
    .await;

    let project = handle
        .add_project(ProjectAddParams {
            path: repo_dir.display().to_string(),
            ..base_project_add_params()
        })
        .await
        .expect("the repository is added");

    // The checkout is registered once: a repeat call answers with the same row.
    let first = handle
        .create_workspace(in_place_params(&project.id))
        .await
        .expect("the checkout is registered in place");
    assert!(
        first.in_place,
        "the workspace is the project's own checkout"
    );
    let repeat = handle
        .create_workspace(in_place_params(&project.id))
        .await
        .expect("the repeat call answers");
    assert_eq!(repeat.id, first.id, "one in-place workspace per project");

    // The project's local branches come from its checkout, with no workspace named.
    let branches = handle
        .git_project_branches(project.id.clone())
        .await
        .expect("the project's branches are listed");
    assert!(
        branches.iter().any(|name| name == "release"),
        "the local branch is listed: {branches:?}"
    );
    assert!(
        branches.iter().all(|name| !name.contains("remote-only")),
        "remote-tracking branches are not listed: {branches:?}"
    );

    drop(bridge);
    stop_service(service).await;
}
