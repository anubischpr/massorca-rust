//! Agent accounts as the bridge reads them (decision 0059). Each kind's list is read after a connect, on each
//! `agent.accounts_changed` event for that kind, and after each write. Answers carry ids, labels, states and
//! identities, never a credential, so the store never holds one.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::agents::{
    AccountSummary, AccountsActivateParams, AccountsAddParams, AccountsAdded, AccountsCheckParams,
    AccountsImportParams, AccountsImported, AccountsRemoveParams, AccountsRemoved,
    ImportCandidates,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::StoreUpdate;

/// Orders the account reads of one bridge, per kind. A read takes a number when it starts, and its answer reaches
/// the store only if no newer read of the same kind has delivered already, so a stale list never wins.
#[derive(Debug, Default)]
pub(super) struct AccountsFreshness {
    requested: AtomicU64,
    delivered: Mutex<BTreeMap<String, u64>>,
}

impl AccountsFreshness {
    /// Numbers a read that is about to start.
    pub(super) fn request(&self) -> u64 {
        self.requested.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Sends the answer of read `number` of `kind`, unless a newer read of that kind has delivered already. The
    /// check and the send share the lock, so the store takes the answers in the order they were accepted.
    pub(super) fn deliver(
        &self,
        number: u64,
        kind: String,
        updates: &mpsc::UnboundedSender<StoreUpdate>,
        result: Result<Vec<AccountSummary>, Refusal>,
    ) {
        let mut delivered = self
            .delivered
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let newest = delivered.entry(kind.clone()).or_default();
        if number <= *newest {
            return;
        }
        *newest = number;
        // A closed channel means the UI is gone; the session notices that and stops.
        let _ = updates.send(StoreUpdate::Accounts { kind, result });
    }
}

/// Reads one kind's accounts in the background and delivers them when they are the newest read of that kind. A
/// failed read is delivered as its refusal.
pub(super) fn refresh_in_background(
    client: &Client,
    kind: &str,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    order: &Arc<AccountsFreshness>,
) {
    let (client, kind) = (client.clone(), kind.to_owned());
    let updates = updates.clone();
    let order = Arc::clone(order);
    let number = order.request();
    tokio::spawn(async move {
        let result = client
            .agent_accounts_list(&kind)
            .await
            .map(|listed| listed.accounts)
            .map_err(|error| Refusal::from(&error));
        order.deliver(number, kind, &updates, result);
    });
}

/// An account write from the UI. Its answer goes back on the oneshot; the store changes through the read of the
/// kind that follows, never from the answer.
#[derive(Debug)]
pub(crate) enum AccountCommand {
    /// Starts a sign-in; the answer names the service terminal that runs it.
    Add {
        params: AccountsAddParams,
        reply: oneshot::Sender<Result<AccountsAdded, Refusal>>,
    },
    Import {
        params: AccountsImportParams,
        reply: oneshot::Sender<Result<AccountsImported, Refusal>>,
    },
    /// Reads where an import without a folder would copy `kind`'s files from. Changes nothing, so no list follows it.
    ImportCandidates {
        kind: String,
        reply: oneshot::Sender<Result<ImportCandidates, Refusal>>,
    },
    Activate {
        params: AccountsActivateParams,
        reply: oneshot::Sender<Result<AccountSummary, Refusal>>,
    },
    Remove {
        params: AccountsRemoveParams,
        reply: oneshot::Sender<Result<AccountsRemoved, Refusal>>,
    },
    Check {
        params: AccountsCheckParams,
        reply: oneshot::Sender<Result<AccountSummary, Refusal>>,
    },
}

impl ServiceHandle {
    /// `agent.accounts.add`: starts a sign-in in a service terminal. The list follows the read the answer triggers.
    pub async fn accounts_add(&self, params: AccountsAddParams) -> Result<AccountsAdded, Refusal> {
        self.ask(|reply| Command::Account(AccountCommand::Add { params, reply }))
            .await
    }

    /// `agent.accounts.import`: copies the kind's login files from a folder into a new account. Never returns a secret.
    pub async fn accounts_import(
        &self,
        params: AccountsImportParams,
    ) -> Result<AccountsImported, Refusal> {
        self.ask(|reply| Command::Account(AccountCommand::Import { params, reply }))
            .await
    }

    /// `agent.accounts.import_candidates` (capability `agent-import-detect/1`); `None` when the service does not offer it
    /// or refuses the read, so the folder stays manual.
    pub async fn accounts_import_candidates(&self, kind: &str) -> Option<ImportCandidates> {
        let kind = kind.to_owned();
        self.ask(|reply| Command::Account(AccountCommand::ImportCandidates { kind, reply }))
            .await
            .ok()
    }

    /// `agent.accounts.activate`: makes an account the kind's active one, or a project's choice.
    pub async fn accounts_activate(
        &self,
        params: AccountsActivateParams,
    ) -> Result<AccountSummary, Refusal> {
        self.ask(|reply| Command::Account(AccountCommand::Activate { params, reply }))
            .await
    }

    /// `agent.accounts.remove`: deletes an account, signing it out first when the service made its login.
    pub async fn accounts_remove(
        &self,
        params: AccountsRemoveParams,
    ) -> Result<AccountsRemoved, Refusal> {
        self.ask(|reply| Command::Account(AccountCommand::Remove { params, reply }))
            .await
    }

    /// `agent.accounts.check`: checks an account now and answers its state.
    pub async fn accounts_check(
        &self,
        params: AccountsCheckParams,
    ) -> Result<AccountSummary, Refusal> {
        self.ask(|reply| Command::Account(AccountCommand::Check { params, reply }))
            .await
    }
}

/// Runs an account write on its own task. The caller is answered first, then the kind is read again whatever the
/// answer was, so the store follows the service even after a refusal.
pub(super) fn spawn_call(
    client: &Client,
    command: AccountCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    order: &Arc<AccountsFreshness>,
) {
    let rig = (client, updates, order);
    match command {
        AccountCommand::Add { params, reply } => {
            write(rig, params.kind.clone(), reply, |c| async move {
                c.agent_accounts_add(&params).await
            })
        }
        AccountCommand::Import { params, reply } => {
            write(rig, params.kind.clone(), reply, |c| async move {
                c.agent_accounts_import(&params).await
            })
        }
        AccountCommand::ImportCandidates { kind, reply } => {
            let client = client.clone();
            tokio::spawn(async move {
                let result = client.agent_accounts_import_candidates(Some(&kind)).await;
                let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
            });
        }
        AccountCommand::Activate { params, reply } => {
            write(rig, params.kind.clone(), reply, |c| async move {
                c.agent_accounts_activate(&params).await
            })
        }
        AccountCommand::Remove { params, reply } => {
            write(rig, params.kind.clone(), reply, |c| async move {
                c.agent_accounts_remove(&params).await
            })
        }
        AccountCommand::Check { params, reply } => {
            write(rig, params.kind.clone(), reply, |c| async move {
                c.agent_accounts_check(&params).await
            })
        }
    }
}

type Rig<'a> = (
    &'a Client,
    &'a mpsc::UnboundedSender<StoreUpdate>,
    &'a Arc<AccountsFreshness>,
);

fn write<T, Fut>(
    (client, updates, order): Rig<'_>,
    kind: String,
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
        refresh_in_background(&client, &kind, &updates, &order);
    });
}

/// Answers an account write with `refusal`, while there is no connection to ask.
pub(super) fn refuse_while_down(command: AccountCommand, refusal: Refusal) {
    match command {
        AccountCommand::Add { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        AccountCommand::Import { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        AccountCommand::ImportCandidates { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        AccountCommand::Activate { reply, .. } | AccountCommand::Check { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        AccountCommand::Remove { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(label: &str) -> Result<Vec<AccountSummary>, Refusal> {
        Ok(vec![AccountSummary {
            id: format!("id-{label}"),
            label: label.to_owned(),
            state: massorca_protocol::methods::agents::AccountState::SignedIn,
            identity: None,
            active: true,
            projects: Vec::new(),
            origin: massorca_protocol::methods::agents::AccountOrigin::Unknown,
        }])
    }

    #[test]
    fn a_stale_read_of_a_kind_never_replaces_a_newer_one_and_kinds_are_ordered_apart() {
        let order = AccountsFreshness::default();
        let (updates, mut received) = mpsc::unbounded_channel();
        let (older, newer, other) = (order.request(), order.request(), order.request());
        order.deliver(newer, "claude".to_owned(), &updates, list("newer"));
        order.deliver(older, "claude".to_owned(), &updates, list("older"));
        order.deliver(other, "codex".to_owned(), &updates, list("codex"));
        let mut delivered = Vec::new();
        while let Ok(update) = received.try_recv() {
            delivered.push(update);
        }
        assert_eq!(
            delivered,
            [
                StoreUpdate::Accounts {
                    kind: "claude".to_owned(),
                    result: list("newer"),
                },
                StoreUpdate::Accounts {
                    kind: "codex".to_owned(),
                    result: list("codex"),
                },
            ]
        );
    }
}
