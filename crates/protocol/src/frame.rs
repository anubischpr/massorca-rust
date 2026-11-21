//! Stream framing: a `u32` big-endian body length, then the body. The body's first byte is the frame kind.

use crate::message::Message;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest body (kind byte included) a local connection accepts unless the peers negotiate otherwise.
pub const MAX_FRAME_LEN_LOCAL: u32 = 8 * 1024 * 1024;

/// The kind byte that opens every frame body. Kinds other than `Json` need a negotiated capability first.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// A UTF-8 JSON [`Message`].
    Json = 0x01,
}

impl TryFrom<u8> for FrameKind {
    type Error = FrameError;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0x01 => Ok(Self::Json),
            other => Err(FrameError::UnknownKind(other)),
        }
    }
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
    /// The stream ended inside a frame, or the body is empty and has no kind byte.
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

/// Reads one frame. `Ok(None)` means the peer closed the stream cleanly between frames.
///
/// `max_len` bounds the body, kind byte included. A longer frame fails with
/// [`FrameError::TooLarge`] before any body byte is read; the caller should then close the connection.
#[cfg(feature = "tokio")]
pub async fn read_message<R>(reader: &mut R, max_len: u32) -> Result<Option<Message>, FrameError>
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
    let (&kind, payload) = body.split_first().ok_or(FrameError::Truncated)?;
    match FrameKind::try_from(kind)? {
        FrameKind::Json => Ok(Some(serde_json::from_slice(payload)?)),
    }
}

/// Writes `message` as one frame and flushes the writer.
#[cfg(feature = "tokio")]
pub async fn write_message<W>(writer: &mut W, message: &Message) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    writer.write_all(&encode_frame(message)).await?;
    writer.flush().await?;
    Ok(())
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
