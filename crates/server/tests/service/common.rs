//! Helpers shared by the service tests: profiles on temporary homes, in-process services and raw frames.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_client::{Client, ClientError, ConnectOptions};
use massorca_profile::{Profile, local_socket_name};
use massorca_protocol::methods::terminals::ShellSpec;
use massorca_protocol::{
    ErrorBody, MAX_FRAME_LEN_LOCAL, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION, PeerInfo,
    ProtocolRange, read_message, write_message,
};
use massorca_server::{
    CapabilityHandle, Owner, Permission, RegistrationHandle, RequestCtx, RunningService, Service,
    ServiceConfig,
};
use serde_json::Value;
use tempfile::TempDir;
use tokio::sync::Notify;
use tokio::time::timeout;

/// How long a test waits for a reply it expects; generous because loaded CI runners stall for seconds.
pub const REPLY_WAIT: Duration = Duration::from_secs(30);

pub const SERVER_NAME: &str = "massorca-test";
pub const SERVER_VERSION: &str = "0.0.0-test";

/// A temporary home, removed when the value drops.
/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
pub struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
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

/// Sets its flag when dropped. A handler future is dropped when it finishes or when the service cancels its request.
pub struct DropFlag(pub Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Registers `ext.demo.stuck`, a method that never answers. Each call notifies `entered`, and the handler's future sets
/// `dropped` when the service cancels it. The returned handles keep the registrations in place.
pub fn register_stuck(
    running: &RunningService,
    entered: Arc<Notify>,
    dropped: Arc<AtomicBool>,
) -> (CapabilityHandle, RegistrationHandle) {
    let demo = Owner::Extension("demo".to_owned());
    let capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let method = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.stuck",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let (entered, dropped) = (Arc::clone(&entered), Arc::clone(&dropped));
                async move {
                    let _sentinel = DropFlag(dropped);
                    entered.notify_one();
                    std::future::pending::<()>().await;
                    Ok::<Value, ErrorBody>(Value::Null)
                }
            },
        )
        .expect("the method registers");
    (capability, method)
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

/// A `hello` offering the one protocol version this build speaks, and the listed capabilities.
pub fn hello_offering(capabilities: &[&str]) -> Message {
    Message::Hello {
        protocol: ProtocolRange {
            min: MIN_PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        },
        client: PeerInfo {
            name: "raw-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        },
        capabilities: capabilities.iter().map(|name| (*name).to_owned()).collect(),
        auth: None,
    }
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
        write_message(&mut writer, message, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the frame is sent");
    }

    /// The next message, or `None` once the service has closed the connection.
    pub async fn recv(&mut self) -> Option<Message> {
        self.recv_within(REPLY_WAIT).await
    }

    pub async fn recv_within(&mut self, limit: Duration) -> Option<Message> {
        let mut reader: &Stream = &self.stream;
        let frame = timeout(limit, read_message(&mut reader, MAX_FRAME_LEN_LOCAL))
            .await
            .expect("the service answers within the limit");
        frame.ok().flatten()
    }
}

/// A shell that stays alive for the test: a long sleep, one per platform.
pub fn shell_with_a_child() -> ShellSpec {
    if cfg!(windows) {
        ShellSpec {
            program: "cmd.exe".to_owned(),
            args: vec![
                "/d".to_owned(),
                "/c".to_owned(),
                "ping -n 120 127.0.0.1 >nul".to_owned(),
            ],
        }
    } else {
        ShellSpec {
            program: "sh".to_owned(),
            args: vec!["-c".to_owned(), "sleep 120; true".to_owned()],
        }
    }
}
