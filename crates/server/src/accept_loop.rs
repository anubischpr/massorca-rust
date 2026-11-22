//! The accept loop: admits connections until shutdown, then stops listening and gives open connections a bounded time to finish.

use std::fs::File;
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::tokio::Listener;
use interprocess::local_socket::traits::tokio::Listener as _;
use tokio::task::JoinSet;

use crate::connection::{self, Origin};
use crate::context::AppState;
use crate::registry::RegistrationHandle;
use crate::shutdown::ShutdownSignal;
use crate::transport;

/// How long open connections may finish their in-flight requests once shutdown starts.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Pause after a failed accept, so a persistent failure does not spin.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Serves until shutdown. The lock and the core registrations live here so that they are released only after the connections
/// and the work they started are gone.
pub(crate) async fn run(
    listener: Listener,
    mut network: JoinSet<()>,
    state: Arc<AppState>,
    mut signal: ShutdownSignal,
    _lock: File,
    _registrations: Vec<RegistrationHandle>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            () = signal.wait() => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    connections.spawn(connection::serve(
                        transport::spawn_local(stream),
                        Origin::Local,
                        Arc::clone(&state),
                        signal.clone(),
                    ));
                }
                Err(error) => {
                    tracing::warn!(%error, "accepting a connection failed");
                    tokio::time::sleep(ACCEPT_RETRY).await;
                }
            },
        }
    }

    drop(listener);
    signal.mark_listener_closed();
    // Network listeners stop on the same signal and drain their own connections.
    let drained = tokio::time::timeout(SHUTDOWN_GRACE, async {
        while connections.join_next().await.is_some() {}
        while network.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        tracing::warn!("connections are still open after the shutdown grace period; closing them");
        connections.abort_all();
        network.abort_all();
        while connections.join_next().await.is_some() {}
        while network.join_next().await.is_some() {}
    }
    // Aborted handlers and store calls that were already running finish first, so the lock covers all store access.
    state.activity.idle().await;
}
