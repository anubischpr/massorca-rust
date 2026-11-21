//! Network listeners: bind, accept, TLS and WebSocket, then the shared connection code (decisions 0040, 0045).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use massorca_net::{Accepted, Keepalive};
use massorca_protocol::MAX_FRAME_LEN_NETWORK;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};
use tokio_rustls::TlsAcceptor;

use crate::connection::{self, Origin};
use crate::context::AppState;
use crate::network::admission::{Admission, PRE_AUTH_MAX_FRAME};
use crate::network::listen::ListenSpec;
use crate::network::state::ListenerState;
use crate::shutdown::ShutdownSignal;
use crate::transport;

/// Network connections served at once; more are closed before the TLS handshake (decision 0040).
const MAX_CONNECTIONS: usize = 128;
/// TLS and the WebSocket upgrade must finish within this; `hello` has its own limit.
const UPGRADE_TIMEOUT: Duration = Duration::from_secs(10);
/// How often an address that is not there yet (Tailscale still starting) is tried again.
const BIND_RETRY: Duration = Duration::from_secs(5);
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Serves one `--listen` value until shutdown. Connections it accepted are drained by the service's grace period.
pub(crate) async fn run(
    index: usize,
    spec: ListenSpec,
    state: Arc<AppState>,
    acceptor: TlsAcceptor,
    mut signal: ShutdownSignal,
) {
    let Some(listener) = bind_when_available(index, &spec, &state, &mut signal).await else {
        return;
    };
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let admission = Arc::new(Admission::default());
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            () = signal.wait() => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => match accepted {
                Ok((tcp, peer)) => {
                    connections.spawn(serve(tcp, peer, Arc::clone(&state), acceptor.clone(), Arc::clone(&slots), Arc::clone(&admission), signal.clone()));
                }
                Err(error) => {
                    tracing::warn!(%error, "accepting a network connection failed");
                    sleep(ACCEPT_RETRY).await;
                }
            },
        }
    }
    drop(listener);
    // The connections finish their requests like local ones; the service bounds the wait.
    while connections.join_next().await.is_some() {}
}

async fn bind_when_available(
    index: usize,
    spec: &ListenSpec,
    state: &AppState,
    signal: &mut ShutdownSignal,
) -> Option<TcpListener> {
    loop {
        let attempt = match resolve(spec).await {
            Ok(address) => match TcpListener::bind(address).await {
                Ok(listener) => Ok(listener),
                Err(error) => Err(format!("cannot listen on {address}: {error}")),
            },
            Err(reason) => Err(reason),
        };
        match attempt {
            Ok(listener) => {
                let bound = listener.local_addr().ok();
                let extra = match spec {
                    ListenSpec::Tailscale { port } => tailscale_name(*port).await,
                    ListenSpec::Address(_) => None,
                };
                state
                    .network
                    .set_listener(index, ListenerState::Listening { bound, extra });
                tracing::info!(listen = %spec, bound = ?bound, "network listener ready");
                return Some(listener);
            }
            Err(reason) => {
                tracing::warn!(listen = %spec, %reason, "network listener waiting");
                state
                    .network
                    .set_listener(index, ListenerState::Waiting { reason });
            }
        }
        tokio::select! {
            () = signal.wait() => return None,
            () = sleep(BIND_RETRY) => {}
        }
    }
}

async fn resolve(spec: &ListenSpec) -> Result<SocketAddr, String> {
    match spec {
        ListenSpec::Address(address) => Ok(*address),
        ListenSpec::Tailscale { port } => {
            let port = *port;
            let status = tokio::task::spawn_blocking(massorca_core_remote::tailscale::status)
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| format!("no tailnet: {error}"))?;
            let ip = status
                .self_node
                .ipv4()
                .ok_or_else(|| "this node has no Tailscale IPv4 address".to_owned())?;
            Ok(SocketAddr::from((ip, port)))
        }
    }
}

/// The MagicDNS name of this node with the port, offered after the address in pairing offers.
async fn tailscale_name(port: u16) -> Option<String> {
    let status = tokio::task::spawn_blocking(massorca_core_remote::tailscale::status)
        .await
        .ok()?
        .ok()?;
    status
        .self_node
        .dns_name
        .map(|name| format!("{name}:{port}"))
}

/// TLS, the WebSocket upgrade, then the shared connection code with the peer's certificate as its origin.
async fn serve(
    tcp: TcpStream,
    peer: SocketAddr,
    state: Arc<AppState>,
    acceptor: TlsAcceptor,
    slots: Arc<Semaphore>,
    admission: Arc<Admission>,
    signal: ShutdownSignal,
) {
    let Ok(_slot) = slots.try_acquire_owned() else {
        tracing::warn!(%peer, "too many network connections; closing a new one");
        return;
    };
    let Some(mut pending) = admission.try_enter(peer.ip()) else {
        tracing::warn!(%peer, "too many unauthenticated network connections; closing a new one");
        return;
    };
    let _ = tcp.set_nodelay(true);
    let accepted = match timeout(UPGRADE_TIMEOUT, massorca_net::accept(tcp, &acceptor)).await {
        Ok(Ok(accepted)) => accepted,
        Ok(Err(error)) => {
            tracing::debug!(%peer, %error, "network handshake failed");
            return;
        }
        Err(_elapsed) => {
            tracing::debug!(%peer, "network handshake timed out");
            return;
        }
    };
    let Accepted { ws, client_pin, .. } = accepted;
    let (opener, gate) = massorca_net::pre_auth_gate(PRE_AUTH_MAX_FRAME);
    pending.set_opener(opener);
    let io = transport::spawn_with(MAX_FRAME_LEN_NETWORK, |queues| {
        massorca_net::spawn_gated_frame_tasks(
            ws,
            MAX_FRAME_LEN_NETWORK,
            Keepalive::default(),
            gate,
            queues.inbound,
            queues.outbound,
        )
    });
    connection::serve(
        io,
        Origin::Network { client_pin, peer },
        state,
        signal,
        Some(pending),
    )
    .await;
}
