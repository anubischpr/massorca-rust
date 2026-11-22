use massorca_protocol::{
    FrameError, FrameKind, MAX_FRAME_LEN_LOCAL, Message, encode_frame, fits_frame,
};
use serde_json::{Value, json};

#[test]
fn fits_frame_agrees_with_the_write_limit() {
    let message = Message::Cancel { stream: 3 };
    let body_len = u32::try_from(encode_frame(&message).len() - 4).expect("body fits u32");
    assert!(fits_frame(&message, body_len));
    assert!(!fits_frame(&message, body_len - 1));
}

#[test]
fn encoded_frame_is_big_endian_length_then_kind_then_json() {
    let frame = encode_frame(&Message::Cancel { stream: 3 });

    let body_len = u32::from_be_bytes(<[u8; 4]>::try_from(&frame[..4]).expect("four bytes"));
    assert_eq!(
        body_len as usize,
        frame.len() - 4,
        "the length counts the kind byte and the JSON"
    );
    assert_eq!(frame[4], 0x01);
    let payload: Value = serde_json::from_slice(&frame[5..]).expect("json payload");
    assert_eq!(payload, json!({ "type": "cancel", "stream": 3 }));
}

#[test]
fn json_and_stream_bytes_are_the_known_frame_kinds() {
    assert_eq!(
        FrameKind::try_from(0x01_u8).expect("json kind"),
        FrameKind::Json
    );
    assert_eq!(
        FrameKind::try_from(0x02_u8).expect("stream bytes kind"),
        FrameKind::StreamBytes
    );
    assert!(matches!(
        FrameKind::try_from(0x03_u8),
        Err(FrameError::UnknownKind(0x03))
    ));
}

#[test]
fn local_frame_limit_is_eight_mebibytes() {
    assert_eq!(MAX_FRAME_LEN_LOCAL, 8 * 1024 * 1024);
}

#[test]
fn a_frame_body_round_trips_without_its_length_prefix() {
    use massorca_protocol::{Frame, decode_frame_body, encode_frame_body};
    let frames = [
        Frame::Message(massorca_protocol::Message::Cancel { stream: 7 }),
        Frame::StreamBytes {
            stream: 3,
            payload: vec![0, 1, 2, 255],
        },
    ];
    for frame in frames {
        let body = encode_frame_body(&frame);
        assert_eq!(decode_frame_body(&body).expect("decodes"), frame);
    }
    assert!(decode_frame_body(&[]).is_err());
    assert!(decode_frame_body(&[0x02, 0, 0]).is_err());
    assert!(decode_frame_body(&[0x7f]).is_err());
}
