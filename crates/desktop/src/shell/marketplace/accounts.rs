//! The Connection part on a connected service (decision 0059): Make active, Check and Remove go to the service, and
//! the rows follow the read that each answer triggers. Nothing changes from an answer on its own.

use std::collections::BTreeSet;

use massorca_protocol::methods::agents::{
    AccountsActivateParams, AccountsCheckParams, AccountsRemoveParams,
};

use super::import::ImportDialog;
use super::sign_in::SignIn;
use crate::model::marketplace::{AccountId, busy_refusal};
use crate::service::{Refusal, StoreState};
use crate::shell::Shell;
use gpui::Context;

/// What the Connection part knows beyond the service's lists.
#[derive(Default)]
pub(in crate::shell) struct AccountsUi {
    /// The accounts the service refused as busy, until a change of theirs works or they are no longer listed.
    pub busy: BTreeSet<AccountId>,
    /// The sign-in this desktop started, while its dialog is open.
    pub sign_in: Option<SignIn>,
    /// The import dialog, while it is open.
    pub import: Option<ImportDialog>,
}

impl std::fmt::Debug for AccountsUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountsUi")
            .field("busy", &self.busy)
            .field("signing_in", &self.sign_in.is_some())
            .field("importing", &self.import.is_some())
            .finish()
    }
}

/// An account change the service makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AccountWrite {
    Activate,
    Check,
    Remove,
}

impl Shell {
    /// Sends `write` for a service account. False when the account is not the service's, so the source handles it.
    pub(super) fn write_service_account(
        &mut self,
        account: AccountId,
        write: AccountWrite,
        cx: &mut Context<Self>,
    ) -> bool {
        let (Some(service), Some((kind, id))) = (
            self.service.clone(),
            self.marketplace.service_account(account),
        ) else {
            return false;
        };
        cx.spawn(async move |this, cx| {
            let result = match write {
                AccountWrite::Activate => service
                    .accounts_activate(AccountsActivateParams {
                        kind,
                        account: id,
                        project: None,
                    })
                    .await
                    .map(drop),
                AccountWrite::Check => service
                    .accounts_check(AccountsCheckParams {
                        kind,
                        account: Some(id),
                    })
                    .await
                    .map(drop),
                AccountWrite::Remove => service
                    .accounts_remove(AccountsRemoveParams { kind, account: id })
                    .await
                    .map(drop),
            };
            let _ = this.update(cx, |shell, cx| {
                shell.finish_account_write(account, result, cx);
            });
        })
        .detach();
        true
    }

    /// A change that worked clears the account's refusal and its busy mark; a refusal stays under its row, and a
    /// busy one marks the row busy.
    fn finish_account_write(
        &mut self,
        account: AccountId,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => {
                self.marketplace_accounts.busy.remove(&account);
                if matches!(&self.marketplace_error, Some((Some(refused), _)) if *refused == account)
                {
                    self.marketplace_error = None;
                }
            }
            Err(refusal) => {
                if busy_refusal(&refusal) {
                    self.marketplace_accounts.busy.insert(account);
                }
                self.marketplace_error = Some((Some(account), refusal.sentence()));
            }
        }
        cx.notify();
    }

    /// Takes the accounts the store lists. A busy mark goes with an account that is no longer listed.
    pub(in crate::shell) fn follow_service_accounts(&mut self, state: &StoreState) {
        self.marketplace.follow_accounts(&state.accounts);
        let listed: BTreeSet<AccountId> = self
            .marketplace
            .snapshot()
            .accounts
            .iter()
            .map(|account| account.id)
            .collect();
        self.marketplace_accounts
            .busy
            .retain(|account| listed.contains(account));
    }
}

/// Why each kind's accounts could not be listed, as sentences, by kind.
pub(super) fn list_refusals(state: &StoreState) -> Vec<(String, String)> {
    state
        .accounts
        .iter()
        .filter_map(|(kind, result)| {
            result
                .as_ref()
                .err()
                .map(|refusal| (kind.clone(), refusal.sentence()))
        })
        .collect()
}
