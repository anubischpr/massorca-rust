//! A paired host's terminal, attached over that host's live connection (STAT-16, REM-07): the attach borrows the
//! connection of the host's link and opens none of its own. A lost connection is reported as out of contact; once the
//! link reconnects the attach starts again from a snapshot, as a local attach does (0034). Input and resize go to the
//! host, never to This PC's service.

use std::future::Future;
use std::sync::Arc;

use massorca_client::{
    AttachUpdate, Client, ClientError, LiveWatch, TerminalAttachment, is_transient,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::live_hosts::LiveHosts;
use super::refusal::Refusal;
use super::terminals::{AttachRole, TerminalWrite, spawn_writes};

/// One item of a paired host's terminal attach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostTerminalItem {
    /// Output and state changes, as a local attach reports them.
    Update(AttachUpdate),
    /// The host cannot be reached now. The attach keeps the last screen and tries again by itself.
    Unreachable,
    /// The terminal ended on the host, or the attach stopped without a loss.
    Finished,
    /// The host refused the attach for good, with the reason.
    Refused(Refusal),
}

/// The UI's side of one attach: its items, and the writer that types into the terminal and resizes it.
#[derive(Debug)]
pub struct HostTerminal {
    items: mpsc::UnboundedReceiver<HostTerminalItem>,
    writer: HostWriter,
}

impl HostTerminal {
    /// The next item, or `None` once the attach task has ended.
    pub async fn next(&mut self) -> Option<HostTerminalItem> {
        self.items.recv().await
    }

    /// The writer of this attach. Its clones type into the same terminal.
    pub fn writer(&self) -> HostWriter {
        self.writer.clone()
    }
}

/// Types into an attached host terminal and resizes it. Input sent while the host is out of contact is dropped.
#[derive(Clone, Debug)]
pub struct HostWriter {
    writes: mpsc::UnboundedSender<HostWrite>,
}

impl HostWriter {
    pub fn input(&self, bytes: Vec<u8>) {
        let _ = self.writes.send(HostWrite::Input(bytes));
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.writes.send(HostWrite::Resize { cols, rows });
    }

    /// Attaches again from a snapshot, for a screen that lost its place.
    pub fn reattach(&self) {
        let _ = self.writes.send(HostWrite::Reattach);
    }
}

/// What a writer asks of its attach.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HostWrite {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Reattach,
}

/// An attach as the bridge receives it: the host, the terminal and its grid, and the channels its items and writes
/// travel on.
#[derive(Debug)]
pub(crate) struct HostAttach {
    pub(crate) host: String,
    pub(crate) terminal: String,
    pub(crate) size: (u16, u16),
    pub(crate) items: mpsc::UnboundedSender<HostTerminalItem>,
    pub(crate) writes: mpsc::UnboundedReceiver<HostWrite>,
}

impl ServiceHandle {
    /// Attaches to `terminal` on the paired host `host` at `cols` x `rows`, over that host's live connection. The attach
    /// runs in the background from now on; the host's items come through the returned handle.
    pub fn attach_host_terminal(
        &self,
        host: &str,
        terminal: &str,
        cols: u16,
        rows: u16,
    ) -> HostTerminal {
        let (items_tx, items) = mpsc::unbounded_channel();
        let (writes_tx, writes) = mpsc::unbounded_channel();
        self.send(Command::HostTerminalAttach(HostAttach {
            host: host.to_owned(),
            terminal: terminal.to_owned(),
            size: (cols, rows),
            items: items_tx,
            writes,
        }));
        HostTerminal {
            items,
            writer: HostWriter { writes: writes_tx },
        }
    }
}

/// Starts an attach's task on the bridge's runtime. A host with no live link refuses the attach as not paired.
pub(super) fn spawn_attach(live: LiveHosts, attach: HostAttach) {
    let HostAttach {
        host,
        terminal,
        size,
        items,
        writes,
    } = attach;
    let Some(link) = live.watch(&host) else {
        let error = ClientError::HostNotFound(host);
        let _ = items.send(HostTerminalItem::Refused(Refusal::from(&error)));
        return;
    };
    let follow = Follow {
        link,
        terminal,
        size,
        items,
        writes,
        out_of_contact: false,
    };
    tokio::spawn(follow.run());
}

/// One attach on its task: the host's link, the terminal and the grid to show, and the channels to the UI.
struct Follow {
    link: LiveWatch,
    terminal: String,
    size: (u16, u16),
    items: mpsc::UnboundedSender<HostTerminalItem>,
    writes: mpsc::UnboundedReceiver<HostWrite>,
    /// Whether the UI has been told the host is out of contact, and not yet that it is back.
    out_of_contact: bool,
}

/// How one connection's attach ended.
enum Leg {
    /// The UI dropped the attach.
    Detached,
    /// The screen lost its place: attach again at once, from a snapshot.
    Reattach,
    /// The connection was lost: attach again once the link has reconnected.
    Lost,
    /// The terminal ended, or the attach stopped without a loss.
    Finished,
    /// The host refused the attach.
    Refused(Refusal),
}

impl Follow {
    async fn run(mut self) {
        loop {
            let size = self.size;
            let mut link = self.link.clone();
            let terminal = self.terminal.clone();
            let connected = match self
                .while_out_of_contact(attach(&mut link, terminal, size))
                .await
            {
                None => return,
                Some(connected) => connected,
            };
            // The connection taken is the one the link has now, so a later change means it was lost or replaced.
            self.link = link;
            match connected {
                Ok((client, attachment)) => match self.pump(client, attachment, size).await {
                    Leg::Detached => return,
                    Leg::Reattach => {}
                    Leg::Lost => {
                        if !self.lose().await {
                            return;
                        }
                    }
                    Leg::Finished => {
                        self.send(HostTerminalItem::Finished);
                        return;
                    }
                    Leg::Refused(refusal) => {
                        self.send(HostTerminalItem::Refused(refusal));
                        return;
                    }
                },
                Err(error) if is_transient(&error) => {
                    if !self.lose().await {
                        return;
                    }
                }
                Err(error) => {
                    self.send(HostTerminalItem::Refused(Refusal::from(&*error)));
                    return;
                }
            }
        }
    }

    /// Hands an item to the UI. False once the UI has dropped the attach.
    fn send(&self, item: HostTerminalItem) -> bool {
        self.items.send(item).is_ok()
    }

    /// Reports the host out of contact, once per loss, and waits for the link's connection to change. False once the UI
    /// has dropped the attach or the link has stopped (the host was forgotten).
    async fn lose(&mut self) -> bool {
        if !self.out_of_contact {
            self.out_of_contact = true;
            if !self.send(HostTerminalItem::Unreachable) {
                return false;
            }
        }
        let mut link = self.link.clone();
        let running = self.while_out_of_contact(link.changed()).await;
        self.link = link;
        running == Some(true)
    }

    /// Waits for `pending` while taking in the UI's writes: input is dropped, and a resize is kept for the next attach.
    /// `None` once the UI has dropped the attach.
    async fn while_out_of_contact<T>(&mut self, pending: impl Future<Output = T>) -> Option<T> {
        tokio::pin!(pending);
        loop {
            tokio::select! {
                output = &mut pending => return Some(output),
                () = self.items.closed() => return None,
                write = self.writes.recv() => match write {
                    Some(HostWrite::Resize { cols, rows }) => self.size = (cols, rows),
                    Some(HostWrite::Input(_) | HostWrite::Reattach) => {}
                    None => return None,
                },
            }
        }
    }

    /// Forwards one connection's attach to the UI, and the UI's input and resizes to the host, until the attach ends.
    /// `attached` is the grid the attach asked for; a resize taken in while connecting is sent at once.
    async fn pump(
        &mut self,
        client: Client,
        mut attachment: TerminalAttachment,
        attached: (u16, u16),
    ) -> Leg {
        let queue = spawn_writes(client.clone());
        if self.size != attached {
            let _ = queue.send(resize_write(&self.terminal, self.size));
        }
        loop {
            tokio::select! {
                () = self.items.closed() => return Leg::Detached,
                next = attachment.next() => match next {
                    Some(Ok(update)) => {
                        self.out_of_contact = false;
                        self.send(HostTerminalItem::Update(update));
                    }
                    Some(Err(error)) if is_transient(&error) => return Leg::Lost,
                    Some(Err(error)) => return Leg::Refused(Refusal::from(&error)),
                    None if attachment.connection_lost() => return Leg::Lost,
                    None => return Leg::Finished,
                },
                write = self.writes.recv() => match write {
                    Some(HostWrite::Input(bytes)) => {
                        let _ = queue.send(TerminalWrite::Input {
                            terminal: self.terminal.clone(),
                            bytes,
                        });
                    }
                    Some(HostWrite::Resize { cols, rows }) => {
                        self.size = (cols, rows);
                        let _ = queue.send(resize_write(&self.terminal, self.size));
                    }
                    Some(HostWrite::Reattach) => return Leg::Reattach,
                    None => return Leg::Detached,
                },
            }
        }
    }
}

/// Attaches `terminal` at `size`, from a snapshot, on the connection the host's link has once its attempt settles.
async fn attach(
    link: &mut LiveWatch,
    terminal: String,
    size: (u16, u16),
) -> Result<(Client, TerminalAttachment), Arc<ClientError>> {
    let client = link.client().await?;
    let mut params = AttachRole::AnswersQueries.attach_params(&terminal, None);
    params.cols = Some(size.0);
    params.rows = Some(size.1);
    let attachment = client.terminal_attach(&params).await.map_err(Arc::new)?;
    Ok((client, attachment))
}

/// A resize of `terminal` to `size`, whose answer nobody waits for.
fn resize_write(terminal: &str, (cols, rows): (u16, u16)) -> TerminalWrite {
    let (reply, _) = oneshot::channel();
    TerminalWrite::Resize {
        terminal: terminal.to_owned(),
        cols,
        rows,
        reply,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_terminal_attach_goes_to_the_bridge_as_a_host_call_only() {
        let (handle, mut script) = ServiceHandle::scripted();
        let _attach = handle.attach_host_terminal("vps", "t1", 100, 30);
        let Some(Command::HostTerminalAttach(attach)) = script.next() else {
            panic!("the attach is sent to the bridge as a host call");
        };
        assert_eq!(attach.host, "vps");
        assert_eq!(attach.terminal, "t1");
        assert_eq!(attach.size, (100, 30));
        assert!(script.next().is_none(), "nothing else is sent");
    }

    #[test]
    fn input_and_resize_go_to_the_attach_and_never_to_this_computers_service() {
        let (handle, mut script) = ServiceHandle::scripted();
        let attach = handle.attach_host_terminal("vps", "t1", 100, 30);
        let writer = attach.writer();
        let Some(Command::HostTerminalAttach(mut host)) = script.next() else {
            panic!("the attach is sent to the bridge as a host call");
        };
        writer.input(b"a".to_vec());
        writer.resize(120, 40);
        writer.reattach();
        assert_eq!(host.writes.try_recv(), Ok(HostWrite::Input(b"a".to_vec())));
        assert_eq!(
            host.writes.try_recv(),
            Ok(HostWrite::Resize {
                cols: 120,
                rows: 40
            })
        );
        assert_eq!(host.writes.try_recv(), Ok(HostWrite::Reattach));
        assert!(script.next().is_none(), "the local service got nothing");
    }
}
