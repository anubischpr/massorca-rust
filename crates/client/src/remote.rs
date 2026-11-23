//! Connecting to a host over the network (decisions 0040, 0041, 0046): TLS with the pinned server key and this device's
//! certificate, the WebSocket upgrade, then the same handshake and client as the local socket.

use std::str::FromStr;
use std::time::Duration;

use massorca_net::{Keepalive, NetError, Pin, TlsIdentity};
use massorca_protocol::methods::devices::Secret;
use massorca_protocol::pairing::{AuthRequest, Offer};
use massorca_protocol::{MAX_FRAME_LEN_NETWORK, PeerInfo};

use crate::client::Client;
use crate::error::ClientError;
use crate::handshake;
use crate::host_storage::{HostRecord, HostStorage};
use crate::transport::FrameIo;

/// How long one address may take to accept a TCP connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl Client {
    /// Connects to a paired host as this device.
    pub async fn connect_host(
        hosts: &(impl HostStorage + ?Sized),
        name: &str,
        client: &PeerInfo,
    ) -> Result<Self, ClientError> {
        let record = hosts.get(name)?;
        let identity = hosts.identity(name)?;
        let pin = parse_pin(&record.pin)?;
        open(
            &record.addresses,
            pin,
            &identity,
            client,
            AuthRequest::Device {},
        )
        .await
    }

    /// Pairs this client with the server of `offer` and stores it as host `name` (decision 0041). The offer is used once.
    pub async fn pair(
        hosts: &(impl HostStorage + ?Sized),
        offer: &Offer,
        name: &str,
        device_name: &str,
        client: &PeerInfo,
    ) -> Result<(Self, HostRecord), ClientError> {
        crate::host_storage::validate_host_name(name)?;
        if hosts.exists(name) {
            return Err(ClientError::HostExists(name.to_owned()));
        }
        // Before the offer is used: it works once, and a refused profile could not keep the pairing.
        hosts.ensure_writable()?;
        let pin = parse_pin(&offer.pin)?;
        let identity = TlsIdentity::generate(device_name).map_err(|error| {
            ClientError::Protocol(format!("cannot create a device key: {error}"))
        })?;
        let auth = AuthRequest::Pair {
            secret: Secret::new(offer.secret.expose()),
            name: device_name.to_owned(),
        };
        let connected = open(&offer.addresses, pin, &identity, client, auth).await?;
        let me = connected.device_whoami().await?;
        let device = me.device.ok_or_else(|| {
            ClientError::Protocol("the server did not report the paired device".to_owned())
        })?;
        let record = HostRecord {
            name: name.to_owned(),
            server: offer.server.clone(),
            addresses: offer.addresses.clone(),
            pin: offer.pin.clone(),
            device_id: device.id,
            device_name: device.name,
            scopes: device.scopes,
            paired_at_ms: device.paired_at_ms,
        };
        hosts.add(&record, &identity)?;
        Ok((connected, record))
    }
}

async fn open(
    addresses: &[String],
    pin: Pin,
    identity: &TlsIdentity,
    client: &PeerInfo,
    auth: AuthRequest,
) -> Result<Client, ClientError> {
    let (ws, _address) = massorca_net::connect(addresses, pin, identity, CONNECT_TIMEOUT)
        .await
        .map_err(network_error)?;
    let mut io = FrameIo::spawn_with(MAX_FRAME_LEN_NETWORK, |queues| {
        massorca_net::spawn_frame_tasks(
            ws,
            MAX_FRAME_LEN_NETWORK,
            Keepalive::default(),
            queues.inbound,
            queues.outbound,
        )
    });
    match handshake::exchange(&mut io, client, Some(auth.to_auth())).await {
        Ok(welcome) => Ok(Client::open(io, welcome)),
        Err(error) => {
            io.abort();
            Err(error)
        }
    }
}

fn parse_pin(text: &str) -> Result<Pin, ClientError> {
    Pin::from_str(text).map_err(|error| ClientError::Protocol(format!("bad server pin: {error}")))
}

fn network_error(error: NetError) -> ClientError {
    match error {
        NetError::ServerPinMismatch => ClientError::ServerIdentityMismatch,
        other => ClientError::HostUnreachable(other.to_string()),
    }
}
