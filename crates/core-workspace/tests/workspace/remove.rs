//! Removing workspaces: refusals, the two halves of the removal, and resumption.

use std::fs;
use std::path::Path;

use massorca_core_store::{
    WorkspaceKind as StoreWorkspaceKind, WorkspaceRow, WorkspaceState as StoreWorkspaceState,
    new_id, now_ms,
};
use massorca_core_workspace::RemovalWaivers;
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{WorkspaceRemoved, WorkspaceState};

use crate::fixture::{
    Fixture, branch_exists, commit_file, create_params, entries, expect_code, path_key_of, run_git,
    worktree_count,
};

fn state_of(fx: &Fixture, id: &str) -> WorkspaceState {
    fx.workspaces.show_workspace(id).expect("show").state
}

#[test]
fn removing_a_clean_worktree_deletes_its_directory_row_and_created_branch() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "feature");

    let begun = fx
        .workspaces
        .begin_remove("feature", RemovalWaivers::NONE)
        .expect("begin removal");

    assert_eq!(begun.state, WorkspaceState::Removing);
    assert!(Path::new(&ws.path).exists());
    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Removing);

    let removed = fx
        .workspaces
        .finish_remove(&ws.id, RemovalWaivers::NONE)
        .expect("finish removal");

    assert_eq!(removed.workspace.id, ws.id);
    assert!(removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(!Path::new(&ws.path).exists());
    assert!(!branch_exists(&fx.repo(), "feature"));
    assert_eq!(worktree_count(&fx.repo()), 1);
    expect_code(
        fx.workspaces.show_workspace(&ws.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_dirty_worktree_is_refused_and_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "dirty");
    fs::write(Path::new(&ws.path).join("scratch.txt"), "unsaved").expect("write file");

    expect_code(
        fx.workspaces.begin_remove("dirty", RemovalWaivers::NONE),
        codes::WORKSPACE_DIRTY,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(Path::new(&ws.path).join("scratch.txt").exists());
}

#[test]
fn force_removes_a_dirty_worktree_after_the_refusal() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "forced");
    fs::write(Path::new(&ws.path).join("scratch.txt"), "unsaved").expect("write file");
    expect_code(
        fx.workspaces.begin_remove("forced", RemovalWaivers::NONE),
        codes::WORKSPACE_DIRTY,
    );

    let begun = fx
        .workspaces
        .begin_remove("forced", RemovalWaivers::FORCE)
        .expect("begin with force");
    let removed = fx
        .workspaces
        .finish_remove(&begun.id, RemovalWaivers::FORCE)
        .expect("finish with force");

    assert!(removed.branch_deleted);
    assert!(!Path::new(&ws.path).exists());
    assert!(!branch_exists(&fx.repo(), "forced"));
}

#[test]
fn a_locked_worktree_is_refused_even_with_force() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "locked");
    run_git(&fx.repo(), &["worktree", "lock", ws.path.as_str()]);

    expect_code(
        fx.workspaces.begin_remove("locked", RemovalWaivers::FORCE),
        codes::WORKSPACE_LOCKED,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(Path::new(&ws.path).exists());
    run_git(&fx.repo(), &["worktree", "unlock", ws.path.as_str()]);
    fx.remove("locked");
    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn the_main_worktree_is_never_removed() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let row = WorkspaceRow {
        id: new_id(),
        project_id: project.id.clone(),
        kind: StoreWorkspaceKind::Worktree,
        name: "main-row".into(),
        path: project.path.clone(),
        path_key: path_key_of(&fx.repo()),
        branch: Some("main".into()),
        base_ref: None,
        created_branch: false,
        state: StoreWorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
        comment: None,
    };
    fx.store.insert_workspace(&row).expect("insert main row");

    expect_code(
        fx.workspaces
            .begin_remove("main-row", RemovalWaivers::FORCE),
        codes::WORKSPACE_MAIN_WORKTREE,
    );

    assert_eq!(state_of(&fx, &row.id), WorkspaceState::Ready);
    assert!(fx.repo().join("README.md").exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn a_missing_directory_is_removed_from_git_and_the_store() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ghost = fx.create(&project.name, "ghost");
    fs::remove_dir_all(&ghost.path).expect("remove directory");
    assert!(
        fx.workspaces
            .show_workspace(&ghost.id)
            .expect("show")
            .missing
    );

    let begun = fx
        .workspaces
        .begin_remove("ghost", RemovalWaivers::NONE)
        .expect("begin removal of a missing workspace");
    let removed = fx
        .workspaces
        .finish_remove(&begun.id, RemovalWaivers::NONE)
        .expect("finish removal");

    assert!(removed.branch_deleted);
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(!branch_exists(&fx.repo(), "ghost"));
    expect_code(
        fx.workspaces.show_workspace(&ghost.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_missing_directory_is_also_removed_from_git_when_forced() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ghost = fx.create(&project.name, "forced-ghost");
    fs::remove_dir_all(&ghost.path).expect("remove directory");

    let begun = fx
        .workspaces
        .begin_remove("forced-ghost", RemovalWaivers::FORCE)
        .expect("begin with force");
    let removed = fx
        .workspaces
        .finish_remove(&begun.id, RemovalWaivers::FORCE)
        .expect("finish with force");

    assert!(removed.branch_deleted);
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(!branch_exists(&fx.repo(), "forced-ghost"));
}

#[test]
fn a_branch_with_commits_outside_its_base_is_kept_as_not_merged() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "unmerged");
    commit_file(
        Path::new(&ws.path),
        "work.txt",
        "work\n",
        "work on unmerged",
    );

    let removed = fx.remove("unmerged");

    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason.as_deref(), Some("not merged"));
    assert!(branch_exists(&fx.repo(), "unmerged"));
    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn an_existing_branch_is_never_deleted() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "keep-me"]);
    let mut params = create_params(&project.name, "keep-wt");
    params.branch = Some("keep-me".into());
    params.existing_branch = true;
    fx.workspaces
        .create_workspace(params)
        .expect("create on existing branch");

    let removed = fx.remove("keep-wt");

    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(branch_exists(&fx.repo(), "keep-me"));
}

#[test]
fn a_folder_workspace_is_forgotten_and_its_files_stay() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    fx.create(&project.name, "main");

    let removed = fx.remove("main");

    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(removed.directory_kept);
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
    assert_eq!(entries(&fx.path("notes")), ["notes.txt"]);
}

#[test]
fn an_interrupted_removal_resumes_when_removed_again() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "resume");
    let interrupted = fx
        .workspaces
        .begin_remove("resume", RemovalWaivers::NONE)
        .expect("first half");
    assert_eq!(interrupted.state, WorkspaceState::Removing);

    let again = fx
        .workspaces
        .begin_remove("resume", RemovalWaivers::NONE)
        .expect("begin again on a removing workspace");
    assert_eq!(again.state, WorkspaceState::Removing);
    let removed = fx
        .workspaces
        .finish_remove(&ws.id, RemovalWaivers::NONE)
        .expect("finish the interrupted removal");

    assert!(removed.branch_deleted);
    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn finish_alone_marks_and_removes_a_ready_workspace() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "direct");

    let removed = fx
        .workspaces
        .finish_remove(&ws.id, RemovalWaivers::NONE)
        .expect("finish without begin");

    assert!(removed.branch_deleted);
    assert!(!Path::new(&ws.path).exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn dirt_that_appears_after_begin_is_refused_by_finish_and_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "late-dirt");
    let begun = fx
        .workspaces
        .begin_remove("late-dirt", RemovalWaivers::NONE)
        .expect("begin on a clean worktree");
    fs::write(Path::new(&ws.path).join("late.txt"), "typed after begin").expect("write file");

    expect_code(
        fx.workspaces.finish_remove(&begun.id, RemovalWaivers::NONE),
        codes::WORKSPACE_DIRTY,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(Path::new(&ws.path).join("late.txt").exists());
}

#[test]
fn a_refused_retry_of_an_interrupted_removal_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "retry");
    fx.workspaces
        .begin_remove("retry", RemovalWaivers::NONE)
        .expect("first half");
    fs::write(Path::new(&ws.path).join("new.txt"), "dirt").expect("write file");

    expect_code(
        fx.workspaces.begin_remove("retry", RemovalWaivers::NONE),
        codes::WORKSPACE_DIRTY,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
}

#[test]
fn removing_an_unknown_workspace_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.begin_remove("nothing", RemovalWaivers::NONE),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn finishing_an_unknown_workspace_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces
            .finish_remove("00000000-0000-0000-0000-000000000000", RemovalWaivers::NONE),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn an_unregistered_directory_is_forgotten_and_kept_untouched() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let stray = fx.create(&project.name, "stray");
    // Git forgets the worktree but the directory is left behind, with work in it.
    run_git(&fx.repo(), &["worktree", "remove", "--force", &stray.path]);
    fs::create_dir_all(&stray.path).expect("recreate directory");
    fs::write(Path::new(&stray.path).join("notes.txt"), "keep me").expect("write notes");

    let removed = remove_with(&fx, "stray", forget_orphan());

    assert!(removed.directory_kept);
    assert_eq!(entries(Path::new(&stray.path)), ["notes.txt"]);
    expect_code(
        fx.workspaces.show_workspace(&stray.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn an_unregistered_directory_with_changes_is_kept_without_force() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let stray = fx.create(&project.name, "dirty-stray");
    run_git(&fx.repo(), &["worktree", "remove", "--force", &stray.path]);
    fs::create_dir_all(&stray.path).expect("recreate directory");
    fs::write(Path::new(&stray.path).join("untracked.txt"), "work").expect("write file");

    let removed = remove_with(&fx, "dirty-stray", forget_orphan());

    assert!(removed.directory_kept);
    assert!(Path::new(&stray.path).join("untracked.txt").exists());
}

#[test]
fn a_removed_worktree_reports_that_no_directory_was_kept() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "gone-clean");

    let removed = fx.remove("gone-clean");

    assert!(!removed.directory_kept);
}

#[test]
fn ignored_files_refuse_removal_until_their_own_waiver_is_given() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    commit_file(
        &fx.repo(),
        ".gitignore",
        "*.env
",
        "ignore env files",
    );
    let ws = fx.create(&project.name, "ignored");
    let secret = Path::new(&ws.path).join("secrets.env");
    fs::write(&secret, "key").expect("write ignored file");

    expect_code(
        fx.workspaces.begin_remove("ignored", RemovalWaivers::FORCE),
        codes::WORKSPACE_IGNORED_FILES,
    );
    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(secret.is_file(), "a refusal deletes nothing");

    fx.workspaces
        .begin_remove(
            "ignored",
            RemovalWaivers {
                force: true,
                discard_ignored: true,
                forget_orphan: false,
            },
        )
        .expect("begin with both waivers");
    fx.workspaces
        .finish_remove(
            &ws.id,
            RemovalWaivers {
                force: true,
                discard_ignored: true,
                forget_orphan: false,
            },
        )
        .expect("finish with both waivers");
    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn a_check_that_git_cannot_run_returns_the_row_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "broken");
    // A .git file that points nowhere leaves git unable to read the worktree at all.
    fs::write(
        Path::new(&ws.path).join(".git"),
        "gitdir: /no/such/git/dir
",
    )
    .expect("break the .git file");

    let outcome = fx.workspaces.begin_remove("broken", RemovalWaivers::NONE);

    assert!(outcome.is_err(), "git cannot read the worktree");
    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
}

fn forget_orphan() -> RemovalWaivers {
    RemovalWaivers {
        force: false,
        discard_ignored: false,
        forget_orphan: true,
    }
}

fn remove_with(fx: &Fixture, selector: &str, waivers: RemovalWaivers) -> WorkspaceRemoved {
    let begun = fx
        .workspaces
        .begin_remove(selector, waivers)
        .expect("begin removal");
    fx.workspaces
        .finish_remove(&begun.id, waivers)
        .expect("finish removal")
}

#[test]
fn an_orphan_directory_is_refused_even_with_force_and_kept_with_forget_orphan() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let stray = fx.create(&project.name, "orphan");
    run_git(&fx.repo(), &["worktree", "remove", "--force", &stray.path]);
    fs::create_dir_all(&stray.path).expect("recreate directory");
    fs::write(Path::new(&stray.path).join("notes.txt"), "keep me").expect("write notes");

    expect_code(
        fx.workspaces.begin_remove("orphan", RemovalWaivers::FORCE),
        codes::WORKSPACE_ORPHAN,
    );
    assert_eq!(state_of(&fx, &stray.id), WorkspaceState::Ready);

    let removed = remove_with(&fx, "orphan", forget_orphan());
    assert!(removed.directory_kept);
    assert_eq!(entries(Path::new(&stray.path)), ["notes.txt"]);
}

#[test]
fn a_worktree_git_no_longer_recognises_is_an_orphan_and_its_files_are_never_deleted() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let lost = fx.create(&project.name, "lost");
    fs::write(
        Path::new(&lost.path).join(".git"),
        "gitdir: /no/such/git/dir
",
    )
    .expect("break the .git file");
    fs::write(Path::new(&lost.path).join("work.txt"), "keep me").expect("write work");

    expect_code(
        fx.workspaces.begin_remove("lost", RemovalWaivers::FORCE),
        codes::WORKSPACE_ORPHAN,
    );

    let removed = remove_with(&fx, "lost", forget_orphan());
    assert!(removed.directory_kept);
    assert_eq!(
        entries(Path::new(&lost.path)),
        [".git", "README.md", "work.txt"]
    );
    expect_code(
        fx.workspaces.show_workspace(&lost.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_folder_workspace_whose_project_disappears_mid_removal_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let ws = fx.create(&project.name, "main");
    fx.workspaces
        .begin_remove("main", RemovalWaivers::NONE)
        .expect("begin removal");
    fx.delete_project_row(&project.id);

    expect_code(
        fx.workspaces.finish_remove(&ws.id, RemovalWaivers::NONE),
        codes::PROJECT_NOT_FOUND,
    );

    let row = fx
        .store
        .get_workspace(&ws.id)
        .expect("read row")
        .expect("the row is kept");
    assert_eq!(row.state, StoreWorkspaceState::Ready);
}

#[test]
fn a_failure_after_git_removed_the_worktree_keeps_the_row_removing_until_a_retry_finishes() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "half");
    fx.workspaces
        .begin_remove("half", RemovalWaivers::NONE)
        .expect("begin removal");
    fx.fail_workspace_deletes();

    assert!(
        fx.workspaces
            .finish_remove(&ws.id, RemovalWaivers::NONE)
            .is_err()
    );

    assert!(!Path::new(&ws.path).exists(), "git already removed it");
    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Removing);
    fx.allow_workspace_deletes();
    let removed = fx
        .workspaces
        .finish_remove(&ws.id, RemovalWaivers::NONE)
        .expect("a retry finishes the removal");
    assert!(removed.branch_deleted);
    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn an_orphan_keeps_its_ignored_files_when_forgotten() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    commit_file(&fx.repo(), ".gitignore", "*.env\n", "ignore env files");
    let lost = fx.create(&project.name, "lost-ignored");
    fs::write(Path::new(&lost.path).join("secrets.env"), "key").expect("write ignored file");
    fs::write(
        Path::new(&lost.path).join(".git"),
        "gitdir: /no/such/git/dir\n",
    )
    .expect("break the .git file");

    let removed = remove_with(&fx, "lost-ignored", forget_orphan());

    assert!(removed.directory_kept);
    assert!(Path::new(&lost.path).join("secrets.env").is_file());
}

#[test]
fn the_ignored_files_refusal_says_that_files_would_be_deleted_not_entries() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    commit_file(&fx.repo(), ".gitignore", "*.env\n", "ignore env files");
    let ws = fx.create(&project.name, "wording");
    fs::write(Path::new(&ws.path).join("secrets.env"), "key").expect("write ignored file");

    let error = fx
        .workspaces
        .begin_remove("wording", RemovalWaivers::FORCE)
        .expect_err("ignored files refuse removal");

    assert_eq!(error.code(), codes::WORKSPACE_IGNORED_FILES);
    assert!(
        error
            .message()
            .contains("holds ignored files that removal would delete"),
        "{}",
        error.message()
    );
}
