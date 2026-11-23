//! The rows of an agent's Connection part: each account's state, where it is signed in and what
//! uses it (decision 0059, "Accounts (Connection)").

use std::collections::BTreeSet;

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AccountsImported, ImportResult};

use super::types::{AccountId, AccountState, MarketplaceSnapshot};
use crate::service::Refusal;

/// Whether a refusal says the account is busy: its sign-in is open or being decided, or it is still undecided after
/// a restart. Such an account refuses activation, checks and removal until it is decided.
pub fn busy_refusal(refusal: &Refusal) -> bool {
    refusal.code() == Some(codes::AGENT_BUSY)
}

/// One account row as the Connection part shows it, with the actions its state allows.
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
    /// The service refuses changes to it until its sign-in is decided.
    pub busy: bool,
    pub can_make_active: bool,
    pub can_check: bool,
    pub can_remove: bool,
}

/// What this desktop knows of the accounts beyond the list: the ones the service refused as busy, and the one whose
/// sign-in terminal is open in the dialog.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccountActivity<'a> {
    pub busy: Option<&'a BTreeSet<AccountId>>,
    pub signing_in: Option<AccountId>,
}

/// The accounts of one agent, in the order the source lists them.
pub fn account_views(snapshot: &MarketplaceSnapshot, agent_kind: &str) -> Vec<AccountView> {
    connection_rows(snapshot, agent_kind, AccountActivity::default())
}

/// The accounts of one agent with what this desktop knows of them. A busy account offers no change; once its
/// sign-in terminal has ended it offers Check, which decides it.
pub fn connection_rows(
    snapshot: &MarketplaceSnapshot,
    agent_kind: &str,
    activity: AccountActivity<'_>,
) -> Vec<AccountView> {
    snapshot
        .accounts_for(agent_kind)
        .into_iter()
        .map(|account| {
            let signing_in = activity.signing_in == Some(account.id);
            let busy = signing_in || activity.busy.is_some_and(|busy| busy.contains(&account.id));
            let signed_in = if account.active && account.state == AccountState::Expired {
                "Signed out, sign in again".to_owned()
            } else {
                signed_in_line(account.state, account.identity.as_deref())
            };
            AccountView {
                id: account.id,
                label: account.label.clone(),
                state: account.state,
                signed_in,
                active: account.active,
                used_by: used_by_line(&account.used_by_projects),
                busy,
                can_make_active: !busy && !account.active,
                can_check: !signing_in,
                can_remove: !busy,
            }
        })
        .collect()
}

/// Where a sign-in in the service's terminal stands, from the dialog's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalSignIn {
    /// The terminal runs; the person signs in there.
    Running,
    /// The terminal has ended and the service is deciding the account.
    Checking,
    /// The service kept the account.
    Kept,
    /// The service discarded the account.
    Discarded,
}

/// The stage of the sign-in of `account`, from whether its terminal has ended and what the service lists. The
/// service lists a sign-in as needing a sign-in until it decides it, and drops it when it discards it.
pub fn terminal_sign_in(
    snapshot: &MarketplaceSnapshot,
    account: AccountId,
    ended: bool,
) -> TerminalSignIn {
    let listed = snapshot.accounts.iter().find(|row| row.id == account);
    match (ended, listed) {
        (false, _) => TerminalSignIn::Running,
        (true, Some(row)) if row.state == AccountState::NeedsSignIn => TerminalSignIn::Checking,
        (true, Some(_)) => TerminalSignIn::Kept,
        (true, None) => TerminalSignIn::Discarded,
    }
}

/// What an import answered, for the import dialog: done, or the sentence that says why not. The import never
/// answers a secret, so nothing here can show one.
pub fn import_outcome(imported: &AccountsImported) -> Result<(), String> {
    // The CLI's words when the service gives no reason.
    let fallback = match imported.result {
        ImportResult::Imported => return Ok(()),
        ImportResult::NotFound => "none of the declared files is in the source",
        ImportResult::Failed => "the import failed",
    };
    Err(as_sentence(imported.reason.as_deref().unwrap_or(fallback)))
}

/// The service's lower-case reason as a sentence: capitalised, with a full stop.
fn as_sentence(reason: &str) -> String {
    let mut chars = reason.trim().chars();
    let mut text: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !text.ends_with(['.', '!', '?']) {
        text.push('.');
    }
    text
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
    use crate::model::marketplace::{AgentAccount, MarketplaceSource, SampleMarketplaceSource};

    /// A snapshot with one Claude account per entry: its state, whether it is active, and its identity.
    fn listed(accounts: &[(AccountState, bool, Option<&str>)]) -> MarketplaceSnapshot {
        MarketplaceSnapshot {
            accounts: accounts
                .iter()
                .enumerate()
                .map(|(index, (state, active, identity))| AgentAccount {
                    id: AccountId(u32::try_from(index + 1).expect("a few rows")),
                    agent_kind: "claude".to_owned(),
                    label: format!("Account {}", index + 1),
                    state: *state,
                    identity: identity.map(str::to_owned),
                    active: *active,
                    used_by_projects: Vec::new(),
                })
                .collect(),
            ..MarketplaceSnapshot::default()
        }
    }

    #[test]
    fn an_active_account_that_expired_says_it_was_signed_out() {
        let snapshot = listed(&[
            (AccountState::Expired, true, Some("user@example.com")),
            (AccountState::Expired, false, Some("second@example.com")),
        ]);
        let rows = account_views(&snapshot, "claude");
        assert_eq!(rows[0].signed_in, "Signed out, sign in again");
        assert_eq!(rows[1].signed_in, "Last signed in as second@example.com");
    }

    #[test]
    fn a_hidden_identity_reads_as_the_state_alone_and_keeps_every_action() {
        let snapshot = listed(&[(AccountState::SignedIn, false, None)]);
        let row = &account_views(&snapshot, "claude")[0];
        assert_eq!(row.signed_in, "Signed in");
        assert!(row.can_make_active && row.can_check && row.can_remove);
        assert!(!row.busy);
    }

    #[test]
    fn a_busy_account_offers_no_change_and_offers_check_once_its_terminal_has_ended() {
        let snapshot = listed(&[
            (AccountState::NeedsSignIn, false, None),
            (AccountState::NeedsSignIn, false, None),
        ]);
        let busy = BTreeSet::from([AccountId(2)]);
        let rows = connection_rows(
            &snapshot,
            "claude",
            AccountActivity {
                busy: Some(&busy),
                signing_in: Some(AccountId(1)),
            },
        );
        // Signing in now: busy, and nothing to press until its terminal ends.
        assert!(rows[0].busy);
        assert!(!rows[0].can_make_active && !rows[0].can_check && !rows[0].can_remove);
        // Refused as busy with no sign-in open here: Check decides it.
        assert!(rows[1].busy);
        assert!(!rows[1].can_make_active && !rows[1].can_remove);
        assert!(rows[1].can_check);
    }

    #[test]
    fn a_terminal_sign_in_runs_then_is_checked_then_kept_or_discarded() {
        let pending = listed(&[(AccountState::NeedsSignIn, false, None)]);
        let kept = listed(&[(AccountState::SignedIn, true, None)]);
        let gone = listed(&[]);
        let id = AccountId(1);
        assert_eq!(
            terminal_sign_in(&pending, id, false),
            TerminalSignIn::Running
        );
        assert_eq!(terminal_sign_in(&gone, id, false), TerminalSignIn::Running);
        assert_eq!(
            terminal_sign_in(&pending, id, true),
            TerminalSignIn::Checking
        );
        assert_eq!(terminal_sign_in(&kept, id, true), TerminalSignIn::Kept);
        assert_eq!(terminal_sign_in(&gone, id, true), TerminalSignIn::Discarded);
    }

    fn imported(result: ImportResult, reason: Option<&str>) -> AccountsImported {
        AccountsImported {
            result,
            account: None,
            reason: reason.map(str::to_owned),
        }
    }

    #[test]
    fn an_import_that_kept_nothing_says_why_in_the_service_words() {
        assert_eq!(
            import_outcome(&imported(ImportResult::Imported, None)),
            Ok(())
        );
        assert_eq!(
            import_outcome(&imported(
                ImportResult::NotFound,
                Some("none of the declared files is in the source")
            )),
            Err("None of the declared files is in the source.".to_owned())
        );
        assert_eq!(
            import_outcome(&imported(ImportResult::NotFound, None)),
            Err("None of the declared files is in the source.".to_owned())
        );
        assert_eq!(
            import_outcome(&imported(
                ImportResult::Failed,
                Some("the check reports the login is not signed in")
            )),
            Err("The check reports the login is not signed in.".to_owned())
        );
        assert_eq!(
            import_outcome(&imported(ImportResult::Failed, None)),
            Err("The import failed.".to_owned())
        );
    }

    #[test]
    fn only_a_busy_refusal_marks_an_account_busy() {
        let busy = Refusal::Failed {
            code: codes::AGENT_BUSY.to_owned(),
            message: "account a is signing in; wait for it".to_owned(),
        };
        assert!(busy_refusal(&busy));
        assert!(!busy_refusal(&Refusal::unavailable(
            "The service is not connected."
        )));
    }

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
