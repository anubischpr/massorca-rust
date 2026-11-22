//! Terminal calls and attach streams on the bridge's runtime (decisions 0028, 0033, 0035).
//! Create, kill and close each run as their own task. Input and resize share one queue per
//! connection, so the service applies them in the order the UI sent them.

use massorca_client::{AttachUpdate, Client, ClientError, TerminalAttachment};
use massorca_protocol::methods::terminals::{
    Terminal, TerminalAttachParams, TerminalCreateParams, TerminalResizeParams,
};
use massorca_ui::components::terminal_surface::feed::FeedEvent;
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::StoreUpdate;

/// One item of an attach stream: output and state changes, then how the stream ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalStreamItem {
    Update(AttachUpdate),
    Ended(StreamEnd),
}

/// Why an attach stream stopped. It stops after an `Exited` update (`Finished`), when the
/// connection to the service is lost (`Lost`: attach again without `since`), or on a refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEnd {
    Finished,
    Lost,
    Failed(Refusal),
}

/// The UI's side of one attach. Dropping it detaches; the terminal keeps running.
#[derive(Debug)]
pub struct TerminalStream {
    items: mpsc::UnboundedReceiver<TerminalStreamItem>,
}

impl TerminalStream {
    /// The next item, or `None` once the bridge has dropped the stream.
    pub async fn next(&mut self) -> Option<TerminalStreamItem> {
        self.items.recv().await
    }
}

/// A terminal write that must keep its order on one connection.
pub(crate) enum TerminalWrite {
    Input {
        terminal: String,
        bytes: Vec<u8>,
    },
    Resize {
        terminal: String,
        cols: u16,
        rows: u16,
        reply: oneshot::Sender<Result<Terminal, Refusal>>,
    },
}

/// A terminal call from the UI, carried by `Command::Terminal`.
pub(crate) enum TerminalCommand {
    Create {
        params: TerminalCreateParams,
        reply: oneshot::Sender<Result<Terminal, Refusal>>,
    },
    Attach {
        params: TerminalAttachParams,
        reply: oneshot::Sender<Result<mpsc::UnboundedReceiver<TerminalStreamItem>, Refusal>>,
    },
    Kill {
        terminal: String,
        force: bool,
        reply: oneshot::Sender<Result<Terminal, Refusal>>,
    },
    Close {
        terminal: String,
        reply: oneshot::Sender<Result<Terminal, Refusal>>,
    },
    Write(TerminalWrite),
}

/// Starts the per-connection queue that applies input and resize in order. Dropping the
/// returned sender, as the session does when it ends, stops the queue.
pub(super) fn spawn_writes(client: Client) -> mpsc::UnboundedSender<TerminalWrite> {
    let (writes, mut queue) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(write) = queue.recv().await {
            match write {
                // Typing that the service refuses is dropped; the surface shows the state.
                TerminalWrite::Input { terminal, bytes } => {
                    let _ = client.terminal_input(&terminal, &bytes).await;
                }
                TerminalWrite::Resize {
                    terminal,
                    cols,
                    rows,
                    reply,
                } => {
                    let params = TerminalResizeParams {
                        terminal,
                        cols,
                        rows,
                    };
                    let result = client
                        .terminal_resize(&params)
                        .await
                        .map_err(|error| Refusal::from(&error));
                    let _ = reply.send(result);
                }
            }
        }
    });
    writes
}

/// Runs one terminal call on its own task, or queues a write. The store changes only from the
/// answer the service gave (0028), never from the request.
pub(super) fn spawn_call(
    client: &Client,
    writes: &mpsc::UnboundedSender<TerminalWrite>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    command: TerminalCommand,
) {
    let client = client.clone();
    match command {
        TerminalCommand::Write(write) => {
            let _ = writes.send(write);
        }
        TerminalCommand::Create { params, reply } => {
            let updates = updates.clone();
            tokio::spawn(async move {
                let result = client
                    .terminal_create(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                if let Ok(terminal) = &result {
                    let _ = updates.send(StoreUpdate::TerminalChanged(terminal.clone()));
                }
                let _ = reply.send(result);
            });
        }
        TerminalCommand::Kill {
            terminal,
            force,
            reply,
        } => {
            let updates = updates.clone();
            tokio::spawn(async move {
                let result = client
                    .terminal_kill(&terminal, force)
                    .await
                    .map_err(|error| Refusal::from(&error));
                if let Ok(terminal) = &result {
                    let _ = updates.send(StoreUpdate::TerminalChanged(terminal.clone()));
                }
                let _ = reply.send(result);
            });
        }
        TerminalCommand::Close { terminal, reply } => {
            let updates = updates.clone();
            tokio::spawn(async move {
                let result = client
                    .terminal_close(&terminal)
                    .await
                    .map_err(|error| Refusal::from(&error));
                if let Ok(closed) = &result {
                    let _ = updates.send(StoreUpdate::TerminalGone(closed.id.clone()));
                }
                let _ = reply.send(result);
            });
        }
        TerminalCommand::Attach { params, reply } => {
            tokio::spawn(async move {
                match client.terminal_attach(&params).await {
                    Ok(attachment) => {
                        let (items, received) = mpsc::unbounded_channel();
                        if reply.send(Ok(received)).is_ok() {
                            pump(attachment, items).await;
                        }
                    }
                    Err(error) => {
                        let _ = reply.send(Err(Refusal::from(&error)));
                    }
                }
            });
        }
    }
}

/// Forwards one attach to its UI stream until the UI drops it, the stream ends, or the
/// connection is lost. Dropping the attachment here is the detach.
async fn pump(
    mut attachment: TerminalAttachment,
    items: mpsc::UnboundedSender<TerminalStreamItem>,
) {
    let mut exited = false;
    loop {
        let next = tokio::select! {
            () = items.closed() => return,
            next = attachment.next() => next,
        };
        let item = match next {
            Some(Ok(update)) => {
                exited |= matches!(update, AttachUpdate::Exited { .. });
                TerminalStreamItem::Update(update)
            }
            Some(Err(error)) => TerminalStreamItem::Ended(StreamEnd::Failed(Refusal::from(&error))),
            None if exited => TerminalStreamItem::Ended(StreamEnd::Finished),
            None => TerminalStreamItem::Ended(StreamEnd::Lost),
        };
        let ended = matches!(item, TerminalStreamItem::Ended(_));
        if items.send(item).is_err() || ended {
            return;
        }
    }
}

/// While there is no connection a call cannot run, and says so at once. Input is dropped.
pub(super) fn refuse_while_down(command: TerminalCommand) {
    let refusal = || Refusal::unavailable("The service is not connected.");
    match command {
        TerminalCommand::Create { reply, .. }
        | TerminalCommand::Kill { reply, .. }
        | TerminalCommand::Close { reply, .. }
        | TerminalCommand::Write(TerminalWrite::Resize { reply, .. }) => {
            let _ = reply.send(Err(refusal()));
        }
        TerminalCommand::Attach { reply, .. } => {
            let _ = reply.send(Err(refusal()));
        }
        TerminalCommand::Write(TerminalWrite::Input { .. }) => {}
    }
}

/// Lists every terminal. A service that predates terminals answers with a remote error, which
/// lists none; a lost link is passed on so the session ends and reconnects.
pub(super) async fn list_terminals(client: &Client) -> Result<Vec<Terminal>, ClientError> {
    match client.terminal_list(None).await {
        Ok(list) => Ok(list.terminals),
        Err(ClientError::Remote(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Whether an attach's client answers the terminal's queries (decision 0036). Only a client that
/// declares it may hold a query; for any other client the service answers them itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachRole {
    /// A surface that feeds its output to an emulator and sends the replies back as input (0034).
    AnswersQueries,
    /// A client that only reads the output; it declares nothing, so the service answers.
    ReadsOutput,
}

impl AttachRole {
    /// The params of `terminal.attach` for this role, resuming from `since` when one is given.
    pub fn attach_params(self, terminal: &str, since: Option<u64>) -> TerminalAttachParams {
        TerminalAttachParams {
            terminal: terminal.to_owned(),
            since,
            cols: None,
            rows: None,
            answers_queries: match self {
                Self::AnswersQueries => Some(true),
                Self::ReadsOutput => None,
            },
        }
    }
}

/// The feed events one attach update stands for: a snapshot is its header, then its bytes (0034).
/// Every attached view feeds its emulator through this, so the desktop and its tests agree.
pub fn feed_events(update: &AttachUpdate) -> Vec<FeedEvent<'_>> {
    match update {
        AttachUpdate::Attached { offset, .. } => vec![FeedEvent::Attached { offset: *offset }],
        AttachUpdate::Snapshot {
            offset,
            cols,
            rows,
            bytes,
        } => vec![
            FeedEvent::Snapshot {
                offset: *offset,
                cols: *cols,
                rows: *rows,
                len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            },
            FeedEvent::SnapshotBytes(bytes),
        ],
        AttachUpdate::Output { offset, bytes } => vec![FeedEvent::Output {
            offset: *offset,
            bytes,
        }],
        AttachUpdate::Resized { cols, rows } => vec![FeedEvent::Resized {
            cols: *cols,
            rows: *rows,
        }],
        AttachUpdate::Exited { terminal } => vec![FeedEvent::Exited {
            code: terminal
                .exit
                .as_ref()
                .and_then(|exit| exit.code)
                .and_then(|code| i32::try_from(code).ok()),
        }],
    }
}

impl ServiceHandle {
    /// `terminal.create` in a workspace. The store changes through the resulting update.
    pub async fn create_terminal(&self, params: TerminalCreateParams) -> Result<Terminal, Refusal> {
        self.ask(|reply| Command::Terminal(TerminalCommand::Create { params, reply }))
            .await
    }

    /// `terminal.attach`. Without `since` the service sends a snapshot first.
    pub async fn attach_terminal(
        &self,
        params: TerminalAttachParams,
    ) -> Result<TerminalStream, Refusal> {
        let items = self
            .ask(|reply| Command::Terminal(TerminalCommand::Attach { params, reply }))
            .await?;
        Ok(TerminalStream { items })
    }

    /// `terminal.input`, queued in the order sent. Bytes sent while the link is down are dropped.
    pub fn send_terminal_input(&self, terminal: &str, bytes: Vec<u8>) {
        let _ = self.commands.send(Command::Terminal(TerminalCommand::Write(
            TerminalWrite::Input {
                terminal: terminal.to_owned(),
                bytes,
            },
        )));
    }

    /// `terminal.resize`, queued behind earlier input so the last size sent wins.
    pub async fn resize_terminal(
        &self,
        terminal: &str,
        cols: u16,
        rows: u16,
    ) -> Result<Terminal, Refusal> {
        self.ask(|reply| {
            Command::Terminal(TerminalCommand::Write(TerminalWrite::Resize {
                terminal: terminal.to_owned(),
                cols,
                rows,
                reply,
            }))
        })
        .await
    }

    /// `terminal.kill`: ends the process tree. The terminal stays listed as exited.
    pub async fn kill_terminal(&self, terminal: &str, force: bool) -> Result<Terminal, Refusal> {
        self.ask(|reply| {
            Command::Terminal(TerminalCommand::Kill {
                terminal: terminal.to_owned(),
                force,
                reply,
            })
        })
        .await
    }

    /// `terminal.close`: kills if live, then forgets the terminal.
    pub async fn close_terminal(&self, terminal: &str) -> Result<Terminal, Refusal> {
        self.ask(|reply| {
            Command::Terminal(TerminalCommand::Close {
                terminal: terminal.to_owned(),
                reply,
            })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;
    use crate::service::Script;

    #[test]
    fn an_answering_attach_declares_it_and_a_reading_one_declares_nothing() {
        let answers = AttachRole::AnswersQueries.attach_params("t1", Some(7));
        assert_eq!(answers.terminal, "t1");
        assert_eq!(answers.since, Some(7));
        assert_eq!(answers.answers_queries, Some(true));
        let reads = AttachRole::ReadsOutput.attach_params("t1", None);
        assert_eq!(
            reads.answers_queries, None,
            "the declaration is omitted, not false"
        );
    }

    /// The declaration an attach sends, read from the call the scripted service receives.
    async fn declared_by(
        handle: ServiceHandle,
        script: &mut Script,
        role: AttachRole,
    ) -> Option<bool> {
        let attach =
            tokio::spawn(
                async move { handle.attach_terminal(role.attach_params("t1", None)).await },
            );
        let (params, reply) = timeout(Duration::from_secs(5), async {
            loop {
                match script.next() {
                    Some(Command::Terminal(TerminalCommand::Attach { params, reply })) => {
                        return (params, reply);
                    }
                    Some(_) => {}
                    None => tokio::task::yield_now().await,
                }
            }
        })
        .await
        .expect("the attach reaches the service");
        reply
            .send(Err(Refusal::unavailable("answered by the test")))
            .expect("the attach waits for its answer");
        let _outcome = attach.await.expect("the attach task ends");
        params.answers_queries
    }

    #[tokio::test]
    async fn the_declaration_reaches_the_service_for_each_role() {
        let (handle, mut script) = ServiceHandle::scripted();
        let answers = declared_by(handle.clone(), &mut script, AttachRole::AnswersQueries);
        assert_eq!(
            answers.await,
            Some(true),
            "the pane declares that it answers"
        );
        let reads = declared_by(handle, &mut script, AttachRole::ReadsOutput);
        assert_eq!(reads.await, None, "a reader declares nothing");
    }
}
