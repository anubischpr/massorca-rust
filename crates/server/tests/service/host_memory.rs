//! `host.memory` (capability `host/1`, decision 0081): the service process plus each live terminal's processes.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use massorca_protocol::capabilities;
use massorca_protocol::methods::host::{self, HostMemory, MemoryMetric};
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::terminals::{
    self, ShellSpec, Terminal, TerminalCreateParams, TerminalRef,
};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};

use crate::common::{connect, start, stop, temp_home};

const TIMEOUT: Duration = Duration::from_secs(30);

/// A shell that starts a child and waits for it.
fn shell_with_a_child() -> ShellSpec {
    if cfg!(windows) {
        ShellSpec {
            program: "cmd.exe".to_owned(),
            args: vec![
                "/d".to_owned(),
                "/c".to_owned(),
                "ping -n 120 127.0.0.1 >nul".to_owned(),
            ],
        }
    } else {
        ShellSpec {
            program: "sh".to_owned(),
            args: vec!["-c".to_owned(), "sleep 120; true".to_owned()],
        }
    }
}

fn utf8(path: &std::path::Path) -> String {
    path.to_str().expect("utf-8 path").to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn host_memory_counts_the_service_and_a_created_terminal() {
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
                path: None,
                request_id: None,
            },
        )
        .await
        .expect("create workspace");
    let terminal: Terminal = client
        .call(
            terminals::CREATE,
            &TerminalCreateParams {
                workspace: workspace.id.clone(),
                shell: Some(shell_with_a_child()),
                cwd: None,
                env: BTreeMap::new(),
                cols: None,
                rows: None,
                title: None,
                request_id: None,
            },
        )
        .await
        .expect("create terminal");

    let deadline = Instant::now() + TIMEOUT;
    let memory = loop {
        let memory = client.host_memory().await.expect("host.memory");
        if memory
            .terminals
            .iter()
            .any(|entry| entry.terminal_id == terminal.id && entry.processes >= 2)
        {
            break memory;
        }
        assert!(Instant::now() < deadline, "{memory:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    let expected = if cfg!(windows) {
        MemoryMetric::WorkingSet
    } else {
        MemoryMetric::Resident
    };
    assert_eq!(memory.metric, expected);
    assert_eq!(memory.service.pid, info.pid);
    assert!(memory.service.bytes > 0, "{memory:?}");
    assert!(memory.total_bytes >= memory.service.bytes, "{memory:?}");
    let terminals_bytes: u64 = memory.terminals.iter().map(|entry| entry.bytes).sum();
    assert_eq!(memory.total_bytes, memory.service.bytes + terminals_bytes);
    let entry = memory
        .terminals
        .iter()
        .find(|entry| entry.terminal_id == terminal.id)
        .expect("the terminal is listed");
    assert_eq!(entry.workspace_id, workspace.id);
    assert_eq!(
        entry.session_id, None,
        "a plain terminal holds no agent run"
    );
    assert!(entry.bytes > 0, "{entry:?}");
    assert!(!entry.unverifiable, "{entry:?}");
    assert!(memory.sampled_at_ms > 0);

    let _: Terminal = client
        .call(
            terminals::CLOSE,
            &TerminalRef {
                terminal: terminal.id.clone(),
            },
        )
        .await
        .expect("close terminal");
    let after: HostMemory = client.call(host::MEMORY, &()).await.expect("host.memory");
    assert!(
        after
            .terminals
            .iter()
            .all(|entry| entry.terminal_id != terminal.id),
        "{after:?}"
    );
    drop(client);
    stop(running).await;
}
