//! The shared core of the watches (decision 0080, section 1): one resilient stream of a paired host, read by a task on
//! the binding's runtime. It holds at most one event ahead of `next`, so the host is read at most one event ahead.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

use massorca_client::{
    Backoff, Client, ClientError, Connect, Link, Reopen, Resilient, ResilientSubscription,
    StreamItem, Subscription,
};
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::agents::AgentWatchParams;
use massorca_protocol::methods::git::GitWorkspaceRef;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use crate::error::FfiError;
use crate::store::SealedHosts;

/// Whether what a watch reports can be trusted right now (decision 0080, section 1).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum LinkState {
    /// Connected again after a loss, with the stream opened afresh.
    Live,
    /// Out of contact. The state last seen may have changed; nothing says that anything ended.
    Unverifiable {
        /// The attempt that runs next, from 1.
        attempt: u32,
        /// The pause before that attempt, in milliseconds.
        retry_in_ms: u64,
        /// What broke the connection, in words.
        reason: String,
    },
}

impl From<Link> for LinkState {
    fn from(link: Link) -> Self {
        match link {
            Link::Live => Self::Live,
            Link::Unverifiable {
                attempt,
                retry_in,
                reason,
            } => Self::Unverifiable {
                attempt,
                retry_in_ms: u64::try_from(retry_in.as_millis()).unwrap_or(u64::MAX),
                reason,
            },
        }
    }
}

/// What a watch follows on its host. The stream is opened again on every connection.
#[derive(Clone)]
pub(crate) enum Watched {
    /// `git.watch` of one workspace.
    Git { workspace: String },
    /// `agent.watch`: every session, or those of one workspace.
    Agents { workspace: Option<String> },
}

impl Watched {
    async fn open(&self, client: &Client) -> Result<Subscription, ClientError> {
        match self {
            Self::Agents { workspace } => {
                client
                    .agent_watch(&AgentWatchParams {
                        workspace: workspace.clone(),
                        session: None,
                    })
                    .await
            }
            Self::Git { workspace } => {
                client
                    .git_watch(&GitWorkspaceRef {
                        workspace: workspace.clone(),
                    })
                    .await
            }
        }
    }
}

/// How to reach a paired host again: its stored record and key, and this app's identity.
#[derive(Clone)]
pub(crate) struct Reach {
    hosts: SealedHosts,
    host: String,
    peer: PeerInfo,
}

impl Reach {
    pub(crate) fn new(hosts: SealedHosts, host: String, peer: PeerInfo) -> Self {
        Self { hosts, host, peer }
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }
}

impl Connect for Reach {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send {
        let reach = self.clone();
        async move { Client::connect_host(&reach.hosts, &reach.host, &reach.peer).await }
    }
}

/// How a watch reconnects: to the host, then it opens the stream again.
pub(crate) struct Reconnect {
    reach: Reach,
    watched: Watched,
}

impl Reconnect {
    pub(crate) fn new(reach: Reach, watched: Watched) -> Self {
        Self { reach, watched }
    }
}

impl Reopen for Reconnect {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send {
        Connect::connect(&mut self.reach)
    }

    fn open(
        &mut self,
        client: &Client,
    ) -> impl Future<Output = Result<Subscription, ClientError>> + Send {
        let watched = self.watched.clone();
        async move { watched.open(client).await }
    }
}

/// Reads one item of a watch; `Ok(None)` skips an item this build does not read.
pub(crate) type Decode<E> = fn(StreamItem) -> Result<Option<E>, ClientError>;

/// One watch. Its stream runs on a task of the binding's runtime from the moment the watch starts. Closing the watch, or
/// dropping it, stops the task and with it the connection.
pub(crate) struct Feed<E> {
    host: String,
    events: Mutex<mpsc::Receiver<Result<E, ClientError>>>,
    task: JoinHandle<()>,
    closed: AtomicBool,
}

impl<E: Send + 'static> Feed<E> {
    /// Runs the task that `run` builds on `runtime`. The task hands each event over through the channel it is given,
    /// which holds one event for `next`.
    pub(crate) fn spawn<F>(
        runtime: &Handle,
        host: String,
        run: impl FnOnce(mpsc::Sender<Result<E, ClientError>>) -> F,
    ) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let (events, received) = mpsc::channel(1);
        let task = runtime.spawn(run(events));
        Self {
            host,
            events: Mutex::new(received),
            task,
            closed: AtomicBool::new(false),
        }
    }
}

impl<E: From<LinkState> + Send + 'static> Feed<E> {
    pub(crate) fn start(runtime: &Handle, reconnect: Reconnect, decode: Decode<E>) -> Self {
        let host = reconnect.reach.host().to_owned();
        Self::spawn(runtime, host, move |events| {
            let stream = ResilientSubscription::new(reconnect, Backoff::default());
            pump(stream, decode, events)
        })
    }

    /// The next event. `Ok(None)` once the watch is closed or the host ended the stream. An `Err` is final.
    pub(crate) async fn next(&self) -> Result<Option<E>, FfiError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(None);
        }
        match self.events.lock().await.recv().await {
            None => Ok(None),
            Some(Ok(event)) => Ok(Some(event)),
            Some(Err(error)) => Err(FfiError::from_client(error, &self.host)),
        }
    }

    /// Ends the watch and its connection; a `next` waiting now returns `None`.
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.task.abort();
    }
}

impl<E> Drop for Feed<E> {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Reads the stream until it ends or fails for good, handing each event over. A refusal or an unreadable item is the
/// last event the watch gives.
async fn pump<E: From<LinkState> + Send + 'static>(
    mut stream: ResilientSubscription<Reconnect>,
    decode: Decode<E>,
    events: mpsc::Sender<Result<E, ClientError>>,
) {
    while let Some(step) = stream.next().await {
        let event = match step {
            Ok(Resilient::Item(item)) => match decode(item) {
                Ok(Some(event)) => Ok(event),
                Ok(None) => continue,
                Err(error) => Err(error),
            },
            Ok(Resilient::Link(link)) => Ok(E::from(LinkState::from(link))),
            Err(error) => Err(error),
        };
        let last = event.is_err();
        if events.send(event).await.is_err() || last {
            return;
        }
    }
}
