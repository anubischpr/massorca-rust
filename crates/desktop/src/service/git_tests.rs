//! The git state the store folds in: an update lands on a workspace the store holds, a removed
//! workspace takes its git state with it, and a watch says whether its edits are observed.

use std::time::Instant;

use massorca_protocol::methods::git::status::{BranchState, GitStatus, StatusTotals};

use super::fixtures::{project, workspace};
use super::git::{RESTART_BACKOFF, WatchRestarts};
use super::{GitUpdate, Refusal, StoreState, StoreUpdate};

fn status() -> GitStatus {
    GitStatus {
        branch: BranchState::default(),
        operation: None,
        entries: Vec::new(),
        totals: StatusTotals::default(),
        truncated: false,
    }
}

fn store_with_one_workspace() -> StoreState {
    let mut state = StoreState::default();
    state.apply(
        StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: vec![workspace("w1", "p1", "one", 1)],
        },
        Instant::now(),
    );
    state
}

fn git(workspace: &str, update: fn(String) -> GitUpdate) -> StoreUpdate {
    StoreUpdate::Git(update(workspace.to_owned()))
}

#[test]
fn a_status_lands_on_the_workspace_it_names_and_the_history_waits_for_its_own_answer() {
    let mut state = store_with_one_workspace();
    state.apply(
        StoreUpdate::Git(GitUpdate::Status {
            workspace: "w1".into(),
            result: Ok(status()),
        }),
        Instant::now(),
    );
    let git = state.git("w1").expect("the workspace has git state");
    assert_eq!(git.status, Some(Ok(status())));
    assert_eq!(git.log, None, "the history was not read yet");
}

#[test]
fn a_refusal_is_the_answer_the_workspace_keeps() {
    let mut state = store_with_one_workspace();
    let refusal = Refusal::Failed {
        code: "git.not_a_repository".into(),
        message: "This folder is not a git repository.".into(),
    };
    state.apply(
        StoreUpdate::Git(GitUpdate::Status {
            workspace: "w1".into(),
            result: Err(refusal.clone()),
        }),
        Instant::now(),
    );
    assert_eq!(
        state.git("w1").and_then(|git| git.status.clone()),
        Some(Err(refusal))
    );
}

#[test]
fn an_update_for_a_workspace_the_store_does_not_hold_changes_nothing() {
    let mut state = store_with_one_workspace();
    state.apply(
        StoreUpdate::Git(GitUpdate::Log {
            workspace: "ghost".into(),
            result: Err(Refusal::unavailable("gone")),
        }),
        Instant::now(),
    );
    assert!(state.git("ghost").is_none());
}

#[test]
fn a_removed_workspace_takes_its_git_state_with_it() {
    let mut state = store_with_one_workspace();
    state.apply(
        git("w1", |workspace| GitUpdate::Watch {
            workspace,
            degraded: None,
        }),
        Instant::now(),
    );
    assert!(state.git("w1").is_some());
    state.apply(StoreUpdate::WorkspaceGone("w1".into()), Instant::now());
    assert!(state.git("w1").is_none());
}

#[test]
fn a_resync_keeps_only_the_git_state_of_the_workspaces_it_lists() {
    let mut state = store_with_one_workspace();
    state.apply(
        git("w1", |workspace| GitUpdate::Watch {
            workspace,
            degraded: None,
        }),
        Instant::now(),
    );
    state.apply(
        StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: Vec::new(),
        },
        Instant::now(),
    );
    assert!(state.git("w1").is_none());
}

#[test]
fn a_watch_reports_degraded_edits_until_they_are_observed_again() {
    let mut state = store_with_one_workspace();
    let degraded = Some("The file system cannot be watched.".to_owned());
    state.apply(
        StoreUpdate::Git(GitUpdate::Watch {
            workspace: "w1".into(),
            degraded: degraded.clone(),
        }),
        Instant::now(),
    );
    assert_eq!(
        state.git("w1").and_then(|git| git.degraded.clone()),
        degraded
    );
    state.apply(
        StoreUpdate::Git(GitUpdate::Watch {
            workspace: "w1".into(),
            degraded: None,
        }),
        Instant::now(),
    );
    assert_eq!(state.git("w1").and_then(|git| git.degraded.clone()), None);
}

#[test]
fn a_git_update_names_its_workspace_for_the_store() {
    let update = GitUpdate::Watch {
        workspace: "w9".into(),
        degraded: None,
    };
    assert_eq!(update.workspace(), "w9");
}

#[test]
fn a_lost_watch_is_started_once_more_and_then_left_stale() {
    let mut restarts = WatchRestarts::new();
    assert_eq!(restarts.lost(), Some(RESTART_BACKOFF));
    assert_eq!(
        restarts.lost(),
        None,
        "the restart is spent, so the panel stays stale"
    );
}

#[test]
fn a_subscription_that_was_healthy_earns_its_restart_back() {
    let mut restarts = WatchRestarts::new();
    assert!(restarts.lost().is_some());
    restarts.healthy();
    assert!(
        restarts.lost().is_some(),
        "a healthy subscription earns its restart back"
    );
    assert!(restarts.lost().is_none());
}
