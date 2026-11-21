//! One client's view of a terminal: a bounded queue the reader thread fills without
//! blocking, recovered by a fresh snapshot when the client falls behind (decision 0034).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use crate::manager::{Terminal, TerminalInfo, lock};

/// Items queued per attachment before it is marked lagging (a chunk is one PTY read).
pub const QUEUE_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachMode {
    /// Output continues from the client's `since`; nothing is lost or repeated.
    Resume,
    /// A snapshot of the current state comes first.
    Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachItem {
    /// Always first: output continues from `offset`.
    Attached {
        info: TerminalInfo,
        mode: AttachMode,
        offset: u64,
    },
    /// Escape bytes that redraw the terminal; output continues from `offset`.
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
    /// Last: the process ended. `next` returns `None` afterwards.
    Exited {
        info: TerminalInfo,
    },
}

#[derive(Debug, Default)]
pub(crate) struct Queue {
    pub(crate) items: VecDeque<AttachItem>,
    /// The queue overflowed; the next read takes a snapshot instead.
    pub(crate) lagging: bool,
    /// `Exited` was queued or is owed; nothing else is accepted.
    pub(crate) ended: bool,
}

/// The part of an attachment the terminal holds.
#[derive(Debug, Default)]
pub(crate) struct AttachShared {
    pub(crate) queue: Mutex<Queue>,
    notify: Notify,
}

impl AttachShared {
    /// Never blocks: the reader thread calls this with the terminal lock held.
    pub(crate) fn push(&self, item: AttachItem) {
        let mut queue = lock(&self.queue);
        if queue.ended {
            return;
        }
        let exited = matches!(item, AttachItem::Exited { .. });
        queue.ended = exited;
        if queue.lagging {
            // The snapshot taken on the next read covers this item, `Exited` included.
        } else if queue.items.len() >= QUEUE_LIMIT && !exited {
            // An undelivered `Attached` stays: it must come first.
            queue
                .items
                .retain(|item| matches!(item, AttachItem::Attached { .. }));
            queue.lagging = true;
        } else {
            queue.items.push_back(item);
        }
        drop(queue);
        self.notify.notify_one();
    }
}

/// A live view of one terminal's output. Dropping it detaches.
pub struct Attachment {
    terminal: Arc<Terminal>,
    shared: Arc<AttachShared>,
    offset: u64,
}

impl std::fmt::Debug for Attachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attachment")
            .field("terminal", &self.terminal.id())
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

impl Attachment {
    pub(crate) fn new(terminal: Arc<Terminal>, shared: Arc<AttachShared>) -> Self {
        Self {
            terminal,
            shared,
            offset: 0,
        }
    }

    /// The offset of the next output byte, after the items read so far.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// The next item, or `None` after `Exited`. Cancel-safe: an item is only taken when returned.
    pub async fn next(&mut self) -> Option<AttachItem> {
        loop {
            if let Some(item) = self.try_next() {
                return item;
            }
            // `notify_one` keeps a permit when nobody waits, so a push between the check
            // and this await is not missed.
            self.shared.notify.notified().await;
        }
    }

    fn try_next(&mut self) -> Option<Option<AttachItem>> {
        let mut queue = lock(&self.shared.queue);
        let item = match queue.items.pop_front() {
            Some(item) => item,
            None if queue.lagging => {
                drop(queue);
                self.terminal.resnapshot(&self.shared)
            }
            None if queue.ended => return Some(None),
            None => return None,
        };
        match &item {
            AttachItem::Attached { offset, .. } | AttachItem::Snapshot { offset, .. } => {
                self.offset = *offset;
            }
            AttachItem::Output { offset, bytes } => self.offset = offset + bytes.len() as u64,
            AttachItem::Resized { .. } | AttachItem::Exited { .. } => {}
        }
        Some(Some(item))
    }
}

impl Drop for Attachment {
    fn drop(&mut self) {
        self.terminal.detach(&self.shared);
    }
}
