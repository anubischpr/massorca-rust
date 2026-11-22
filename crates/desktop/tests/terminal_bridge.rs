//! Terminal calls and attach streams through the bridge, against a real service in a temporary
//! profile. The attached feed answers the program's queries, as an attached client must (0034).
//! No window opens: the feed is the emulator the desktop surface uses, without GPUI.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use massorca_client::{AttachUpdate, Client, ConnectOptions};
use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, StreamEnd, TerminalStream,
    TerminalStreamItem, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::terminals::{
    AttachMode, ExitReason, ShellSpec, TerminalAttachParams, TerminalCreateParams, TerminalState,
};
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;
use massorca_server::{RunningService, Service, ServiceConfig};
use massorca_ui::components::terminal_surface::feed::{FeedEvent, TerminalFeed};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

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
    let output = Command::new("git")
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

fn attach_params(terminal: &str, since: Option<u64>) -> TerminalAttachParams {
    TerminalAttachParams {
        terminal: terminal.to_owned(),
        since,
        cols: None,
        rows: None,
    }
}

/// Feeds one attach update to the emulator, the way the desktop surface does.
fn apply(feed: &mut TerminalFeed, update: &AttachUpdate) {
    let outcome = match update {
        AttachUpdate::Attached { offset, .. } => {
            feed.apply(FeedEvent::Attached { offset: *offset })
        }
        AttachUpdate::Snapshot {
            offset,
            cols,
            rows,
            bytes,
        } => {
            feed.apply(FeedEvent::Snapshot {
                offset: *offset,
                cols: *cols,
                rows: *rows,
                len: bytes.len() as u64,
            })
            .expect("a snapshot starts");
            feed.apply(FeedEvent::SnapshotBytes(bytes))
        }
        AttachUpdate::Output { offset, bytes } => feed.apply(FeedEvent::Output {
            offset: *offset,
            bytes,
        }),
        AttachUpdate::Resized { cols, rows } => feed.apply(FeedEvent::Resized {
            cols: *cols,
            rows: *rows,
        }),
        AttachUpdate::Exited { terminal } => feed.apply(FeedEvent::Exited {
            code: terminal
                .exit
                .as_ref()
                .and_then(|exit| exit.code)
                .and_then(|code| i32::try_from(code).ok()),
        }),
    };
    outcome.expect("the feed accepts the item");
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
    let home = tempfile::tempdir().expect("a temporary home");
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
