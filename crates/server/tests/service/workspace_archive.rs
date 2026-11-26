//! Archive scripts on workspace removal (decision 0095): the project's `orca.yaml` archive script runs in the worktree
//! before anything is stopped or removed, only once its content is trusted. A failure refuses the removal unless it is
//! waived; a folder workspace has no archive script.

use std::path::Path;

use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectKind, ProjectScript, ProjectScripts,
    ProjectScriptsParams, ProjectTrustScriptParams, ScriptKind,
};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceArchive, WorkspaceCreateParams, WorkspaceKind, WorkspaceRemoveParams,
    WorkspaceRemoved, WorkspaceState,
};

use super::common::{connect, remote_code, start, stop, temp_home};
use super::git_fixture::git;
use super::workspace_setup::{add_project, shown_script, trust};

/// What the archive script leaves in the project folder.
const ARCHIVED: &str = "archived.txt";

/// Copies the worktree's `keep.txt` to `archived.txt` in the project folder, which proves it ran in the worktree
/// before the worktree was removed.
fn copy_script() -> &'static str {
    if cfg!(windows) {
        "copy keep.txt \"%MASSORCA_ROOT_PATH%\\archived.txt\""
    } else {
        "cp keep.txt \"$MASSORCA_ROOT_PATH/archived.txt\""
    }
}

/// A repository whose committed `orca.yaml` holds `script` as `scripts.archive`, and a committed `keep.txt`.
fn repo_with_archive(dir: &Path, script: &str) {
    std::fs::create_dir_all(dir).expect("create repository dir");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("keep.txt"), "work worth keeping\n").expect("write keep.txt");
    let body: String = script.lines().map(|line| format!("    {line}\n")).collect();
    std::fs::write(
        dir.join("orca.yaml"),
        format!("scripts:\n  archive: |\n{body}"),
    )
    .expect("write orca.yaml");
    git(dir, &["add", "keep.txt", "orca.yaml"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

async fn create(client: &Client, project: &str, name: &str) -> Workspace {
    client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                in_place: false,
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
                name_generated: false,
            },
        )
        .await
        .expect("create")
}

fn remove_params(workspace: &str, allow_failed_archive: bool) -> WorkspaceRemoveParams {
    WorkspaceRemoveParams {
        workspace: workspace.to_owned(),
        force: false,
        discard_ignored: false,
        forget_orphan: false,
        allow_failed_archive,
    }
}

async fn remove(client: &Client, workspace: &str) -> WorkspaceRemoved {
    client
        .call(workspaces::REMOVE, &remove_params(workspace, false))
        .await
        .expect("remove")
}

#[tokio::test]
async fn a_trusted_archive_script_runs_in_the_worktree_before_it_is_removed() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, copy_script());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    trust(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;

    let removed = remove(&client, &created.id).await;
    let Some(WorkspaceArchive::Succeeded { terminal }) = removed.archive.clone() else {
        panic!("the archive did not run: {:?}", removed.archive);
    };
    assert!(!terminal.is_empty());
    // Git may check the worktree's copy out with CRLF line ends.
    let archived = std::fs::read_to_string(repo.join(ARCHIVED)).expect("the archive script ran");
    assert_eq!(archived.replace("\r\n", "\n"), "work worth keeping\n");
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn an_untrusted_archive_script_is_skipped_and_the_removal_goes_ahead() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, copy_script());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    let shown = shown_script(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;

    let removed = remove(&client, &created.id).await;
    let Some(WorkspaceArchive::Untrusted { reason, hash }) = removed.archive.clone() else {
        panic!(
            "an untrusted archive was not skipped: {:?}",
            removed.archive
        );
    };
    assert_eq!(hash, shown.hash);
    assert!(
        reason.contains("massorca project trust-scripts"),
        "{reason}"
    );
    assert!(!repo.join(ARCHIVED).exists());
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn an_archive_script_is_trusted_only_from_the_project_copy_removal_reads() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, copy_script());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    let shown = shown_script(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;

    let copy: ProjectScripts = client
        .call(
            projects::SCRIPTS,
            &ProjectScriptsParams {
                project: project.id.clone(),
                workspace: Some(created.id.clone()),
            },
        )
        .await
        .expect("show the workspace's scripts");
    assert!(
        copy.scripts
            .iter()
            .all(|script| script.kind != ScriptKind::Archive),
        "a workspace's archive copy is shown although removal never runs it: {:?}",
        copy.scripts
    );
    let refused = client
        .call::<_, ProjectScript>(
            projects::TRUST_SCRIPT,
            &ProjectTrustScriptParams {
                project: project.id.clone(),
                kind: ScriptKind::Archive,
                hash: shown.hash.clone(),
                workspace: Some(created.id.clone()),
            },
        )
        .await
        .expect_err("a workspace's archive copy cannot be trusted");
    assert_eq!(remote_code(&refused), codes::REQUEST_INVALID_PARAMS);
    stop(running).await;
}

#[tokio::test]
async fn a_failed_archive_script_refuses_the_removal_unless_it_is_waived() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, "exit 3");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    trust(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;

    let refused = client
        .call::<_, WorkspaceRemoved>(workspaces::REMOVE, &remove_params(&created.id, false))
        .await
        .expect_err("a failed archive refuses the removal");
    assert_eq!(remote_code(&refused), codes::WORKSPACE_ARCHIVE_FAILED);
    let kept: Workspace = client
        .call(
            workspaces::SHOW,
            &workspaces::WorkspaceRef {
                workspace: created.id.clone(),
            },
        )
        .await
        .expect("the workspace is still there");
    assert_eq!(kept.state, WorkspaceState::Ready);
    assert!(Path::new(&created.path).is_dir());

    let removed: WorkspaceRemoved = client
        .call(workspaces::REMOVE, &remove_params(&created.id, true))
        .await
        .expect("a waived failure removes the workspace");
    let Some(WorkspaceArchive::Failed { reason, .. }) = removed.archive.clone() else {
        panic!("the waived failure is not reported: {:?}", removed.archive);
    };
    assert!(reason.contains('3'), "{reason}");
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn a_folder_workspace_has_no_archive_script() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    std::fs::write(folder.join("keep.txt"), "keep\n").expect("write keep.txt");
    std::fs::write(
        folder.join("orca.yaml"),
        format!("scripts:\n  archive: |\n    {}\n", copy_script()),
    )
    .expect("write orca.yaml");
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
    trust(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "main").await;
    assert_eq!(created.kind, WorkspaceKind::Folder);

    let removed = remove(&client, &created.id).await;
    assert_eq!(removed.archive, None);
    assert!(!folder.join(ARCHIVED).exists());
    stop(running).await;
}
