//! Element tests of the Connection part on a connected service: the rows follow the service's lists, a change goes
//! to the service and is answered by hand, and a busy refusal marks the row busy. No window opens.

use super::service_tests::{Rig, click, draw, rig};
use crate::model::marketplace::MarketplaceItemId;
use crate::model::navigation::MarketplacePage;
use crate::service::fixtures::agent_kind;
use crate::service::{AccountCommand, Command, Refusal, StoreUpdate};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AccountState, AccountSummary, PackageState};

fn summary(id: &str, label: &str, state: AccountState, active: bool) -> AccountSummary {
    AccountSummary {
        id: id.to_owned(),
        label: label.to_owned(),
        state,
        identity: None,
        active,
        projects: Vec::new(),
    }
}

/// A connected shell whose service lists Claude with `accounts`, on Claude's item page.
fn on_claude(
    cx: &mut TestAppContext,
    accounts: Vec<AccountSummary>,
) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[]);
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            "claude",
            "Claude",
            PackageState::Enabled,
        )]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::Accounts {
            kind: "claude".to_owned(),
            result: Ok(accounts),
        })
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Item(MarketplaceItemId(1)), cx);
    });
    draw(cx);
    (rig, cx)
}

fn shown(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    draw(cx);
    cx.debug_bounds(selector).is_some()
}

#[gpui::test]
fn make_active_goes_to_the_service_and_a_busy_refusal_marks_the_row_busy(cx: &mut TestAppContext) {
    let (mut rig, cx) = on_claude(
        cx,
        vec![
            summary("acct-a", "Work", AccountState::SignedIn, true),
            summary("acct-b", "Personal", AccountState::NeedsSignIn, false),
        ],
    );
    assert!(shown(cx, "button:marketplace-remove-2"));
    click(cx, "button:marketplace-make-active-2");
    let Some(Command::Account(AccountCommand::Activate { params, reply })) = rig.script.next()
    else {
        panic!("an agent.accounts.activate call was expected");
    };
    assert_eq!(
        (params.kind.as_str(), params.account.as_str()),
        ("claude", "acct-b")
    );
    reply
        .send(Err(Refusal::Failed {
            code: codes::AGENT_BUSY.to_owned(),
            message: "account acct-b is signing in; wait for it".to_owned(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();

    // Busy: no change is offered, and Check decides it since no sign-in is open here.
    assert!(!shown(cx, "button:marketplace-make-active-2"));
    assert!(!shown(cx, "button:marketplace-remove-2"));
    assert!(shown(cx, "button:marketplace-check-2"));
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.marketplace_error,
            Some((
                Some(crate::model::marketplace::AccountId(2)),
                "Account acct-b is signing in; wait for it".to_owned()
            ))
        );
    });

    click(cx, "button:marketplace-check-2");
    let Some(Command::Account(AccountCommand::Check { params, reply })) = rig.script.next() else {
        panic!("an agent.accounts.check call was expected");
    };
    assert_eq!(params.account.as_deref(), Some("acct-b"));
    reply
        .send(Ok(summary(
            "acct-b",
            "Personal",
            AccountState::SignedIn,
            false,
        )))
        .expect("the shell waits");
    cx.run_until_parked();
    // The check worked: the row offers its changes again.
    assert!(shown(cx, "button:marketplace-make-active-2"));
    assert!(shown(cx, "button:marketplace-remove-2"));
}

#[gpui::test]
fn the_rows_follow_the_service_list_and_a_refused_list_is_said(cx: &mut TestAppContext) {
    let (rig, cx) = on_claude(
        cx,
        vec![summary("acct-a", "Work", AccountState::Expired, true)],
    );
    rig.shell.read_with(cx, |shell, _| {
        let accounts = &shell.model().marketplace().accounts;
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].label, "Work");
    });
    rig.updates
        .send(StoreUpdate::Accounts {
            kind: "claude".to_owned(),
            result: Err(Refusal::unavailable("The service is not connected.")),
        })
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.model().marketplace().accounts.is_empty());
    });
    assert!(
        shown(cx, "marketplace-connection-alert"),
        "the refusal of the list is said"
    );
}
