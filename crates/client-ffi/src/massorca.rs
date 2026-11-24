//! The app's entry point (decision 0080, section 1): `MassOrca` holds the runtime, the sealed host store and who the app
//! is; `Host` is one connection to a paired host.

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use massorca_client::{Client, HostRecord, HostStorage};
use massorca_protocol::PeerInfo;
use tokio::runtime::Runtime;

use crate::error::FfiError;
use crate::offer;
use crate::store::{HostStore, SealedHosts};
use crate::watch::{Decode, Feed, LinkState, Reconnect, Watched};

/// A paired host as the app lists it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HostSummary {
    pub name: String,
    /// The server's own name.
    pub server: String,
    pub addresses: Vec<String>,
    /// `sha256:<base64url>` of the server key this phone trusts.
    pub pin: String,
    /// The name this phone has on that host.
    pub device_name: String,
    pub scopes: Vec<String>,
    pub paired_at_ms: i64,
}

impl From<HostRecord> for HostSummary {
    fn from(record: HostRecord) -> Self {
        Self {
            name: record.name,
            server: record.server,
            addresses: record.addresses,
            pin: record.pin,
            device_name: record.device_name,
            scopes: record.scopes,
            paired_at_ms: record.paired_at_ms,
        }
    }
}

/// Who this phone is on a host.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WhoAmI {
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub scopes: Vec<String>,
}

/// What the host said at the handshake, for the protocol gate (M-SH-02).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProtocolInfo {
    pub protocol: u32,
    pub server_name: String,
    pub server_version: String,
    pub capabilities: Vec<String>,
}

/// The binding's root object. One per app; it starts its own runtime, so the app's thread never blocks on the network.
#[derive(uniffi::Object)]
pub struct MassOrca {
    runtime: Option<Runtime>,
    hosts: SealedHosts,
    peer: PeerInfo,
    device_name: String,
}

impl fmt::Debug for MassOrca {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MassOrca")
            .field("device_name", &self.device_name)
            .finish_non_exhaustive()
    }
}

impl Drop for MassOrca {
    fn drop(&mut self) {
        // The app may drop it from any thread, a runtime's included.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

#[uniffi::export]
impl MassOrca {
    /// `device_name` is what hosts call this phone; `app_version` is sent in every handshake.
    #[uniffi::constructor]
    pub fn new(
        store: Arc<dyn HostStore>,
        device_name: String,
        app_version: String,
    ) -> Result<Arc<Self>, FfiError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("massorca")
            .enable_all()
            .build()
            .map_err(|error| FfiError::Internal {
                message: format!("The network runtime could not start: {error}."),
            })?;
        Ok(Arc::new(Self {
            runtime: Some(runtime),
            hosts: SealedHosts::new(store),
            peer: PeerInfo {
                name: "massorca-android".to_owned(),
                version: app_version,
            },
            device_name,
        }))
    }

    /// The paired hosts, by name. Makes no connection.
    pub fn hosts(&self) -> Result<Vec<HostSummary>, FfiError> {
        let hosts = self
            .hosts
            .list()
            .map_err(|error| FfiError::from_client(error, "this phone"))?;
        Ok(hosts.into_iter().map(HostSummary::from).collect())
    }

    /// Pairs with the server of `offer` and keeps it as `host_name`. The offer works once.
    pub async fn pair(&self, offer: String, host_name: String) -> Result<HostSummary, FfiError> {
        let offer = offer::decode(&offer)?;
        let hosts = self.hosts.clone();
        let peer = self.peer.clone();
        let device_name = self.device_name.clone();
        let shown = offer.server.clone();
        self.run(async move {
            Client::pair(&hosts, &offer, &host_name, &device_name, &peer)
                .await
                .map(|(_, record)| HostSummary::from(record))
                .map_err(|error| FfiError::from_client(error, &shown))
        })
        .await
    }

    /// Forgets a host and deletes this phone's key for it.
    pub fn forget(&self, host_name: String) -> Result<(), FfiError> {
        self.hosts
            .remove(&host_name)
            .map(|_| ())
            .map_err(|error| FfiError::from_client(error, &host_name))
    }

    /// Opens a connection to a paired host.
    pub async fn connect(&self, host_name: String) -> Result<Arc<Host>, FfiError> {
        let hosts = self.hosts.clone();
        let peer = self.peer.clone();
        let name = host_name.clone();
        let client = self
            .run(async move {
                Client::connect_host(&hosts, &name, &peer)
                    .await
                    .map_err(|error| FfiError::from_client(error, &name))
            })
            .await?;
        Ok(Arc::new(Host {
            name: host_name,
            client,
            runtime: self.handle(),
            hosts: self.hosts.clone(),
            peer: self.peer.clone(),
        }))
    }
}

impl MassOrca {
    fn handle(&self) -> tokio::runtime::Handle {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.handle().clone())
            .expect("the runtime lives until drop")
    }

    async fn run<T: Send + 'static>(
        &self,
        work: impl Future<Output = Result<T, FfiError>> + Send + 'static,
    ) -> Result<T, FfiError> {
        run_on(&self.handle(), work).await
    }
}

/// Runs `work` on the binding's runtime and waits for it from whatever executor the app polls with.
pub(crate) async fn run_on<T: Send + 'static>(
    runtime: &tokio::runtime::Handle,
    work: impl Future<Output = Result<T, FfiError>> + Send + 'static,
) -> Result<T, FfiError> {
    runtime.spawn(work).await.unwrap_or_else(|error| {
        Err(FfiError::Internal {
            message: format!("The request stopped: {error}."),
        })
    })
}

/// One connection to a paired host. Dropping it closes the connection.
#[derive(uniffi::Object)]
pub struct Host {
    pub(crate) name: String,
    pub(crate) client: Client,
    pub(crate) runtime: tokio::runtime::Handle,
    hosts: SealedHosts,
    peer: PeerInfo,
}

impl fmt::Debug for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Host")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[uniffi::export]
impl Host {
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Who this phone is on the host, and what it may do there.
    pub async fn whoami(&self) -> Result<WhoAmI, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let me = client
                .device_whoami()
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(WhoAmI {
                device_id: me.device.as_ref().map(|device| device.id.clone()),
                device_name: me.device.map(|device| device.name),
                scopes: me.scopes,
            })
        })
        .await
    }

    /// The protocol version, the server and what it can do.
    pub fn protocol(&self) -> ProtocolInfo {
        let welcome = self.client.welcome();
        ProtocolInfo {
            protocol: welcome.protocol,
            server_name: welcome.server.name.clone(),
            server_version: welcome.server.version.clone(),
            capabilities: welcome.capabilities.clone(),
        }
    }
}

impl Host {
    /// Follows `watched` on this host. The stream reconnects by itself and reports each loss (decision 0040).
    pub(crate) fn follow<E: From<LinkState> + Send + 'static>(
        &self,
        watched: Watched,
        decode: Decode<E>,
    ) -> Feed<E> {
        let reconnect = Reconnect::new(
            self.hosts.clone(),
            self.name.clone(),
            self.peer.clone(),
            watched,
        );
        Feed::start(&self.runtime, reconnect, decode)
    }
}
