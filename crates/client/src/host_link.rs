//! A paired host kept in reach (REM-07): one connection held open, and reconnected with the network backoff
//! (decision 0040) when it drops. Losing contact says nothing about whether the host or its work stopped. A refusal,
//! a changed server key or any other final failure is not retried until `reconnect` asks. Everything else that talks
//! to the host borrows the link's connection through a [`LiveWatch`] instead of dialing its own.

use std::sync::Arc;
use std::time::{Duration, Instant};

use massorca_protocol::PeerInfo;
use tokio::sync::{Notify, watch};
use tokio::task::JoinHandle;

use crate::client::Client;
use crate::error::ClientError;
use crate::host_storage::HostStorage;
use crate::reconnect::{Backoff, Connect, RECONNECT_RESET_AFTER, is_transient, jittered};

/// Where a host's link stands.
#[derive(Clone, Debug)]
pub enum LiveState {
    /// An attempt is under way: the first one, or one asked for with `reconnect`.
    Connecting,
    Connected {
        server: String,
        version: String,
    },
    /// Out of contact: attempt `attempt` runs after `retry_in`. The host may still be running.
    Unreachable {
        error: Arc<ClientError>,
        attempt: u32,
        retry_in: Duration,
    },
    /// The host refused this device, presented another server key, or failed in a way a retry would not fix. Nothing
    /// is tried again until `reconnect`.
    Ended {
        error: Arc<ClientError>,
    },
}

impl LiveState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

/// Connects to a paired host as this device, once per attempt. Each address and the handshake have their own time
/// limits, so a silent first address still leaves time for the next, and a changed key there is reported as such.
#[derive(Clone, Debug)]
pub struct HostDial<H> {
    hosts: H,
    name: String,
    peer: PeerInfo,
}

impl<H> HostDial<H> {
    pub fn new(hosts: H, name: impl Into<String>, peer: PeerInfo) -> Self {
        Self {
            hosts,
            name: name.into(),
            peer,
        }
    }
}

impl<H: HostStorage + Clone + 'static> Connect for HostDial<H> {
    async fn connect(&mut self) -> Result<Client, ClientError> {
        Client::connect_host(&self.hosts, &self.name, &self.peer).await
    }
}

/// One host's live link: a task that keeps it connected and publishes its state. Dropping it stops the task and
/// closes the connection, for every borrower too.
#[derive(Debug)]
pub struct LiveHost {
    state: watch::Receiver<LiveState>,
    connection: watch::Receiver<Option<Client>>,
    nudge: Arc<Notify>,
    task: JoinHandle<()>,
}

/// A view of one link that its borrowers keep: the link's state and its connection while it has one. Every receiver
/// closes once the link stops.
#[derive(Clone, Debug)]
pub struct LiveWatch {
    state: watch::Receiver<LiveState>,
    connection: watch::Receiver<Option<Client>>,
}

impl LiveWatch {
    /// The link's connection once its attempt under way has settled, or why there is none. A stopped link is
    /// `Disconnected`. The connection taken counts as seen by [`LiveWatch::changed`].
    pub async fn client(&mut self) -> Result<Client, Arc<ClientError>> {
        let settled = self
            .state
            .wait_for(|state| !matches!(state, LiveState::Connecting))
            .await
            .map(|state| state.clone());
        let connection = self.connection.borrow_and_update().clone();
        match settled {
            Ok(LiveState::Connected { .. }) => {
                connection.ok_or_else(|| Arc::new(ClientError::Disconnected))
            }
            Ok(LiveState::Unreachable { error, .. } | LiveState::Ended { error }) => Err(error),
            Ok(LiveState::Connecting) | Err(_) => Err(Arc::new(ClientError::Disconnected)),
        }
    }

    /// The connection now, if the link has one, marked as seen.
    pub fn current(&mut self) -> Option<Client> {
        self.connection.borrow_and_update().clone()
    }

    /// Waits until the link's connection changes from the one last seen: lost, or replaced. `false` once the link has
    /// stopped. Cancel-safe.
    pub async fn changed(&mut self) -> bool {
        self.connection.changed().await.is_ok()
    }
}

impl LiveHost {
    /// Starts the link on the current Tokio runtime; the first attempt runs at once.
    pub fn start<C: Connect + Send + 'static>(connect: C, backoff: Backoff) -> Self {
        let (state, receiver) = watch::channel(LiveState::Connecting);
        let (connection, held) = watch::channel(None);
        let nudge = Arc::new(Notify::new());
        let link = Link {
            state,
            connection,
            nudge: Arc::clone(&nudge),
        };
        let task = tokio::spawn(run(connect, backoff, link));
        Self {
            state: receiver,
            connection: held,
            nudge,
            task,
        }
    }

    /// A view of this link for a borrower of its connection.
    pub fn watch(&self) -> LiveWatch {
        LiveWatch {
            state: self.state.clone(),
            connection: self.connection.clone(),
        }
    }

    /// The state now.
    pub fn state(&self) -> LiveState {
        self.state.borrow().clone()
    }

    /// The state now and each change after it.
    pub fn subscribe(&self) -> watch::Receiver<LiveState> {
        self.state.clone()
    }

    /// Tries again at once with the backoff started over: after a final failure, while waiting to retry, or on a live
    /// connection that may be dead (after sleep or a network change), which is closed and opened again.
    pub fn reconnect(&self) {
        self.nudge.notify_one();
    }
}

impl Drop for LiveHost {
    fn drop(&mut self) {
        self.task.abort();
        if let Some(client) = self.connection.borrow().as_ref() {
            client.close();
        }
    }
}

/// The task's side of a link.
struct Link {
    state: watch::Sender<LiveState>,
    connection: watch::Sender<Option<Client>>,
    nudge: Arc<Notify>,
}

async fn run<C: Connect>(mut connect: C, mut backoff: Backoff, link: Link) {
    let Link {
        state,
        connection,
        nudge,
    } = link;
    loop {
        let error = match connect.connect().await {
            Ok(client) => {
                // A request that came while the attempt ran is answered by this fresh connection.
                consume(&nudge).await;
                let server = &client.welcome().server;
                // Lent before it is announced, so a borrower that sees `Connected` finds it.
                connection.send_replace(Some(client.clone()));
                state.send_replace(LiveState::Connected {
                    server: server.name.clone(),
                    version: server.version.clone(),
                });
                let since = Instant::now();
                let asked = tokio::select! {
                    () = client.closed() => false,
                    () = nudge.notified() => true,
                };
                connection.send_replace(None);
                // Borrowers lose it too, so a reconnect never leaves a second connection open.
                client.close();
                drop(client);
                if asked {
                    backoff.reset();
                    state.send_replace(LiveState::Connecting);
                    continue;
                }
                if since.elapsed() >= RECONNECT_RESET_AFTER {
                    backoff.reset();
                }
                ClientError::Disconnected
            }
            Err(error) if is_transient(&error) => error,
            Err(error) => {
                state.send_replace(LiveState::Ended {
                    error: Arc::new(error),
                });
                nudge.notified().await;
                backoff.reset();
                state.send_replace(LiveState::Connecting);
                continue;
            }
        };
        let retry_in = jittered(backoff.next_delay());
        state.send_replace(LiveState::Unreachable {
            error: Arc::new(error),
            attempt: backoff.failures(),
            retry_in,
        });
        tokio::select! {
            () = tokio::time::sleep(retry_in) => {}
            () = nudge.notified() => {
                backoff.reset();
                state.send_replace(LiveState::Connecting);
            }
        }
    }
}

/// Takes a pending `reconnect` request, if there is one, without waiting.
async fn consume(nudge: &Notify) {
    tokio::select! {
        biased;
        () = nudge.notified() => {}
        () = std::future::ready(()) => {}
    }
}

#[cfg(test)]
#[path = "host_link_tests.rs"]
mod tests;
