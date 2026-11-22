//! The secret vault as the bridge reads it (decision 0071). The status and the names of every scope are read
//! together: after each connect, on each `secret.changed` event and after each write. A read answers only
//! with names and times, so the store never holds a value.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use massorca_client::Client;
use massorca_protocol::methods::secrets::SecretListParams;
use tokio::sync::mpsc;

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
        answers: [SecretsUpdate; 2],
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

/// Reads the status and the names in the background and delivers them when they are the newest read. A failed
/// read is delivered as its refusal, so the store shows what the service said or that it could not be reached.
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
        let (status, list) = tokio::join!(
            client.secret_status(),
            client.secret_list(&SecretListParams { scope: None })
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
            ],
        );
    });
}
