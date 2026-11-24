//! Terminal calls and attach streams through the bridge, against a real service in a temporary
//! profile. The attached feed answers the program's queries and declares it (0034, 0036).
//! No window opens: the feed is the emulator the desktop surface uses, without GPUI.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{AttachUpdate, Client, ConnectOptions};
use massorca_desktop::model::terminals::{TerminalStatus, may_run, status_of};
use massorca_desktop::model::work_panel::{
    CloseStep, TerminalSelection, close_step, detached_text, status_banner, terminal_strip,
};
use massorca_desktop::service::{
    AttachRole, ServiceBridge, ServiceHandle, StoreState, StoreUpdate, StreamEnd, TerminalStream,
    TerminalStreamItem, connect_options, feed_events,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::terminals::{
    AttachMode, ExitReason, ShellSpec, TerminalAttachParams, TerminalCreateParams, TerminalState,
};
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;
use massorca_server::{RunningService, Service, ServiceConfig};
use massorca_ui::components::terminal_surface::feed::TerminalFeed;
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

fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Registers the repository as a project through a client of its own, as the CLI would.
async fn add_project(home: &Path, repo_dir: &Path) -> String {
    let client = Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("a client connects");
    client
        .project_add(&ProjectAddParams {
            icon: None,
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            kind: None,
            workspaces_root: Some(
                repo_dir
                    .parent()
                    .expect("a parent")
                    .join("demo-worktrees")
                    .display()
                    .to_string(),
            ),
        })
        .await
        .expect("the project is added")
        .id
}

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
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

/// The shell every platform CI has: cmd.exe on Windows, POSIX sh elsewhere (0032 names the
/// default; the test asks for a fixed one so the output is predictable).
fn shell() -> ShellSpec {
    if cfg!(windows) {
        ShellSpec {
            program: "cmd.exe".to_owned(),
            args: vec!["/Q".to_owned()],
        }
    } else {
        ShellSpec {
            program: "/bin/sh".to_owned(),
            args: Vec::new(),
        }
    }
}

fn workspace_params(project: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: "terminal".to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
    }
}

fn create_params(workspace: &str) -> TerminalCreateParams {
    TerminalCreateParams {
        workspace: workspace.to_owned(),
        shell: Some(shell()),
        cwd: None,
        env: Default::default(),
        cols: Some(80),
        rows: Some(24),
        title: None,
        request_id: None,
    }
}

/// Attaches as an emulator: the feed answers the queries its output carries.
fn attach_params(terminal: &str, since: Option<u64>) -> TerminalAttachParams {
    AttachRole::AnswersQueries.attach_params(terminal, since)
}

/// Feeds one attach update to the emulator, the way the desktop surface does.
fn apply(feed: &mut TerminalFeed, update: &AttachUpdate) {
    for event in feed_events(update) {
        feed.apply(event).expect("the feed accepts the item");
    }
}

/// Reads the attach until `done` holds on the feed, answering the program's queries as it goes.
/// Returns the attach updates seen, so a test can check the order.
async fn read_until(
    stream: &mut TerminalStream,
    feed: &mut TerminalFeed,
    handle: &ServiceHandle,
    terminal: &str,
    what: &str,
    done: impl Fn(&[AttachUpdate], &TerminalFeed) -> bool,
) -> Vec<AttachUpdate> {
    let mut seen = Vec::new();
    let read = timeout(WAIT, async {
        while !done(&seen, feed) {
            match stream.next().await {
                Some(TerminalStreamItem::Update(update)) => {
                    apply(feed, &update);
                    seen.push(update);
                    let replies = feed.take_replies();
                    if !replies.is_empty() {
                        handle.send_terminal_input(terminal, replies);
                    }
                }
                other => panic!("the stream ended while waiting for {what}: {other:?}"),
            }
        }
    })
    .await;
    assert!(
        read.is_ok(),
        "no {what} within {WAIT:?}; the screen was {:?}",
        (0..feed.lines())
            .map(|row| feed.row_text(row))
            .collect::<Vec<_>>()
    );
    seen
}

fn has_row(feed: &TerminalFeed, text: &str) -> bool {
    (0..feed.lines()).any(|row| feed.row_text(row) == text)
}

#[tokio::test]
async fn a_terminal_is_created_attached_typed_into_resized_detached_and_killed() {
    let home = temp_home();
    let repo_dir = home.path().join("demo");
    repo(&repo_dir);
    let service = start_service(home.path()).await;
    let project_id = add_project(home.path(), &repo_dir).await;

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first list", |s| {
        s.connection.is_connected() && s.loaded
    })
    .await;
    let workspace = handle
        .create_workspace(workspace_params(&project_id))
        .await
        .expect("the workspace is created");

    let terminal = handle
        .create_terminal(create_params(&workspace.id))
        .await
        .expect("the terminal is created");
    assert_eq!(terminal.state, TerminalState::Live);
    assert_eq!(terminal.workspace_id, workspace.id);
    let id = terminal.id.clone();
    until(&mut updates, &mut state, "the terminal in the store", |s| {
        s.terminals.iter().any(|t| t.id == id)
    })
    .await;

    // First attach: a snapshot, then output.
    let mut stream = handle
        .attach_terminal(attach_params(&id, None))
        .await
        .expect("the terminal attaches");
    let mut feed = TerminalFeed::new(80, 24);
    let seen = read_until(
        &mut stream,
        &mut feed,
        &handle,
        &id,
        "the snapshot",
        |seen, _| {
            seen.iter()
                .any(|u| matches!(u, AttachUpdate::Snapshot { .. }))
        },
    )
    .await;
    assert!(
        matches!(
            seen.first(),
            Some(AttachUpdate::Attached {
                mode: AttachMode::Snapshot,
                ..
            })
        ),
        "a first attach starts with a snapshot: {seen:?}"
    );

    // Input reaches the shell, and its output reaches the feed's grid.
    handle.send_terminal_input(&id, b"echo hello\r".to_vec());
    read_until(
        &mut stream,
        &mut feed,
        &handle,
        &id,
        "hello on screen",
        |_, f| has_row(f, "hello"),
    )
    .await;

    // Resize: the service answers with the new size and the attach hears it.
    let resized = handle
        .resize_terminal(&id, 100, 30)
        .await
        .expect("the terminal resizes");
    assert_eq!((resized.cols, resized.rows), (100, 30));
    read_until(
        &mut stream,
        &mut feed,
        &handle,
        &id,
        "the resize",
        |_, f| f.columns() == 100 && f.lines() == 30,
    )
    .await;

    // Detach by dropping the stream; the terminal keeps running and its output keeps coming.
    let since = feed.next_offset().expect("the feed has read output");
    drop(stream);

    // Reattach from the offset: resume, and output keeps flowing.
    let mut stream = handle
        .attach_terminal(attach_params(&id, Some(since)))
        .await
        .expect("the terminal reattaches");
    let seen = read_until(
        &mut stream,
        &mut feed,
        &handle,
        &id,
        "the resumed attach",
        |seen, _| {
            seen.iter()
                .any(|u| matches!(u, AttachUpdate::Attached { .. }))
        },
    )
    .await;
    assert!(
        matches!(
            seen.first(),
            Some(AttachUpdate::Attached {
                mode: AttachMode::Resume,
                ..
            })
        ),
        "a reattach from an offset in the ring resumes: {seen:?}"
    );
    handle.send_terminal_input(&id, b"echo again\r".to_vec());
    read_until(
        &mut stream,
        &mut feed,
        &handle,
        &id,
        "output after reattach",
        |_, f| has_row(f, "again"),
    )
    .await;

    // Detach and attach again without an offset: a snapshot redraws the screen, and output keeps
    // flowing after it.
    drop(stream);
    let mut stream = handle
        .attach_terminal(attach_params(&id, None))
        .await
        .expect("the terminal reattaches from a snapshot");
    let mut fresh = TerminalFeed::new(100, 30);
    let seen = read_until(
        &mut stream,
        &mut fresh,
        &handle,
        &id,
        "the snapshot",
        |seen, _| {
            seen.iter()
                .any(|u| matches!(u, AttachUpdate::Snapshot { .. }))
        },
    )
    .await;
    assert!(
        matches!(
            seen.first(),
            Some(AttachUpdate::Attached {
                mode: AttachMode::Snapshot,
                ..
            })
        ),
        "an attach without an offset starts with a snapshot: {seen:?}"
    );
    assert!(
        has_row(&fresh, "again"),
        "the snapshot redraws the screen it was taken from"
    );
    handle.send_terminal_input(&id, b"echo third\r".to_vec());
    read_until(
        &mut stream,
        &mut fresh,
        &handle,
        &id,
        "output after a snapshot reattach",
        |_, f| has_row(f, "third"),
    )
    .await;

    // Kill: the terminal is exited with its reason, and the attach ends after Exited.
    let killed = handle
        .kill_terminal(&id, true)
        .await
        .expect("the terminal is killed");
    assert_eq!(killed.state, TerminalState::Exited);
    assert_eq!(
        killed.exit.as_ref().map(|exit| exit.reason),
        Some(ExitReason::Killed)
    );
    read_until(&mut stream, &mut fresh, &handle, &id, "the exit", |_, f| {
        f.exit().is_some()
    })
    .await;
    let end = timeout(WAIT, stream.next())
        .await
        .expect("the stream ends in time");
    assert_eq!(end, Some(TerminalStreamItem::Ended(StreamEnd::Finished)));

    stop_service(service).await;
}

#[tokio::test]
async fn the_work_panel_strip_follows_the_store_and_closing_asks_by_status() {
    let home = temp_home();
    let repo_dir = home.path().join("demo");
    repo(&repo_dir);
    let service = start_service(home.path()).await;
    let project_id = add_project(home.path(), &repo_dir).await;

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first list", |s| {
        s.connection.is_connected() && s.loaded
    })
    .await;
    let workspace = handle
        .create_workspace(workspace_params(&project_id))
        .await
        .expect("the workspace is created");
    let terminal = handle
        .create_terminal(create_params(&workspace.id))
        .await
        .expect("the terminal is created");
    let id = terminal.id.clone();
    until(&mut updates, &mut state, "the terminal in the store", |s| {
        s.terminals.iter().any(|t| t.id == id)
    })
    .await;

    // A running terminal is the strip's selected tab, and closing it asks first.
    let mut selection = TerminalSelection::default();
    let strip = terminal_strip(&state, &workspace.id, &selection);
    let listed: Vec<&str> = strip.tabs.iter().map(|tab| tab.id.as_str()).collect();
    assert_eq!(listed, [id.as_str()]);
    assert_eq!(strip.selected, Some(0));
    assert_eq!(
        close_step(strip.tabs[0].status),
        CloseStep::AskWhetherToStop
    );
    assert_eq!(status_banner(strip.tabs[0].status), None);

    // Killed, the tab says it has ended, and closing it needs no question.
    handle
        .kill_terminal(&id, true)
        .await
        .expect("the terminal is killed");
    until(&mut updates, &mut state, "the kill in the store", |s| {
        s.terminals
            .iter()
            .any(|t| t.id == id && !may_run(status_of(t)))
    })
    .await;
    let strip = terminal_strip(&state, &workspace.id, &selection);
    assert!(matches!(
        strip.tabs[0].status,
        TerminalStatus::Exited {
            reason: ExitReason::Killed,
            ..
        }
    ));
    assert_eq!(close_step(strip.tabs[0].status), CloseStep::Forget);
    assert_eq!(
        detached_text(strip.tabs[0].status),
        "This terminal has exited."
    );

    // Closing forgets the terminal, and the selection of it goes with it.
    selection.select(&workspace.id, &id);
    handle
        .close_terminal(&id)
        .await
        .expect("the terminal is closed");
    until(&mut updates, &mut state, "the terminal is gone", |s| {
        s.terminals.iter().all(|t| t.id != id)
    })
    .await;
    selection.forget(&id);
    let strip = terminal_strip(&state, &workspace.id, &selection);
    assert!(strip.tabs.is_empty());
    assert_eq!(strip.selected, None);

    stop_service(service).await;
}
