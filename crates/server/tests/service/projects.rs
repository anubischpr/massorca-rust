//! Projects over the wire: add, list, show and remove, their events, their refusals, and the default root read per call.

use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_protocol::codes;
use massorca_protocol::methods::config::{self, ConfigSet};
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_PROJECT_ADDED, TOPIC_PROJECT_REMOVED};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectKind, ProjectList, ProjectRef,
};
use serde_json::{Value, json};
use tokio::time::timeout;

use super::common::{connect, remote_code, start, stop, temp_home};

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
    let event = timeout(Duration::from_secs(5), events.next_as::<ChangeEvent>())
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
            &ProjectRef {
                project: added.id.clone(),
            },
        )
        .await
        .expect("remove by id");
    assert_eq!(removed.id, added.id);
    let event = timeout(Duration::from_secs(5), events.next_as::<ChangeEvent>())
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
