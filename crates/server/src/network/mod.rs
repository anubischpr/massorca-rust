//! The network side of the service (decisions 0040 to 0045): listeners, the server identity, pairing and devices.

mod admission;
mod advertise;
mod auth;
mod listen;
mod listener;
mod live;
mod pairing;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use massorca_net::TlsIdentity;
use massorca_profile::Profile;
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;

pub(crate) use admission::Pending;
pub(crate) use advertise::normalize_all as normalize_addresses;
pub(crate) use auth::{Authenticated, authenticate};
pub use listen::{ListenSpec, ListenSpecError, NetworkConfig};
pub(crate) use live::{LiveGuard, revoked_error};
pub(crate) use pairing::{DEFAULT_TTL, MAX_TTL};
pub use state::NetworkState;

use crate::context::AppState;
use crate::error::ServiceError;
use crate::shutdown::ShutdownSignal;

/// Where the server's key and certificate live: `<profile>/network/`.
pub fn identity_paths(profile: &Profile) -> (PathBuf, PathBuf) {
    let dir = profile.dir().join("network");
    (dir.join("identity.key"), dir.join("identity.crt"))
}

/// Loads or creates the server identity when the service listens on the network.
pub(crate) fn prepare(
    profile: &Profile,
    config: &NetworkConfig,
    server_name: &str,
) -> Result<(NetworkState, Option<TlsAcceptor>), ServiceError> {
    if config.listen.is_empty() {
        return Ok((NetworkState::new(None, &[]), None));
    }
    let (key, cert) = identity_paths(profile);
    let identity = TlsIdentity::load_or_create(&key, &cert, server_name)
        .map_err(|error| ServiceError::Network(error.to_string()))?;
    let tls = massorca_net::server_tls_config(&identity)
        .map_err(|error| ServiceError::Network(error.to_string()))?;
    Ok((
        NetworkState::new(Some(identity.pin()), &config.listen),
        Some(TlsAcceptor::from(tls)),
    ))
}

/// Starts one task per listener; the service waits for them at shutdown like its local connections.
pub(crate) fn spawn_listeners(
    config: &NetworkConfig,
    acceptor: Option<TlsAcceptor>,
    state: &Arc<AppState>,
    signal: &ShutdownSignal,
) -> JoinSet<()> {
    let mut tasks = JoinSet::new();
    if let Some(acceptor) = acceptor {
        for (index, spec) in config.listen.iter().enumerate() {
            tasks.spawn(listener::run(
                index,
                spec.clone(),
                Arc::clone(state),
                acceptor.clone(),
                signal.clone(),
            ));
        }
    }
    tasks
}
