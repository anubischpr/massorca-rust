//! The Marketplace's extension calls (decision 0062). The installed packages and the built-in catalogue are read
//! on each connect and after an `extension.*` event. A write answers its caller, then both lists are read again,
//! so the store follows the service and never the answer. Each read takes a number and the store keeps the newest.

use std::future::Future;
use std::sync::Arc;

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::extensions::{
    ExtensionEnabled, ExtensionInstallParams, ExtensionInstalled, ExtensionItem, ExtensionList,
    ExtensionRemoveParams, ExtensionRemoved, ExtensionReview, ExtensionSetEnabledParams,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::teams::Sequence;
use super::update::StoreUpdate;

/// The installed packages and the built-in catalogue, read together. Each list is refused or listed on its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionsRead {
    pub number: u64,
    /// The packages this service has installed, with their state, in install order.
    pub installed: Result<Vec<ExtensionItem>, Refusal>,
    /// Every built-in catalogue entry with its state, installed or not.
    pub catalogue: Result<Vec<ExtensionItem>, Refusal>,
}

/// A Marketplace call the UI makes. Each answers on its oneshot; the store changes through the reads that follow.
pub(crate) enum ExtensionCommand {
    /// `extensions.inspect`: the review of a source. Changes nothing, so no read follows it.
    Inspect {
        source: String,
        reply: oneshot::Sender<Result<ExtensionReview, Refusal>>,
    },
    Install {
        params: ExtensionInstallParams,
        reply: oneshot::Sender<Result<ExtensionInstalled, Refusal>>,
    },
    SetEnabled {
        params: ExtensionSetEnabledParams,
        reply: oneshot::Sender<Result<ExtensionEnabled, Refusal>>,
    },
    Remove {
        params: ExtensionRemoveParams,
        reply: oneshot::Sender<Result<ExtensionRemoved, Refusal>>,
    },
}

/// Reads both lists. A refusal from the service is the answer for its list; a lost link is passed on, so the
/// bridge reconnects instead of showing the packages as absent.
pub(super) async fn read(client: &Client, reads: &Sequence) -> Result<ExtensionsRead, ClientError> {
    let number = reads.next();
    let (installed, catalogue) =
        tokio::join!(client.extension_list(), client.extension_catalogue());
    Ok(ExtensionsRead {
        number,
        installed: part(installed)?,
        catalogue: part(catalogue)?,
    })
}

fn part(
    result: Result<ExtensionList, ClientError>,
) -> Result<Result<Vec<ExtensionItem>, Refusal>, ClientError> {
    match result {
        Ok(list) => Ok(Ok(list.extensions)),
        Err(error @ (ClientError::Remote(_) | ClientError::Protocol(_))) => {
            Ok(Err(Refusal::from(&error)))
        }
        Err(error) => Err(error),
    }
}

impl ServiceHandle {
    /// `extensions.inspect`: the review of a source the person is about to install. Nothing changes on the service.
    pub async fn inspect_extension(&self, source: String) -> Result<ExtensionReview, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Inspect { source, reply }))
            .await
    }

    /// `extensions.install`. The list changes through the reads that follow the answer, not from the answer.
    pub async fn install_extension(
        &self,
        params: ExtensionInstallParams,
    ) -> Result<ExtensionInstalled, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Install { params, reply }))
            .await
    }

    /// `extensions.set_enabled`. The list changes through the reads that follow the answer.
    pub async fn set_extension_enabled(
        &self,
        params: ExtensionSetEnabledParams,
    ) -> Result<ExtensionEnabled, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::SetEnabled { params, reply }))
            .await
    }

    /// `extensions.remove`. The list changes through the reads that follow the answer.
    pub async fn remove_extension(
        &self,
        params: ExtensionRemoveParams,
    ) -> Result<ExtensionRemoved, Refusal> {
        self.ask(|reply| Command::Extension(ExtensionCommand::Remove { params, reply }))
            .await
    }
}

/// Runs one Marketplace call on its own task, so a slow installer never holds up events.
pub(super) fn spawn_call(
    client: &Client,
    command: ExtensionCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    reads: &Arc<Sequence>,
) {
    match command {
        ExtensionCommand::Inspect { source, reply } => {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client
                    .extension_inspect(&source)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        ExtensionCommand::Install { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                client.extension_install(&params).await
            });
        }
        ExtensionCommand::SetEnabled { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                client.extension_set_enabled(&params).await
            });
        }
        ExtensionCommand::Remove { params, reply } => {
            write(client, updates, reads, reply, move |client| async move {
                client.extension_remove(&params).await
            });
        }
    }
}

/// Runs a write, answers its caller, then reads both lists again whatever the answer was.
fn write<T, Fut>(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    reads: &Arc<Sequence>,
    reply: oneshot::Sender<Result<T, Refusal>>,
    call: impl FnOnce(Client) -> Fut + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T, ClientError>> + Send + 'static,
{
    let client = client.clone();
    let updates = updates.clone();
    let reads = Arc::clone(reads);
    tokio::spawn(async move {
        let result = call(client.clone()).await;
        let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        refresh(&client, &updates, &reads).await;
    });
}

/// Sends a fresh read. A lost link sends nothing: the bridge reconnects and reads again on connect.
async fn refresh(client: &Client, updates: &mpsc::UnboundedSender<StoreUpdate>, reads: &Sequence) {
    if let Ok(read) = read(client, reads).await {
        let _ = updates.send(StoreUpdate::Extensions(read));
    }
}

/// Answers a Marketplace call with `refusal`, while there is no connection to ask.
pub(super) fn refuse_while_down(command: ExtensionCommand, refusal: Refusal) {
    match command {
        ExtensionCommand::Inspect { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::Install { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::SetEnabled { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ExtensionCommand::Remove { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}
