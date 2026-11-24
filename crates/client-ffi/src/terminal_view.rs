//! A terminal attached on a host and followed live (decision 0080, section 1): output arrives in batches, and input,
//! resizes and close go back to the host. The attach reconnects by itself and resumes after the output already read.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_client::{AttachStep, AttachUpdate, Backoff, Client, ClientError, ResilientAttach};
use massorca_protocol::methods::terminals::{Terminal, TerminalAttachParams, TerminalResizeParams};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};

use crate::error::FfiError;
use crate::massorca::{Host, run_on};
use crate::watch::{Feed, LinkState, Reach};
use crate::wire::word;

/// Most output one event carries: output that arrives while the app is busy joins the event up to this size.
const BATCH_BYTES: usize = 64 * 1024;

/// One step of a terminal view. Output comes in order, each run at the byte offset it starts at.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TerminalEvent {
    /// Output from `offset` on: everything that arrived since the last `next`, up to 64 KiB. Output past a lost gap
    /// starts a new run at its own offset.
    Output { offset: u64, bytes: Vec<u8> },
    /// The screen as the host holds it, from `offset` on. It opens the view, and it replaces the missed output when the
    /// host no longer holds that output.
    Snapshot {
        offset: u64,
        cols: u16,
        rows: u16,
        bytes: Vec<u8>,
    },
    /// The terminal was resized, here or from another attachment.
    Resized { cols: u16, rows: u16 },
    /// The process ended. The view ends after this event. `reason` is `exited`, `killed` or `service_stopped`.
    Exited {
        reason: String,
        code: Option<i64>,
        signal: Option<String>,
    },
    /// Whether the view can be trusted right now. Loss of contact is never shown as the terminal ending.
    Link { link: LinkState },
}

impl From<LinkState> for TerminalEvent {
    fn from(link: LinkState) -> Self {
        Self::Link { link }
    }
}

/// A live view of one terminal. `close`, or dropping it, detaches; the terminal keeps running on the host.
#[derive(uniffi::Object)]
pub struct TerminalView {
    feed: Feed<TerminalEvent>,
    terminal: String,
    host: String,
    runtime: Handle,
    live: Arc<Mutex<Live>>,
    sizes: watch::Sender<(u16, u16)>,
}

impl fmt::Debug for TerminalView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalView")
            .field("terminal", &self.terminal)
            .finish_non_exhaustive()
    }
}

#[uniffi::export]
impl TerminalView {
    /// The next event, or `Ok(None)` once the view is over: closed, or the attach ended. An `Err` is final.
    pub async fn next(&self) -> Result<Option<TerminalEvent>, FfiError> {
        self.feed.next().await
    }

    /// Types `bytes` into the terminal. Fails while the host is out of contact; the view reconnects by itself.
    pub async fn input(&self, bytes: Vec<u8>) -> Result<(), FfiError> {
        let client = self.connection()?;
        let terminal = self.terminal.clone();
        let host = self.host.clone();
        run_on(&self.runtime, async move {
            client
                .terminal_input(&terminal, &bytes)
                .await
                .map_err(|error| FfiError::from_client(error, &host))
        })
        .await
    }

    /// Sets the terminal's size on the host. A reconnect asks for this size again.
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<(), FfiError> {
        self.sizes.send_replace((cols, rows));
        let client = self.connection()?;
        let params = TerminalResizeParams {
            terminal: self.terminal.clone(),
            cols,
            rows,
        };
        let host = self.host.clone();
        run_on(&self.runtime, async move {
            client
                .terminal_resize(&params)
                .await
                .map(|_| ())
                .map_err(|error| FfiError::from_client(error, &host))
        })
        .await
    }

    /// Detaches; the terminal keeps running on the host. A `next` in progress returns `None`.
    pub fn close(&self) {
        lock(&self.live).close();
        self.feed.close();
    }
}

impl TerminalView {
    /// The connection input and resizes use. Refused once closed, and while the host is out of contact.
    fn connection(&self) -> Result<Client, FfiError> {
        let live = lock(&self.live);
        if live.closed {
            return Err(FfiError::Internal {
                message: "This terminal view is closed.".to_owned(),
            });
        }
        live.client
            .clone()
            .ok_or_else(|| FfiError::from_client(ClientError::Disconnected, &self.host))
    }
}

#[uniffi::export]
impl Host {
    /// Attaches to `terminal`, showing its output from now on and the screen first. The host's first answer is read
    /// here, so a refusal fails this call. After that the view reconnects by itself and resumes after the output it read.
    pub async fn attach_terminal(
        &self,
        terminal: String,
        cols: u16,
        rows: u16,
    ) -> Result<Arc<TerminalView>, FfiError> {
        let params = TerminalAttachParams {
            terminal: terminal.clone(),
            since: None,
            cols: Some(cols),
            rows: Some(rows),
            // The app is a terminal emulator: it answers the cursor queries of the program through `input`.
            answers_queries: Some(true),
        };
        let (reach, name) = (self.reach(), self.name.clone());
        let (stream, first) = run_on(&self.runtime, async move {
            let mut stream = ResilientAttach::new(reach, params, Backoff::default());
            let first = match stream.next().await {
                Some(Ok(step)) => Some(step),
                Some(Err(error)) => return Err(FfiError::from_client(error, &name)),
                None => None,
            };
            Ok((stream, first))
        })
        .await?;
        let live = Arc::new(Mutex::new(Live {
            client: stream.client().cloned(),
            closed: false,
        }));
        let (sizes, reattach_size) = watch::channel((cols, rows));
        let feed = Feed::spawn(&self.runtime, self.name.clone(), {
            let live = Arc::clone(&live);
            move |events| async move {
                attach_pump(stream, first, reattach_size, Arc::clone(&live), events).await;
                // The attach is over; nothing should keep its connection open.
                publish(&live, None);
            }
        });
        Ok(Arc::new(TerminalView {
            feed,
            terminal,
            host: self.name.clone(),
            runtime: self.runtime.clone(),
            live,
            sizes,
        }))
    }
}

/// The connection the attach holds now, for input and resizes. Once `closed`, nothing sets it again.
struct Live {
    client: Option<Client>,
    closed: bool,
}

impl Live {
    fn close(&mut self) {
        self.closed = true;
        self.client = None;
    }
}

/// Events read from the attach and not yet handed over, in order.
type Queue = VecDeque<Result<TerminalEvent, ClientError>>;

/// Reads the attach and hands its events over one at a time. Output that arrives while the app is busy joins the event
/// waiting for it, so a busy terminal is read in batches.
async fn attach_pump(
    mut stream: ResilientAttach<Reach>,
    first: Option<AttachStep>,
    mut reattach_size: watch::Receiver<(u16, u16)>,
    live: Arc<Mutex<Live>>,
    events: mpsc::Sender<Result<TerminalEvent, ClientError>>,
) {
    let mut queue = Queue::new();
    let mut ended = first.is_none();
    if let Some(event) = first.and_then(event_of) {
        queue_event(&mut queue, event);
    }
    loop {
        if ended && queue.is_empty() {
            return;
        }
        publish(&live, stream.client());
        tokio::select! {
            biased;
            permit = events.reserve(), if !queue.is_empty() => {
                let Ok(permit) = permit else { return };
                if let Some(item) = queue.pop_front() {
                    permit.send(item);
                }
            }
            changed = reattach_size.changed() => {
                if changed.is_err() {
                    return;
                }
                let (cols, rows) = *reattach_size.borrow_and_update();
                stream.set_size(cols, rows);
            }
            step = stream.next(), if !ended && has_room(&queue) => match step {
                Some(Ok(step)) => {
                    if let Some(event) = event_of(step) {
                        queue_event(&mut queue, event);
                    }
                }
                Some(Err(error)) => {
                    queue.push_back(Err(error));
                    ended = true;
                }
                None => ended = true,
            },
        }
    }
}

/// Keeps `live` on the connection the attach holds now, so that input reaches it.
fn publish(live: &Mutex<Live>, client: Option<&Client>) {
    let mut live = lock(live);
    if !live.closed {
        live.client = client.cloned();
    }
}

/// Whether the pump may read another step: it reads while nothing waits for the app, or while one output run that can
/// still grow waits.
fn has_room(queue: &Queue) -> bool {
    match queue.back() {
        None => true,
        Some(Ok(TerminalEvent::Output { bytes, .. })) => {
            queue.len() == 1 && bytes.len() < BATCH_BYTES
        }
        Some(_) => false,
    }
}

/// Queues `event`, joining it to the output run at the back when that run can take it.
fn queue_event(queue: &mut Queue, event: TerminalEvent) {
    if let TerminalEvent::Output { offset, bytes } = &event
        && let Some(Ok(run)) = queue.back_mut()
        && join_output(run, *offset, bytes)
    {
        return;
    }
    queue.push_back(Ok(event));
}

/// Appends `bytes` at `offset` to the output run `run`, when they are contiguous and the run stays within `BATCH_BYTES`.
fn join_output(run: &mut TerminalEvent, offset: u64, bytes: &[u8]) -> bool {
    match run {
        TerminalEvent::Output {
            offset: start,
            bytes: run_bytes,
        } if *start + run_bytes.len() as u64 == offset
            && run_bytes.len() + bytes.len() <= BATCH_BYTES =>
        {
            run_bytes.extend_from_slice(bytes);
            true
        }
        _ => false,
    }
}

/// The event an attach step is for; `None` for the attach's own announcement, which the view does not show.
fn event_of(step: AttachStep) -> Option<TerminalEvent> {
    Some(match step {
        AttachStep::Update(AttachUpdate::Attached { .. }) => return None,
        AttachStep::Update(AttachUpdate::Output { offset, bytes }) => {
            TerminalEvent::Output { offset, bytes }
        }
        AttachStep::Update(AttachUpdate::Snapshot {
            offset,
            cols,
            rows,
            bytes,
        }) => TerminalEvent::Snapshot {
            offset,
            cols,
            rows,
            bytes,
        },
        AttachStep::Update(AttachUpdate::Resized { cols, rows }) => {
            TerminalEvent::Resized { cols, rows }
        }
        AttachStep::Update(AttachUpdate::Exited { terminal }) => exited(terminal),
        AttachStep::Link(link) => TerminalEvent::Link {
            link: LinkState::from(link),
        },
    })
}

fn exited(terminal: Terminal) -> TerminalEvent {
    match terminal.exit {
        Some(exit) => TerminalEvent::Exited {
            reason: word(exit.reason),
            code: exit.code,
            signal: exit.signal,
        },
        None => TerminalEvent::Exited {
            reason: "unknown".to_owned(),
            code: None,
            signal: None,
        },
    }
}

/// The guard of `mutex`; a view stays usable after a panic elsewhere holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(offset: u64, len: usize) -> TerminalEvent {
        TerminalEvent::Output {
            offset,
            bytes: vec![b'x'; len],
        }
    }

    /// The `(offset, length)` of each output run in `queue`, in order.
    fn runs(queue: &Queue) -> Vec<(u64, usize)> {
        queue
            .iter()
            .filter_map(|item| match item {
                Ok(TerminalEvent::Output { offset, bytes }) => Some((*offset, bytes.len())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn contiguous_output_joins_one_run() {
        let mut queue = Queue::new();
        queue_event(&mut queue, output(0, 10));
        queue_event(&mut queue, output(10, 5));
        assert_eq!(runs(&queue), vec![(0, 15)]);
    }

    #[test]
    fn output_past_a_gap_starts_a_new_run_at_its_own_offset() {
        let mut queue = Queue::new();
        queue_event(&mut queue, output(0, 10));
        queue_event(&mut queue, output(20, 5));
        assert_eq!(runs(&queue), vec![(0, 10), (20, 5)]);
    }

    #[test]
    fn a_run_never_grows_past_the_batch_limit() {
        let limit = BATCH_BYTES as u64;
        let mut queue = Queue::new();
        queue_event(&mut queue, output(0, BATCH_BYTES - 10));
        queue_event(&mut queue, output(limit - 10, 10));
        queue_event(&mut queue, output(limit, 1));
        assert_eq!(runs(&queue), vec![(0, BATCH_BYTES), (limit, 1)]);
    }

    #[test]
    fn other_events_are_never_joined() {
        let mut queue = Queue::new();
        queue_event(&mut queue, output(0, 3));
        queue_event(&mut queue, TerminalEvent::Resized { cols: 80, rows: 24 });
        queue_event(&mut queue, output(3, 3));
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn the_pump_reads_on_only_while_the_queued_output_can_grow() {
        let mut queue = Queue::new();
        assert!(has_room(&queue));
        queue_event(&mut queue, output(0, 10));
        assert!(has_room(&queue), "a short run may still grow");
        queue_event(&mut queue, output(10, BATCH_BYTES));
        assert!(!has_room(&queue), "two events wait for the app");

        let mut full = Queue::new();
        queue_event(&mut full, output(0, BATCH_BYTES));
        assert!(!has_room(&full), "a full run cannot grow");
    }
}
