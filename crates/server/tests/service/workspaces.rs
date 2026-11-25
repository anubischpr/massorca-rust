//! Workspaces over the wire: create, list, show and remove for worktrees and folders, their events in order,
//! and their refusals.

use std::path::Path;
use std::time::Duration;

use massorca_client::Subscription;
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
use massorca_protocol::{capabilities, codes};
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
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
        unique_name: false,
    }
}

fn remove_params(workspace: &str, force: bool) -> WorkspaceRemoveParams {
    WorkspaceRemoveParams {
        workspace: workspace.to_owned(),
        force,
        discard_ignored: false,
        forget_orphan: false,
        allow_failed_archive: false,
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
                icon: None,
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

/// The full id of `HEAD` in `dir`.
fn head_of(dir: &Path) -> String {
    let output = massorca_process::command("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("spawn git");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// Pushes one commit to the branch `origin` holds, from a second clone of it.
fn push_new_commit(home: &Path, origin: &Path) {
    let other = home.join("other");
    git(
        home,
        &[
            "clone",
            "-q",
            origin.to_str().expect("utf-8"),
            other.to_str().expect("utf-8"),
        ],
    );
    std::fs::write(other.join("next.txt"), "next\n").expect("write file");
    git(&other, &["add", "next.txt"]);
    git(&other, &["commit", "-q", "-m", "next"]);
    git(&other, &["push", "-q", "origin", "HEAD"]);
}

#[tokio::test]
async fn a_refresh_base_create_starts_from_the_remote_branch_and_a_plain_one_from_the_local_copy() {
    let home = temp_home();
    let seed = home.path().join("seed");
    git_repo(&seed);
    let origin = home.path().join("origin.git");
    git(
        home.path(),
        &[
            "clone",
            "-q",
            "--bare",
            seed.to_str().expect("utf-8"),
            origin.to_str().expect("utf-8"),
        ],
    );
    let repo = home.path().join("repo");
    git(
        home.path(),
        &[
            "clone",
            "-q",
            origin.to_str().expect("utf-8"),
            repo.to_str().expect("utf-8"),
        ],
    );
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_repo_project(&client, home.path(), &repo).await;
    let local = head_of(&repo);
    push_new_commit(home.path(), &origin);
    let remote = head_of(&origin);

    let plain: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "plain"))
        .await
        .expect("plain create");
    assert_eq!(head_of(Path::new(&plain.path)), local);

    let mut refresh = create_params(&project.name, "fresh");
    refresh.refresh_base = true;
    let fresh: Workspace = client
        .call(workspaces::CREATE, &refresh)
        .await
        .expect("refreshing create");
    assert_eq!(head_of(Path::new(&fresh.path)), remote);
    stop(running).await;
}

#[tokio::test]
async fn unique_name_moves_a_taken_name_on_and_a_typed_name_alone_is_still_refused() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    assert!(client.has_capability(capabilities::WORKSPACE_UNIQUE_NAME));
    let project = add_repo_project(&client, home.path(), &repo).await;
    let first: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "explore"))
        .await
        .expect("first create");

    let refused = client
        .call::<_, Workspace>(workspaces::CREATE, &create_params(&project.name, "explore"))
        .await
        .expect_err("a typed name alone is refused");
    assert_eq!(remote_code(&refused), codes::WORKSPACE_NAME_TAKEN);

    let mut unique = create_params(&project.name, "explore");
    unique.unique_name = true;
    let second: Workspace = client
        .call(workspaces::CREATE, &unique)
        .await
        .expect("unique create");
    assert_eq!(first.name, "explore");
    assert_eq!(second.name, "explore-2");
    assert_eq!(second.branch.as_deref(), Some("explore-2"));
    stop(running).await;
}

#[tokio::test]
async fn a_locked_worktree_reports_its_lock_reason_in_list_and_show() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_repo_project(&client, home.path(), &repo).await;
    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "usb"))
        .await
        .expect("create");
    git(
        &repo,
        &[
            "worktree",
            "lock",
            "--reason",
            "on a usb disk",
            created.path.as_str(),
        ],
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
    assert_eq!(list.workspaces[0].locked.as_deref(), Some("on a usb disk"));
    let shown: Workspace = client
        .call(
            workspaces::SHOW,
            &WorkspaceRef {
                workspace: "usb".to_owned(),
            },
        )
        .await
        .expect("show");
    assert_eq!(shown.locked.as_deref(), Some("on a usb disk"));
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
                icon: None,
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
                allow_failed_archive: false,
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

#[tokio::test]
async fn current_names_the_workspace_that_holds_a_directory_and_refuses_one_outside_all() {
    use massorca_protocol::methods::workspaces::WorkspaceCurrentParams;

    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    assert!(
        client
            .welcome()
            .capabilities
            .iter()
            .any(|name| name == massorca_protocol::capabilities::WORKSPACE_CURRENT),
        "the service offers workspace-current/1"
    );
    let project = add_repo_project(&client, home.path(), &repo).await;
    let created: Workspace = client
        .call(workspaces::CREATE, &create_params(&project.name, "deep"))
        .await
        .expect("create");
    let inside = Path::new(&created.path).join("src");
    std::fs::create_dir_all(&inside).expect("create subdirectory");

    let found: Workspace = client
        .call(
            workspaces::CURRENT,
            &WorkspaceCurrentParams {
                path: inside.to_str().expect("utf-8").to_owned(),
            },
        )
        .await
        .expect("the subdirectory is inside the worktree");
    assert_eq!(found.id, created.id);

    let outside = home.path().join("elsewhere");
    std::fs::create_dir_all(&outside).expect("create directory");
    let refused = client
        .call::<_, Workspace>(
            workspaces::CURRENT,
            &WorkspaceCurrentParams {
                path: outside.to_str().expect("utf-8").to_owned(),
            },
        )
        .await;
    assert_eq!(
        remote_code(&refused.expect_err("outside")),
        codes::WORKSPACE_NOT_FOUND
    );
    stop(running).await;
}
