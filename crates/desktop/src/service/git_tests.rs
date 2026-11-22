//! The git state the store folds in: an update lands on a workspace the store holds, a removed
//! workspace takes its git state with it, and a watch says whether its edits are observed.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use massorca_client::ClientError;
use massorca_protocol::methods::git::log::GitLog;
use massorca_protocol::methods::git::status::{BranchState, GitStatus, StatusEntry, StatusTotals};
use massorca_protocol::methods::git::watch::{GitWatchEvent, WatchKind};
use tokio::sync::Notify;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::fixtures::{project, workspace};
use super::git::{Freshness, GitEvents, GitPeer, RESTART_BACKOFF, WatchRestarts, watch};
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

/// A service on a script: the watch hands over the items the test sends, and its first status read is held
/// until the test releases it, as a git call is under load. Later reads answer at once.
/// The items a scripted watch yields, in order.
type Items = UnboundedReceiver<Result<GitWatchEvent, ClientError>>;

#[derive(Clone)]
struct ScriptedPeer {
    items: Arc<Mutex<Option<Items>>>,
    gate: Arc<Notify>,
    reads: Arc<AtomicUsize>,
    answered: Arc<AtomicUsize>,
    first_answered: Arc<AtomicBool>,
}

impl ScriptedPeer {
    fn new(items: Items) -> Self {
        Self {
            items: Arc::new(Mutex::new(Some(items))),
            gate: Arc::new(Notify::new()),
            reads: Arc::new(AtomicUsize::new(0)),
            answered: Arc::new(AtomicUsize::new(0)),
            first_answered: Arc::new(AtomicBool::new(false)),
        }
    }

    fn release_first_status(&self) {
        self.gate.notify_one();
    }

    fn answered(&self) -> usize {
        self.answered.load(Ordering::SeqCst)
    }

    fn first_answered(&self) -> bool {
        self.first_answered.load(Ordering::SeqCst)
    }
}

struct ScriptedEvents(Items);

impl GitEvents for ScriptedEvents {
    async fn next_event(&mut self) -> Option<Result<GitWatchEvent, ClientError>> {
        self.0.recv().await
    }
}

impl GitPeer for ScriptedPeer {
    type Events = ScriptedEvents;

    async fn subscribe(&self, _workspace: &str) -> Result<ScriptedEvents, ClientError> {
        let items = self
            .items
            .lock()
            .expect("the script lock is not poisoned")
            .take()
            .expect("the test subscribes once");
        Ok(ScriptedEvents(items))
    }

    async fn status(&self, _workspace: &str) -> Result<GitStatus, ClientError> {
        let read = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if read == 1 {
            self.gate.notified().await;
        }
        if read == 1 {
            self.first_answered.store(true, Ordering::SeqCst);
        }
        self.answered.fetch_add(1, Ordering::SeqCst);
        Ok(if read == 1 {
            status()
        } else {
            GitStatus {
                entries: vec![StatusEntry {
                    path: "notes.txt".into(),
                    original_path: None,
                    staged: None,
                    unstaged: None,
                    untracked: true,
                    conflict: None,
                    staged_lines: None,
                    unstaged_lines: None,
                    submodule: false,
                    path_lossy: false,
                }],
                ..status()
            }
        })
    }

    async fn log(&self, _workspace: &str) -> Result<GitLog, ClientError> {
        Ok(GitLog {
            commits: Vec::new(),
            next_cursor: None,
        })
    }
}

fn ready_item() -> Result<GitWatchEvent, ClientError> {
    Ok(GitWatchEvent {
        ready: true,
        kinds: Vec::new(),
        degraded: false,
        reason: None,
    })
}

fn change_item() -> Result<GitWatchEvent, ClientError> {
    Ok(GitWatchEvent {
        ready: false,
        kinds: vec![WatchKind::Worktree],
        degraded: false,
        reason: None,
    })
}

fn holds_notes(state: &StoreState) -> bool {
    matches!(
        state.git("w1").and_then(|git| git.status.as_ref()),
        Some(Ok(status)) if status.entries.iter().any(|entry| entry.path == "notes.txt")
    )
}

/// Folds the updates until `done` holds, for at most five seconds, and returns whether it did. `watch_items`
/// counts the watch items folded so far.
async fn fold_within(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    watch_items: &mut usize,
    done: impl Fn(&StoreState, usize) -> bool,
) -> bool {
    let started = Instant::now();
    while !done(state, *watch_items) {
        let remaining = Duration::from_secs(5).saturating_sub(started.elapsed());
        let Ok(Some(update)) = tokio::time::timeout(remaining, updates.recv()).await else {
            return false;
        };
        if matches!(update, StoreUpdate::Git(GitUpdate::Watch { .. })) {
            *watch_items += 1;
        }
        state.apply(update, Instant::now());
    }
    true
}

#[tokio::test]
async fn a_slow_first_status_holds_back_neither_the_ready_item_nor_a_change() {
    let (items, script) = mpsc::unbounded_channel();
    let peer = ScriptedPeer::new(script);
    let (updates_tx, mut updates) = mpsc::unbounded_channel();
    let watching = tokio::spawn(watch(
        peer.clone(),
        "w1".to_owned(),
        updates_tx,
        Arc::new(Freshness::default()),
    ));
    let mut state = store_with_one_workspace();
    let mut watch_items = 0;

    // The ready item and then an edit are sent while the first status is still held.
    items.send(ready_item()).expect("the watch listens");
    items.send(change_item()).expect("the watch listens");
    assert!(
        fold_within(
            &mut updates,
            &mut state,
            &mut watch_items,
            |state, items| { items >= 2 && holds_notes(state) }
        )
        .await,
        "the ready item and the edit must reach the store before the slow first status returns \
         (watch items folded: {watch_items}, answered reads: {}, status: {:?})",
        peer.answered(),
        state.git("w1").and_then(|git| git.status.clone()),
    );
    assert!(
        !peer.first_answered(),
        "the first status is still held while they arrive"
    );

    // The held status returns last, carrying the older answer. The store must keep the newest.
    peer.release_first_status();
    let settled = tokio::time::timeout(Duration::from_secs(5), async {
        while peer.answered() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(settled.is_ok(), "both status reads answer");
    tokio::time::sleep(Duration::from_millis(100)).await;
    while let Ok(update) = updates.try_recv() {
        state.apply(update, Instant::now());
    }
    assert!(
        holds_notes(&state),
        "the newest status is kept, got {:?}",
        state.git("w1").and_then(|git| git.status.clone())
    );
    watching.abort();
}
