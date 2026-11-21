//! Helpers shared by the service tests: profiles on temporary homes, in-process services and raw frames.

use std::path::Path;
use std::time::Duration;

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_client::{Client, ClientError, ConnectOptions};
use massorca_profile::{Profile, local_socket_name};
use massorca_protocol::{
    ErrorBody, MAX_FRAME_LEN_LOCAL, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION, PeerInfo,
    ProtocolRange, read_message, write_message,
};
use massorca_server::{RequestCtx, RunningService, Service, ServiceConfig};
use serde_json::Value;
use tempfile::TempDir;
use tokio::time::timeout;

pub const SERVER_NAME: &str = "massorca-test";
pub const SERVER_VERSION: &str = "0.0.0-test";

/// A temporary home, removed when the value drops.
pub fn temp_home() -> TempDir {
    tempfile::tempdir().expect("a temporary home")
}

/// The profile `name` under `home`, resolved without reading the process environment.
pub fn profile(home: &Path, name: &str) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some(name.to_owned()), |_| None)
        .expect("the profile resolves")
}

pub fn service_config(home: &Path, name: &str) -> ServiceConfig {
    ServiceConfig {
        profile: profile(home, name),
        server_name: SERVER_NAME.to_owned(),
        server_version: SERVER_VERSION.to_owned(),
    }
}

/// Starts an in-process service for `name` under `home`.
pub async fn start(home: &Path, name: &str) -> RunningService {
    Service::start(service_config(home, name))
        .await
        .expect("the service starts")
}

/// Requests shutdown and waits until the service has stopped and released its profile.
pub async fn stop(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

pub fn client_options() -> ConnectOptions {
    ConnectOptions::new(PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    })
}

/// A client connected to the service of `name` under `home`.
pub async fn connect(home: &Path, name: &str) -> Client {
    Client::connect(&profile(home, name), client_options())
        .await
        .expect("the client connects")
}

/// The code of a failed call; panics for any other kind of failure.
pub fn remote_code(error: &ClientError) -> String {
    match error {
        ClientError::Remote(body) => body.code.clone(),
        other => panic!("expected a remote error, got {other:?}"),
    }
}

/// An echo handler: the answer is the params.
pub async fn echo(_ctx: RequestCtx, params: Value) -> Result<Value, ErrorBody> {
    Ok(params)
}

/// `service.json` of the profile, parsed.
pub fn service_json(home: &Path, name: &str) -> Value {
    let text =
        std::fs::read_to_string(profile(home, name).info_path()).expect("service.json exists");
    serde_json::from_str(&text).expect("service.json is JSON")
}

/// A `hello` offering `range`.
pub fn hello_with(range: ProtocolRange) -> Message {
    Message::Hello {
        protocol: range,
        client: PeerInfo {
            name: "raw-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        },
        capabilities: Vec::new(),
        auth: None,
    }
}

/// A `hello` offering the one protocol version this build speaks.
pub fn hello() -> Message {
    hello_with(ProtocolRange {
        min: MIN_PROTOCOL_VERSION,
        max: PROTOCOL_VERSION,
    })
}

pub fn request(id: u64, method: &str, params: Value) -> Message {
    Message::Request {
        id,
        method: method.to_owned(),
        params,
    }
}

/// The code of a handshake refusal, if `message` is one.
pub fn refusal_code(message: &Message) -> Option<String> {
    match message {
        Message::Rejected { error, .. } => Some(error.code.clone()),
        _ => None,
    }
}

/// The protocol range a refusal names.
pub fn refusal_range(message: &Message) -> Option<ProtocolRange> {
    match message {
        Message::Rejected { supported, .. } => Some(*supported),
        _ => None,
    }
}

/// A connection that speaks raw protocol frames, for behaviour the client never produces.
pub struct Raw {
    stream: Stream,
}

impl Raw {
    pub async fn open(home: &Path, name: &str) -> Self {
        let socket = local_socket_name(&profile(home, name)).expect("the socket name");
        let stream = Stream::connect(socket)
            .await
            .expect("the raw connection opens");
        Self { stream }
    }

    pub async fn send(&mut self, message: &Message) {
        let mut writer: &Stream = &self.stream;
        write_message(&mut writer, message)
            .await
            .expect("the frame is sent");
    }

    /// The next message, or `None` once the service has closed the connection.
    pub async fn recv(&mut self) -> Option<Message> {
        self.recv_within(Duration::from_secs(5)).await
    }

    pub async fn recv_within(&mut self, limit: Duration) -> Option<Message> {
        let mut reader: &Stream = &self.stream;
        let frame = timeout(limit, read_message(&mut reader, MAX_FRAME_LEN_LOCAL))
            .await
            .expect("the service answers within the limit");
        frame.ok().flatten()
    }
}
