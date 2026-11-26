//! A real service for the screenshot tests. It serves a temporary profile on a thread of its own,
//! as the app's bridge finds it, until it is stopped. No test ever reaches the user's service.

use std::future::Future;
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;

use massorca_client::{Client, ConnectOptions};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_server::{Service, ServiceConfig};
use tokio::sync::oneshot;

/// The profile of a temporary home, named `default` as the app names it.
pub fn profile(home: &Path) -> anyhow::Result<Profile> {
    Ok(Profile::resolve_with(
        Some(home.to_path_buf()),
        Some("default".to_owned()),
        |_| None,
    )?)
}

/// Runs `future` to completion on the calling thread.
pub fn block_on<F: Future>(future: F) -> anyhow::Result<F::Output> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(future))
}

/// A service serving `profile` on a thread of its own, until `stop` is called.
pub struct LocalService {
    stop: oneshot::Sender<()>,
    thread: JoinHandle<()>,
}

impl LocalService {
    /// Starts serving `profile`, and returns once the endpoint is bound.
    pub fn start(profile: Profile) -> anyhow::Result<Self> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop, stopped) = oneshot::channel();
        let thread =
            std::thread::spawn(move || {
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(serve(profile, ready_tx, stopped)),
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                    }
                }
            });
        ready_rx
            .recv()?
            .map_err(|error| anyhow::anyhow!("the service did not start: {error}"))?;
        Ok(Self { stop, thread })
    }

    /// Stops the service and waits for its thread to finish.
    pub fn stop(self) {
        let Self { stop, thread } = self;
        let _ = stop.send(());
        let _ = thread.join();
    }
}

async fn serve(
    profile: Profile,
    ready: mpsc::Sender<Result<(), String>>,
    stopped: oneshot::Receiver<()>,
) {
    let config = ServiceConfig {
        profile,
        server_name: "massorca-shots".to_owned(),
        server_version: "0.0.0-shots".to_owned(),
    };
    match Service::start(config).await {
        Ok(running) => {
            let _ = ready.send(Ok(()));
            let _ = stopped.await;
            running.shutdown_handle().shutdown();
            let _ = running.wait().await;
        }
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
        }
    }
}

/// Runs git in `dir` with a fixed identity, and fails when git does.
pub fn git(dir: &Path, args: &[&str]) -> anyhow::Result<()> {
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
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// A repository with one commit and a `.gitignore` that ignores `*.log`.
pub fn repo(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    git(dir, &["init", "-q"])?;
    std::fs::write(dir.join("README.md"), "hello\n")?;
    std::fs::write(dir.join(".gitignore"), "*.log\n")?;
    git(dir, &["add", "."])?;
    git(dir, &["commit", "-q", "-m", "initial"])
}

/// Adds `repo_dir` as the project "demo" and creates the workspace `name` in it, through a client
/// of the service. The worktree folder is returned, so a shot can change files in it.
pub async fn add_project_and_workspace(
    profile: &Profile,
    repo_dir: &Path,
    name: &str,
) -> anyhow::Result<Workspace> {
    let client = Client::connect(
        profile,
        ConnectOptions::new(PeerInfo {
            name: "massorca-shots".to_owned(),
            version: "0.0.0-shots".to_owned(),
        }),
    )
    .await?;
    let project = client
        .project_add(&ProjectAddParams {
            icon: None,
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            kind: None,
            workspaces_root: repo_dir
                .parent()
                .map(|parent| parent.join("demo-worktrees").display().to_string()),
        })
        .await?;
    let workspace = client
        .workspace_create(&WorkspaceCreateParams {
            project: project.id,
            name: name.to_owned(),
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
        .await?;
    Ok(workspace)
}
