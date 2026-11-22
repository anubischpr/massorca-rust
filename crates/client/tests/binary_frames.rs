//! Binary frames on the client, against a stand-in service: `hello` lists `binary-frames/1`, a subscription yields events and
//! bytes in order, `next` skips bytes, bytes for a stream nobody holds are dropped,
//! and a stream cut off by the connection says so.

mod support;

use std::path::Path;
use std::time::Duration;

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::traits::tokio::Listener as _;
use massorca_client::{Client, ConnectOptions, StreamItem, Subscription};
use massorca_profile::{Profile, ensure_dir, local_socket_name};
use massorca_protocol::{
    ErrorBody, Frame, MAX_FRAME_LEN_LOCAL, Message, PROTOCOL_VERSION, PeerInfo, capabilities,
    read_frame, write_frame,
};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
use tokio::time::timeout;

const STREAM: u64 = 7;
const UNKNOWN_STREAM: u64 = 99;

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

fn options() -> ConnectOptions {
    ConnectOptions::new(PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    })
}

/// A stand-in service for one connection. It welcomes the client, opens `STREAM` for its request, and sends events and
/// bytes for it with bytes for `UNKNOWN_STREAM` between them, then ends the stream, or with `drop_connection` closes the
/// connection instead. Its task returns the capabilities the client's hello named.
async fn stand_in(home: &Path, drop_connection: bool) -> (Profile, JoinHandle<Vec<String>>) {
    let profile = profile(home);
    ensure_dir(&profile).expect("the profile directory exists");
    let listener = ListenerOptions::new()
        .name(local_socket_name(&profile).expect("the socket name"))
        .create_tokio()
        .expect("the stand-in listens");

    let task = tokio::spawn(async move {
        let mut stream = listener.accept().await.expect("the client connects");
        let Some(Frame::Message(Message::Hello {
            capabilities: offered,
            ..
        })) = read_frame(&mut stream, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the hello arrives")
        else {
            panic!("the first frame is hello");
        };

        let welcome = Message::Welcome {
            protocol: PROTOCOL_VERSION,
            server: PeerInfo {
                name: "stand-in".to_owned(),
                version: "0.0.0-test".to_owned(),
            },
            capabilities: vec![
                capabilities::EVENTS.to_owned(),
                capabilities::SERVICE.to_owned(),
                capabilities::BINARY_FRAMES.to_owned(),
            ],
            connection_id: "connection-1".to_owned(),
            max_frame_len: None,
        };
        write_frame(&mut stream, &Frame::Message(welcome), MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the welcome is sent");

        let Some(Frame::Message(Message::Request { id, .. })) =
            read_frame(&mut stream, MAX_FRAME_LEN_LOCAL)
                .await
                .expect("the request arrives")
        else {
            panic!("the second frame is a request");
        };
        let answer = Message::response_ok(id, json!({ "stream": STREAM }));
        write_frame(&mut stream, &Frame::Message(answer), MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the answer is sent");

        let mut frames = vec![
            Frame::Message(Message::Event {
                stream: STREAM,
                data: json!({"n": 1}),
            }),
            Frame::StreamBytes {
                stream: STREAM,
                payload: vec![0x00, 0xff, 0x80],
            },
            Frame::StreamBytes {
                stream: UNKNOWN_STREAM,
                payload: vec![9],
            },
            Frame::Message(Message::Event {
                stream: STREAM,
                data: json!({"n": 2}),
            }),
            Frame::StreamBytes {
                stream: STREAM,
                payload: vec![1, 2, 3],
            },
            Frame::Message(Message::End {
                stream: STREAM,
                error: None,
            }),
        ];
        if drop_connection {
            frames.pop();
        }
        for frame in &frames {
            write_frame(&mut stream, frame, MAX_FRAME_LEN_LOCAL)
                .await
                .expect("the frame is sent");
        }
        offered
    });
    (profile, task)
}

async fn next_item(subscription: &mut Subscription) -> Option<Result<StreamItem, ErrorBody>> {
    timeout(Duration::from_secs(5), subscription.next_item())
        .await
        .expect("the item arrives")
}

async fn next_event(subscription: &mut Subscription) -> Option<Result<Value, ErrorBody>> {
    timeout(Duration::from_secs(5), subscription.next())
        .await
        .expect("the item arrives")
}

#[tokio::test]
async fn hello_lists_binary_frames_and_items_arrive_in_order() {
    let home = support::temp_home();
    let (profile, stand_in) = stand_in(home.path(), false).await;
    let client = Client::connect(&profile, options())
        .await
        .expect("the client connects");
    let mut subscription = client
        .subscribe("ext.demo.stream", &json!({}))
        .await
        .expect("the subscription opens");

    assert_eq!(
        next_item(&mut subscription).await,
        Some(Ok(StreamItem::Event(json!({"n": 1}))))
    );
    assert_eq!(
        next_item(&mut subscription).await,
        Some(Ok(StreamItem::Bytes(vec![0x00, 0xff, 0x80])))
    );
    assert_eq!(
        next_item(&mut subscription).await,
        Some(Ok(StreamItem::Event(json!({"n": 2}))))
    );
    assert_eq!(
        next_item(&mut subscription).await,
        Some(Ok(StreamItem::Bytes(vec![1, 2, 3])))
    );
    assert_eq!(next_item(&mut subscription).await, None);
    assert!(!subscription.connection_lost(), "the service ended it");

    let offered = stand_in.await.expect("the stand-in finishes");
    assert!(
        offered
            .iter()
            .any(|name| name == capabilities::BINARY_FRAMES),
        "hello lists {offered:?}"
    );
}

#[tokio::test]
async fn next_skips_bytes_and_unknown_stream_bytes_never_arrive() {
    let home = support::temp_home();
    let (profile, stand_in) = stand_in(home.path(), false).await;
    let client = Client::connect(&profile, options())
        .await
        .expect("the client connects");
    let mut subscription = client
        .subscribe("ext.demo.stream", &json!({}))
        .await
        .expect("the subscription opens");

    assert_eq!(
        next_event(&mut subscription).await,
        Some(Ok(json!({"n": 1})))
    );
    assert_eq!(
        next_event(&mut subscription).await,
        Some(Ok(json!({"n": 2})))
    );
    assert_eq!(next_event(&mut subscription).await, None);
    stand_in.await.expect("the stand-in finishes");
}

#[tokio::test]
async fn a_stream_cut_off_by_the_connection_reports_it_was_lost() {
    let home = support::temp_home();
    let (profile, stand_in) = stand_in(home.path(), true).await;
    let client = Client::connect(&profile, options())
        .await
        .expect("the client connects");
    let mut subscription = client
        .subscribe("ext.demo.stream", &json!({}))
        .await
        .expect("the subscription opens");

    assert_eq!(
        next_event(&mut subscription).await,
        Some(Ok(json!({"n": 1})))
    );
    assert_eq!(
        next_event(&mut subscription).await,
        Some(Ok(json!({"n": 2})))
    );
    assert_eq!(next_event(&mut subscription).await, None);
    assert!(subscription.connection_lost());
    stand_in.await.expect("the stand-in finishes");
}
