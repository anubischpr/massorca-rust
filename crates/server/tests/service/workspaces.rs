//! Workspaces over the wire: create, list, show and remove for worktrees and folders, their events in order,
//! and their refusals.

use std::path::Path;
use std::time::Duration;

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_WORKSPACE_CREATED, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectKind, ProjectRemoveParams,
};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCreateParams, WorkspaceKind, WorkspaceList, WorkspaceListParams,
    WorkspaceRef, WorkspaceRemoveParams, WorkspaceRemoved, WorkspaceState,
};
use tokio::time::timeout;

use super::common::{connect, remote_code, start, stop, temp_home};

fn git_repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create repository dir");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("write readme");
    git(dir, &["add", "README.md"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
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

fn create_params(project: &str, name: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: name.to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        path: None,
        request_id: None,
    }
}

fn remove_params(workspace: &str, force: bool) -> WorkspaceRemoveParams {
    WorkspaceRemoveParams {
        workspace: workspace.to_owned(),
        force,
        discard_ignored: false,
        forget_orphan: false,
    }
}

async fn next_event(events: &mut Subscription) -> ChangeEvent {
    timeout(Duration::from_secs(10), events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes")
}

/// A git project at `repo` whose workspaces go under `<home>/ws`.
async fn add_repo_project(client: &massorca_client::Client, home: &Path, repo: &Path) -> Project {
    client
        .call(
            projects::ADD,
            &ProjectAddParams {
                path: repo.to_str().expect("utf-8").to_owned(),
                name: None,
                kind: None,
                workspaces_root: Some(home.join("ws").to_str().expect("utf-8").to_owned()),
            },
        )
        .await
        .expect("add project")
}

#[tokio::test]
async fn a_worktree_is_created_listed_shown_and_removed_with_its_events_in_order() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_repo_project(&client, home.path(), &repo).await;
    let mut events = client.subscribe_events().await.expect("subscribe");

    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "feature"))
        .await
        .expect("create");
    assert_eq!(created.kind, WorkspaceKind::Worktree);
    assert!(Path::new(&created.path).is_dir());
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_WORKSPACE_CREATED.to_owned(),
            id: created.id.clone()
        }
    );

    let list: WorkspaceList = client
        .call(
            workspaces::LIST,
            &WorkspaceListParams {
                project: Some(project.id.clone()),
            },
        )
        .await
        .expect("list");
    assert_eq!(list.workspaces, vec![created.clone()]);
    let shown: Workspace = client
        .call(
            workspaces::SHOW,
            &WorkspaceRef {
                workspace: "feature".to_owned(),
            },
        )
        .await
        .expect("show by name");
    assert_eq!(shown, created);

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &remove_params(&created.id, false))
        .await
        .expect("remove");
    assert!(removed.branch_deleted);
    assert!(!removed.directory_kept);
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_WORKSPACE_REMOVING.to_owned(),
            id: created.id.clone()
        }
    );
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_WORKSPACE_REMOVED.to_owned(),
            id: created.id.clone()
        }
    );
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn a_folder_workspace_is_forgotten_on_removal_and_its_directory_stays() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    std::fs::write(folder.join("notes.txt"), "keep").expect("write notes");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project: Project = client
        .call(
            projects::ADD,
            &ProjectAddParams {
                path: folder.to_str().expect("utf-8").to_owned(),
                name: None,
                kind: Some(ProjectKind::Folder),
                workspaces_root: None,
            },
        )
        .await
        .expect("add folder project");
    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "main"))
        .await
        .expect("create folder workspace");
    assert_eq!(created.kind, WorkspaceKind::Folder);

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &remove_params("main", false))
        .await
        .expect("remove");

    assert!(removed.directory_kept);
    assert!(folder.join("notes.txt").is_file());
    let gone = client
        .call::<_, Workspace>(
            workspaces::SHOW,
            &WorkspaceRef {
                workspace: "main".to_owned(),
            },
        )
        .await
        .expect_err("forgotten");
    assert_eq!(remote_code(&gone), codes::WORKSPACE_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn a_dirty_worktree_is_refused_and_a_project_with_workspaces_cannot_be_removed() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_repo_project(&client, home.path(), &repo).await;
    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "dirty"))
        .await
        .expect("create");
    std::fs::write(Path::new(&created.path).join("scratch.txt"), "work").expect("write change");

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &remove_params("dirty", false))
        .await
        .expect_err("dirty");
    assert_eq!(remote_code(&refused), codes::WORKSPACE_DIRTY);
    let after_refusal: Workspace = client
        .call(
            workspaces::SHOW,
            &WorkspaceRef {
                workspace: created.id.clone(),
            },
        )
        .await
        .expect("show after refusal");
    assert_eq!(after_refusal.state, WorkspaceState::Ready);

    let blocked = client
        .call::<_, Project>(
            projects::REMOVE,
            &ProjectRemoveParams {
                project: project.id.clone(),
                forget_workspaces: false,
            },
        )
        .await
        .expect_err("project with workspaces");
    assert_eq!(remote_code(&blocked), codes::PROJECT_HAS_WORKSPACES);

    let forced: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &remove_params(&created.id, true))
        .await
        .expect("forced removal");
    assert!(!forced.directory_kept);
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn ignored_files_refuse_removal_over_the_wire_until_discard_ignored_is_sent() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    std::fs::write(
        repo.join(".gitignore"),
        "*.env
",
    )
    .expect("write ignore rules");
    git(&repo, &["add", ".gitignore"]);
    git(&repo, &["commit", "-q", "-m", "ignore env files"]);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_repo_project(&client, home.path(), &repo).await;
    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "ignored"))
        .await
        .expect("create");
    std::fs::write(Path::new(&created.path).join("secrets.env"), "key").expect("write ignored");

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &remove_params("ignored", true))
        .await
        .expect_err("ignored files");
    assert_eq!(remote_code(&refused), codes::WORKSPACE_IGNORED_FILES);

    let removed: WorkspaceRemoved = client
        .call(
            workspaces::REMOVE,
            &WorkspaceRemoveParams {
                workspace: "ignored".to_owned(),
                force: true,
                discard_ignored: true,
                forget_orphan: false,
            },
        )
        .await
        .expect("removal with discard_ignored");
    assert!(!removed.directory_kept);
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_workspace_stops_its_terminals_first() {
    use massorca_protocol::methods::terminals::{
        self, Terminal, TerminalCreateParams, TerminalList, TerminalListParams, TerminalState,
    };

    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_repo_project(&client, home.path(), &repo).await;
    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "busy"))
        .await
        .expect("create");
    let terminal: Terminal = client
        .call(
            terminals::CREATE,
            &TerminalCreateParams {
                workspace: created.id.clone(),
                shell: None,
                cwd: None,
                env: std::collections::BTreeMap::new(),
                cols: None,
                rows: None,
                title: None,
                request_id: None,
            },
        )
        .await
        .expect("a terminal starts in the workspace");
    assert_eq!(terminal.state, TerminalState::Live);

    let _: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &remove_params(&created.id, false))
        .await
        .expect("the removal stops the terminal, then removes the worktree");
    assert!(!Path::new(&created.path).exists());
    let listed: TerminalList = client
        .call(terminals::LIST, &TerminalListParams { workspace: None })
        .await
        .expect("list");
    let after = listed
        .terminals
        .iter()
        .find(|listed| listed.id == terminal.id)
        .expect("the stopped terminal stays listed as exited");
    assert_eq!(after.state, TerminalState::Exited);
    stop(running).await;
}
