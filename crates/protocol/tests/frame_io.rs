#![cfg(feature = "tokio")]

use massorca_protocol::{
    FrameError, MAX_FRAME_LEN_LOCAL, Message, encode_frame, read_message, write_message,
};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn sample_messages() -> [Message; 3] {
    [
        Message::Cancel { stream: 1 },
        Message::Event {
            stream: 2,
            data: json!({ "blob": "x".repeat(4096) }),
        },
        Message::Request {
            id: 3,
            method: "service.info".to_owned(),
            params: Value::Null,
        },
    ]
}

/// Writes `bytes` to a finished stream and reads one frame from it.
async fn read_from_bytes(bytes: &[u8], max_len: u32) -> Result<Option<Message>, FrameError> {
    let (mut client, mut server) = tokio::io::duplex(bytes.len() + 16);
    client.write_all(bytes).await.expect("write bytes");
    drop(client);
    read_message(&mut server, max_len).await
}

#[tokio::test]
async fn messages_round_trip_over_a_small_duplex_buffer() {
    // A 16-byte buffer forces every frame through many partial reads and writes.
    let (mut client, mut server) = tokio::io::duplex(16);
    let sent = sample_messages();
    let writer = async move {
        for message in &sent {
            write_message(&mut client, message)
                .await
                .expect("write frame");
        }
    };
    let reader = async move {
        let mut received = Vec::new();
        while let Some(message) = read_message(&mut server, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("read frame")
        {
            received.push(message);
        }
        received
    };
    let ((), received) = tokio::join!(writer, reader);
    assert_eq!(received, sample_messages());
}

#[tokio::test]
async fn clean_eof_before_a_frame_is_none() {
    let (client, mut server) = tokio::io::duplex(64);
    drop(client);
    let frame = read_message(&mut server, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("clean eof");
    assert!(frame.is_none());
}

#[tokio::test]
async fn oversize_frame_is_rejected_before_its_body_is_read() {
    let (mut client, mut server) = tokio::io::duplex(4096);
    client
        .write_all(&1000_u32.to_be_bytes())
        .await
        .expect("write prefix");
    client.write_all(&[0_u8; 1000]).await.expect("write body");
    drop(client);

    let error = read_message(&mut server, 16)
        .await
        .expect_err("oversize frame");
    assert!(matches!(error, FrameError::TooLarge(1000)));

    // The body is still in the stream, so the rejection did not consume it.
    let mut unread = Vec::new();
    server.read_to_end(&mut unread).await.expect("drain stream");
    assert_eq!(unread.len(), 1000);
}

#[tokio::test]
async fn frame_at_exactly_the_limit_is_accepted() {
    let message = Message::Event {
        stream: 1,
        data: json!({ "pad": "x".repeat(300) }),
    };
    let frame = encode_frame(&message);
    let body_len = u32::try_from(frame.len() - 4).expect("body fits u32");

    let at_limit = read_from_bytes(&frame, body_len)
        .await
        .expect("frame at limit");
    assert_eq!(at_limit, Some(message));

    let over_limit = read_from_bytes(&frame, body_len - 1).await;
    assert!(matches!(over_limit, Err(FrameError::TooLarge(len)) if len == body_len));
}

#[tokio::test]
async fn unknown_frame_kind_is_an_error() {
    let error = read_from_bytes(&[0, 0, 0, 3, 0x02, b'{', b'}'], MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("unknown kind");
    assert!(matches!(error, FrameError::UnknownKind(0x02)));
}

#[tokio::test]
async fn invalid_json_payload_is_a_json_error() {
    let error = read_from_bytes(&[0, 0, 0, 4, 0x01, b'n', b'o', b't'], MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("invalid json");
    assert!(matches!(error, FrameError::Json(_)));
}

#[tokio::test]
async fn body_shorter_than_its_length_prefix_is_truncated() {
    let error = read_from_bytes(&[0, 0, 0, 10, 0x01, b'{', b'}'], MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("truncated body");
    assert!(matches!(error, FrameError::Truncated));
}

#[tokio::test]
async fn stream_ending_inside_the_length_prefix_is_truncated() {
    let error = read_from_bytes(&[0, 0], MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("truncated prefix");
    assert!(matches!(error, FrameError::Truncated));
}

#[tokio::test]
async fn zero_length_body_is_truncated_because_it_has_no_kind_byte() {
    let error = read_from_bytes(&[0, 0, 0, 0], MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("empty body");
    assert!(matches!(error, FrameError::Truncated));
}

#[tokio::test]
async fn write_message_writes_exactly_the_encoded_frame() {
    let message = Message::Cancel { stream: 9 };
    let mut bytes = Vec::new();
    write_message(&mut bytes, &message)
        .await
        .expect("write to vec");
    assert_eq!(bytes, encode_frame(&message));
}
