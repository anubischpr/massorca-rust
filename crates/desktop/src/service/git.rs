//! The git calls and watches the bridge runs (decisions 0010, 0012 and 0028). Each call runs on the
//! bridge runtime and answers through its oneshot. A mutation refetches the workspace's git state
//! whatever its answer, so the store follows the repository. A watch subscribes first, reads the
//! state, then refetches on every event, so no change falls between the two.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use massorca_client::{Client, ClientError, Subscription};
use massorca_protocol::codes;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::commit::{GitCommitParams, GitCommitted};
use massorca_protocol::methods::git::diff::{GitDiffFile, GitDiffFileParams};
use massorca_protocol::methods::git::index::{GitDiscardParams, GitDiscarded, GitPathsParams};
use massorca_protocol::methods::git::log::{DEFAULT_LOG_LIMIT, GitLogParams};
use massorca_protocol::methods::git::status::GitStatusParams;
use massorca_protocol::methods::git::watch::{GitWatchEvent, WatchKind};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::{GitUpdate, StoreUpdate};

/// The reason shown when the service does not observe working-tree edits and gives none.
const NO_REASON: &str = "Working-tree edits are not observed.";

/// The reason shown when a watch ends while the link is up, until a new subscription starts.
const STALE_REASON: &str = "Live updates stopped, so the panel may be out of date.";
/// The pause before a lost subscription is started again; each later pause doubles it.
pub(super) const RESTART_BACKOFF: Duration = Duration::from_millis(500);
/// How many times a lost subscription is started again before the panel is left stale.
const RESTART_LIMIT: u32 = 1;

impl ServiceHandle {
    /// `git.diff_file`: the patch of one file, capped by the service (decision 0011).
    pub async fn git_diff_file(&self, params: GitDiffFileParams) -> Result<GitDiffFile, Refusal> {
        self.ask(|reply| Command::GitDiffFile { params, reply })
            .await
    }

    /// `git.stage`: stages the named files, or every change when `all` is set.
    pub async fn git_stage(&self, params: GitPathsParams) -> Result<(), Refusal> {
        self.ask(|reply| Command::GitStage { params, reply }).await
    }

    /// `git.unstage`: unstages the named files, or everything when `all` is set.
    pub async fn git_unstage(&self, params: GitPathsParams) -> Result<(), Refusal> {
        self.ask(|reply| Command::GitUnstage { params, reply })
            .await
    }

    /// `git.discard`. Each waiver sent covers one refusal; a refusal names the files it would affect.
    pub async fn git_discard(&self, params: GitDiscardParams) -> Result<GitDiscarded, Refusal> {
        self.ask(|reply| Command::GitDiscard { params, reply })
            .await
    }

    /// `git.commit`. The status and history change through the refetch that follows, not this answer.
    pub async fn git_commit(&self, params: GitCommitParams) -> Result<GitCommitted, Refusal> {
        self.ask(|reply| Command::GitCommit { params, reply }).await
    }

    /// Follows the git state of `workspace` until `git_unwatch`. The bridge keeps the watch across
    /// reconnects and refetches the state on each change.
    pub fn git_watch(&self, workspace: &str) {
        self.send(Command::GitWatch {
            workspace: workspace.to_owned(),
        });
    }

    /// Stops following `workspace`; its watcher on the service ends with the last subscriber.
    pub fn git_unwatch(&self, workspace: &str) {
        self.send(Command::GitUnwatch {
            workspace: workspace.to_owned(),
        });
    }
}

/// Runs a git command on its own task, so a slow one never holds up events.
pub(super) fn spawn(
    client: &Client,
    command: Command,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    match command {
        Command::GitDiffFile { params, reply } => {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client
                    .git_diff_file(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::GitStage { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                false,
                reply,
                move |client| async move { client.git_stage(&params).await },
            );
        }
        Command::GitUnstage { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                false,
                reply,
                move |client| async move { client.git_unstage(&params).await },
            );
        }
        Command::GitDiscard { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                false,
                reply,
                move |client| async move { client.git_discard(&params).await },
            );
        }
        Command::GitCommit { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                true,
                reply,
                move |client| async move { client.git_commit(&params).await },
            );
        }
        // The session starts and stops watches itself; other commands are not git's.
        _ => {}
    }
}

/// Answers a git command with `refusal`, while there is no connection to ask.
pub(super) fn refuse(command: Command, refusal: Refusal) {
    match command {
        Command::GitDiffFile { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::GitStage { reply, .. } | Command::GitUnstage { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::GitDiscard { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::GitCommit { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        _ => {}
    }
}

/// Runs one mutation, then refetches the workspace's status (and history when `history` is set),
/// whatever the answer was, and only then replies.
fn mutate<T, Fut>(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    workspace: String,
    history: bool,
    reply: oneshot::Sender<Result<T, Refusal>>,
    call: impl FnOnce(Client) -> Fut + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T, ClientError>> + Send + 'static,
{
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let result = call(client.clone()).await;
        // The answer goes first: a slow status read must not keep the caller waiting for it.
        let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        refresh(&client, &updates, &workspace, history).await;
    });
}

/// The watch tasks of one connection, one per followed workspace. Dropping the set aborts them, so
/// they end with the connection that started them.
#[derive(Debug, Default)]
pub(super) struct Watches {
    tasks: HashMap<String, JoinHandle<()>>,
}

impl Watches {
    /// Starts following `workspace` unless its task already runs.
    pub(super) fn start(
        &mut self,
        client: &Client,
        workspace: &str,
        updates: &mpsc::UnboundedSender<StoreUpdate>,
    ) {
        if self
            .tasks
            .get(workspace)
            .is_some_and(|task| !task.is_finished())
        {
            return;
        }
        let task = tokio::spawn(watch(client.clone(), workspace.to_owned(), updates.clone()));
        self.tasks.insert(workspace.to_owned(), task);
    }

    pub(super) fn stop(&mut self, workspace: &str) {
        if let Some(task) = self.tasks.remove(workspace) {
            task.abort();
        }
    }
}

impl Drop for Watches {
    fn drop(&mut self) {
        for (_, task) in self.tasks.drain() {
            task.abort();
        }
    }
}

/// The restart budget of one workspace's watch (decision 0012). A subscription lost while the link is up is
/// started again after a backoff, while the budget lasts. A subscription that delivered its first item earns
/// the budget back.
#[derive(Debug)]
pub(super) struct WatchRestarts {
    left: u32,
}

impl WatchRestarts {
    pub(super) fn new() -> Self {
        Self {
            left: RESTART_LIMIT,
        }
    }

    /// The subscription delivered its first item: a later loss gets a restart again.
    pub(super) fn healthy(&mut self) {
        self.left = RESTART_LIMIT;
    }

    /// A subscription was lost while the link was up. The pause before its restart, or None when the budget
    /// is spent and the panel stays stale.
    pub(super) fn lost(&mut self) -> Option<Duration> {
        if self.left == 0 {
            return None;
        }
        let used = RESTART_LIMIT - self.left;
        self.left -= 1;
        Some(RESTART_BACKOFF * 2u32.pow(used))
    }
}

/// Follows one workspace until its watch ends for good. A subscription lost while the link is up is reported
/// stale, then started again within the restart budget. Lost events (an overflow) are read again at once.
async fn watch(client: Client, workspace: String, updates: mpsc::UnboundedSender<StoreUpdate>) {
    let mut restarts = WatchRestarts::new();
    loop {
        let subscription = client
            .git_watch(&GitWorkspaceRef {
                workspace: workspace.clone(),
            })
            .await;
        refresh(&client, &updates, &workspace, true).await;
        let ending = match subscription {
            Ok(mut events) => follow(&client, &updates, &workspace, &mut events).await,
            Err(_) => Ending::Lost { healthy: false },
        };
        let Ending::Lost { healthy } = ending else {
            continue;
        };
        if healthy {
            restarts.healthy();
        }
        // The panel must not look live once its subscription is gone.
        send(
            &updates,
            GitUpdate::Watch {
                workspace: workspace.clone(),
                degraded: Some(STALE_REASON.to_owned()),
            },
        );
        let Some(backoff) = restarts.lost() else {
            return;
        };
        tokio::time::sleep(backoff).await;
    }
}

/// How a subscription ended.
#[derive(Debug, PartialEq, Eq)]
enum Ending {
    /// Its events were lost: the watch starts again at once, after a full read.
    Overflow,
    /// It ended while the link was up. `healthy`: it had delivered its first item.
    Lost { healthy: bool },
}

/// Serves one subscription until it ends.
async fn follow(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    workspace: &str,
    events: &mut Subscription,
) -> Ending {
    let mut healthy = false;
    loop {
        match events.next_as::<GitWatchEvent>().await {
            Some(Ok(event)) => {
                healthy |= event.ready;
                let history = event
                    .kinds
                    .iter()
                    .any(|kind| matches!(kind, WatchKind::Head | WatchKind::Refs));
                let degraded = event
                    .degraded
                    .then(|| event.reason.unwrap_or_else(|| NO_REASON.to_owned()));
                send(
                    updates,
                    GitUpdate::Watch {
                        workspace: workspace.to_owned(),
                        degraded,
                    },
                );
                if !event.ready {
                    refresh(client, updates, workspace, history).await;
                }
            }
            // An item this build cannot read changes nothing it shows (decision 0002).
            Some(Err(ClientError::Protocol(_))) => {}
            Some(Err(ClientError::Remote(body))) if body.code == codes::STREAM_OVERFLOW => {
                return Ending::Overflow;
            }
            Some(Err(_)) | None => return Ending::Lost { healthy },
        }
    }
}

/// Reads the status, and the first page of history when `history` is set, and sends each answer.
async fn refresh(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    workspace: &str,
    history: bool,
) {
    let status = client
        .git_status(&GitStatusParams {
            workspace: workspace.to_owned(),
            limit: None,
        })
        .await;
    send(
        updates,
        GitUpdate::Status {
            workspace: workspace.to_owned(),
            result: status.map_err(|error| Refusal::from(&error)),
        },
    );
    if history {
        let log = client
            .git_log(&GitLogParams {
                workspace: workspace.to_owned(),
                revision: None,
                limit: Some(DEFAULT_LOG_LIMIT),
                cursor: None,
            })
            .await;
        send(
            updates,
            GitUpdate::Log {
                workspace: workspace.to_owned(),
                result: log.map_err(|error| Refusal::from(&error)),
            },
        );
    }
}

fn send(updates: &mpsc::UnboundedSender<StoreUpdate>, update: GitUpdate) {
    // A closed channel means the UI is gone; the loops notice that and stop.
    let _ = updates.send(StoreUpdate::Git(update));
}
