//! Terminal attach decoding, against a stand-in service: snapshot parts reassemble, output offsets skip repeats and trim
//! overlaps, unknown events and chunk tags are ignored,
//! and an attach cut mid-stream resumes without repeating or losing output.

mod support;

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::traits::tokio::Listener as _;
use massorca_client::{
    AttachStep, AttachUpdate, Backoff, Client, ClientError, Connect, ConnectOptions, Link,
    ResilientAttach, TerminalAttachment,
};
use massorca_profile::{Profile, ensure_dir, local_socket_name};
use massorca_protocol::methods::terminals::{
    AttachMode, ChunkKind, TerminalAttachParams, encode_chunk,
};
use massorca_protocol::{
    ErrorBody, Frame, MAX_FRAME_LEN_LOCAL, Message, PROTOCOL_VERSION, PeerInfo, capabilities,
    read_frame, write_frame,
};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
use tokio::time::timeout;

const STREAM: u64 = 3;
const TERMINAL: &str = "terminal-1";

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

fn terminal() -> Value {
    json!({
        "id": TERMINAL,
        "workspace_id": "workspace-1",
        "state": "live",
        "cols": 80,
        "rows": 24,
        "shell": "sh",
        "cwd": "/work",
        "created_at_ms": 1,
    })
}

fn event(data: Value) -> Frame {
    Frame::Message(Message::Event {
        stream: STREAM,
        data,
    })
}

fn bytes(kind: ChunkKind, offset: u64, data: &[u8]) -> Frame {
    Frame::StreamBytes {
        stream: STREAM,
        payload: encode_chunk(kind, offset, data),
    }
}

/// A stand-in service that answers the attach with `STREAM`, then sends `frames` for it.
async fn stand_in(home: &Path, frames: Vec<Frame>) -> (Profile, JoinHandle<()>) {
    let profile = profile(home);
    ensure_dir(&profile).expect("the profile directory exists");
    let listener = ListenerOptions::new()
        .name(local_socket_name(&profile).expect("the socket name"))
        .create_tokio()
        .expect("the stand-in listens");

    let task = tokio::spawn(async move {
        let mut stream = listener.accept().await.expect("the client connects");
        let Some(Frame::Message(Message::Hello { .. })) =
            read_frame(&mut stream, MAX_FRAME_LEN_LOCAL)
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
                capabilities::TERMINALS.to_owned(),
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
        for frame in &frames {
            write_frame(&mut stream, frame, MAX_FRAME_LEN_LOCAL)
                .await
                .expect("the frame is sent");
        }
    });
    (profile, task)
}

async fn attach(home: &Path, frames: Vec<Frame>) -> (Client, TerminalAttachment, JoinHandle<()>) {
    let (profile, stand_in) = stand_in(home, frames).await;
    let client = Client::connect(&profile, options())
        .await
        .expect("the client connects");
    let attachment = client
        .terminal_attach(&TerminalAttachParams {
            terminal: TERMINAL.to_owned(),
            since: None,
            cols: None,
            rows: None,
            answers_queries: None,
        })
        .await
        .expect("the attach opens");
    (client, attachment, stand_in)
}

async fn next(attachment: &mut TerminalAttachment) -> Option<Result<AttachUpdate, ClientError>> {
    timeout(Duration::from_secs(5), attachment.next())
        .await
        .expect("the update arrives")
}

#[tokio::test]
async fn snapshot_parts_reassemble_and_output_offsets_skip_repeats_and_trim_overlaps() {
    let home = support::temp_home();
    let frames = vec![
        event(
            json!({"kind": "attached", "terminal": terminal(), "mode": "snapshot", "offset": 10}),
        ),
        event(json!({"kind": "snapshot", "offset": 10, "cols": 80, "rows": 24, "len": 7})),
        bytes(ChunkKind::Snapshot, 10, b"abc"),
        bytes(ChunkKind::Snapshot, 10, b"defg"),
        // A tag nobody knows, and an event kind nobody knows, are both ignored.
        Frame::StreamBytes {
            stream: STREAM,
            payload: vec![0x07, 0, 0, 0, 0, 0, 0, 0, 0, 9, 9],
        },
        event(json!({"kind": "from_the_future", "value": 1})),
        bytes(ChunkKind::Output, 10, b"hi"),
        // Overlaps the bytes read so far: only the new byte is passed on.
        bytes(ChunkKind::Output, 11, b"ix"),
        // Entirely before the offset read so far: skipped.
        bytes(ChunkKind::Output, 5, b"old"),
        bytes(ChunkKind::Output, 13, b"!?"),
        event(json!({"kind": "exited", "terminal": terminal()})),
    ];
    let (client, mut attachment, stand_in) = attach(home.path(), frames).await;

    let Some(Ok(AttachUpdate::Attached {
        terminal,
        mode,
        offset,
    })) = next(&mut attachment).await
    else {
        panic!("the attach is announced first");
    };
    assert_eq!(terminal.id, TERMINAL);
    assert_eq!(mode, AttachMode::Snapshot);
    assert_eq!(offset, 10);

    assert_eq!(
        next(&mut attachment)
            .await
            .map(|item| item.expect("no error")),
        Some(AttachUpdate::Snapshot {
            offset: 10,
            cols: 80,
            rows: 24,
            bytes: b"abcdefg".to_vec(),
        })
    );
    assert_eq!(
        next(&mut attachment)
            .await
            .map(|item| item.expect("no error")),
        Some(AttachUpdate::Output {
            offset: 10,
            bytes: b"hi".to_vec(),
        })
    );
    assert_eq!(
        next(&mut attachment)
            .await
            .map(|item| item.expect("no error")),
        Some(AttachUpdate::Output {
            offset: 12,
            bytes: b"x".to_vec(),
        })
    );
    assert_eq!(attachment.next_offset(), 13);
    assert_eq!(
        next(&mut attachment)
            .await
            .map(|item| item.expect("no error")),
        Some(AttachUpdate::Output {
            offset: 13,
            bytes: b"!?".to_vec(),
        })
    );
    assert_eq!(attachment.next_offset(), 15);
    assert!(matches!(
        next(&mut attachment).await,
        Some(Ok(AttachUpdate::Exited { .. }))
    ));
    assert_eq!(next(&mut attachment).await.map(|item| item.is_ok()), None);

    stand_in.await.expect("the stand-in finishes");
    drop(client);
}

#[tokio::test]
async fn a_resumed_attach_starts_at_its_offset_and_a_chunk_past_it_keeps_its_offset() {
    let home = support::temp_home();
    let frames = vec![
        event(json!({"kind": "attached", "terminal": terminal(), "mode": "resume", "offset": 40})),
        bytes(ChunkKind::Output, 40, b"resume"),
        // A gap the client did not expect: passed on at its own offset, and the offset moves past it.
        bytes(ChunkKind::Output, 50, b"gap"),
        Frame::Message(Message::End {
            stream: STREAM,
            error: None,
        }),
    ];
    let (client, mut attachment, stand_in) = attach(home.path(), frames).await;

    let Some(Ok(AttachUpdate::Attached { mode, offset, .. })) = next(&mut attachment).await else {
        panic!("the attach is announced first");
    };
    assert_eq!(mode, AttachMode::Resume);
    assert_eq!(offset, 40);
    assert_eq!(
        next(&mut attachment)
            .await
            .map(|item| item.expect("no error")),
        Some(AttachUpdate::Output {
            offset: 40,
            bytes: b"resume".to_vec(),
        })
    );
    assert_eq!(
        next(&mut attachment)
            .await
            .map(|item| item.expect("no error")),
        Some(AttachUpdate::Output {
            offset: 50,
            bytes: b"gap".to_vec(),
        })
    );
    assert_eq!(attachment.next_offset(), 53);
    assert_eq!(next(&mut attachment).await.map(|item| item.is_ok()), None);

    stand_in.await.expect("the stand-in finishes");
    drop(client);
}

#[tokio::test]
async fn a_service_error_ends_the_attach_with_that_error() {
    let home = support::temp_home();
    let frames = vec![Frame::Message(Message::End {
        stream: STREAM,
        error: Some(ErrorBody::new("terminal.not_found", "no such terminal")),
    })];
    let (client, mut attachment, stand_in) = attach(home.path(), frames).await;

    match next(&mut attachment).await {
        Some(Err(ClientError::Remote(error))) => assert_eq!(error.code, "terminal.not_found"),
        other => panic!("expected the service error, got {other:?}"),
    }
    assert!(next(&mut attachment).await.is_none());

    stand_in.await.expect("the stand-in finishes");
    drop(client);
}

/// A stand-in that serves one connection per entry of `connections`. Each answers the attach with `STREAM`, records
/// the attach's `since`, then sends its frames; a connection whose frames do not end the stream is cut off.
async fn reconnecting_stand_in(
    home: &Path,
    connections: Vec<Vec<Frame>>,
) -> (Profile, JoinHandle<Vec<Option<u64>>>) {
    let profile = profile(home);
    ensure_dir(&profile).expect("the profile directory exists");
    let listener = ListenerOptions::new()
        .name(local_socket_name(&profile).expect("the socket name"))
        .create_tokio()
        .expect("the stand-in listens");

    let task = tokio::spawn(async move {
        let mut sinces = Vec::new();
        for frames in connections {
            let mut stream = listener.accept().await.expect("the client connects");
            let Some(Frame::Message(Message::Hello { .. })) =
                read_frame(&mut stream, MAX_FRAME_LEN_LOCAL)
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
                    capabilities::TERMINALS.to_owned(),
                    capabilities::BINARY_FRAMES.to_owned(),
                ],
                connection_id: "connection-1".to_owned(),
                max_frame_len: None,
            };
            write_frame(&mut stream, &Frame::Message(welcome), MAX_FRAME_LEN_LOCAL)
                .await
                .expect("the welcome is sent");
            let Some(Frame::Message(Message::Request { id, params, .. })) =
                read_frame(&mut stream, MAX_FRAME_LEN_LOCAL)
                    .await
                    .expect("the request arrives")
            else {
                panic!("the second frame is a request");
            };
            sinces.push(params.get("since").and_then(Value::as_u64));
            let answer = Message::response_ok(id, json!({ "stream": STREAM }));
            write_frame(&mut stream, &Frame::Message(answer), MAX_FRAME_LEN_LOCAL)
                .await
                .expect("the answer is sent");
            for frame in &frames {
                write_frame(&mut stream, frame, MAX_FRAME_LEN_LOCAL)
                    .await
                    .expect("the frame is sent");
            }
            // Dropping the socket here cuts the connection unless the frames ended the stream.
        }
        sinces
    });
    (profile, task)
}

struct Local(Profile);

impl Connect for Local {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send {
        let profile = self.0.clone();
        async move { Client::connect(&profile, options()).await }
    }
}

#[tokio::test]
async fn an_attach_cut_mid_stream_resumes_without_repeating_or_losing_output() {
    let home = support::temp_home();
    let attached = |offset: u64| {
        event(
            json!({"kind": "attached", "terminal": terminal(), "mode": "resume", "offset": offset}),
        )
    };
    let (profile, stand_in) = reconnecting_stand_in(
        home.path(),
        vec![
            // Cut off after "hello wor".
            vec![
                attached(0),
                bytes(ChunkKind::Output, 0, b"hello "),
                bytes(ChunkKind::Output, 6, b"wor"),
            ],
            // The service resends from an earlier offset than asked; the repeat must not show.
            vec![
                attached(6),
                bytes(ChunkKind::Output, 6, b"world!"),
                Frame::Message(Message::End {
                    stream: STREAM,
                    error: None,
                }),
            ],
        ],
    )
    .await;
    let mut attach = ResilientAttach::new(
        Local(profile),
        TerminalAttachParams {
            terminal: TERMINAL.to_owned(),
            since: None,
            cols: None,
            rows: None,
            answers_queries: None,
        },
        Backoff::new(Duration::from_millis(10), Duration::from_millis(50)),
    );

    let mut shown = Vec::new();
    let mut links = Vec::new();
    while let Some(step) = timeout(Duration::from_secs(5), attach.next())
        .await
        .expect("the attach reports in time")
    {
        match step.expect("no final error") {
            AttachStep::Update(AttachUpdate::Output { bytes, .. }) => shown.extend(bytes),
            AttachStep::Update(_) => {}
            AttachStep::Link(link) => links.push(link),
        }
    }

    assert_eq!(String::from_utf8(shown).expect("text"), "hello world!");
    assert!(
        matches!(
            links.as_slice(),
            [Link::Unverifiable { attempt: 1, .. }, Link::Live]
        ),
        "{links:?}"
    );
    let sinces = stand_in.await.expect("the stand-in finishes");
    assert_eq!(
        sinces,
        vec![None, Some(9)],
        "the reattach resumes after the output read"
    );
}
