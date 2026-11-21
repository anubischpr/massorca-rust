//! Binary frames: the welcome offers `binary-frames/1`, byte items interleave with events in order on the raw socket,
//! and a handler sees the capabilities its client's `hello` named.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_client::{Client, ConnectOptions};
use massorca_profile::{Profile, local_socket_name};
use massorca_protocol::methods::service;
use massorca_protocol::{
    ErrorBody, Frame, MAX_FRAME_LEN_LOCAL, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION,
    PeerInfo, ProtocolRange, capabilities, codes, read_frame, write_frame,
};
use massorca_server::{
    ByteStreamSource, Owner, Permission, RequestCtx, RunningService, Service, ServiceConfig,
    StreamItem,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::time::timeout;

const SERVER_NAME: &str = "massorca-test";
const SERVER_VERSION: &str = "0.0.0-test";

fn temp_home() -> TempDir {
    tempfile::tempdir().expect("a temporary home")
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: SERVER_NAME.to_owned(),
        server_version: SERVER_VERSION.to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

fn demo() -> Owner {
    Owner::Extension("demo".to_owned())
}

fn hello(offered: &[&str]) -> Frame {
    Frame::Message(Message::Hello {
        protocol: ProtocolRange {
            min: MIN_PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        },
        client: PeerInfo {
            name: "raw-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        },
        capabilities: offered.iter().map(|name| (*name).to_owned()).collect(),
        auth: None,
    })
}

fn request(id: u64, method: &str, params: Value) -> Frame {
    Frame::Message(Message::Request {
        id,
        method: method.to_owned(),
        params,
    })
}

/// A connection that reads and writes whole frames, binary ones included.
struct RawFrames {
    stream: Stream,
}

impl RawFrames {
    async fn open(home: &Path) -> Self {
        let socket = local_socket_name(&profile(home)).expect("the socket name");
        let stream = Stream::connect(socket)
            .await
            .expect("the raw connection opens");
        Self { stream }
    }

    async fn send(&mut self, frame: &Frame) {
        let mut writer: &Stream = &self.stream;
        write_frame(&mut writer, frame, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the frame is sent");
    }

    async fn recv(&mut self) -> Frame {
        let mut reader: &Stream = &self.stream;
        timeout(
            Duration::from_secs(5),
            read_frame(&mut reader, MAX_FRAME_LEN_LOCAL),
        )
        .await
        .expect("the service answers within the limit")
        .expect("the frame reads")
        .expect("the service has not closed the connection")
    }

    /// Sends `hello` offering `offered` and returns the welcome's capabilities.
    async fn handshake(&mut self, offered: &[&str]) -> Vec<String> {
        self.send(&hello(offered)).await;
        match self.recv().await {
            Frame::Message(Message::Welcome { capabilities, .. }) => capabilities,
            other => panic!("expected welcome, got {other:?}"),
        }
    }

    /// Reads frames until the response to `id` arrives and returns its result.
    async fn result_of(&mut self, id: u64) -> Value {
        loop {
            if let Frame::Message(Message::Response {
                id: answered,
                result,
                error,
            }) = self.recv().await
                && answered == id
            {
                assert!(error.is_none(), "expected a result, got {error:?}");
                return result.expect("a successful response carries a result");
            }
        }
    }
}

async fn peer_capabilities(ctx: RequestCtx, _params: Value) -> Result<Vec<String>, ErrorBody> {
    let mut names: Vec<String> = ctx.client_capabilities.iter().cloned().collect();
    names.sort();
    Ok(names)
}

async fn interleaved_items(
    _ctx: RequestCtx,
    _params: Value,
) -> Result<ByteStreamSource, ErrorBody> {
    let (items, source) = mpsc::channel(8);
    tokio::spawn(async move {
        for item in [
            StreamItem::Event(json!({"n": 1})),
            StreamItem::Bytes(vec![0x00, 0xff, 0x80]),
            StreamItem::Event(json!({"n": 2})),
            StreamItem::Bytes(vec![1, 2, 3]),
        ] {
            if items.send(item).await.is_err() {
                return;
            }
        }
    });
    Ok(source)
}

#[tokio::test]
async fn welcome_offers_binary_frames() {
    let home = temp_home();
    let running = start(home.path()).await;
    let client = Client::connect(
        &profile(home.path()),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("the client connects");

    assert!(client.has_capability(capabilities::BINARY_FRAMES));
    stop(running).await;
}

#[tokio::test]
async fn handlers_see_the_capabilities_the_hello_named() {
    let home = temp_home();
    let running = start(home.path()).await;
    let _method = running
        .registries()
        .methods
        .register(
            demo(),
            "ext.demo.peer_capabilities",
            capabilities::BINARY_FRAMES,
            Permission::Read,
            peer_capabilities,
        )
        .expect("the method registers");

    let mut with = RawFrames::open(home.path()).await;
    with.handshake(&[capabilities::BINARY_FRAMES]).await;
    with.send(&request(1, "ext.demo.peer_capabilities", Value::Null))
        .await;
    assert_eq!(
        with.result_of(1).await,
        json!([capabilities::BINARY_FRAMES])
    );

    let mut without = RawFrames::open(home.path()).await;
    without.handshake(&[]).await;
    without
        .send(&request(1, "ext.demo.peer_capabilities", Value::Null))
        .await;
    assert_eq!(without.result_of(1).await, json!([]));
    stop(running).await;
}

#[tokio::test]
async fn byte_items_interleave_with_events_in_order() {
    let home = temp_home();
    let running = start(home.path()).await;
    let _method = running
        .registries()
        .methods
        .register_byte_stream(
            demo(),
            "ext.demo.interleaved",
            capabilities::BINARY_FRAMES,
            Permission::Read,
            interleaved_items,
        )
        .expect("the byte stream method registers");

    let mut raw = RawFrames::open(home.path()).await;
    raw.handshake(&[capabilities::BINARY_FRAMES]).await;
    raw.send(&request(1, "ext.demo.interleaved", Value::Null))
        .await;
    let stream = result_of_stream(&mut raw, 1).await;

    assert_eq!(
        raw.recv().await,
        Frame::Message(Message::Event {
            stream,
            data: json!({"n": 1}),
        })
    );
    assert_eq!(
        raw.recv().await,
        Frame::StreamBytes {
            stream,
            payload: vec![0x00, 0xff, 0x80],
        }
    );
    assert_eq!(
        raw.recv().await,
        Frame::Message(Message::Event {
            stream,
            data: json!({"n": 2}),
        })
    );
    assert_eq!(
        raw.recv().await,
        Frame::StreamBytes {
            stream,
            payload: vec![1, 2, 3],
        }
    );
    assert_eq!(
        raw.recv().await,
        Frame::Message(Message::End {
            stream,
            error: None,
        })
    );
    stop(running).await;
}

async fn oversized_item(_ctx: RequestCtx, _params: Value) -> Result<ByteStreamSource, ErrorBody> {
    let (items, source) = mpsc::channel(2);
    tokio::spawn(async move {
        let too_big = vec![0_u8; usize::try_from(MAX_FRAME_LEN_LOCAL).expect("u32 fits usize")];
        if items.send(StreamItem::Bytes(too_big)).await.is_ok() {
            let _ = items.send(StreamItem::Bytes(vec![1])).await;
        }
    });
    Ok(source)
}

#[tokio::test]
async fn oversized_byte_item_ends_its_stream_and_keeps_the_connection() {
    let home = temp_home();
    let running = start(home.path()).await;
    let _method = running
        .registries()
        .methods
        .register_byte_stream(
            demo(),
            "ext.demo.oversized",
            capabilities::BINARY_FRAMES,
            Permission::Read,
            oversized_item,
        )
        .expect("the byte stream method registers");

    let mut raw = RawFrames::open(home.path()).await;
    raw.handshake(&[capabilities::BINARY_FRAMES]).await;
    raw.send(&request(1, "ext.demo.oversized", Value::Null))
        .await;
    let stream = result_of_stream(&mut raw, 1).await;

    // The item cannot be framed, so its stream ends with an error and nothing of it is sent.
    let Frame::Message(Message::End {
        stream: ended,
        error: Some(error),
    }) = raw.recv().await
    else {
        panic!("expected the stream to end with an error");
    };
    assert_eq!(ended, stream);
    assert_eq!(error.code, codes::STREAM_ITEM_TOO_LARGE);

    raw.send(&request(2, service::INFO, Value::Null)).await;
    raw.result_of(2).await;
    stop(running).await;
}

#[tokio::test]
async fn stream_bytes_from_a_client_are_ignored() {
    let home = temp_home();
    let running = start(home.path()).await;
    let mut raw = RawFrames::open(home.path()).await;
    raw.handshake(&[capabilities::BINARY_FRAMES]).await;

    // No client sends stream bytes yet; the service drops them and keeps the connection.
    raw.send(&Frame::StreamBytes {
        stream: 1,
        payload: vec![1],
    })
    .await;
    raw.send(&request(2, service::INFO, Value::Null)).await;
    raw.result_of(2).await;
    stop(running).await;
}

/// The stream id a subscribing request answered with.
async fn result_of_stream(raw: &mut RawFrames, id: u64) -> u64 {
    raw.result_of(id)
        .await
        .get("stream")
        .and_then(Value::as_u64)
        .expect("the answer names a stream")
}

#[tokio::test]
async fn byte_streams_are_refused_to_a_client_without_binary_frames() {
    let home = temp_home();
    let running = start(home.path()).await;
    let ran = Arc::new(AtomicBool::new(false));
    let handler_ran = Arc::clone(&ran);
    let _method = running
        .registries()
        .methods
        .register_byte_stream(
            demo(),
            "ext.demo.bytes",
            capabilities::BINARY_FRAMES,
            Permission::Read,
            move |ctx: RequestCtx, params: Value| {
                handler_ran.store(true, Ordering::SeqCst);
                interleaved_items(ctx, params)
            },
        )
        .expect("the byte stream method registers");

    let mut old = RawFrames::open(home.path()).await;
    old.handshake(&[]).await;
    old.send(&request(1, "ext.demo.bytes", Value::Null)).await;
    old.send(&request(2, service::INFO, Value::Null)).await;

    let mut refused = None;
    loop {
        match old.recv().await {
            Frame::StreamBytes { .. } => {
                panic!("a client without binary-frames/1 got stream bytes")
            }
            Frame::Message(Message::Response { id: 1, error, .. }) => refused = error,
            Frame::Message(Message::Response { id: 2, error, .. }) => {
                assert!(error.is_none(), "service.info failed: {error:?}");
                break;
            }
            _ => {}
        }
    }
    let refused = refused.expect("the byte stream request is answered with an error");
    assert_eq!(refused.code, codes::STREAM_BINARY_FRAMES_REQUIRED);
    assert!(
        !ran.load(Ordering::SeqCst),
        "the handler ran for an old client"
    );
    stop(running).await;
}
