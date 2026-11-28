use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use massorca_protocol::MAX_FRAME_LEN_NETWORK;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::tls::PresentedPin;
use crate::{NetError, Pin, TlsIdentity, WS_PATH, client_tls_config};

/// The server end of a network connection.
pub type ServerWs = WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>;
/// The client end of a network connection.
pub type ClientWs = ClientWsOver<TcpStream>;
/// The client end of a network connection over any byte stream.
pub type ClientWsOver<S> = WebSocketStream<tokio_rustls::client::TlsStream<S>>;

/// A server-side connection after TLS and the WebSocket upgrade.
pub struct Accepted {
    pub ws: ServerWs,
    /// Pin of the certificate the client presented; the server maps it to a device.
    pub client_pin: Pin,
    pub peer: SocketAddr,
}

impl fmt::Debug for Accepted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Accepted")
            .field("client_pin", &self.client_pin)
            .field("peer", &self.peer)
            .finish_non_exhaustive()
    }
}

fn ws_config() -> WebSocketConfig {
    let max = usize::try_from(MAX_FRAME_LEN_NETWORK).unwrap_or(usize::MAX);
    WebSocketConfig::default()
        .max_message_size(Some(max))
        .max_frame_size(Some(max))
}

/// TLS handshake, then the WebSocket upgrade on [`WS_PATH`]; any other path gets HTTP 404. The caller bounds the time.
pub async fn accept(tcp: TcpStream, acceptor: &TlsAcceptor) -> Result<Accepted, NetError> {
    let peer = tcp.peer_addr()?;
    let tls = acceptor.accept(tcp).await?;
    let client_cert = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs| certs.first())
        .ok_or_else(|| NetError::Certificate("the client presented no certificate".to_owned()))?;
    let client_pin = Pin::of_certificate(client_cert)?;
    let ws =
        tokio_tungstenite::accept_hdr_async_with_config(tls, check_path, Some(ws_config())).await?;
    Ok(Accepted {
        ws,
        client_pin,
        peer,
    })
}

#[expect(
    clippy::result_large_err,
    reason = "the signature is tungstenite's handshake `Callback`"
)]
fn check_path(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    if request.uri().path() == WS_PATH {
        Ok(response)
    } else {
        let mut refusal = ErrorResponse::new(None);
        *refusal.status_mut() = StatusCode::NOT_FOUND;
        Err(refusal)
    }
}

/// Tries `addresses` ("host:port", IPv6 as "[::1]:7347") in order, each attempt bounded by `connect_timeout`. A pin
/// mismatch is final: the next address would be the same server or an impostor.
pub async fn connect(
    addresses: &[String],
    server_pin: Pin,
    identity: &TlsIdentity,
    connect_timeout: Duration,
) -> Result<(ClientWs, String), NetError> {
    let connector = TlsConnector::from(client_tls_config(server_pin, identity)?);
    let mut last_error = NetError::NoAddress;
    for address in addresses {
        let attempt = tokio::time::timeout(connect_timeout, connect_one(address, &connector));
        match attempt.await {
            Ok(Ok(ws)) => return Ok((ws, address.clone())),
            Ok(Err(mismatch @ NetError::ServerPinMismatch { .. })) => return Err(mismatch),
            Ok(Err(error)) => {
                tracing::debug!(%address, %error, "network address failed");
                last_error = error;
            }
            Err(_) => {
                tracing::debug!(%address, "network address timed out");
                last_error = NetError::Timeout;
            }
        }
    }
    Err(last_error)
}

async fn connect_one(address: &str, connector: &TlsConnector) -> Result<ClientWs, NetError> {
    server_name(address)?;
    let tcp = TcpStream::connect(address).await?;
    tcp.set_nodelay(true)?;
    upgrade(tcp, address, connector).await
}

/// TLS with the pinned server key and the WebSocket upgrade over a byte stream that already reaches the server at
/// `address`, such as an SSH stdio forward (decision 0133). The pin is checked exactly as on a direct connection; the
/// stream is trusted for nothing. The caller bounds the time.
pub async fn connect_stream<S>(
    stream: S,
    address: &str,
    server_pin: Pin,
    identity: &TlsIdentity,
) -> Result<ClientWsOver<S>, NetError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let connector = TlsConnector::from(client_tls_config(server_pin, identity)?);
    upgrade(stream, address, &connector).await
}

async fn upgrade<S>(
    stream: S,
    address: &str,
    connector: &TlsConnector,
) -> Result<ClientWsOver<S>, NetError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls = connector
        .connect(server_name(address)?, stream)
        .await
        .map_err(pin_mismatch_or_io)?;
    let url = format!("wss://{address}{WS_PATH}");
    let (ws, _) = tokio_tungstenite::client_async_with_config(url, tls, Some(ws_config())).await?;
    Ok(ws)
}

/// The host part of "host:port" or "[v6]:port" as a TLS server name; it is sent but never checked.
fn server_name(address: &str) -> Result<ServerName<'static>, NetError> {
    let host = match address.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map(|(host, _)| host),
        None => address.rsplit_once(':').map(|(host, _)| host),
    }
    .ok_or_else(|| invalid_address(format!("{address} is not host:port")))?;
    ServerName::try_from(host.to_owned())
        .map_err(|_| invalid_address(format!("{host} is not a valid host name")))
}

fn pin_mismatch_or_io(error: std::io::Error) -> NetError {
    let refused = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>());
    match refused {
        Some(rustls::Error::InvalidCertificate(
            rustls::CertificateError::ApplicationVerificationFailure,
        )) => NetError::ServerPinMismatch { actual: None },
        Some(rustls::Error::InvalidCertificate(rustls::CertificateError::Other(other))) => {
            match other.0.downcast_ref::<PresentedPin>() {
                Some(presented) => NetError::ServerPinMismatch {
                    actual: Some(presented.0),
                },
                None => NetError::Io(error),
            }
        }
        _ => NetError::Io(error),
    }
}

fn invalid_address(reason: String) -> NetError {
    NetError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        reason,
    ))
}
