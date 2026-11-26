//! The git calls and watches the bridge runs (decisions 0010, 0012 and 0028). Each call runs on the
//! bridge runtime and answers through its oneshot. A mutation refetches the workspace's git state
//! whatever its answer, so the store follows the repository. A watch subscribes first, folds its items as
//! they come, and reads the state beside the stream, refetching on every event. Answers are ordered, so an
//! older read never replaces a newer one.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use massorca_client::{Client, ClientError, Subscription};
use massorca_protocol::codes;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::commit::{GitCommitParams, GitCommitted};
use massorca_protocol::methods::git::diff::{GitDiffFile, GitDiffFileParams};
use massorca_protocol::methods::git::index::{GitDiscardParams, GitDiscarded, GitPathsParams};
use massorca_protocol::methods::git::log::{DEFAULT_LOG_LIMIT, GitLog, GitLogParams};
use massorca_protocol::methods::git::remote::{
    GitFetchParams, GitFetched, GitPullParams, GitPulled, GitPushParams, GitPushed,
};
use massorca_protocol::methods::git::status::{GitStatus, GitStatusParams};
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

    /// `git.fetch`: fetches the named remote, the default one, or every remote when `all` is set.
    pub async fn git_fetch(&self, params: GitFetchParams) -> Result<GitFetched, Refusal> {
        self.ask(|reply| Command::GitFetch { params, reply }).await
    }

    /// `git.pull`: brings the branch up to its upstream with the strategy the params name.
    pub async fn git_pull(&self, params: GitPullParams) -> Result<GitPulled, Refusal> {
        self.ask(|reply| Command::GitPull { params, reply }).await
    }

    /// `git.push`: pushes the current branch to its upstream, creating the upstream on the remote if needed.
    pub async fn git_push(&self, params: GitPushParams) -> Result<GitPushed, Refusal> {
        self.ask(|reply| Command::GitPush { params, reply }).await
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
    freshness: &Arc<Freshness>,
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
                freshness,
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
                freshness,
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
                freshness,
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
                freshness,
                true,
                reply,
                move |client| async move { client.git_commit(&params).await },
            );
        }
        Command::GitFetch { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                freshness,
                true,
                reply,
                move |client| async move { client.git_fetch(&params).await },
            );
        }
        Command::GitPull { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                freshness,
                true,
                reply,
                move |client| async move { client.git_pull(&params).await },
            );
        }
        Command::GitPush { params, reply } => {
            let workspace = params.workspace.clone();
            mutate(
                client,
                updates,
                workspace,
                freshness,
                true,
                reply,
                move |client| async move { client.git_push(&params).await },
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
        Command::GitFetch { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::GitPull { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::GitPush { reply, .. } => {
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
    freshness: &Arc<Freshness>,
    history: bool,
    reply: oneshot::Sender<Result<T, Refusal>>,
    call: impl FnOnce(Client) -> Fut + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T, ClientError>> + Send + 'static,
{
    let client = client.clone();
    let updates = updates.clone();
    let freshness = freshness.clone();
    tokio::spawn(async move {
        let result = call(client.clone()).await;
        // The answer goes first: a slow status read must not keep the caller waiting for it.
        let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        let numbers = freshness.request(history);
        refresh(&client, &updates, &freshness, &workspace, numbers, history).await;
    });
}

/// The service calls a watch and its reads make. The client is the real one; a test scripts the service.
pub(super) trait GitPeer: Clone + Send + Sync + 'static {
    type Events: GitEvents;

    fn subscribe(
        &self,
        workspace: &str,
    ) -> impl Future<Output = Result<Self::Events, ClientError>> + Send;
    fn status(
        &self,
        workspace: &str,
    ) -> impl Future<Output = Result<GitStatus, ClientError>> + Send;
    fn log(&self, workspace: &str) -> impl Future<Output = Result<GitLog, ClientError>> + Send;
}

/// The items of one watch, in order; None once it has ended.
pub(super) trait GitEvents: Send + 'static {
    fn next_event(
        &mut self,
    ) -> impl Future<Output = Option<Result<GitWatchEvent, ClientError>>> + Send;
}

impl GitPeer for Client {
    type Events = Subscription;

    fn subscribe(
        &self,
        workspace: &str,
    ) -> impl Future<Output = Result<Subscription, ClientError>> + Send {
        let client = self.clone();
        let workspace = workspace.to_owned();
        async move { client.git_watch(&GitWorkspaceRef { workspace }).await }
    }

    fn status(
        &self,
        workspace: &str,
    ) -> impl Future<Output = Result<GitStatus, ClientError>> + Send {
        let client = self.clone();
        let workspace = workspace.to_owned();
        async move {
            client
                .git_status(&GitStatusParams {
                    workspace,
                    limit: None,
                })
                .await
        }
    }

    fn log(&self, workspace: &str) -> impl Future<Output = Result<GitLog, ClientError>> + Send {
        let client = self.clone();
        let workspace = workspace.to_owned();
        async move {
            client
                .git_log(&GitLogParams {
                    workspace,
                    revision: None,
                    limit: Some(DEFAULT_LOG_LIMIT),
                    cursor: None,
                })
                .await
        }
    }
}

impl GitEvents for Subscription {
    fn next_event(
        &mut self,
    ) -> impl Future<Output = Option<Result<GitWatchEvent, ClientError>>> + Send {
        self.next_as::<GitWatchEvent>()
    }
}

/// The numbers of one refresh's reads, taken when the refresh is requested.
#[derive(Clone, Copy, Debug)]
pub(super) struct Numbers {
    status: u64,
    log: u64,
}

/// Which read an answer is.
#[derive(Clone, Copy)]
enum Read {
    Status,
    Log,
}

/// Orders the git answers of one bridge. Each read takes a number when it is requested, and its answer
/// reaches the store only if no newer read of the same kind of the same workspace has reached it already.
/// So an older answer never replaces a newer one, whichever task asked for it, and the store ends on the
/// newest state. The bridge keeps one for its whole life, so the numbers stay ordered across reconnects.
#[derive(Debug, Default)]
pub(super) struct Freshness {
    requested: AtomicU64,
    applied: Mutex<HashMap<String, Applied>>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Applied {
    status: u64,
    log: u64,
}

impl Freshness {
    /// Numbers the reads of one refresh; the history read is numbered only when `history` is set.
    pub(super) fn request(&self, history: bool) -> Numbers {
        let status = self.requested.fetch_add(1, Ordering::SeqCst) + 1;
        let log = if history {
            self.requested.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            0
        };
        Numbers { status, log }
    }

    /// Sends `update` unless a newer read of `read` for `workspace` has been sent already. The check and the
    /// send share the lock, so the store takes the answers in the order they were accepted.
    fn deliver(
        &self,
        workspace: &str,
        read: Read,
        number: u64,
        updates: &mpsc::UnboundedSender<StoreUpdate>,
        update: GitUpdate,
    ) {
        let mut applied = self.applied.lock().unwrap_or_else(PoisonError::into_inner);
        let last = applied.entry(workspace.to_owned()).or_default();
        let last = match read {
            Read::Status => &mut last.status,
            Read::Log => &mut last.log,
        };
        if number <= *last {
            return;
        }
        *last = number;
        send(updates, update);
    }
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
        freshness: &Arc<Freshness>,
    ) {
        if self
            .tasks
            .get(workspace)
            .is_some_and(|task| !task.is_finished())
        {
            return;
        }
        let task = tokio::spawn(watch(
            client.clone(),
            workspace.to_owned(),
            updates.clone(),
            freshness.clone(),
        ));
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
pub(super) async fn watch<P: GitPeer>(
    peer: P,
    workspace: String,
    updates: mpsc::UnboundedSender<StoreUpdate>,
    freshness: Arc<Freshness>,
) {
    let mut restarts = WatchRestarts::new();
    loop {
        // Subscribe first, then read: the stream holds every change from its start, so none falls between.
        let subscription = peer.subscribe(&workspace).await;
        // The first read runs beside the stream. The ready item and the changes never wait for its git calls.
        let numbers = freshness.request(true);
        refresh_in_background(
            peer.clone(),
            updates.clone(),
            freshness.clone(),
            workspace.clone(),
            numbers,
            true,
        );
        let ending = match subscription {
            Ok(mut events) => follow(&peer, &updates, &freshness, &workspace, &mut events).await,
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
async fn follow<P: GitPeer>(
    peer: &P,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    freshness: &Arc<Freshness>,
    workspace: &str,
    events: &mut P::Events,
) -> Ending {
    let mut healthy = false;
    loop {
        match events.next_event().await {
            Some(Ok(event)) => {
                healthy |= event.ready;
                let history = event
                    .kinds
                    .iter()
                    .any(|kind| matches!(kind, WatchKind::Head | WatchKind::Refs));
                let degraded = event
                    .degraded
                    .then(|| event.reason.unwrap_or_else(|| NO_REASON.to_owned()));
                // Folded at once: the panel learns of the change before its git calls answer.
                send(
                    updates,
                    GitUpdate::Watch {
                        workspace: workspace.to_owned(),
                        degraded,
                    },
                );
                if !event.ready {
                    // Beside the stream: an event that comes while a read is out asks for a read of its own.
                    let numbers = freshness.request(history);
                    refresh_in_background(
                        peer.clone(),
                        updates.clone(),
                        freshness.clone(),
                        workspace.to_owned(),
                        numbers,
                        history,
                    );
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
/// Runs one refresh beside the caller, so neither the stream nor a later event waits for its git calls.
fn refresh_in_background<P: GitPeer>(
    peer: P,
    updates: mpsc::UnboundedSender<StoreUpdate>,
    freshness: Arc<Freshness>,
    workspace: String,
    numbers: Numbers,
    history: bool,
) {
    tokio::spawn(async move {
        refresh(&peer, &updates, &freshness, &workspace, numbers, history).await;
    });
}

/// Reads the status, and the first page of history when `history` is set, and delivers each answer that is
/// still the newest of its kind.
async fn refresh<P: GitPeer>(
    peer: &P,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    freshness: &Freshness,
    workspace: &str,
    numbers: Numbers,
    history: bool,
) {
    let status = peer.status(workspace).await;
    freshness.deliver(
        workspace,
        Read::Status,
        numbers.status,
        updates,
        GitUpdate::Status {
            workspace: workspace.to_owned(),
            result: status.map_err(|error| Refusal::from(&error)),
        },
    );
    if history {
        let log = peer.log(workspace).await;
        freshness.deliver(
            workspace,
            Read::Log,
            numbers.log,
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
