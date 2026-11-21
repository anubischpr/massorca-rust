//! The attach stream of `terminal.attach`, decoded: events, snapshots reassembled from their parts, and output whose offsets
//! let a reconnect resume without losing or repeating bytes (decision 0035).

use massorca_protocol::methods::terminals::{
    AttachEvent, AttachMode, ChunkKind, Terminal, decode_chunk,
};
use serde_json::Value;

use crate::error::ClientError;
use crate::subscription::{StreamItem, Subscription};

/// One decoded item of an attach. Snapshot bytes arrive whole; output arrives trimmed to the offset it continues from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachUpdate {
    Attached {
        terminal: Terminal,
        mode: AttachMode,
        offset: u64,
    },
    Snapshot {
        offset: u64,
        cols: u16,
        rows: u16,
        bytes: Vec<u8>,
    },
    Output {
        offset: u64,
        bytes: Vec<u8>,
    },
    Resized {
        cols: u16,
        rows: u16,
    },
    Exited {
        terminal: Terminal,
    },
}

/// A snapshot whose parts are still arriving.
#[derive(Debug)]
struct PendingSnapshot {
    offset: u64,
    cols: u16,
    rows: u16,
    len: usize,
    bytes: Vec<u8>,
}

/// One attachment to a terminal. Dropping it detaches.
#[derive(Debug)]
pub struct TerminalAttachment {
    subscription: Subscription,
    next_offset: u64,
    snapshot: Option<PendingSnapshot>,
}

impl TerminalAttachment {
    pub(crate) fn new(subscription: Subscription) -> Self {
        Self {
            subscription,
            next_offset: 0,
            snapshot: None,
        }
    }

    /// Where output continues after the items read so far. Pass it as `since` to resume after a reconnect.
    pub fn next_offset(&self) -> u64 {
        self.next_offset
    }

    /// The next update, or `None` once the stream has ended. A failure the service reports, or a malformed event, is `Some(Err)`.
    pub async fn next(&mut self) -> Option<Result<AttachUpdate, ClientError>> {
        loop {
            let item = match self.subscription.next_item().await? {
                Ok(item) => item,
                Err(error) => return Some(Err(ClientError::Remote(error))),
            };
            let update = match item {
                StreamItem::Event(value) => match self.event(value) {
                    Ok(Some(update)) => update,
                    Ok(None) => continue,
                    Err(error) => return Some(Err(error)),
                },
                StreamItem::Bytes(payload) => match self.chunk(&payload) {
                    Some(update) => update,
                    None => continue,
                },
            };
            return Some(Ok(update));
        }
    }

    fn event(&mut self, value: Value) -> Result<Option<AttachUpdate>, ClientError> {
        let event: AttachEvent = serde_json::from_value(value)
            .map_err(|error| ClientError::Protocol(error.to_string()))?;
        Ok(match event {
            AttachEvent::Attached {
                terminal,
                mode,
                offset,
            } => {
                self.snapshot = None;
                self.next_offset = offset;
                Some(AttachUpdate::Attached {
                    terminal,
                    mode,
                    offset,
                })
            }
            AttachEvent::Snapshot {
                offset,
                cols,
                rows,
                len,
            } => {
                self.snapshot = Some(PendingSnapshot {
                    offset,
                    cols,
                    rows,
                    len: usize::try_from(len).unwrap_or(usize::MAX),
                    bytes: Vec::new(),
                });
                self.finish_snapshot_if_complete()
            }
            AttachEvent::Resized { cols, rows } => Some(AttachUpdate::Resized { cols, rows }),
            AttachEvent::Exited { terminal } => Some(AttachUpdate::Exited { terminal }),
            AttachEvent::Unknown => None,
        })
    }

    fn chunk(&mut self, payload: &[u8]) -> Option<AttachUpdate> {
        let chunk = decode_chunk(payload)?;
        match chunk.kind {
            ChunkKind::Snapshot => {
                let pending = self.snapshot.as_mut()?;
                if pending.offset != chunk.offset {
                    return None;
                }
                let room = pending.len.saturating_sub(pending.bytes.len());
                pending
                    .bytes
                    .extend_from_slice(&chunk.bytes[..chunk.bytes.len().min(room)]);
                self.finish_snapshot_if_complete()
            }
            ChunkKind::Output => self.output(chunk.offset, chunk.bytes),
            ChunkKind::Unknown(_) => None,
        }
    }

    /// Returns the snapshot once its announced length has arrived; output continues from its offset.
    fn finish_snapshot_if_complete(&mut self) -> Option<AttachUpdate> {
        let complete = self
            .snapshot
            .as_ref()
            .is_some_and(|pending| pending.bytes.len() >= pending.len);
        if !complete {
            return None;
        }
        let pending = self.snapshot.take()?;
        self.next_offset = pending.offset;
        Some(AttachUpdate::Snapshot {
            offset: pending.offset,
            cols: pending.cols,
            rows: pending.rows,
            bytes: pending.bytes,
        })
    }

    /// Skips bytes already read and trims an overlap. A chunk past `next_offset` is passed on at its own offset, so the gap shows.
    fn output(&mut self, offset: u64, bytes: &[u8]) -> Option<AttachUpdate> {
        let end = offset.saturating_add(bytes.len() as u64);
        if bytes.is_empty() || end <= self.next_offset {
            return None;
        }
        let skip = usize::try_from(self.next_offset.saturating_sub(offset)).unwrap_or(usize::MAX);
        let start = offset.max(self.next_offset);
        self.next_offset = end;
        Some(AttachUpdate::Output {
            offset: start,
            bytes: bytes[skip..].to_vec(),
        })
    }
}
