//! `host.ports` (capability `host/1`, decision 0081): the listening sockets of each live terminal's processes, read on this
//! host when asked. The service runs in this test process, so the test's own listeners are the service's own.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::Client;
use massorca_protocol::capabilities;
use massorca_protocol::methods::host::HostPorts;
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::terminals::{
    self, ShellSpec, Terminal, TerminalCreateParams, TerminalRef,
};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};

use crate::common::{connect, start, stop, temp_home};

const TIMEOUT: Duration = Duration::from_secs(30);
/// Set in the terminal's environment: the `listener` test then listens and writes its port to the file it names.
const LISTENER_ENV: &str = "MASSORCA_HOST_PORTS_LISTENER";

/// Not a test of its own: the child a terminal runs. It listens on a free port, writes the port to the file named in
/// `LISTENER_ENV`, and waits. Returns at once unless started that way.
#[test]
fn listener() {
    let Some(file) = std::env::var_os(LISTENER_ENV) else {
        return;
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a local listener");
    let port = listener.local_addr().expect("its address").port();
    std::fs::write(file, port.to_string()).expect("the port is written");
    std::thread::sleep(Duration::from_secs(120));
}

/// The terminal's program: this test binary, running only `listener`, so the listener is the terminal's child.
fn listener_shell() -> ShellSpec {
    let exe = std::env::current_exe().expect("the test binary");
    ShellSpec {
        program: exe.to_str().expect("a utf-8 path").to_owned(),
        args: [
            "--exact",
            "host_ports::listener",
            "--test-threads",
            "1",
            "--nocapture",
        ]
        .map(str::to_owned)
        .to_vec(),
    }
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("utf-8 path").to_owned()
}

/// The port the listener child wrote, once it is listening.
async fn written_port(file: &Path) -> u16 {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(port) = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            return port;
        }
        assert!(Instant::now() < deadline, "the listener wrote no port");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// `host.ports`, asked until the terminal's listener shows (a sample is shared for about a second).
async fn listed(client: &Client, terminal: &str, port: u16) -> HostPorts {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let ports = client.host_ports().await.expect("host.ports");
        if ports
            .ports
            .iter()
            .any(|entry| entry.port == port && entry.terminal_id == terminal)
        {
            return ports;
        }
        assert!(Instant::now() < deadline, "{ports:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg_attr(
    target_os = "macos",
    ignore = "macOS reads no socket table yet (decision 0081)"
)]
#[tokio::test(flavor = "multi_thread")]
async fn a_listener_of_a_terminal_is_listed_with_its_workspace_and_the_terminals_status() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let info = client.service_info().await.expect("service.info");
    assert!(
        info.capabilities
            .iter()
            .any(|name| name == capabilities::HOST),
        "{:?}",
        info.capabilities
    );

    let folder = home.path().join("plain");
    std::fs::create_dir_all(&folder).expect("create folder");
    let project: Project = client
        .call(
            projects::ADD,
            &ProjectAddParams {
                icon: None,
                path: utf8(&folder),
                name: Some("plain".to_owned()),
                kind: Some(ProjectKind::Folder),
                workspaces_root: Some(utf8(&home.path().join("ws"))),
            },
        )
        .await
        .expect("add folder project");
    let workspace: Workspace = client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project: project.id,
                name: "notes".to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
                no_setup: false,
                unique_name: false,
                name_generated: false,
            },
        )
        .await
        .expect("create workspace");
    let port_file = home.path().join("listener.port");
    let terminal: Terminal = client
        .call(
            terminals::CREATE,
            &TerminalCreateParams {
                workspace: workspace.id.clone(),
                shell: Some(listener_shell()),
                cwd: None,
                env: BTreeMap::from([(LISTENER_ENV.to_owned(), utf8(&port_file))]),
                cols: None,
                rows: None,
                title: None,
                request_id: None,
            },
        )
        .await
        .expect("create terminal");

    let port = written_port(&port_file).await;
    let ports = listed(&client, &terminal.id, port).await;
    let entry = ports
        .ports
        .iter()
        .find(|entry| entry.port == port && entry.terminal_id == terminal.id)
        .expect("the listener is listed");
    assert_eq!(entry.address, IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(entry.connect_host, "127.0.0.1");
    assert_eq!(entry.workspace_id, workspace.id);
    assert_eq!(
        entry.session_id, None,
        "a plain terminal holds no agent run"
    );
    assert!(entry.pid > 0, "{entry:?}");
    assert!(!entry.process_name.is_empty(), "{entry:?}");
    assert!(!ports.unverifiable, "{ports:?}");
    let status = ports
        .terminals
        .iter()
        .find(|status| status.terminal_id == terminal.id)
        .expect("the terminal has a status");
    assert!(!status.unverifiable && !status.partial, "{status:?}");

    // The service's own listener is not listed: this test process is the service.
    let own = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a listener of the service");
    let own_port = own.local_addr().expect("its address").port();
    let again = client.host_ports().await.expect("host.ports");
    assert!(
        again.ports.iter().all(|entry| entry.port != own_port),
        "the service's own listener is listed: {again:?}"
    );

    let _: Terminal = client
        .call(
            terminals::CLOSE,
            &TerminalRef {
                terminal: terminal.id.clone(),
            },
        )
        .await
        .expect("close terminal");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let after = client.host_ports().await.expect("host.ports");
        if after
            .ports
            .iter()
            .all(|entry| entry.terminal_id != terminal.id)
        {
            break;
        }
        assert!(Instant::now() < deadline, "{after:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    drop(own);
    drop(client);
    stop(running).await;
}

/// macOS reads no socket table yet (decision 0081): the answer is unverifiable, and it lists no port rather than none.
#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread")]
async fn macos_answers_host_ports_as_unverifiable_and_lists_none() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let ports = client.host_ports().await.expect("host.ports");
    assert!(ports.unverifiable, "{ports:?}");
    assert!(ports.ports.is_empty(), "{ports:?}");
    drop(client);
    stop(running).await;
}
