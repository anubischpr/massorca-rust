//! Port forwards (capability `port-forward/1`, decision 0131): a client tunnels TCP connections to a loopback port on
//! this host. A forward lives while its `host.forward` stream is open; each connection is a `host.forward_connect` duplex
//! stream carried by [`massorca_net::splice`]. Opening, connecting and stopping need `host-admin`; listing needs `read`.
//! Opens and closes are logged with ports and byte counts, never payload.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use massorca_core_store::now_ms;
use massorca_net::{SpliceEnd, SpliceLimits, splice};
use massorca_protocol::methods::forwards::{
    self, ForwardConnectParams, ForwardEvent, ForwardInfo, ForwardList, ForwardParams,
    ForwardStopParams, ForwardStopped, IDLE_TIMEOUT_SECS, MAX_CHUNK, MAX_CONNECTIONS_PER_FORWARD,
    MAX_FORWARDS_PER_CONNECTION, WINDOW_CHUNKS,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::method::{DuplexSource, StreamItem};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Items of one connection waiting for the client: a window of data, its acknowledgements and end of data fit.
const TUNNEL_QUEUE: usize = 2 * WINDOW_CHUNKS as usize + 8;

/// The open forwards of this service, across connections.
#[derive(Default)]
pub(crate) struct ForwardTable {
    last_id: AtomicU64,
    open: Mutex<HashMap<String, OpenForward>>,
}

struct OpenForward {
    info: ForwardInfo,
    connection_id: String,
    /// Set to true to stop the forward and every connection it carries.
    stop: watch::Sender<bool>,
}

impl ForwardTable {
    fn list(&self) -> Vec<ForwardInfo> {
        let mut forwards: Vec<ForwardInfo> = lock(&self.open)
            .values()
            .map(|open| open.info.clone())
            .collect();
        forwards.sort_by_key(|info| info.opened_at_ms);
        forwards
    }
}

pub(crate) fn register(
    registries: &Registries,
    table: &Arc<ForwardTable>,
) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let opening = Arc::clone(table);
    let connecting = Arc::clone(table);
    let listing = Arc::clone(table);
    let stopping = Arc::clone(table);
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::PORT_FORWARD, Owner::Core)?,
        registries.methods.register_stream(
            Owner::Core,
            forwards::FORWARD,
            capabilities::PORT_FORWARD,
            Permission::HostAdmin,
            move |ctx: RequestCtx, params: ForwardParams| {
                let table = Arc::clone(&opening);
                async move { open_forward(&table, &ctx, params) }
            },
        )?,
        registries.methods.register_duplex_stream(
            Owner::Core,
            forwards::FORWARD_CONNECT,
            capabilities::PORT_FORWARD,
            Permission::HostAdmin,
            move |ctx: RequestCtx, params: ForwardConnectParams| {
                let table = Arc::clone(&connecting);
                async move { connect(table, &ctx, &params.forward).await }
            },
        )?,
        registries.methods.register(
            Owner::Core,
            forwards::FORWARDS,
            capabilities::PORT_FORWARD,
            Permission::Read,
            move |_ctx: RequestCtx, _params: NoParams| {
                let forwards = listing.list();
                async move { Ok::<_, ErrorBody>(ForwardList { forwards }) }
            },
        )?,
        registries.methods.register(
            Owner::Core,
            forwards::FORWARD_STOP,
            capabilities::PORT_FORWARD,
            Permission::HostAdmin,
            move |_ctx: RequestCtx, params: ForwardStopParams| {
                let stopped = stop(&stopping, &params.forward);
                async move { stopped }
            },
        )?,
    ])
}

/// Opens a forward for this connection. Its stream sends `opened` and stays open until the client ends it, the
/// connection closes or `host.forward_stop` stops it.
fn open_forward(
    table: &Arc<ForwardTable>,
    ctx: &RequestCtx,
    params: ForwardParams,
) -> Result<mpsc::Receiver<Value>, ErrorBody> {
    if params.port == 0 {
        return Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            "a forward needs a port above 0",
        ));
    }
    if let Some(address) = params.address
        && !address.is_loopback()
    {
        return Err(ErrorBody::new(
            codes::FORWARD_TARGET_NOT_LOOPBACK,
            format!("a forward reaches loopback addresses on the host only, not {address}"),
        ));
    }
    let (stop_sender, stopped) = watch::channel(false);
    let info = {
        let mut open = lock(&table.open);
        let held = open
            .values()
            .filter(|forward| forward.connection_id == ctx.connection_id)
            .count();
        if held >= MAX_FORWARDS_PER_CONNECTION {
            return Err(ErrorBody::new(
                codes::FORWARD_LIMIT,
                format!("one connection holds at most {MAX_FORWARDS_PER_CONNECTION} forwards"),
            ));
        }
        let id = format!("f-{}", table.last_id.fetch_add(1, Ordering::Relaxed) + 1);
        let info = ForwardInfo {
            id: id.clone(),
            port: params.port,
            address: params.address,
            local_port: params.local_port,
            device_id: ctx.principal.device_id().map(str::to_owned),
            opened_at_ms: now_ms(),
            connections: 0,
            total_connections: 0,
            bytes_to_host: 0,
            bytes_from_host: 0,
        };
        open.insert(
            id,
            OpenForward {
                info: info.clone(),
                connection_id: ctx.connection_id.clone(),
                stop: stop_sender,
            },
        );
        info
    };
    tracing::info!(
        forward = %info.id,
        port = info.port,
        device = info.device_id.as_deref().unwrap_or("owner"),
        "a port forward opened"
    );
    let (events, source) = mpsc::channel(2);
    let opened = serde_json::to_value(ForwardEvent::Opened {
        forward: info.clone(),
    })
    .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?;
    let _ = events.try_send(opened);
    tokio::spawn(hold(Arc::clone(table), info.id, events, stopped));
    Ok(source)
}

/// Keeps a forward until its stream ends or it is stopped, then forgets it and ends its connections.
async fn hold(
    table: Arc<ForwardTable>,
    id: String,
    events: mpsc::Sender<Value>,
    mut stopped: watch::Receiver<bool>,
) {
    let by_stop = tokio::select! {
        () = events.closed() => false,
        _ = stopped.wait_for(|stopped| *stopped) => true,
    };
    let Some(forward) = lock(&table.open).remove(&id) else {
        return;
    };
    let _ = forward.stop.send(true);
    let info = forward.info;
    tracing::info!(
        forward = %info.id,
        port = info.port,
        connections = info.total_connections,
        bytes_to_host = info.bytes_to_host,
        bytes_from_host = info.bytes_from_host,
        stopped = by_stop,
        "a port forward closed"
    );
    if by_stop && let Ok(event) = serde_json::to_value(ForwardEvent::Stopped { forward: info }) {
        let _ = events.send(event).await;
    }
}

fn stop(table: &ForwardTable, id: &str) -> Result<ForwardStopped, ErrorBody> {
    let open = lock(&table.open);
    let forward = open.get(id).ok_or_else(|| not_found(id))?;
    let _ = forward.stop.send(true);
    Ok(ForwardStopped {
        forward: forward.info.clone(),
    })
}

/// Connects to the forward's port on loopback and carries the connection on a duplex stream. Only the connection that
/// opened the forward may use it.
async fn connect(
    table: Arc<ForwardTable>,
    ctx: &RequestCtx,
    id: &str,
) -> Result<DuplexSource, ErrorBody> {
    let (port, address, stopped) = {
        let mut open = lock(&table.open);
        let forward = open
            .get_mut(id)
            .filter(|forward| forward.connection_id == ctx.connection_id)
            .ok_or_else(|| not_found(id))?;
        if forward.info.connections as usize >= MAX_CONNECTIONS_PER_FORWARD {
            return Err(ErrorBody::new(
                codes::FORWARD_LIMIT,
                format!(
                    "a forward carries at most {MAX_CONNECTIONS_PER_FORWARD} connections at a time"
                ),
            ));
        }
        forward.info.connections += 1;
        forward.info.total_connections += 1;
        (
            forward.info.port,
            forward.info.address,
            forward.stop.subscribe(),
        )
    };
    let mut slot = ConnectionSlot {
        table: Arc::clone(&table),
        id: id.to_owned(),
        sent: 0,
        received: 0,
    };
    let socket = dial(port, address).await?;

    let (items, source) = mpsc::channel(TUNNEL_QUEUE);
    let (inbound, incoming) = mpsc::channel(TUNNEL_QUEUE);
    let (outgoing, mut chunks) = mpsc::channel::<Vec<u8>>(TUNNEL_QUEUE);
    // Chunks become stream bytes; a client that cancelled drops the items, which ends the splice.
    tokio::spawn(async move {
        while let Some(chunk) = chunks.recv().await {
            if items.send(StreamItem::Bytes(chunk)).await.is_err() {
                return;
            }
        }
    });
    tokio::spawn(async move {
        let mut stopped = stopped;
        let stop = async move {
            let _ = stopped.wait_for(|stopped| *stopped).await;
        };
        let limits = SpliceLimits {
            window: WINDOW_CHUNKS,
            max_chunk: MAX_CHUNK,
            idle: Duration::from_secs(IDLE_TIMEOUT_SECS),
        };
        let report = splice(socket, outgoing, incoming, stop, limits).await;
        slot.sent = report.sent;
        slot.received = report.received;
        match &report.end {
            SpliceEnd::Protocol(reason) => {
                tracing::info!(forward = %slot.id, %reason, "a forwarded connection broke the chunk rules");
            }
            SpliceEnd::Io(error) => {
                tracing::debug!(forward = %slot.id, kind = %error.kind(), "a forwarded connection failed");
            }
            SpliceEnd::Idle => {
                tracing::info!(forward = %slot.id, "a forwarded connection idled out")
            }
            SpliceEnd::Closed | SpliceEnd::PeerGone | SpliceEnd::Stopped => {}
        }
    });
    Ok(DuplexSource {
        items: source,
        inbound,
    })
}

/// One connection's place in its forward: released, with its byte counts, when dropped.
struct ConnectionSlot {
    table: Arc<ForwardTable>,
    id: String,
    sent: u64,
    received: u64,
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        if let Some(forward) = lock(&self.table.open).get_mut(&self.id) {
            forward.info.connections = forward.info.connections.saturating_sub(1);
            forward.info.bytes_from_host += self.sent;
            forward.info.bytes_to_host += self.received;
        }
    }
}

/// Connects to `port` on the given loopback address, or on `127.0.0.1` and then `::1`.
async fn dial(port: u16, address: Option<IpAddr>) -> Result<TcpStream, ErrorBody> {
    let candidates = match address {
        Some(address) => vec![address],
        None => vec![
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ],
    };
    for address in &candidates {
        let target = SocketAddr::new(*address, port);
        if let Ok(Ok(socket)) =
            tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(target)).await
        {
            let _ = socket.set_nodelay(true);
            return Ok(socket);
        }
    }
    Err(ErrorBody::new(
        codes::FORWARD_CONNECT_FAILED,
        format!("nothing accepted a connection on loopback port {port} on the host"),
    ))
}

fn not_found(id: &str) -> ErrorBody {
    ErrorBody::new(
        codes::FORWARD_NOT_FOUND,
        format!("no open forward named {id} is open to this caller"),
    )
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}
