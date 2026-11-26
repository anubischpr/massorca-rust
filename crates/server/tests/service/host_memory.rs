//! `host.memory` (capability `host/1`, decision 0081): the service process plus each live terminal's processes.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use massorca_protocol::capabilities;
use massorca_protocol::methods::host::{self, HostMemory, MemoryMetric};
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::terminals::{self, Terminal, TerminalCreateParams, TerminalRef};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};

use crate::common::{connect, shell_with_a_child, start, stop, temp_home};

const TIMEOUT: Duration = Duration::from_secs(30);

/// A shell that starts a child and waits for it.
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
    // A sample is shared for about a second (decision 0081), so the closed terminal leaves the answer within that time.
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let after: HostMemory = client.call(host::MEMORY, &()).await.expect("host.memory");
        if after
            .terminals
            .iter()
            .all(|entry| entry.terminal_id != terminal.id)
        {
            break;
        }
        assert!(Instant::now() < deadline, "{after:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    drop(client);
    stop(running).await;
}
