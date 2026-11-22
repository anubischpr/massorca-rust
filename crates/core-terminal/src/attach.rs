//! One client's view of a terminal: a bounded queue the reader thread fills without
//! blocking, recovered by a fresh snapshot when the client falls behind (decision 0034).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use crate::manager::{Terminal, TerminalInfo, lock};

/// Payload bytes queued per attachment before it is marked lagging: the 1 MiB a resume can
/// carry from the ring plus 1 MiB of live output. Snapshots count too.
pub const QUEUE_BYTES: usize = 2 * 1024 * 1024;
/// Attachments one terminal accepts at once.
pub const MAX_ATTACHMENTS: usize = 16;
/// A client that keeps falling behind gets at most one snapshot per this interval.
pub const RESNAPSHOT_INTERVAL: Duration = Duration::from_millis(250);

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

impl AttachItem {
    /// What the item counts against `QUEUE_BYTES`: its payload.
    fn queued_bytes(&self) -> usize {
        match self {
            Self::Snapshot { bytes, .. } | Self::Output { bytes, .. } => bytes.len(),
            Self::Attached { .. } | Self::Resized { .. } | Self::Exited { .. } => 0,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Queue {
    items: VecDeque<AttachItem>,
    /// Payload bytes in `items`.
    bytes: usize,
    /// The queue overflowed; the next read takes a snapshot instead.
    pub(crate) lagging: bool,
    /// `Exited` was queued or is owed; nothing else is accepted.
    pub(crate) ended: bool,
    /// When this attachment last got a snapshot.
    pub(crate) snapshot_at: Option<Instant>,
}

impl Queue {
    /// Queues `item` whatever the budget: the items an attach starts with.
    pub(crate) fn push_back(&mut self, item: AttachItem) {
        self.bytes += item.queued_bytes();
        self.items.push_back(item);
    }

    /// Queues a snapshot after the `Attached` item an attach queued before building it, unless
    /// the queue overflowed meanwhile: the next read then takes a fresh snapshot instead.
    pub(crate) fn insert_snapshot(&mut self, item: AttachItem) {
        if self.lagging {
            return;
        }
        let at = self
            .items
            .iter()
            .take_while(|queued| matches!(queued, AttachItem::Attached { .. }))
            .count();
        self.bytes += item.queued_bytes();
        self.items.insert(at, item);
    }

    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.bytes = 0;
    }

    fn pop_front(&mut self) -> Option<AttachItem> {
        let item = self.items.pop_front()?;
        self.bytes -= item.queued_bytes();
        Some(item)
    }
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
        } else if queue.bytes + item.queued_bytes() > QUEUE_BYTES && !exited {
            // An undelivered `Attached` stays: it must come first. It carries no payload.
            queue
                .items
                .retain(|item| matches!(item, AttachItem::Attached { .. }));
            queue.bytes = 0;
            queue.lagging = true;
        } else {
            queue.push_back(item);
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

enum Next {
    Ready(Option<AttachItem>),
    /// Nothing queued yet.
    Wait,
    /// Lagging, but the last snapshot is too recent: the next one is due then.
    SnapshotAt(Instant),
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

    /// Payload bytes queued and not yet read.
    #[doc(hidden)]
    pub fn queued_bytes(&self) -> usize {
        lock(&self.shared.queue).bytes
    }

    /// The next item, or `None` after `Exited`. Cancel-safe: an item is only taken when returned.
    pub async fn next(&mut self) -> Option<AttachItem> {
        loop {
            match self.try_next() {
                Next::Ready(item) => return item,
                // `notify_one` keeps a permit when nobody waits, so a push between the check
                // and this await is not missed.
                Next::Wait => self.shared.notify.notified().await,
                Next::SnapshotAt(at) => tokio::time::sleep_until(at.into()).await,
            }
        }
    }

    fn try_next(&mut self) -> Next {
        let mut queue = lock(&self.shared.queue);
        let item = match queue.pop_front() {
            Some(item) => item,
            None if queue.lagging => {
                if let Some(due) = queue.snapshot_at.map(|at| at + RESNAPSHOT_INTERVAL)
                    && Instant::now() < due
                {
                    return Next::SnapshotAt(due);
                }
                drop(queue);
                self.terminal.resnapshot(&self.shared)
            }
            None if queue.ended => return Next::Ready(None),
            None => return Next::Wait,
        };
        match &item {
            AttachItem::Attached { offset, .. } | AttachItem::Snapshot { offset, .. } => {
                self.offset = *offset;
            }
            AttachItem::Output { offset, bytes } => self.offset = offset + bytes.len() as u64,
            AttachItem::Resized { .. } | AttachItem::Exited { .. } => {}
        }
        Next::Ready(Some(item))
    }
}

impl Drop for Attachment {
    fn drop(&mut self) {
        self.terminal.detach(&self.shared);
    }
}
