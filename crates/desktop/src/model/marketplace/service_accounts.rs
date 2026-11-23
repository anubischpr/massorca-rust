//! The accounts a connected service lists (decision 0059), as the rows of the Connection part. The pages name an
//! account by a number; the service names it by kind and id. A number is given once and never reused, so a row
//! keeps its number across lists and a refusal stays under the account it concerns.

use std::collections::BTreeMap;

use massorca_protocol::methods::agents::{AccountState as WireState, AccountSummary};

use super::types::{AccountId, AccountState, AgentAccount};
use crate::service::Refusal;

/// The service's accounts of every kind, with the number the pages use for each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceAccounts {
    /// Kind and service id, by number: the account numbered `n` is at `n - 1`.
    keys: Vec<(String, String)>,
    rows: Vec<AgentAccount>,
}

impl ServiceAccounts {
    /// Takes the lists the store holds. A kind whose list was refused keeps no rows: what it listed before may
    /// be stale.
    pub fn follow(&mut self, listed: &BTreeMap<String, Result<Vec<AccountSummary>, Refusal>>) {
        let mut rows = Vec::new();
        for (kind, result) in listed {
            for account in result.iter().flatten() {
                let id = self.number(kind, &account.id);
                rows.push(row(id, kind, account));
            }
        }
        self.rows = rows;
    }

    pub fn rows(&self) -> &[AgentAccount] {
        &self.rows
    }

    /// The kind and service id of a numbered account.
    pub fn key(&self, account: AccountId) -> Option<(String, String)> {
        let index = usize::try_from(account.0).ok()?.checked_sub(1)?;
        self.keys.get(index).cloned()
    }

    /// The number of a service account, giving it one the first time it is seen.
    pub fn number(&mut self, kind: &str, account: &str) -> AccountId {
        let found = self
            .keys
            .iter()
            .position(|(known_kind, known)| known_kind == kind && known == account);
        let index = found.unwrap_or_else(|| {
            self.keys.push((kind.to_owned(), account.to_owned()));
            self.keys.len() - 1
        });
        AccountId(u32::try_from(index + 1).unwrap_or(u32::MAX))
    }
}

/// One listed account as a row. A device without the `workspace` permission gets no identity: it is hidden, not
/// missing.
fn row(id: AccountId, kind: &str, account: &AccountSummary) -> AgentAccount {
    AgentAccount {
        id,
        agent_kind: kind.to_owned(),
        label: account.label.clone(),
        state: state(account.state),
        identity: account.identity.clone(),
        active: account.active,
        used_by_projects: account.projects.clone(),
    }
}

fn state(state: WireState) -> AccountState {
    match state {
        WireState::SignedIn => AccountState::SignedIn,
        WireState::Expired => AccountState::Expired,
        WireState::NeedsSignIn => AccountState::NeedsSignIn,
        WireState::NotSetUp => AccountState::NotSetUp,
        WireState::Migrated => AccountState::Migrated,
        WireState::Conflict => AccountState::Conflict,
        WireState::Unknown => AccountState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn summary(id: &str, state: WireState, active: bool) -> AccountSummary {
        AccountSummary {
            id: id.to_owned(),
            label: format!("Account {id}"),
            state,
            identity: None,
            active,
            projects: Vec::new(),
            origin: massorca_protocol::methods::agents::AccountOrigin::Unknown,
        }
    }

    fn listed(
        lists: &[(&str, Result<Vec<AccountSummary>, Refusal>)],
    ) -> BTreeMap<String, Result<Vec<AccountSummary>, Refusal>> {
        lists
            .iter()
            .map(|(kind, result)| ((*kind).to_owned(), result.clone()))
            .collect()
    }

    #[test]
    fn an_account_keeps_its_number_across_lists_and_a_number_is_never_reused() {
        let mut accounts = ServiceAccounts::default();
        accounts.follow(&listed(&[(
            "claude",
            Ok(vec![
                summary("a", WireState::SignedIn, true),
                summary("b", WireState::Expired, false),
            ]),
        )]));
        let b = accounts.rows()[1].id;
        accounts.follow(&listed(&[(
            "claude",
            Ok(vec![
                summary("b", WireState::Expired, false),
                summary("c", WireState::SignedIn, false),
            ]),
        )]));
        let ids: Vec<AccountId> = accounts.rows().iter().map(|row| row.id).collect();
        assert_eq!(ids, [b, AccountId(3)]);
        assert_eq!(accounts.key(b), Some(("claude".to_owned(), "b".to_owned())));
        assert_eq!(accounts.key(AccountId(0)), None);
    }

    #[test]
    fn a_refused_list_shows_no_rows_for_its_kind_and_a_missing_identity_is_hidden() {
        let mut accounts = ServiceAccounts::default();
        accounts.follow(&listed(&[
            ("claude", Ok(vec![summary("a", WireState::SignedIn, true)])),
            (
                "codex",
                Err(Refusal::unavailable("The service is not connected.")),
            ),
        ]));
        assert_eq!(accounts.rows().len(), 1);
        assert_eq!(accounts.rows()[0].agent_kind, "claude");
        assert_eq!(accounts.rows()[0].identity, None);
        assert_eq!(accounts.rows()[0].state, AccountState::SignedIn);
    }
}
