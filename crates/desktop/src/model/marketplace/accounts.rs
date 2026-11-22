//! The rows of an agent's Connection part: each account's state, where it is signed in and what
//! uses it (decision 0059, "Accounts (Connection)").

use super::types::{AccountId, AccountState, MarketplaceSnapshot};

/// One account row as the Connection part shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountView {
    pub id: AccountId,
    pub label: String,
    pub state: AccountState,
    /// Where the account is signed in, as one line.
    pub signed_in: String,
    pub active: bool,
    /// What uses the account, as one line.
    pub used_by: String,
}

/// The accounts of one agent, in the order the source lists them.
pub fn account_views(snapshot: &MarketplaceSnapshot, agent_kind: &str) -> Vec<AccountView> {
    snapshot
        .accounts_for(agent_kind)
        .into_iter()
        .map(|account| AccountView {
            id: account.id,
            label: account.label.clone(),
            state: account.state,
            signed_in: signed_in_line(account.state, account.identity.as_deref()),
            active: account.active,
            used_by: used_by_line(&account.used_by_projects),
        })
        .collect()
}

/// Where the account is signed in. The identity comes from the agent's own check, so it reads as
/// "last signed in" when the account is not signed in now.
pub fn signed_in_line(state: AccountState, identity: Option<&str>) -> String {
    match (state, identity) {
        (AccountState::SignedIn, Some(identity)) => format!("Signed in as {identity}"),
        (AccountState::SignedIn, None) => "Signed in".to_owned(),
        (_, Some(identity)) => format!("Last signed in as {identity}"),
        (_, None) => "Not signed in".to_owned(),
    }
}

/// What uses the account: the projects that choose it, or none.
pub fn used_by_line(projects: &[String]) -> String {
    if projects.is_empty() {
        "Not used by any project".to_owned()
    } else {
        format!("Used by {}", projects.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::{MarketplaceSource, SampleMarketplaceSource};

    #[test]
    fn the_connection_rows_show_identity_use_and_the_active_flag() {
        let rows = account_views(&SampleMarketplaceSource::default().snapshot(), "claude");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Work account");
        assert_eq!(rows[0].signed_in, "Signed in as user@example.com");
        assert_eq!(rows[0].used_by, "Used by Project one");
        assert!(rows[0].active);
        assert_eq!(rows[1].signed_in, "Last signed in as second@example.com");
        assert_eq!(rows[1].used_by, "Not used by any project");
        assert!(!rows[1].active);
    }

    #[test]
    fn a_signed_in_account_without_an_identity_still_says_so() {
        assert_eq!(signed_in_line(AccountState::SignedIn, None), "Signed in");
        assert_eq!(
            signed_in_line(AccountState::NeedsSignIn, None),
            "Not signed in"
        );
    }
}
