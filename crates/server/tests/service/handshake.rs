//! The handshake: the welcome a client receives, and how the service refuses a handshake it cannot accept.

use std::time::{Duration, Instant};

use massorca_protocol::{
    MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ProtocolRange, capabilities, codes, methods::service,
};
use serde_json::Value;

use super::common::{
    Raw, SERVER_NAME, SERVER_VERSION, connect, hello_with, refusal_code, refusal_range, request,
    start, stop, temp_home,
};

#[tokio::test]
async fn welcome_offers_the_registered_capabilities() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let welcome = client.welcome();
    assert_eq!(welcome.protocol, PROTOCOL_VERSION);
    assert_eq!(welcome.server.name, SERVER_NAME);
    assert_eq!(welcome.server.version, SERVER_VERSION);
    assert_eq!(
        welcome.capabilities,
        [
            capabilities::CONFIG,
            capabilities::EVENTS,
            capabilities::SERVICE
        ]
    );
    assert!(!welcome.connection_id.is_empty());
    assert!(client.has_capability(capabilities::SERVICE));
    assert!(
        !client.has_capability(capabilities::PROJECTS),
        "projects/1 waits for its handlers"
    );
    assert!(
        !client.has_capability(capabilities::WORKSPACES),
        "workspaces/1 waits for its handlers"
    );
    stop(running).await;
}

#[tokio::test]
async fn a_hello_with_no_shared_version_is_refused_and_the_connection_closed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&hello_with(ProtocolRange { min: 5, max: 9 }))
        .await;

    let refusal = raw
        .recv()
        .await
        .expect("the service explains the refusal before closing");
    assert_eq!(
        refusal_code(&refusal).as_deref(),
        Some(codes::PROTOCOL_UNSUPPORTED_VERSION)
    );
    assert_eq!(
        refusal_range(&refusal),
        Some(ProtocolRange {
            min: MIN_PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        })
    );
    assert!(
        raw.recv().await.is_none(),
        "the connection closes after the refusal"
    );
    stop(running).await;
}

#[tokio::test]
async fn a_first_frame_other_than_hello_is_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&request(1, service::INFO, Value::Null)).await;

    let refusal = raw
        .recv()
        .await
        .expect("the service explains the refusal before closing");
    assert_eq!(
        refusal_code(&refusal).as_deref(),
        Some(codes::PROTOCOL_HANDSHAKE)
    );
    assert!(
        raw.recv().await.is_none(),
        "the connection closes after the refusal"
    );
    stop(running).await;
}

#[tokio::test]
async fn hello_must_arrive_within_ten_seconds() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    let started = Instant::now();
    // The service may explain itself before it closes; only the moment of the close matters here.
    let waited = loop {
        if raw.recv_within(Duration::from_secs(15)).await.is_none() {
            break started.elapsed();
        }
    };
    assert!(
        waited >= Duration::from_secs(9),
        "the connection closed after {waited:?}, before the 10 second hello window"
    );
    assert!(
        waited < Duration::from_secs(14),
        "the connection closed after {waited:?}, too late"
    );
    stop(running).await;
}
