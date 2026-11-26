//! The secret vault as the bridge reads it (decision 0071). The status, the names of every scope and the requests, the
//! resolved ones included for the transcripts' cards, are read together: after each connect, on each `secret.changed`
//! or request event and after each write.
//! A read answers only with names and times, so the store never holds a value.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::secrets::{
    SecretDecline, SecretEntry, SecretLink, SecretListParams, SecretName, SecretRemove,
    SecretRemoved, SecretRequest, SecretRequestsParams, SecretSet,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::{SecretsUpdate, StoreUpdate};

/// Orders the vault reads of one bridge. A read takes a number when it starts, and its answers reach the store
/// only if no newer read has delivered already, so an older answer never replaces a newer one.
#[derive(Debug, Default)]
pub(super) struct SecretsFreshness {
    requested: AtomicU64,
    delivered: Mutex<u64>,
}

impl SecretsFreshness {
    /// Numbers a read that is about to start.
    pub(super) fn request(&self) -> u64 {
        self.requested.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Sends the two answers of read `number`, unless a newer read has delivered already. The check and the
    /// sends share the lock, so the store takes the answers in the order they were accepted.
    pub(super) fn deliver(
        &self,
        number: u64,
        updates: &mpsc::UnboundedSender<StoreUpdate>,
        answers: [SecretsUpdate; 3],
    ) {
        let mut delivered = self
            .delivered
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if number <= *delivered {
            return;
        }
        *delivered = number;
        for answer in answers {
            // A closed channel means the UI is gone; the session notices that and stops.
            let _ = updates.send(StoreUpdate::Secrets(answer));
        }
    }
}

/// Reads the status, the names and the pending requests in the background and delivers them when they are the newest
/// read. A failed read is delivered as its refusal, so the store shows what the service said or that it is unreachable.
pub(super) fn refresh_in_background(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    order: &Arc<SecretsFreshness>,
) {
    let client = client.clone();
    let updates = updates.clone();
    let order = Arc::clone(order);
    let number = order.request();
    tokio::spawn(async move {
        let (status, list, requests) = tokio::join!(
            client.secret_status(),
            client.secret_list(&SecretListParams { scope: None }),
            client.secret_requests(&SecretRequestsParams { all: true })
        );
        order.deliver(
            number,
            &updates,
            [
                SecretsUpdate::Status(status.map_err(|error| Refusal::from(&error))),
                SecretsUpdate::List(
                    list.map(|list| list.secrets)
                        .map_err(|error| Refusal::from(&error)),
                ),
                SecretsUpdate::Requests(
                    requests
                        .map(|requests| requests.requests)
                        .map_err(|error| Refusal::from(&error)),
                ),
            ],
        );
    });
}

/// A vault write from the UI. Its answer goes back on the oneshot; the store changes through the refetch that
/// follows, never from the answer. `Debug` prints no value, because `SecretSet` redacts its own.
#[derive(Debug)]
pub(crate) enum SecretCommand {
    Set {
        params: SecretSet,
        reply: oneshot::Sender<Result<SecretEntry, Refusal>>,
    },
    /// Removes an owned secret; its links go with it only when `include_links` says so.
    Remove {
        params: SecretRemove,
        reply: oneshot::Sender<Result<SecretRemoved, Refusal>>,
    },
    /// Links a name to the owned secret of another scope: how a request is answered without storing a value.
    Link {
        params: SecretLink,
        reply: oneshot::Sender<Result<SecretEntry, Refusal>>,
    },
    /// Removes one scope's link to a name. The owner keeps the secret; only the link goes.
    Unlink {
        params: SecretName,
        reply: oneshot::Sender<Result<SecretRemoved, Refusal>>,
    },
    /// Declines a pending request. No secret changes.
    Decline {
        params: SecretDecline,
        reply: oneshot::Sender<Result<SecretRequest, Refusal>>,
    },
}

impl ServiceHandle {
    /// `secret.set`: stores a secret, or rotates the owned one of that name with `replace`. Nothing is shown
    /// optimistically; the list follows the refetch that the answer triggers.
    pub async fn secret_set(&self, params: SecretSet) -> Result<SecretEntry, Refusal> {
        self.ask(|reply| Command::Secret(SecretCommand::Set { params, reply }))
            .await
    }

    /// `secret.remove`: deletes an owned secret. The list follows the refetch that the answer triggers.
    pub async fn secret_remove(&self, params: SecretRemove) -> Result<SecretRemoved, Refusal> {
        self.ask(|reply| Command::Secret(SecretCommand::Remove { params, reply }))
            .await
    }

    /// `secret.link`: links a name to the owned secret of another scope. The service answers a request for it too.
    pub async fn secret_link(&self, params: SecretLink) -> Result<SecretEntry, Refusal> {
        self.ask(|reply| Command::Secret(SecretCommand::Link { params, reply }))
            .await
    }

    /// `secret.unlink`: removes one scope's link to a name. The list follows the refetch that the answer triggers.
    pub async fn secret_unlink(&self, params: SecretName) -> Result<SecretRemoved, Refusal> {
        self.ask(|reply| Command::Secret(SecretCommand::Unlink { params, reply }))
            .await
    }

    /// `secret.decline`: declines a pending request. The request list follows the refetch that the answer triggers.
    pub async fn secret_decline(&self, params: SecretDecline) -> Result<SecretRequest, Refusal> {
        self.ask(|reply| Command::Secret(SecretCommand::Decline { params, reply }))
            .await
    }
}

/// Runs a vault write on its own task. The caller is answered first, then the vault is read again whatever the
/// answer was, so the store follows the vault even after a refusal.
pub(super) fn spawn_call(
    client: &Client,
    command: SecretCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    order: &Arc<SecretsFreshness>,
) {
    match command {
        SecretCommand::Set { params, reply } => {
            write(client, updates, order, reply, move |client| async move {
                client.secret_set(&params).await
            })
        }
        SecretCommand::Remove { params, reply } => {
            write(client, updates, order, reply, move |client| async move {
                client.secret_remove(&params).await
            })
        }
        SecretCommand::Link { params, reply } => {
            write(client, updates, order, reply, move |client| async move {
                client.secret_link(&params).await
            })
        }
        SecretCommand::Unlink { params, reply } => {
            write(client, updates, order, reply, move |client| async move {
                client.secret_unlink(&params).await
            })
        }
        SecretCommand::Decline { params, reply } => {
            write(client, updates, order, reply, move |client| async move {
                client.secret_decline(&params).await
            })
        }
    }
}

fn write<T, Fut>(
    client: &Client,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    order: &Arc<SecretsFreshness>,
    reply: oneshot::Sender<Result<T, Refusal>>,
    call: impl FnOnce(Client) -> Fut + Send + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = Result<T, ClientError>> + Send + 'static,
{
    let client = client.clone();
    let updates = updates.clone();
    let order = Arc::clone(order);
    tokio::spawn(async move {
        let result = call(client.clone()).await;
        let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        refresh_in_background(&client, &updates, &order);
    });
}

/// Answers a vault write with `refusal`, while there is no connection to ask.
pub(super) fn refuse_while_down(command: SecretCommand, refusal: Refusal) {
    match command {
        SecretCommand::Set { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        SecretCommand::Remove { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        SecretCommand::Link { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        SecretCommand::Unlink { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        SecretCommand::Decline { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}
