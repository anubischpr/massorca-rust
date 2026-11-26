//! Archive scripts on workspace removal (decision 0095): the project's `orca.yaml` archive script runs in the worktree
//! before anything is stopped or removed, only once its content is trusted. A failure refuses the removal unless it is
//! waived; a folder workspace has no archive script.

use std::path::Path;

use massorca_client::Client;
use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, RunMode, Store, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    self, AgentForgetParams, AgentForgotten, AgentSession, AgentSessionRef,
};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectKind, ProjectScript, ProjectScripts,
    ProjectScriptsParams, ProjectTrustScriptParams, ScriptKind,
};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceArchive, WorkspaceCreateParams, WorkspaceKind, WorkspaceRemoveParams,
    WorkspaceRemoved, WorkspaceState,
};

use super::common::{connect, profile, remote_code, start, stop, temp_home};
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
                project: project.to_owned(),
                name: name.to_owned(),
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

/// Writes an exited agent session in workspace `workspace` straight to the store, as the agent tests do (no agent starts).
fn exited_session(home: &Path, workspace: &str) -> String {
    let store = Store::open(profile(home, "default").store_path()).expect("the store");
    let row = AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace.to_owned(),
        parent_id: None,
        title: None,
        read_only: false,
        agent_ref: None,
        terminal_id: None,
        request_id: None,
        created_at: now_ms(),
        exit: Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: now_ms(),
        }),
        activity: None,
        result: None,
        brought_back_at: None,
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
        retained: false,
        released_at: None,
    };
    store.insert_agent_session(&row).expect("the session");
    row.id
}

fn forget_params(session: &str) -> AgentForgetParams {
    AgentForgetParams {
        session: session.to_owned(),
        remove_workspace: true,
        force: false,
        discard_ignored: false,
        expect_dirty: None,
        allow_failed_archive: false,
    }
}

#[tokio::test]
async fn agent_forget_with_failed_archive_script_and_the_waiver_removes_the_workspace() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, "exit 3");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    trust(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;
    let session = exited_session(home.path(), &created.id);

    let forgotten: AgentForgotten = client
        .call(
            agents::FORGET,
            &AgentForgetParams {
                allow_failed_archive: true,
                ..forget_params(&session)
            },
        )
        .await
        .expect("agent.forget with the waiver");
    assert_eq!(forgotten.workspace_error, None);
    let removed = forgotten
        .workspace_removed
        .expect("the waived failure removes the workspace");
    assert!(matches!(
        removed.archive,
        Some(WorkspaceArchive::Failed { .. })
    ));
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn agent_forget_with_remove_workspace_runs_a_trusted_archive_script_before_removal() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, copy_script());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    trust(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;
    let session = exited_session(home.path(), &created.id);

    let forgotten: AgentForgotten = client
        .call(agents::FORGET, &forget_params(&session))
        .await
        .expect("agent.forget");
    assert_eq!(forgotten.session.id, session);
    assert_eq!(forgotten.workspace_error, None);
    let removed = forgotten
        .workspace_removed
        .expect("the workspace was removed");
    let Some(WorkspaceArchive::Succeeded { .. }) = removed.archive else {
        panic!("the archive did not run: {:?}", removed.archive);
    };
    // The script copied the worktree's file, so it ran in the worktree before the worktree was removed.
    let archived = std::fs::read_to_string(repo.join(ARCHIVED)).expect("the archive script ran");
    // Git may check the worktree's copy out with CRLF line ends.
    assert_eq!(archived.replace("\r\n", "\n"), "work worth keeping\n");
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}

#[tokio::test]
async fn agent_forget_with_untrusted_archive_script_skips_it_and_removes_the_workspace() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, copy_script());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    let shown = shown_script(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;
    let session = exited_session(home.path(), &created.id);

    let forgotten: AgentForgotten = client
        .call(agents::FORGET, &forget_params(&session))
        .await
        .expect("agent.forget");
    let removed = forgotten
        .workspace_removed
        .expect("the workspace was removed");
    let Some(WorkspaceArchive::Untrusted { reason, hash }) = removed.archive else {
        panic!("an untrusted archive was not skipped");
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
async fn agent_forget_with_failed_archive_script_forgets_the_session_and_keeps_the_workspace() {
    let home = temp_home();
    let repo = home.path().join("repo");
    repo_with_archive(&repo, "exit 3");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let project = add_project(&client, home.path(), &repo).await;
    trust(&client, &project.id, ScriptKind::Archive).await;
    let created = create(&client, &project.id, "feature").await;
    let session = exited_session(home.path(), &created.id);

    let forgotten: AgentForgotten = client
        .call(agents::FORGET, &forget_params(&session))
        .await
        .expect("the forget itself goes ahead");
    assert_eq!(forgotten.workspace_removed, None);
    let refusal = forgotten
        .workspace_error
        .expect("the refused archive is reported after the forget");
    assert_eq!(
        refusal.code,
        codes::WORKSPACE_ARCHIVE_FAILED,
        "{}",
        refusal.message
    );
    let gone = client
        .call::<_, AgentSession>(
            agents::SHOW,
            &AgentSessionRef {
                session: session.clone(),
            },
        )
        .await
        .expect_err("the session record is forgotten");
    assert_eq!(remote_code(&gone), codes::AGENT_NOT_FOUND);
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
        .expect("workspace.remove still removes it with the waiver");
    assert!(matches!(
        removed.archive,
        Some(WorkspaceArchive::Failed { .. })
    ));
    assert!(!Path::new(&created.path).exists());
    stop(running).await;
}
