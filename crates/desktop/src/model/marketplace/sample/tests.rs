//! Tests of the sample source: changes stay in the sample, and each one is refused or applied the way the pages expect.

use super::*;

fn fresh() -> SampleMarketplaceSource {
    SampleMarketplaceSource::default()
}

fn names(items: &[&MarketplaceItem]) -> Vec<String> {
    items.iter().map(|item| item.name.clone()).collect()
}

fn sign_in_step(source: &SampleMarketplaceSource) -> Option<SignInStep> {
    source.snapshot().sign_in.map(|flow| flow.step)
}

fn refusal(result: Result<(), SourceError>) -> String {
    result.expect_err("the change is refused").message()
}

#[test]
fn make_active_moves_the_active_flag_within_one_kind() {
    let mut source = fresh();
    source.accounts.push(AgentAccount {
        id: AccountId(9),
        agent_kind: "codex".to_owned(),
        label: "Codex account".to_owned(),
        state: AccountState::SignedIn,
        identity: None,
        active: true,
        used_by_projects: Vec::new(),
    });
    source
        .activate_account(AccountId(2))
        .expect("the account exists");
    let snapshot = source.snapshot();
    let claude: Vec<(u32, bool)> = snapshot
        .accounts_for("claude")
        .iter()
        .map(|account| (account.id.0, account.active))
        .collect();
    assert_eq!(claude, vec![(1, false), (2, true)]);
    assert!(
        snapshot.accounts_for("codex")[0].active,
        "another agent keeps its own active account"
    );
    assert_eq!(
        refusal(source.activate_account(AccountId(7))),
        "No such account."
    );
}

#[test]
fn remove_is_refused_for_an_account_a_live_session_uses() {
    let mut source = fresh();
    assert_eq!(source.remove_account(AccountId(1)), Err(SourceError::Busy));
    assert_eq!(
        refusal(source.remove_account(AccountId(1))),
        "A live session uses this account. Stop the session, then remove it."
    );
    source
        .remove_account(AccountId(2))
        .expect("no session uses the second account");
    let left: Vec<AccountId> = source
        .snapshot()
        .accounts
        .iter()
        .map(|account| account.id)
        .collect();
    assert_eq!(left, vec![AccountId(1)]);
}

#[test]
fn uninstall_returns_the_item_to_not_installed_in_the_sample() {
    let mut source = fresh();
    source
        .uninstall(MarketplaceItemId(1))
        .expect("Claude is installed");
    assert_eq!(
        source
            .snapshot()
            .item(MarketplaceItemId(1))
            .map(|item| &item.state),
        Some(&ItemState::NotInstalled)
    );
    assert_eq!(
        refusal(source.uninstall(MarketplaceItemId(3))),
        "This item is not installed."
    );
    assert_eq!(
        refusal(source.set_enabled(MarketplaceItemId(1), false)),
        "This item is not installed."
    );
}

#[test]
fn sign_in_runs_browser_then_code_then_done_and_adds_an_account() {
    let mut source = fresh();
    assert_eq!(
        refusal(source.add_account("unknown-kind")),
        "No such agent."
    );
    source.add_account("claude").expect("claude can sign in");
    assert_eq!(
        sign_in_step(&source),
        Some(SignInStep::WaitingBrowser {
            url: SIGN_IN_URL.to_owned()
        })
    );
    source.open_sign_in_page().expect("the page opens");
    assert_eq!(sign_in_step(&source), Some(SignInStep::WaitingCode));
    source
        .submit_sign_in_code("482913")
        .expect("a code completes the sign-in");
    assert_eq!(sign_in_step(&source), Some(SignInStep::Done));
    let added = source
        .snapshot()
        .accounts
        .into_iter()
        .find(|account| account.id == AccountId(3))
        .expect("the new account is listed");
    assert_eq!(
        (added.label.as_str(), added.state, added.active),
        ("Account 3", AccountState::SignedIn, false),
        "the Work account stays the active one"
    );
    source.cancel_sign_in();
    assert_eq!(sign_in_step(&source), None);

    let mut first = fresh();
    first.accounts.clear();
    first.add_account("claude").expect("claude can sign in");
    first.open_sign_in_page().expect("the page opens");
    first
        .submit_sign_in_code("482913")
        .expect("a code completes the sign-in");
    assert!(
        first.snapshot().accounts[0].active,
        "the first account of an agent is active"
    );
}

#[test]
fn a_blank_code_keeps_the_code_step_and_an_expired_code_fails_then_retry_restarts() {
    let mut source = fresh();
    assert_eq!(
        refusal(source.submit_sign_in_code("")),
        "No sign-in is waiting for a code."
    );
    source.add_account("claude").expect("claude can sign in");
    assert_eq!(
        refusal(source.retry_sign_in()),
        "Only a failed sign-in can be tried again."
    );
    source.open_sign_in_page().expect("the page opens");
    assert_eq!(
        refusal(source.submit_sign_in_code("   ")),
        "Enter the code from the sign-in page."
    );
    assert_eq!(sign_in_step(&source), Some(SignInStep::WaitingCode));
    source
        .submit_sign_in_code("expired")
        .expect("an expired code fails the step, not the call");
    assert_eq!(
        sign_in_step(&source),
        Some(SignInStep::Failed {
            reason: "The code expired. Try again.".to_owned()
        })
    );
    source
        .retry_sign_in()
        .expect("a failed sign-in starts again");
    assert_eq!(
        sign_in_step(&source),
        Some(SignInStep::WaitingBrowser {
            url: SIGN_IN_URL.to_owned()
        })
    );
}

#[test]
fn search_matches_names_and_sorts_by_name() {
    let snapshot = fresh().snapshot();
    assert_eq!(
        names(&snapshot.search("")),
        vec![
            "Claude",
            "Codex",
            "GitHub",
            "Log tail",
            "Model usage panel",
            "Quick notes",
            "Release notes",
            "Workspace board",
        ]
    );
    assert_eq!(names(&snapshot.search("  GITHUB ")), vec!["GitHub"]);
    assert_eq!(
        names(&snapshot.search("massorca extras")),
        vec!["Codex", "Quick notes", "Release notes", "Workspace board"]
    );
    assert_eq!(names(&snapshot.search("agents")), vec!["Claude", "Codex"]);
    assert_eq!(
        names(&snapshot.search("pull requests")),
        vec!["GitHub", "Release notes"]
    );
    assert!(snapshot.search("zzz").is_empty());
}

#[test]
fn installed_and_updates_follow_the_sample_state() {
    let mut source = fresh();
    let snapshot = source.snapshot();
    assert_eq!(
        names(&snapshot.installed()),
        vec![
            "Claude",
            "Codex",
            "Model usage panel",
            "Quick notes",
            "Workspace board",
        ]
    );
    assert_eq!(names(&snapshot.updates()), vec!["Model usage panel"]);

    source
        .set_enabled(MarketplaceItemId(6), false)
        .expect("Quick notes is installed");
    source
        .uninstall(MarketplaceItemId(5))
        .expect("Model usage panel is installed");
    let snapshot = source.snapshot();
    assert_eq!(
        names(&snapshot.installed()),
        vec!["Claude", "Codex", "Quick notes", "Workspace board"]
    );
    assert!(snapshot.updates().is_empty());
    assert_eq!(
        snapshot
            .item(MarketplaceItemId(6))
            .map(|item| item.state.clone()),
        Some(ItemState::Installed {
            enabled: false,
            update: None
        })
    );
}

#[test]
fn account_states_use_the_spec_words() {
    let words = [
        AccountState::SignedIn,
        AccountState::Expired,
        AccountState::NeedsSignIn,
        AccountState::NotSetUp,
        AccountState::Migrated,
        AccountState::Conflict,
        AccountState::Unknown,
    ]
    .map(AccountState::word);
    assert_eq!(
        words,
        [
            "Signed in",
            "Expired",
            "Needs sign-in",
            "Not set up",
            "Migrated",
            "Conflict",
            "Can't check",
        ]
    );
}
