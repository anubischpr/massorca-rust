//! Terminal attach decoding, against a stand-in service: snapshot parts reassemble, output offsets skip repeats and trim
//! overlaps, and unknown events and chunk tags are ignored.

use std::path::Path;
use std::time::Duration;

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::traits::tokio::Listener as _;
use massorca_client::{AttachUpdate, Client, ClientError, ConnectOptions, TerminalAttachment};
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
    let home = tempfile::tempdir().expect("a temporary home");
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
    let home = tempfile::tempdir().expect("a temporary home");
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
    let home = tempfile::tempdir().expect("a temporary home");
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
