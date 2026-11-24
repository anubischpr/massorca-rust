//! Projects over the wire: add, list, show, inspect and remove, their events, their refusals, and the default root read per call.

use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::config::{self, ConfigSet};
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_PROJECT_ADDED, TOPIC_PROJECT_CHANGED, TOPIC_PROJECT_REMOVED,
    TOPIC_WORKSPACE_REMOVED,
};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectIcon, ProjectInspectParams, ProjectInspection,
    ProjectKind, ProjectList, ProjectRef, ProjectRemoveParams, ProjectSetBaseParams,
    ProjectUpdateParams,
};
use massorca_protocol::methods::terminals::{
    self, Terminal, TerminalCreateParams, TerminalKillParams, TerminalList,
};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tokio::time::{Instant, sleep, timeout};

use super::common::{connect, remote_code, shell_with_a_child, start, stop, temp_home};

/// A git repository with one commit at `dir`.
fn git_repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create repository dir");
    run_git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("write readme");
    run_git(dir, &["add", "README.md"]);
    run_git(dir, &["commit", "-q", "-m", "initial"]);
}

fn run_git(dir: &Path, args: &[&str]) {
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

fn add_params(path: &Path) -> ProjectAddParams {
    ProjectAddParams {
        icon: None,
        path: path.to_str().expect("utf-8 path").to_owned(),
        name: None,
        kind: None,
        workspaces_root: None,
    }
}

#[tokio::test]
async fn adding_a_repository_registers_it_as_a_git_project_and_publishes_project_added() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.expect("subscribe");

    let project: Project = client
        .call(projects::ADD, &add_params(&repo))
        .await
        .expect("project.add");

    assert_eq!(project.kind, ProjectKind::Git);
    assert_eq!(project.name, "repo");
    let event = timeout(super::common::REPLY_WAIT, events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes");
    assert_eq!(
        event,
        ChangeEvent {
            topic: TOPIC_PROJECT_ADDED.to_owned(),
            id: project.id.clone()
        }
    );
    stop(running).await;
}

#[tokio::test]
async fn list_and_show_answer_what_was_added_and_remove_publishes_project_removed() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.expect("subscribe");
    let mut params = add_params(&folder);
    params.kind = Some(ProjectKind::Folder);
    let added: Project = client.call(projects::ADD, &params).await.expect("add");
    let _ = events.next_as::<ChangeEvent>().await;

    let list: ProjectList = client
        .call(projects::LIST, &Value::Null)
        .await
        .expect("list");
    assert_eq!(list.projects, vec![added.clone()]);
    let shown: Project = client
        .call(
            projects::SHOW,
            &ProjectRef {
                project: "notes".to_owned(),
            },
        )
        .await
        .expect("show by name");
    assert_eq!(shown, added);

    let removed: Project = client
        .call(
            projects::REMOVE,
            &ProjectRemoveParams {
                project: added.id.clone(),
                forget_workspaces: false,
            },
        )
        .await
        .expect("remove by id");
    assert_eq!(removed.id, added.id);
    let event = timeout(super::common::REPLY_WAIT, events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes");
    assert_eq!(event.topic, TOPIC_PROJECT_REMOVED);
    assert!(
        folder.is_dir(),
        "removing a project never deletes its files"
    );
    stop(running).await;
}

#[tokio::test]
async fn unknown_and_invalid_project_requests_answer_typed_codes() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let missing = client
        .call::<_, Project>(
            projects::SHOW,
            &ProjectRef {
                project: "nothing".to_owned(),
            },
        )
        .await
        .expect_err("unknown project");
    assert_eq!(remote_code(&missing), codes::PROJECT_NOT_FOUND);

    let relative = client
        .call::<_, Project>(projects::ADD, &add_params(Path::new("relative/dir")))
        .await
        .expect_err("relative path");
    assert_eq!(remote_code(&relative), codes::REQUEST_INVALID_PARAMS);
    stop(running).await;
}

fn inspect_params(path: &Path) -> ProjectInspectParams {
    ProjectInspectParams {
        path: path.to_str().expect("utf-8 path").to_owned(),
    }
}

fn canonical_text(path: &Path) -> String {
    dunce::canonicalize(path)
        .expect("canonical path")
        .to_str()
        .expect("utf-8 path")
        .to_owned()
}

#[tokio::test]
async fn inspect_answers_what_add_without_a_kind_registers_and_who_is_registered() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let nested = repo.join("src");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let notes = home.path().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let git: ProjectInspection = client
        .call(projects::INSPECT, &inspect_params(&nested))
        .await
        .expect("project.inspect");
    assert_eq!(git.kind, ProjectKind::Git);
    assert_eq!(git.path, canonical_text(&repo));
    assert_eq!(git.registered, None);

    let folder: ProjectInspection = client
        .call(projects::INSPECT, &inspect_params(&notes))
        .await
        .expect("project.inspect");
    assert_eq!(folder.kind, ProjectKind::Folder);
    assert_eq!(folder.path, canonical_text(&notes));
    assert_eq!(folder.registered, None);

    for (dir, inspected) in [(&nested, &git), (&notes, &folder)] {
        let project: Project = client
            .call(projects::ADD, &add_params(dir))
            .await
            .expect("project.add");
        assert_eq!(project.kind, inspected.kind);
        assert_eq!(project.path, inspected.path);
        let again: ProjectInspection = client
            .call(projects::INSPECT, &inspect_params(dir))
            .await
            .expect("project.inspect");
        assert_eq!(again.registered, Some(project.id));
    }

    let missing = home.path().join("missing");
    let inspect_error = client
        .call::<_, ProjectInspection>(projects::INSPECT, &inspect_params(&missing))
        .await
        .expect_err("missing path");
    let add_error = client
        .call::<_, Project>(projects::ADD, &add_params(&missing))
        .await
        .expect_err("missing path");
    assert_eq!(remote_code(&inspect_error), codes::PATH_NOT_FOUND);
    assert_eq!(remote_code(&inspect_error), remote_code(&add_error));
    stop(running).await;
}

#[tokio::test]
async fn set_base_publishes_project_changed_and_show_reports_the_base_until_it_is_cleared() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    run_git(&repo, &["branch", "develop"]);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project: Project = client
        .call(projects::ADD, &add_params(&repo))
        .await
        .expect("add");
    let mut events = client.subscribe_events().await.expect("subscribe");

    let set: Project = client
        .call(
            projects::SET_BASE,
            &ProjectSetBaseParams {
                project: project.id.clone(),
                base: Some("develop".to_owned()),
            },
        )
        .await
        .expect("project.set_base");

    assert_eq!(set.base_ref.as_deref(), Some("develop"));
    let event = timeout(super::common::REPLY_WAIT, events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes");
    assert_eq!(
        event,
        ChangeEvent {
            topic: TOPIC_PROJECT_CHANGED.to_owned(),
            id: project.id.clone()
        }
    );
    let shown: Project = client
        .call(
            projects::SHOW,
            &ProjectRef {
                project: "repo".to_owned(),
            },
        )
        .await
        .expect("show");
    assert_eq!(shown.base_ref.as_deref(), Some("develop"));

    let cleared: Project = client
        .call(
            projects::SET_BASE,
            &ProjectSetBaseParams {
                project: project.id.clone(),
                base: None,
            },
        )
        .await
        .expect("clear the base");
    assert_eq!(cleared.base_ref, None);
    stop(running).await;
}

#[tokio::test]
async fn set_base_refuses_a_folder_project_and_a_base_that_is_not_a_commit() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let repo_project: Project = client
        .call(projects::ADD, &add_params(&repo))
        .await
        .expect("add repository");
    let mut folder_params = add_params(&folder);
    folder_params.kind = Some(ProjectKind::Folder);
    let folder_project: Project = client
        .call(projects::ADD, &folder_params)
        .await
        .expect("add folder");

    let on_folder = client
        .call::<_, Project>(
            projects::SET_BASE,
            &ProjectSetBaseParams {
                project: folder_project.id.clone(),
                base: Some("main".to_owned()),
            },
        )
        .await
        .expect_err("a folder has no base");
    assert_eq!(remote_code(&on_folder), codes::REQUEST_INVALID_PARAMS);

    let unresolved = client
        .call::<_, Project>(
            projects::SET_BASE,
            &ProjectSetBaseParams {
                project: repo_project.id.clone(),
                base: Some("no-such-branch".to_owned()),
            },
        )
        .await
        .expect_err("a base that is not a commit");
    assert_eq!(remote_code(&unresolved), codes::BASE_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn a_new_default_root_applies_to_the_next_project_without_restarting() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let elsewhere: PathBuf = home.path().join("elsewhere");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Value>(
            config::SET,
            &ConfigSet {
                owner: None,
                key: "workspaces.default_root".to_owned(),
                value: json!(elsewhere.to_str().expect("utf-8")),
            },
        )
        .await
        .expect("config.set");

    let project: Project = client
        .call(projects::ADD, &add_params(&repo))
        .await
        .expect("add");

    assert_eq!(
        Path::new(&project.workspaces_root),
        elsewhere.join("repo").as_path()
    );
    stop(running).await;
}

#[tokio::test]
async fn a_flat_layout_puts_the_next_projects_workspaces_in_the_default_root() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let elsewhere: PathBuf = home.path().join("elsewhere");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    for (key, value) in [
        (
            "workspaces.default_root",
            json!(elsewhere.to_str().expect("utf-8")),
        ),
        ("workspaces.layout", json!("flat")),
    ] {
        client
            .call::<_, Value>(
                config::SET,
                &ConfigSet {
                    owner: None,
                    key: key.to_owned(),
                    value,
                },
            )
            .await
            .expect("config.set");
    }

    let project: Project = client
        .call(projects::ADD, &add_params(&repo))
        .await
        .expect("add");

    assert_eq!(Path::new(&project.workspaces_root), elsewhere.as_path());
    stop(running).await;
}

#[tokio::test]
async fn adding_a_linked_worktree_registers_its_main_repository() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let linked = home.path().join("linked-wt");
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked-wt",
            linked.to_str().expect("utf-8 path"),
        ],
    );
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let project: Project = client
        .call(projects::ADD, &add_params(&linked))
        .await
        .expect("project.add");

    assert_eq!(project.kind, ProjectKind::Git);
    assert_eq!(project.name, "repo");
    assert_eq!(
        project.path,
        dunce::canonicalize(&repo)
            .expect("canonical repo")
            .to_str()
            .expect("utf-8 path")
    );
    stop(running).await;
}

async fn next_change(events: &mut Subscription) -> ChangeEvent {
    timeout(Duration::from_secs(10), events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes")
}

async fn next_change_on(events: &mut Subscription, topic: &str, id: &str) {
    loop {
        let event = next_change(events).await;
        if event.topic == topic && event.id == id {
            return;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn forgetting_a_project_is_refused_while_a_terminal_runs_in_it_and_publishes_its_removals() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.expect("subscribe");
    let mut params = add_params(&folder);
    params.kind = Some(ProjectKind::Folder);
    let project: Project = client.call(projects::ADD, &params).await.expect("add");
    let workspace: Workspace = client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project: project.id.clone(),
                name: "main".to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
                no_setup: false,
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
    let forget = ProjectRemoveParams {
        project: project.id.clone(),
        forget_workspaces: true,
    };

    let busy = client
        .call::<_, Project>(projects::REMOVE, &forget)
        .await
        .expect_err("a live terminal in a workspace");
    assert_eq!(remote_code(&busy), codes::WORKSPACE_BUSY);
    let still: Project = client
        .call(
            projects::SHOW,
            &ProjectRef {
                project: project.id.clone(),
            },
        )
        .await
        .expect("the project is still registered");
    assert_eq!(still.id, project.id);

    client
        .call::<_, Value>(
            terminals::KILL,
            &TerminalKillParams {
                terminal: terminal.id.clone(),
                force: true,
            },
        )
        .await
        .expect("kill the terminal");
    wait_until_exited(&client, &terminal.id).await;

    let removed: Project = client
        .call(projects::REMOVE, &forget)
        .await
        .expect("forget once the terminal has ended");
    assert_eq!(removed.id, project.id);
    next_change_on(&mut events, TOPIC_PROJECT_REMOVED, &project.id).await;
    next_change_on(&mut events, TOPIC_WORKSPACE_REMOVED, &workspace.id).await;
    assert!(
        folder.is_dir(),
        "forgetting a project never deletes its files"
    );
    stop(running).await;
}

async fn wait_until_exited(client: &massorca_client::Client, terminal: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let list: TerminalList = client
            .call(terminals::LIST, &json!({}))
            .await
            .expect("list terminals");
        if list
            .terminals
            .iter()
            .any(|item| item.id == terminal && item.exit.is_some())
        {
            return;
        }
        assert!(Instant::now() < deadline, "the terminal did not exit");
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn project_update_sets_keeps_and_clears_the_icon_and_publishes_project_changed() {
    let home = temp_home();
    let repo = home.path().join("repo");
    git_repo(&repo);
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project: Project = client
        .call(projects::ADD, &add_params(&repo))
        .await
        .expect("project.add");
    let mut events = client.subscribe_events().await.expect("subscribe");
    let icon = ProjectIcon::Emoji {
        emoji: "🚀".to_owned(),
    };
    let update = |icon| ProjectUpdateParams {
        project: project.id.clone(),
        icon,
    };

    let set: Project = client
        .call(projects::UPDATE, &update(Some(Some(icon.clone()))))
        .await
        .expect("project.update sets the icon");
    assert_eq!(set.icon, Some(icon.clone()));
    let event = timeout(Duration::from_secs(5), events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes");
    assert_eq!(
        event,
        ChangeEvent {
            topic: TOPIC_PROJECT_CHANGED.to_owned(),
            id: project.id.clone()
        }
    );

    let kept: Project = client
        .call(projects::UPDATE, &update(None))
        .await
        .expect("an absent icon keeps it");
    assert_eq!(kept.icon, Some(icon));

    let refused = client
        .call::<_, Project>(
            projects::UPDATE,
            &update(Some(Some(ProjectIcon::Emoji {
                emoji: "   ".to_owned(),
            }))),
        )
        .await;
    assert_eq!(
        remote_code(&refused.expect_err("an empty emoji is refused")),
        codes::REQUEST_INVALID_PARAMS
    );

    let cleared: Project = client
        .call(projects::UPDATE, &update(Some(None)))
        .await
        .expect("a null icon clears it");
    assert_eq!(cleared.icon, None);
    let shown: Project = client
        .call(
            projects::SHOW,
            &ProjectRef {
                project: project.id.clone(),
            },
        )
        .await
        .expect("project.show");
    assert_eq!(shown.icon, None);
    stop(running).await;
}
