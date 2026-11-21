//! Stream framing: a `u32` big-endian body length, then the body. The body's first byte is the frame kind.

use crate::message::Message;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest body (kind byte included) a local connection accepts unless the peers negotiate otherwise.
pub const MAX_FRAME_LEN_LOCAL: u32 = 8 * 1024 * 1024;

/// The `u32` length prefix that opens every frame.
const LENGTH_PREFIX_LEN: usize = 4;

/// Body bytes before a stream-bytes payload: the kind byte and the `u64` stream id.
const STREAM_BYTES_HEADER_LEN: usize = 9;

/// The kind byte that opens every frame body. Kinds other than `Json` need a negotiated capability first.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// A UTF-8 JSON [`Message`].
    Json = 0x01,
    /// `stream` (u64 big-endian), then an opaque payload. Needs `binary-frames/1`.
    StreamBytes = 0x02,
}

impl TryFrom<u8> for FrameKind {
    type Error = FrameError;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0x01 => Ok(Self::Json),
            0x02 => Ok(Self::StreamBytes),
            other => Err(FrameError::UnknownKind(other)),
        }
    }
}

/// One decoded frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Message(Message),
    /// Bytes of one stream; what the payload means belongs to that stream.
    StreamBytes {
        stream: u64,
        payload: Vec<u8>,
    },
}

/// Why a frame could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame i/o failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame body of {0} bytes exceeds the limit")]
    TooLarge(u32),
    #[error("unknown frame kind 0x{0:02x}")]
    UnknownKind(u8),
    #[error("frame payload is not a valid message: {0}")]
    Json(#[from] serde_json::Error),
    /// The stream ended inside a frame, or the body is too short for its kind: no kind byte, or no stream id.
    #[error("frame is truncated")]
    Truncated,
}

/// Encodes `message` as one frame: `u32` big-endian body length, kind byte, JSON.
///
/// # Panics
///
/// Only if the body would exceed `u32::MAX` bytes.
pub fn encode_frame(message: &Message) -> Vec<u8> {
    let json = serde_json::to_vec(message).expect("protocol messages always serialize");
    let body_len = u32::try_from(json.len() + 1).expect("frame body exceeds u32::MAX bytes");
    let mut frame = Vec::with_capacity(json.len() + 5);
    frame.extend_from_slice(&body_len.to_be_bytes());
    frame.push(FrameKind::Json as u8);
    frame.extend_from_slice(&json);
    frame
}

/// Encodes stream bytes as one frame: `u32` big-endian body length, kind `0x02`, `stream`, payload.
///
/// # Panics
///
/// Only if the body would exceed `u32::MAX` bytes.
pub fn encode_stream_bytes(stream: u64, payload: &[u8]) -> Vec<u8> {
    let body_len = u32::try_from(STREAM_BYTES_HEADER_LEN + payload.len())
        .expect("frame body exceeds u32::MAX bytes");
    let mut frame = Vec::with_capacity(4 + STREAM_BYTES_HEADER_LEN + payload.len());
    frame.extend_from_slice(&body_len.to_be_bytes());
    frame.push(FrameKind::StreamBytes as u8);
    frame.extend_from_slice(&stream.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Reads one frame. `Ok(None)` means the peer closed the stream cleanly between frames.
///
/// `max_len` bounds the body, kind byte included. A longer frame fails with
/// [`FrameError::TooLarge`] before any body byte is read; the caller should then close the connection.
#[cfg(feature = "tokio")]
pub async fn read_frame<R>(reader: &mut R, max_len: u32) -> Result<Option<Frame>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let Some(body) = read_body(reader, max_len).await? else {
        return Ok(None);
    };
    let (&kind, payload) = body.split_first().ok_or(FrameError::Truncated)?;
    match FrameKind::try_from(kind)? {
        FrameKind::Json => Ok(Some(Frame::Message(serde_json::from_slice(payload)?))),
        FrameKind::StreamBytes => {
            let (stream, rest) = payload
                .split_first_chunk::<8>()
                .ok_or(FrameError::Truncated)?;
            Ok(Some(Frame::StreamBytes {
                stream: u64::from_be_bytes(*stream),
                payload: rest.to_vec(),
            }))
        }
    }
}

/// Reads one JSON frame. `Ok(None)` means the peer closed the stream cleanly between frames.
///
/// A binary kind fails with [`FrameError::UnknownKind`] only after its whole body is consumed,
/// so the stream stays framed. [`read_frame`] decodes binary kinds.
#[cfg(feature = "tokio")]
pub async fn read_message<R>(reader: &mut R, max_len: u32) -> Result<Option<Message>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let Some(body) = read_body(reader, max_len).await? else {
        return Ok(None);
    };
    let (&kind, payload) = body.split_first().ok_or(FrameError::Truncated)?;
    match FrameKind::try_from(kind)? {
        FrameKind::Json => Ok(Some(serde_json::from_slice(payload)?)),
        // Judged by the kind alone, so a short stream-bytes body is unknown here, not truncated.
        FrameKind::StreamBytes => Err(FrameError::UnknownKind(kind)),
    }
}

/// Writes `message` as one frame and flushes the writer. A frame longer than `max_len` (kind byte included) fails with
/// [`FrameError::TooLarge`] before any byte is written, so the stream stays at a frame boundary.
#[cfg(feature = "tokio")]
pub async fn write_message<W>(
    writer: &mut W,
    message: &Message,
    max_len: u32,
) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    write_encoded(writer, &encode_frame(message), max_len).await
}

/// Writes `frame` as one frame and flushes the writer, under the same `max_len` check as [`write_message`].
#[cfg(feature = "tokio")]
pub async fn write_frame<W>(writer: &mut W, frame: &Frame, max_len: u32) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    let bytes = match frame {
        Frame::Message(message) => encode_frame(message),
        Frame::StreamBytes { stream, payload } => encode_stream_bytes(*stream, payload),
    };
    write_encoded(writer, &bytes, max_len).await
}

/// Writes one encoded frame if its body fits `max_len`, else fails before writing any byte.
#[cfg(feature = "tokio")]
async fn write_encoded<W>(writer: &mut W, bytes: &[u8], max_len: u32) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    let len = u32::try_from(bytes.len() - LENGTH_PREFIX_LEN)
        .expect("encoded frames bound the body to u32");
    if len > max_len {
        return Err(FrameError::TooLarge(len));
    }
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one frame body, kind byte included. The length limit and truncation checks are shared by every reader.
#[cfg(feature = "tokio")]
async fn read_body<R>(reader: &mut R, max_len: u32) -> Result<Option<Vec<u8>>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let Some(prefix) = read_length_prefix(reader).await? else {
        return Ok(None);
    };
    let len = u32::from_be_bytes(prefix);
    if len > max_len {
        return Err(FrameError::TooLarge(len));
    }
    // `take` grows the buffer only as bytes arrive, so a lying length cannot allocate `len` up front.
    let mut body = Vec::new();
    reader.take(u64::from(len)).read_to_end(&mut body).await?;
    if body.len() as u64 != u64::from(len) {
        return Err(FrameError::Truncated);
    }
    Ok(Some(body))
}

/// Whether `message` fits in one frame of `max_len` bytes, the check [`write_message`] makes.
pub fn fits_frame(message: &Message, max_len: u32) -> bool {
    let body = encode_frame(message).len() - LENGTH_PREFIX_LEN;
    u64::try_from(body).is_ok_and(|body| body <= u64::from(max_len))
}

/// Reads the 4-byte length prefix; `None` if the stream ends before its first byte.
#[cfg(feature = "tokio")]
async fn read_length_prefix<R>(reader: &mut R) -> Result<Option<[u8; 4]>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0_u8; 4];
    let mut filled = 0;
    while filled < prefix.len() {
        let read = reader.read(&mut prefix[filled..]).await?;
        if read == 0 {
            return if filled == 0 {
                Ok(None)
            } else {
                Err(FrameError::Truncated)
            };
        }
        filled += read;
    }
    Ok(Some(prefix))
}
