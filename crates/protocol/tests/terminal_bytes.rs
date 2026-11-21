//! Bytes that cross the terminal wire: base64 input and binary output chunks (decision 0035).

use massorca_protocol::methods::terminals;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Debug;

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

#[test]
fn input_params_carry_base64_of_the_raw_bytes() {
    // Not UTF-8 and not printable: the bytes must survive the JSON string unchanged.
    let params = terminals::TerminalInputParams::from_bytes("t-1", &[0xff, 0x00, 0x1b]);
    assert_wire(&params, &json!({ "terminal": "t-1", "data": "/wAb" }));
    assert_eq!(
        params.decode_data().expect("decodes"),
        vec![0xff, 0x00, 0x1b]
    );
}

#[test]
fn input_decode_rejects_text_that_is_not_padded_base64() {
    for data in ["not base64!", "AAA"] {
        let params = terminals::TerminalInputParams {
            terminal: "t-1".to_owned(),
            data: data.to_owned(),
        };
        assert_eq!(
            params.decode_data(),
            Err(terminals::InputDecodeError::InvalidBase64)
        );
    }
}

#[test]
fn input_decode_accepts_exactly_one_mebibyte_and_rejects_more() {
    let at_limit = vec![0_u8; terminals::MAX_INPUT_BYTES];
    let decoded = terminals::TerminalInputParams::from_bytes("t-1", &at_limit)
        .decode_data()
        .expect("at the limit");
    assert_eq!(decoded.len(), terminals::MAX_INPUT_BYTES);

    let over_limit = vec![0_u8; terminals::MAX_INPUT_BYTES + 1];
    let params = terminals::TerminalInputParams::from_bytes("t-1", &over_limit);
    assert_eq!(
        params.decode_data(),
        Err(terminals::InputDecodeError::TooLarge {
            len: terminals::MAX_INPUT_BYTES + 1
        })
    );
}

#[test]
fn chunk_is_a_tag_then_the_big_endian_offset_then_the_bytes() {
    let payload = terminals::encode_chunk(terminals::ChunkKind::Output, 0x0102, &[0xff, b'a']);
    let expected: [u8; 11] = [0x00, 0, 0, 0, 0, 0, 0, 0x01, 0x02, 0xff, b'a'];
    assert_eq!(payload, expected);
}

#[test]
fn chunk_round_trips_both_known_tags_with_non_utf8_bytes() {
    for kind in [terminals::ChunkKind::Output, terminals::ChunkKind::Snapshot] {
        let payload = terminals::encode_chunk(kind, 42, b"\xff\x00\x1b");
        let chunk = terminals::decode_chunk(&payload).expect("decodes");
        assert_eq!(
            chunk,
            terminals::Chunk {
                kind,
                offset: 42,
                bytes: b"\xff\x00\x1b",
            }
        );
    }
}

#[test]
fn chunk_with_an_unknown_tag_decodes_as_unknown() {
    let payload: [u8; 10] = [0x07, 0, 0, 0, 0, 0, 0, 0, 0x01, b'x'];
    let chunk = terminals::decode_chunk(&payload).expect("decodes");
    assert_eq!(
        chunk,
        terminals::Chunk {
            kind: terminals::ChunkKind::Unknown(0x07),
            offset: 1,
            bytes: b"x",
        }
    );
    assert_eq!(
        terminals::encode_chunk(terminals::ChunkKind::Unknown(0x07), 1, b"x"),
        payload
    );
}

#[test]
fn chunk_shorter_than_its_nine_byte_header_is_none() {
    let eight: [u8; 8] = [0; 8];
    assert!(terminals::decode_chunk(&[]).is_none());
    assert!(terminals::decode_chunk(&eight).is_none());
    let header_only: [u8; 9] = [0; 9];
    let chunk = terminals::decode_chunk(&header_only).expect("header only");
    assert!(chunk.bytes.is_empty());
}
