//! Terminals over the wire: an in-process service on a temporary home, driven through the client crate. Terminals outlive
//! their clients, resume from offsets, and end with the service.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_client::{AttachUpdate, Client, ClientError, ConnectOptions, TerminalAttachment};
use massorca_profile::{Profile, local_socket_name};
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::terminals::{
    AttachMode, MAX_INPUT_BYTES, Terminal, TerminalAttachParams, TerminalCreateParams,
    TerminalResizeParams, TerminalState,
};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_protocol::{
    MAX_FRAME_LEN_LOCAL, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION, PeerInfo, ProtocolRange,
    capabilities, codes, read_message, write_message,
};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::time::timeout;

const SERVER_NAME: &str = "massorca-test";
const SERVER_VERSION: &str = "0.0.0-test";

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

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: SERVER_NAME.to_owned(),
        server_version: SERVER_VERSION.to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(Duration::from_secs(15), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

async fn connect(home: &Path) -> Client {
    Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("the client connects")
}

fn remote_code(error: &ClientError) -> String {
    match error {
        ClientError::Remote(body) => body.code.clone(),
        other => panic!("expected a remote error, got {other:?}"),
    }
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
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A git repository project under `home` with one worktree workspace named `feature`.
async fn workspace(client: &Client, home: &Path) -> Workspace {
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("create repository dir");
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("README.md"), "hello\n").expect("write readme");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-q", "-m", "initial"]);
    let project = client
        .project_add(&ProjectAddParams {
            icon: None,
            path: repo.to_str().expect("utf-8").to_owned(),
            name: None,
            kind: None,
            workspaces_root: Some(home.join("ws").to_str().expect("utf-8").to_owned()),
        })
        .await
        .expect("the project is added");
    client
        .workspace_create(&WorkspaceCreateParams {
            in_place: false,
            project: project.id,
            name: "feature".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            refresh_base: false,
            path: None,
            request_id: None,
            no_setup: false,
            unique_name: false,
            name_generated: false,
        })
        .await
        .expect("the workspace is created")
}

fn create_params(workspace: &str) -> TerminalCreateParams {
    TerminalCreateParams {
        workspace: workspace.to_owned(),
        shell: None,
        cwd: None,
        env: BTreeMap::new(),
        cols: None,
        rows: None,
        title: None,
        request_id: None,
    }
}

/// Attaches as an emulator: `read_until` answers the queries the output carries.
fn attach_params(terminal: &str) -> TerminalAttachParams {
    TerminalAttachParams {
        terminal: terminal.to_owned(),
        since: None,
        cols: None,
        rows: None,
        answers_queries: Some(true),
    }
}

/// A shell command that prints `left` followed by `right`, without the joined word in the typed text.
fn joined_echo(left: &str, right: &str) -> String {
    if cfg!(windows) {
        format!("echo ('{left}'+'{right}')")
    } else {
        format!("echo {left}''{right}")
    }
}

fn newline() -> &'static [u8] {
    if cfg!(windows) { b"\r" } else { b"\n" }
}

/// Cursor position query (DSR 6) that ConPTY sends and waits on.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// Reads the attachment's output until `needle` appears in it, and returns everything read. While an emulator is attached
/// the service leaves cursor-position queries to it, so this answers each one, as a terminal emulator would.
async fn read_until(
    client: &Client,
    terminal: &str,
    attachment: &mut TerminalAttachment,
    needle: &str,
) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut seen = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let update = timeout(remaining, attachment.next())
            .await
            .unwrap_or_else(|_| panic!("{needle:?} did not appear in time"))
            .expect("the attach stays open")
            .expect("the attach reports no error");
        match update {
            AttachUpdate::Snapshot { bytes, .. } => seen.extend(bytes),
            AttachUpdate::Output { bytes, .. } => {
                for _ in 0..count_occurrences(&bytes, CURSOR_QUERY) {
                    client
                        .terminal_input(terminal, b"\x1b[1;1R")
                        .await
                        .expect("the cursor position answer is written");
                }
                seen.extend(bytes);
            }
            AttachUpdate::Exited { .. } => panic!("the terminal exited before {needle:?} appeared"),
            AttachUpdate::Attached { .. } | AttachUpdate::Resized { .. } => {}
        }
        if contains(&seen, needle) {
            return seen;
        }
    }
}

/// Reads the attach's first item, which announces how the attach started.
async fn attached(attachment: &mut TerminalAttachment) -> (AttachMode, u64) {
    match timeout(Duration::from_secs(10), attachment.next())
        .await
        .expect("the attach is announced")
        .expect("the stream is open")
        .expect("no error")
    {
        AttachUpdate::Attached { mode, offset, .. } => (mode, offset),
        other => panic!("the first item is not attached: {other:?}"),
    }
}

/// Whether a process with `pid` exists. Windows lists the process table; Unix probes the pid with signal 0.
fn process_alive(pid: u32) -> bool {
    if cfg!(windows) {
        let output = massorca_process::command("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist runs");
        String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    } else {
        massorca_process::command("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[tokio::test]
async fn a_terminal_is_created_in_its_workspace_listed_and_shown() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    std::fs::create_dir_all(Path::new(&workspace.path).join("sub")).expect("create sub dir");

    let created: Terminal = client
        .terminal_create(&TerminalCreateParams {
            cwd: Some("sub".to_owned()),
            cols: Some(100),
            rows: Some(30),
            ..create_params("feature")
        })
        .await
        .expect("the terminal is created");
    assert_eq!(created.workspace_id, workspace.id);
    assert_eq!(created.state, TerminalState::Live);
    assert!(created.pid.is_some(), "a live terminal has a process");
    assert_eq!((created.cols, created.rows), (100, 30));
    assert!(
        Path::new(&created.cwd).ends_with("sub"),
        "the cwd is relative to the workspace: {}",
        created.cwd
    );

    let listed = client
        .terminal_list(Some(&workspace.id))
        .await
        .expect("the terminals are listed");
    // The shell sets its window title on its own, so the title can change between two snapshots.
    let listed: Vec<Terminal> = listed.terminals.into_iter().map(without_title).collect();
    assert_eq!(listed, vec![without_title(created.clone())]);
    let everywhere = client
        .terminal_list(None)
        .await
        .expect("the terminals are listed");
    assert_eq!(everywhere.terminals.len(), 1);
    let shown = client
        .terminal_show(&created.id)
        .await
        .expect("the terminal is shown");
    assert_eq!(without_title(shown), without_title(created.clone()));

    client
        .terminal_close(&created.id)
        .await
        .expect("cleanup closes the terminal");
    stop(running).await;
}

#[tokio::test]
async fn an_unknown_workspace_is_refused_as_the_workspace_show_would_refuse_it() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;

    let error = client
        .terminal_create(&create_params("nope"))
        .await
        .expect_err("there is no such workspace");
    assert_eq!(remote_code(&error), codes::WORKSPACE_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn attach_is_refused_when_the_client_did_not_offer_binary_frames() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");

    let socket = local_socket_name(&profile(home.path())).expect("the socket name");
    let stream = Stream::connect(socket)
        .await
        .expect("the raw connection opens");
    let mut writer: &Stream = &stream;
    write_message(
        &mut writer,
        &Message::Hello {
            protocol: ProtocolRange {
                min: MIN_PROTOCOL_VERSION,
                max: PROTOCOL_VERSION,
            },
            client: PeerInfo {
                name: "raw-test-client".to_owned(),
                version: "0.0.0-test".to_owned(),
            },
            capabilities: vec![capabilities::TERMINALS.to_owned()],
            auth: None,
        },
        MAX_FRAME_LEN_LOCAL,
    )
    .await
    .expect("hello is sent");
    let mut reader: &Stream = &stream;
    let welcome = read_message(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("the welcome reads")
        .expect("the welcome arrives");
    assert!(matches!(welcome, Message::Welcome { .. }));

    let mut writer: &Stream = &stream;
    write_message(
        &mut writer,
        &Message::Request {
            id: 1,
            method: "terminal.attach".to_owned(),
            params: serde_json::json!({ "terminal": created.id }),
        },
        MAX_FRAME_LEN_LOCAL,
    )
    .await
    .expect("the attach request is sent");
    let mut reader: &Stream = &stream;
    let answer = timeout(
        Duration::from_secs(5),
        read_message(&mut reader, MAX_FRAME_LEN_LOCAL),
    )
    .await
    .expect("the service answers")
    .expect("the answer reads")
    .expect("the answer arrives");
    match answer {
        Message::Response {
            id: 1,
            error: Some(error),
            ..
        } => assert_eq!(error.code, codes::STREAM_BINARY_FRAMES_REQUIRED),
        other => panic!("expected a refusal, got {other:?}"),
    }

    client
        .terminal_close(&created.id)
        .await
        .expect("cleanup closes the terminal");
    stop(running).await;
}

#[tokio::test]
async fn input_reaches_the_shell_and_the_attach_shows_its_output() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");

    let mut attachment = client
        .terminal_attach(&attach_params(&created.id))
        .await
        .expect("the attach opens");
    attached(&mut attachment).await;
    let mut command = joined_echo("ma", "rker").into_bytes();
    command.extend_from_slice(newline());
    client
        .terminal_input(&created.id, &command)
        .await
        .expect("the input is written");

    read_until(&client, &created.id, &mut attachment, "marker").await;
    drop(attachment);
    client
        .terminal_close(&created.id)
        .await
        .expect("cleanup closes the terminal");
    stop(running).await;
}

#[tokio::test]
async fn a_terminal_outlives_its_client_and_a_new_client_resumes_it() {
    let home = temp_home();
    let running = start(home.path()).await;
    let first = connect(home.path()).await;
    let workspace = workspace(&first, home.path()).await;
    let created = first
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");

    let mut attachment = first
        .terminal_attach(&attach_params(&created.id))
        .await
        .expect("the attach opens");
    attached(&mut attachment).await;
    let mut command = joined_echo("ma", "rker").into_bytes();
    command.extend_from_slice(newline());
    first
        .terminal_input(&created.id, &command)
        .await
        .expect("the input is written");
    read_until(&first, &created.id, &mut attachment, "marker").await;
    let read_up_to = attachment.next_offset();
    drop(attachment);
    drop(first);

    let second = connect(home.path()).await;
    let still_live = second
        .terminal_show(&created.id)
        .await
        .expect("the terminal is still listed");
    assert_eq!(still_live.state, TerminalState::Live);

    let mut fresh = second
        .terminal_attach(&attach_params(&created.id))
        .await
        .expect("the attach opens");
    let (mode, _) = attached(&mut fresh).await;
    assert_eq!(mode, AttachMode::Snapshot);
    read_until(&second, &created.id, &mut fresh, "marker").await;
    drop(fresh);

    let mut resumed = second
        .terminal_attach(&TerminalAttachParams {
            since: Some(read_up_to),
            ..attach_params(&created.id)
        })
        .await
        .expect("the resumed attach opens");
    let (mode, offset) = attached(&mut resumed).await;
    assert_eq!(mode, AttachMode::Resume);
    assert_eq!(
        offset, read_up_to,
        "output resumes exactly where the client stopped"
    );

    let mut next_command = joined_echo("wor", "ld").into_bytes();
    next_command.extend_from_slice(newline());
    second
        .terminal_input(&created.id, &next_command)
        .await
        .expect("the input is written");
    read_until(&second, &created.id, &mut resumed, "world").await;

    drop(resumed);
    second
        .terminal_close(&created.id)
        .await
        .expect("cleanup closes the terminal");
    stop(running).await;
}

#[tokio::test]
async fn a_resize_reaches_every_attachment_and_the_terminal_keeps_one_size() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");
    assert_eq!((created.cols, created.rows), (120, 40));

    let mut attachment = client
        .terminal_attach(&attach_params(&created.id))
        .await
        .expect("the attach opens");
    attached(&mut attachment).await;
    let resized = client
        .terminal_resize(&TerminalResizeParams {
            terminal: created.id.clone(),
            cols: 100,
            rows: 30,
        })
        .await
        .expect("the resize is accepted");
    assert_eq!((resized.cols, resized.rows), (100, 30));

    let heard = timeout(Duration::from_secs(10), async {
        loop {
            match attachment.next().await {
                Some(Ok(AttachUpdate::Resized { cols, rows })) => return (cols, rows),
                Some(Ok(_)) => {}
                other => panic!("the attach ended before the resize: {other:?}"),
            }
        }
    })
    .await
    .expect("the resize is announced");
    assert_eq!(heard, (100, 30));

    let shown = client
        .terminal_show(&created.id)
        .await
        .expect("the terminal is shown");
    assert_eq!((shown.cols, shown.rows), (100, 30));
    drop(attachment);
    client
        .terminal_close(&created.id)
        .await
        .expect("cleanup closes the terminal");
    stop(running).await;
}

#[tokio::test]
async fn an_oversized_or_badly_encoded_input_is_refused() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");

    let oversized = vec![b'a'; MAX_INPUT_BYTES + 1];
    let error = client
        .terminal_input(&created.id, &oversized)
        .await
        .expect_err("the input is over the limit");
    assert_eq!(remote_code(&error), codes::TERMINAL_INPUT_TOO_LARGE);
    let error = client
        .call::<_, serde_json::Value>(
            "terminal.input",
            &serde_json::json!({ "terminal": created.id, "data": "not base64!" }),
        )
        .await
        .expect_err("the data is not base64");
    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);

    client
        .terminal_close(&created.id)
        .await
        .expect("cleanup closes the terminal");
    stop(running).await;
}

#[tokio::test]
async fn kill_ends_the_process_with_reason_killed_and_input_then_fails() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");

    let killed = client
        .terminal_kill(&created.id, true)
        .await
        .expect("the kill is accepted");
    assert_eq!(killed.state, TerminalState::Exited);
    let exit = killed.exit.expect("an exited terminal says how it ended");
    assert_eq!(
        exit.reason,
        massorca_protocol::methods::terminals::ExitReason::Killed
    );

    let error = client
        .terminal_input(&created.id, b"x")
        .await
        .expect_err("input to an exited terminal fails");
    assert_eq!(remote_code(&error), codes::TERMINAL_EXITED);
    let shown = client
        .terminal_show(&created.id)
        .await
        .expect("an exited terminal stays listed");
    assert_eq!(shown.state, TerminalState::Exited);
    stop(running).await;
}

#[tokio::test]
async fn close_forgets_the_terminal() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");

    client
        .terminal_close(&created.id)
        .await
        .expect("the close is accepted");
    let error = client
        .terminal_show(&created.id)
        .await
        .expect_err("a closed terminal is gone");
    assert_eq!(remote_code(&error), codes::TERMINAL_NOT_FOUND);
    let listed = client
        .terminal_list(None)
        .await
        .expect("the terminals are listed");
    assert!(listed.terminals.is_empty());
    stop(running).await;
}

#[tokio::test]
async fn service_shutdown_kills_the_live_terminals() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = connect(home.path()).await;
    let workspace = workspace(&client, home.path()).await;
    let created = client
        .terminal_create(&create_params(&workspace.id))
        .await
        .expect("the terminal is created");
    let pid = created.pid.expect("a live terminal has a process");
    assert!(process_alive(pid), "the shell runs before the stop");
    drop(client);

    stop(running).await;
    let deadline = Instant::now() + Duration::from_secs(15);
    while process_alive(pid) {
        assert!(
            Instant::now() < deadline,
            "the terminal's process {pid} outlived the service"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The terminal with its title cleared: the shell sets its window title asynchronously (pwsh does on Windows), so a
/// comparison of two snapshots leaves it out and keeps every other field.
fn without_title(terminal: Terminal) -> Terminal {
    Terminal {
        title: None,
        ..terminal
    }
}
