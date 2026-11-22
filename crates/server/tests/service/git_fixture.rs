//! A running service with one git project and one worktree workspace, for the `git/1` method tests.

use std::path::{Path, PathBuf};

use super::common::{TempHome, connect, start, stop, temp_home};
use massorca_client::Client;
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};
use massorca_server::RunningService;

pub struct GitService {
    pub home: TempHome,
    pub running: RunningService,
    pub client: Client,
    /// The workspace's worktree; the git methods act on it.
    pub workspace: Workspace,
}

impl GitService {
    /// A repository on `main` with `README.md` committed, and a workspace `feature` on a new branch.
    pub async fn start() -> Self {
        let home = temp_home();
        let repo = home.path().join("repo");
        std::fs::create_dir_all(&repo).expect("create repo dir");
        git(&repo, &["init", "-q"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        git(&repo, &["config", "core.autocrlf", "false"]);
        // Commits the service makes use this identity, whatever the machine's global config holds.
        git(&repo, &["config", "user.name", "t"]);
        git(&repo, &["config", "user.email", "t@example.invalid"]);
        git(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("README.md"), "hello\n").expect("write readme");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "-q", "-m", "initial"]);
        let running = start(home.path(), "default").await;
        let client = connect(home.path(), "default").await;
        let project: Project = client
            .call(
                projects::ADD,
                &ProjectAddParams {
                    path: utf8(&repo),
                    name: Some("repo".to_owned()),
                    kind: None,
                    workspaces_root: Some(utf8(&home.path().join("ws"))),
                },
            )
            .await
            .expect("add project");
        let workspace: Workspace = client
            .call(
                workspaces::CREATE,
                &WorkspaceCreateParams {
                    project: project.id,
                    name: "feature".to_owned(),
                    branch: None,
                    base: None,
                    existing_branch: false,
                    path: None,
                    request_id: None,
                },
            )
            .await
            .expect("create workspace");
        Self {
            home,
            running,
            client,
            workspace,
        }
    }

    pub fn dir(&self) -> PathBuf {
        PathBuf::from(&self.workspace.path)
    }

    /// Adds a folder project with one folder workspace and returns that workspace.
    pub async fn folder_workspace(&self) -> Workspace {
        let folder = self.home.path().join("plain");
        std::fs::create_dir_all(&folder).expect("create folder");
        let project: Project = self
            .client
            .call(
                projects::ADD,
                &ProjectAddParams {
                    path: utf8(&folder),
                    name: Some("plain".to_owned()),
                    kind: Some(ProjectKind::Folder),
                    workspaces_root: Some(utf8(&self.home.path().join("ws-plain"))),
                },
            )
            .await
            .expect("add folder project");
        self.client
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
            .expect("create folder workspace")
    }

    pub async fn stop(self) {
        drop(self.client);
        stop(self.running).await;
    }
}

/// Runs git in `dir` with a fixed identity and returns stdout; panics on failure.
pub fn git(dir: &Path, args: &[&str]) -> String {
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
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("test paths are UTF-8").to_owned()
}
