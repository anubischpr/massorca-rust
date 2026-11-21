//! Terminals over the wire: an in-process service on a temporary home, driven through the client crate. Terminals outlive
//! their clients and end with the service.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_client::{Client, ClientError, ConnectOptions};
use massorca_profile::{Profile, local_socket_name};
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::terminals::{
    MAX_INPUT_BYTES, Terminal, TerminalCreateParams, TerminalState,
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

fn temp_home() -> TempDir {
    tempfile::tempdir().expect("a temporary home")
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
            path: repo.to_str().expect("utf-8").to_owned(),
            name: None,
            kind: None,
            workspaces_root: Some(home.join("ws").to_str().expect("utf-8").to_owned()),
        })
        .await
        .expect("the project is added");
    client
        .workspace_create(&WorkspaceCreateParams {
            project: project.id,
            name: "feature".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            path: None,
            request_id: None,
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

/// Whether a process with `pid` exists. Windows lists the process table; Unix probes the pid with signal 0.
fn process_alive(pid: u32) -> bool {
    if cfg!(windows) {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist runs");
        String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    } else {
        Command::new("kill")
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
    assert_eq!(listed.terminals, vec![created.clone()]);
    let everywhere = client
        .terminal_list(None)
        .await
        .expect("the terminals are listed");
    assert_eq!(everywhere.terminals.len(), 1);
    let shown = client
        .terminal_show(&created.id)
        .await
        .expect("the terminal is shown");
    assert_eq!(shown, created);

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
