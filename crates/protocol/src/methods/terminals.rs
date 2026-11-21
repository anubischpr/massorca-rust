//! Terminal methods: shells in pseudo-terminals that outlive their clients (capability `terminals/1`, decision 0035).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Params: [`TerminalCreateParams`]. Result: [`Terminal`].
pub const CREATE: &str = "terminal.create";
/// Params: [`TerminalListParams`]. Result: [`TerminalList`].
pub const LIST: &str = "terminal.list";
/// Params: [`TerminalRef`]. Result: [`Terminal`].
pub const SHOW: &str = "terminal.show";
/// Params: [`TerminalAttachParams`]. Result: `{ stream }`, as `events.subscribe`; then [`AttachEvent`]s and binary chunks.
pub const ATTACH: &str = "terminal.attach";
/// Params: [`TerminalInputParams`]. Result: `null`.
pub const INPUT: &str = "terminal.input";
/// Params: [`TerminalResizeParams`]. Result: [`Terminal`].
pub const RESIZE: &str = "terminal.resize";
/// Params: [`TerminalKillParams`]. Result: [`Terminal`].
pub const KILL: &str = "terminal.kill";
/// Params: [`TerminalRef`]. Result: [`Terminal`].
pub const CLOSE: &str = "terminal.close";

/// Most bytes one `terminal.input` call may write, after base64 decoding.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Size of a new terminal when `cols` or `rows` is omitted.
pub const DEFAULT_COLS: u16 = 120;
pub const DEFAULT_ROWS: u16 = 40;
/// Accepted terminal sizes, inclusive.
pub const MIN_COLS: u16 = 2;
pub const MAX_COLS: u16 = 1000;
pub const MIN_ROWS: u16 = 1;
pub const MAX_ROWS: u16 = 500;
/// Most output or snapshot bytes in one binary chunk, excluding its header (0035: 64 KiB plus the header).
pub const MAX_CHUNK_BYTES: usize = 64 * 1024;

/// A program to run in the terminal instead of the default shell; never text typed into a shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellSpec {
    pub program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

/// Params of `terminal.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCreateParams {
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellSpec>,
    /// Relative to the workspace root, or absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Added to the inherited environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A retry with the same id returns the first result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Params of `terminal.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// Result of `terminal.list`: live and exited terminals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalList {
    pub terminals: Vec<Terminal>,
}

/// Selects one terminal by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRef {
    pub terminal: String,
}

/// Params of `terminal.attach`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalAttachParams {
    pub terminal: String,
    /// The output offset the client has read up to; resumes there while the ring still holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    /// Applied before the attach starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
}

/// Params of `terminal.input`. `data` is standard base64 with padding of the bytes to write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalInputParams {
    pub terminal: String,
    pub data: String,
}

impl TerminalInputParams {
    /// Params that type `bytes` into `terminal`.
    pub fn from_bytes(terminal: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            terminal: terminal.into(),
            data: STANDARD.encode(bytes),
        }
    }

    /// The bytes to write. Fails on bad base64 and on more than [`MAX_INPUT_BYTES`] decoded bytes.
    pub fn decode_data(&self) -> Result<Vec<u8>, InputDecodeError> {
        let bytes = STANDARD
            .decode(&self.data)
            .map_err(|_| InputDecodeError::InvalidBase64)?;
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(InputDecodeError::TooLarge { len: bytes.len() });
        }
        Ok(bytes)
    }
}

/// Why the `data` of `terminal.input` cannot be written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputDecodeError {
    #[error("terminal input is not padded standard base64")]
    InvalidBase64,
    #[error("terminal input decodes to {len} bytes, above the per-call limit")]
    TooLarge { len: usize },
}

/// Params of `terminal.resize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalResizeParams {
    pub terminal: String,
    pub cols: u16,
    pub rows: u16,
}

/// Params of `terminal.kill`. `force` skips the grace period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalKillParams {
    pub terminal: String,
    #[serde(default)]
    pub force: bool,
}

/// Whether the host that runs the process can prove it is alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    Live,
    /// The host cannot prove either way right now; never read as death.
    Unverifiable,
    Exited,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Why a terminal ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    /// It ended on its own.
    Exited,
    /// Stopped by `terminal.kill` or `terminal.close`.
    Killed,
    /// The service stopped, which kills every terminal.
    ServiceStopped,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// How an exited terminal ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalExit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    pub reason: ExitReason,
    pub at_ms: i64,
}

/// A terminal of a workspace. `exit` is set once it has exited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terminal {
    pub id: String,
    pub workspace_id: String,
    pub state: TerminalState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub cols: u16,
    pub rows: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The program the terminal runs.
    pub shell: String,
    pub cwd: String,
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<TerminalExit>,
}

/// How an attach starts: output resumes from `since`, or a snapshot redraws the screen first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachMode {
    Resume,
    Snapshot,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// A JSON item of an attach stream, tagged by `kind`. Readers ignore kinds they do not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttachEvent {
    /// The first item: the terminal, how the attach started and the offset output continues from.
    Attached {
        terminal: Terminal,
        mode: AttachMode,
        offset: u64,
    },
    /// A redraw follows: `len` bytes of snapshot chunks, announced at `offset`.
    Snapshot {
        offset: u64,
        cols: u16,
        rows: u16,
        len: u64,
    },
    Resized {
        cols: u16,
        rows: u16,
    },
    /// The process ended; the stream ends after this item.
    Exited {
        terminal: Terminal,
    },
    /// A kind this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Bytes before a chunk's data: the tag and the `u64` offset.
const CHUNK_HEADER_LEN: usize = 9;

/// What a binary chunk carries, read from its tag byte (0035).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    /// Tag `0x00`: PTY output starting at the chunk's offset.
    Output,
    /// Tag `0x01`: part of the snapshot that the preceding `snapshot` event announced.
    Snapshot,
    /// Any other tag; readers ignore these.
    Unknown(u8),
}

impl ChunkKind {
    fn tag(self) -> u8 {
        match self {
            Self::Output => 0x00,
            Self::Snapshot => 0x01,
            Self::Unknown(tag) => tag,
        }
    }

    fn from_tag(tag: u8) -> Self {
        match tag {
            0x00 => Self::Output,
            0x01 => Self::Snapshot,
            other => Self::Unknown(other),
        }
    }
}

/// One decoded binary chunk. `bytes` borrows from the frame payload it came in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub kind: ChunkKind,
    /// Offset of `bytes[0]`; for a snapshot part, the snapshot's offset.
    pub offset: u64,
    pub bytes: &'a [u8],
}

/// Encodes a chunk: tag, `offset` (u64 big-endian), then `bytes`.
pub fn encode_chunk(kind: ChunkKind, offset: u64, bytes: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(CHUNK_HEADER_LEN + bytes.len());
    payload.push(kind.tag());
    payload.extend_from_slice(&offset.to_be_bytes());
    payload.extend_from_slice(bytes);
    payload
}

/// Decodes a chunk; `None` when the payload is shorter than the 9-byte header.
pub fn decode_chunk(payload: &[u8]) -> Option<Chunk<'_>> {
    let (&tag, rest) = payload.split_first()?;
    let (offset, bytes) = rest.split_first_chunk::<8>()?;
    Some(Chunk {
        kind: ChunkKind::from_tag(tag),
        offset: u64::from_be_bytes(*offset),
        bytes,
    })
}
