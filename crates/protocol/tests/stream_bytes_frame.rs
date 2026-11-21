#![cfg(feature = "tokio")]

use massorca_protocol::{
    Frame, FrameError, FrameKind, MAX_FRAME_LEN_LOCAL, Message, encode_frame, encode_stream_bytes,
    read_frame, read_message, write_frame,
};

#[test]
fn stream_bytes_kind_is_0x02_and_try_from_accepts_it() {
    assert_eq!(FrameKind::StreamBytes as u8, 0x02);
    assert_eq!(
        FrameKind::try_from(0x02_u8).expect("stream bytes kind"),
        FrameKind::StreamBytes
    );
}

#[test]
fn encoded_stream_bytes_frame_matches_the_documented_layout() {
    let frame = encode_stream_bytes(0x0102_0304_0506_0708, &[0xff, 0x00]);
    let expected: [u8; 15] = [0, 0, 0, 11, 0x02, 1, 2, 3, 4, 5, 6, 7, 8, 0xff, 0x00];
    assert_eq!(frame, expected);
}

#[test]
fn stream_bytes_frame_with_no_payload_is_a_nine_byte_body() {
    let expected: [u8; 13] = [0, 0, 0, 9, 0x02, 0, 0, 0, 0, 0, 0, 0, 1];
    assert_eq!(encode_stream_bytes(1, &[]), expected);
}

#[tokio::test]
async fn read_frame_reads_json_and_stream_bytes_back_to_back_in_order() {
    let mut bytes = encode_frame(&Message::Cancel { stream: 3 });
    bytes.extend(encode_stream_bytes(7, &[0xff, 0x00, 0x1b]));
    bytes.extend(encode_frame(&Message::Cancel { stream: 4 }));
    let mut reader: &[u8] = &bytes;

    let first = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("json frame");
    assert_eq!(first, Some(Frame::Message(Message::Cancel { stream: 3 })));
    let second = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("stream bytes frame");
    assert_eq!(
        second,
        Some(Frame::StreamBytes {
            stream: 7,
            payload: vec![0xff, 0x00, 0x1b],
        })
    );
    let third = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("second json frame");
    assert_eq!(third, Some(Frame::Message(Message::Cancel { stream: 4 })));
    let end = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("clean eof");
    assert_eq!(end, None);
}

#[tokio::test]
async fn read_frame_accepts_a_stream_bytes_frame_with_no_payload() {
    let bytes = encode_stream_bytes(1, &[]);
    let mut reader: &[u8] = &bytes;
    let frame = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("empty payload");
    assert_eq!(
        frame,
        Some(Frame::StreamBytes {
            stream: 1,
            payload: Vec::new(),
        })
    );
}

#[tokio::test]
async fn frames_round_trip_over_a_small_duplex_buffer() {
    // A 16-byte buffer forces the stream-bytes body through many partial reads and writes.
    let (mut client, mut server) = tokio::io::duplex(16);
    let sent = vec![
        Frame::Message(Message::Cancel { stream: 1 }),
        Frame::StreamBytes {
            stream: 2,
            payload: vec![0xff; 40],
        },
    ];
    let to_send = sent.clone();
    let writer = async move {
        for frame in &to_send {
            write_frame(&mut client, frame, MAX_FRAME_LEN_LOCAL)
                .await
                .expect("write frame");
        }
    };
    let reader = async move {
        let mut received = Vec::new();
        while let Some(frame) = read_frame(&mut server, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("read frame")
        {
            received.push(frame);
        }
        received
    };
    let ((), received) = tokio::join!(writer, reader);
    assert_eq!(received, sent);
}

#[tokio::test]
async fn write_frame_writes_exactly_the_bytes_of_its_encoder() {
    let mut json = Vec::new();
    write_frame(
        &mut json,
        &Frame::Message(Message::Cancel { stream: 9 }),
        MAX_FRAME_LEN_LOCAL,
    )
    .await
    .expect("write json frame");
    assert_eq!(json, encode_frame(&Message::Cancel { stream: 9 }));

    let mut binary = Vec::new();
    write_frame(
        &mut binary,
        &Frame::StreamBytes {
            stream: 9,
            payload: vec![1, 2, 3],
        },
        MAX_FRAME_LEN_LOCAL,
    )
    .await
    .expect("write stream bytes frame");
    assert_eq!(binary, encode_stream_bytes(9, &[1, 2, 3]));
}

#[tokio::test]
async fn write_frame_refuses_an_oversized_stream_bytes_frame_before_any_byte_is_written() {
    let frame = Frame::StreamBytes {
        stream: 1,
        payload: vec![0_u8; 100],
    };
    let body_len =
        u32::try_from(encode_stream_bytes(1, &[0_u8; 100]).len() - 4).expect("body fits u32");
    let mut bytes = Vec::new();

    let error = write_frame(&mut bytes, &frame, body_len - 1)
        .await
        .expect_err("over limit");
    assert!(matches!(error, FrameError::TooLarge(len) if len == body_len));
    assert!(bytes.is_empty(), "a refused frame writes no byte");
}

#[tokio::test]
async fn over_limit_stream_bytes_frame_is_rejected_before_its_body_is_read() {
    let frame = encode_stream_bytes(1, &[0_u8; 100]);
    let body_len = u32::try_from(frame.len() - 4).expect("body fits u32");
    let mut reader: &[u8] = &frame;

    let error = read_frame(&mut reader, body_len - 1)
        .await
        .expect_err("over limit");
    assert!(matches!(error, FrameError::TooLarge(len) if len == body_len));
    assert_eq!(reader.len(), frame.len() - 4, "the body is still unread");
}

#[tokio::test]
async fn stream_bytes_frame_cut_inside_its_payload_is_truncated() {
    let mut frame = encode_stream_bytes(1, &[7_u8; 10]);
    frame.truncate(frame.len() - 3);
    let mut reader: &[u8] = &frame;

    let error = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("cut frame");
    assert!(matches!(error, FrameError::Truncated));
}

#[tokio::test]
async fn stream_bytes_body_without_a_whole_stream_id_is_truncated() {
    let mut reader: &[u8] = &[0, 0, 0, 5, 0x02, 1, 2, 3, 4];

    let error = read_frame(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("short body");
    assert!(matches!(error, FrameError::Truncated));
}

#[tokio::test]
async fn read_message_reports_a_stream_bytes_frame_as_an_unknown_kind() {
    let bytes = encode_stream_bytes(7, b"x");
    let mut reader: &[u8] = &bytes;

    let error = read_message(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("json-only reader");
    assert!(matches!(error, FrameError::UnknownKind(0x02)));
}

#[tokio::test]
async fn read_message_reports_a_short_stream_bytes_body_as_an_unknown_kind() {
    // Existing behaviour: the kind decides before the stream id is looked at.
    let mut reader: &[u8] = &[0, 0, 0, 5, 0x02, 1, 2, 3, 4];

    let error = read_message(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect_err("json-only reader");
    assert!(matches!(error, FrameError::UnknownKind(0x02)));
}

#[tokio::test]
async fn read_message_consumes_a_rejected_stream_bytes_body_so_the_next_frame_reads() {
    let mut bytes = encode_stream_bytes(7, b"output");
    bytes.extend(encode_frame(&Message::Cancel { stream: 2 }));
    let mut reader: &[u8] = &bytes;

    let first = read_message(&mut reader, MAX_FRAME_LEN_LOCAL).await;
    assert!(matches!(first, Err(FrameError::UnknownKind(0x02))));
    let second = read_message(&mut reader, MAX_FRAME_LEN_LOCAL)
        .await
        .expect("next frame");
    assert_eq!(second, Some(Message::Cancel { stream: 2 }));
}
