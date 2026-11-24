//! Real TLS and WebSocket connections on 127.0.0.1 with throwaway identities.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use massorca_net::{
    Accepted, ClientWs, Keepalive, NetError, TlsIdentity, WS_PATH, accept, client_tls_config,
    connect, server_tls_config, spawn_frame_tasks,
};
use massorca_protocol::codes::RESPONSE_TOO_LARGE;
use massorca_protocol::{Frame, MAX_FRAME_LEN_NETWORK, Message};
use rustls::pki_types::ServerName;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_tungstenite::tungstenite::Message as WsMessage;

const TIMEOUT: Duration = Duration::from_secs(5);

struct Server {
    identity: TlsIdentity,
    address: String,
    accepted: JoinHandle<Result<Accepted, NetError>>,
}

/// A server that accepts one connection.
async fn server() -> Server {
    let identity = TlsIdentity::generate("server").expect("server identity");
    let acceptor = TlsAcceptor::from(server_tls_config(&identity).expect("server config"));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address").to_string();
    let accepted = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await?;
        accept(tcp, &acceptor).await
    });
    Server {
        identity,
        address,
        accepted,
    }
}

async fn accepted(server: Server) -> Result<Accepted, NetError> {
    tokio::time::timeout(TIMEOUT, server.accepted)
        .await
        .expect("accept in time")
        .expect("accept task")
}

/// A connected pair: the client's raw WebSocket and the server's accepted connection.
async fn pair() -> (ClientWs, Accepted) {
    let server = server().await;
    let client_identity = TlsIdentity::generate("client").expect("client identity");
    let addresses = vec![server.address.clone()];
    let (client_ws, _) = connect(&addresses, server.identity.pin(), &client_identity, TIMEOUT)
        .await
        .expect("connect");
    (client_ws, accepted(server).await.expect("accepted"))
}

fn quick() -> Keepalive {
    Keepalive {
        interval: Duration::from_millis(100),
        silence: Duration::from_millis(400),
    }
}

#[tokio::test]
async fn frames_flow_both_ways_over_mutual_tls() {
    let server = server().await;
    let client_identity = TlsIdentity::generate("client").expect("client identity");
    let addresses = vec![server.address.clone()];
    let (client_ws, answered) =
        connect(&addresses, server.identity.pin(), &client_identity, TIMEOUT)
            .await
            .expect("connect");
    assert_eq!(answered, server.address);
    let accepted = accepted(server).await.expect("accepted");
    assert_eq!(accepted.client_pin, client_identity.pin());

    let (server_in_tx, mut server_in) = mpsc::channel(8);
    let (server_out, server_out_rx) = mpsc::channel::<Frame>(8);
    spawn_frame_tasks(
        accepted.ws,
        MAX_FRAME_LEN_NETWORK,
        Keepalive::default(),
        server_in_tx,
        server_out_rx,
    );
    let (client_in_tx, mut client_in) = mpsc::channel(8);
    let (client_out, client_out_rx) = mpsc::channel::<Message>(8);
    spawn_frame_tasks(
        client_ws,
        MAX_FRAME_LEN_NETWORK,
        Keepalive::default(),
        client_in_tx,
        client_out_rx,
    );

    let request = Message::Request {
        id: 7,
        method: "device.whoami".to_owned(),
        params: serde_json::json!({}),
    };
    client_out.send(request.clone()).await.expect("send");
    let got = tokio::time::timeout(TIMEOUT, server_in.recv())
        .await
        .expect("in time");
    assert_eq!(got, Some(Frame::Message(request)));

    let bytes = Frame::StreamBytes {
        stream: 3,
        payload: vec![0, 1, 2, 255],
    };
    server_out.send(bytes.clone()).await.expect("send");
    let got = tokio::time::timeout(TIMEOUT, client_in.recv())
        .await
        .expect("in time");
    assert_eq!(got, Some(bytes));
}

#[tokio::test]
async fn wrong_pin_is_refused_and_never_upgraded() {
    let server = server().await;
    let client_identity = TlsIdentity::generate("client").expect("client identity");
    let stranger = TlsIdentity::generate("stranger").expect("stranger");
    // The same server twice: a mismatch must not move on to the next address.
    let addresses = vec![server.address.clone(), server.address.clone()];
    let presented = server.identity.pin();
    let result = connect(&addresses, stranger.pin(), &client_identity, TIMEOUT).await;
    // The refusal names the key the server did present, so the client can show both.
    assert!(
        matches!(result, Err(NetError::ServerPinMismatch { actual: Some(pin) }) if pin == presented),
        "{result:?}"
    );
    assert!(accepted(server).await.is_err());
}

#[tokio::test]
async fn connect_skips_an_address_that_does_not_answer() {
    let dead = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let dead_address = dead.local_addr().expect("address").to_string();
    drop(dead);
    let server = server().await;
    let client_identity = TlsIdentity::generate("client").expect("client identity");
    let addresses = vec![dead_address, server.address.clone()];
    let (_ws, answered) = connect(&addresses, server.identity.pin(), &client_identity, TIMEOUT)
        .await
        .expect("connect");
    assert_eq!(answered, server.address);
    let none = connect(&[], server.identity.pin(), &client_identity, TIMEOUT).await;
    assert!(matches!(none, Err(NetError::NoAddress)), "{none:?}");
}

/// Accepts any server; only to build a client that presents no certificate.
#[derive(Debug)]
struct AnyServer(rustls::crypto::WebPkiSupportedAlgorithms);

impl rustls::client::danger::ServerCertVerifier for AnyServer {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("no TLS 1.2".to_owned()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.supported_schemes()
    }
}

#[tokio::test]
async fn client_without_certificate_is_refused() {
    let server = server().await;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = AnyServer(provider.signature_verification_algorithms);
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("tls13")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    let tcp = TcpStream::connect(&server.address).await.expect("tcp");
    let name = ServerName::try_from("127.0.0.1").expect("name");
    // TLS 1.3 may finish on the client before the server reads the missing certificate.
    if let Ok(tls) = TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
    {
        let url = format!("wss://{}{WS_PATH}", server.address);
        assert!(tokio_tungstenite::client_async(url, tls).await.is_err());
    }
    assert!(accepted(server).await.is_err());
}

#[tokio::test]
async fn upgrade_on_another_path_gets_404() {
    let server = server().await;
    let client_identity = TlsIdentity::generate("client").expect("client identity");
    let config = client_tls_config(server.identity.pin(), &client_identity).expect("client config");
    let tcp = TcpStream::connect(&server.address).await.expect("tcp");
    let name = ServerName::try_from("127.0.0.1").expect("name");
    let tls = TlsConnector::from(config)
        .connect(name, tcp)
        .await
        .expect("tls");
    let url = format!("wss://{}/elsewhere", server.address);
    match tokio_tungstenite::client_async(url, tls).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 404);
        }
        other => panic!("expected a 404 refusal, got {:?}", other.map(|_| ())),
    }
    assert!(accepted(server).await.is_err());
}

#[tokio::test]
async fn oversized_response_becomes_too_large_and_other_frames_end_the_writer() {
    let (client_ws, accepted) = pair().await;
    let (client_in_tx, mut client_in) = mpsc::channel(8);
    let (_client_out, client_out_rx) = mpsc::channel::<Frame>(8);
    spawn_frame_tasks(
        client_ws,
        MAX_FRAME_LEN_NETWORK,
        Keepalive::default(),
        client_in_tx,
        client_out_rx,
    );
    let (server_in_tx, _server_in) = mpsc::channel(8);
    let (server_out, server_out_rx) = mpsc::channel::<Frame>(8);
    let (_, writer) = spawn_frame_tasks(
        accepted.ws,
        1024,
        Keepalive::default(),
        server_in_tx,
        server_out_rx,
    );

    let big = Message::response_ok(42, serde_json::json!("x".repeat(4096)));
    server_out.send(big.into()).await.expect("send");
    let got = tokio::time::timeout(TIMEOUT, client_in.recv())
        .await
        .expect("in time");
    match got {
        Some(Frame::Message(Message::Response { id, result, error })) => {
            assert_eq!(id, 42);
            assert!(result.is_none());
            assert_eq!(error.expect("error").code, RESPONSE_TOO_LARGE);
        }
        other => panic!("expected a too_large response, got {other:?}"),
    }

    let bytes = Frame::StreamBytes {
        stream: 1,
        payload: vec![0; 4096],
    };
    server_out.send(bytes).await.expect("send");
    tokio::time::timeout(TIMEOUT, writer)
        .await
        .expect("writer ends")
        .expect("writer task");
    let got = tokio::time::timeout(TIMEOUT, client_in.recv())
        .await
        .expect("in time");
    assert_eq!(
        got, None,
        "the oversized frame is never written and the connection closes"
    );
}

#[tokio::test]
async fn oversized_message_received_ends_the_connection() {
    let (mut client_ws, accepted) = pair().await;
    let (server_in_tx, mut server_in) = mpsc::channel(8);
    let (_server_out, server_out_rx) = mpsc::channel::<Frame>(8);
    let (reader, _) = spawn_frame_tasks(
        accepted.ws,
        1024,
        Keepalive::default(),
        server_in_tx,
        server_out_rx,
    );
    let mut body = vec![0x02];
    body.extend_from_slice(&[0; 4096]);
    client_ws.send(WsMessage::binary(body)).await.expect("send");
    tokio::time::timeout(TIMEOUT, reader)
        .await
        .expect("reader ends")
        .expect("reader task");
    assert_eq!(server_in.recv().await, None);
    let next = tokio::time::timeout(TIMEOUT, client_ws.next())
        .await
        .expect("closed in time");
    assert!(!matches!(next, Some(Ok(WsMessage::Binary(_)))), "{next:?}");
}

#[tokio::test]
async fn silence_closes_a_dead_peer() {
    let (client_ws, accepted) = pair().await;
    let (server_in_tx, mut server_in) = mpsc::channel(8);
    let (_server_out, server_out_rx) = mpsc::channel::<Frame>(8);
    let started = tokio::time::Instant::now();
    let (reader, writer) = spawn_frame_tasks(
        accepted.ws,
        MAX_FRAME_LEN_NETWORK,
        quick(),
        server_in_tx,
        server_out_rx,
    );
    // The client neither reads nor answers pings.
    tokio::time::timeout(TIMEOUT, reader)
        .await
        .expect("reader ends")
        .expect("reader task");
    let elapsed = started.elapsed();
    assert!(elapsed >= quick().silence, "{elapsed:?}");
    assert!(elapsed < quick().silence * 3, "{elapsed:?}");
    tokio::time::timeout(TIMEOUT, writer)
        .await
        .expect("writer ends")
        .expect("writer task");
    assert_eq!(server_in.recv().await, None);
    drop(client_ws);
}

#[tokio::test]
async fn a_peer_that_only_reads_stays_connected() {
    let (mut client_ws, accepted) = pair().await;
    let (server_in_tx, server_in) = mpsc::channel(8);
    let (server_out, server_out_rx) = mpsc::channel::<Frame>(8);
    let (reader, _) = spawn_frame_tasks(
        accepted.ws,
        MAX_FRAME_LEN_NETWORK,
        quick(),
        server_in_tx,
        server_out_rx,
    );
    // Reading lets tungstenite flush its automatic pongs; the client never sends anything itself.
    let client = tokio::spawn(async move {
        let mut pings = 0;
        let mut frames = Vec::new();
        while let Some(Ok(message)) = client_ws.next().await {
            match message {
                WsMessage::Ping(_) => pings += 1,
                WsMessage::Binary(body) => frames.push(body.to_vec()),
                _ => {}
            }
            if !frames.is_empty() {
                break;
            }
        }
        (pings, frames)
    });
    tokio::time::sleep(quick().silence * 3).await;
    assert!(!reader.is_finished(), "the server dropped a live peer");
    assert!(!server_in.is_closed());
    let frame = Frame::StreamBytes {
        stream: 9,
        payload: vec![1],
    };
    server_out.send(frame).await.expect("send");
    let (pings, frames) = tokio::time::timeout(TIMEOUT, client)
        .await
        .expect("in time")
        .expect("client");
    assert!(pings >= 3, "{pings} pings");
    assert_eq!(frames.len(), 1);
}
