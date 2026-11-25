//! Element tests of the Connection part on a connected service: the rows follow the service's lists, a change goes
//! to the service and is answered by hand, and a busy refusal marks the row busy. No window opens.

use super::path_field::{PickKind, answering};
use super::service_tests::{Rig, click, draw, rig};
use crate::model::marketplace::{HostTarget, MarketplaceItemId};
use crate::model::navigation::MarketplacePage;
use crate::service::fixtures::agent_kind;
use crate::service::{
    AccountCommand, Command, Refusal, StoreUpdate, StreamEnd, TerminalCommand, TerminalStreamItem,
};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AccountState, AccountSummary, AccountsAdded, AccountsImported, ImportResult, PackageState,
};
use tokio::sync::mpsc::unbounded_channel;

fn summary(id: &str, label: &str, state: AccountState, active: bool) -> AccountSummary {
    AccountSummary {
        id: id.to_owned(),
        label: label.to_owned(),
        state,
        identity: None,
        active,
        projects: Vec::new(),
        origin: massorca_protocol::methods::agents::AccountOrigin::Unknown,
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

#[gpui::test]
fn add_account_opens_the_sign_in_dialog_on_the_terminal_the_service_returns(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = on_claude(cx, Vec::new());
    click(cx, "button:marketplace-add-account");
    let Some(Command::Account(AccountCommand::Add { params, reply })) = rig.script.next() else {
        panic!("an agent.accounts.add call was expected");
    };
    assert_eq!(params.kind, "claude");
    assert!(
        !shown(cx, "marketplace-sign-in-terminal"),
        "no terminal before the service answers"
    );
    reply
        .send(Ok(AccountsAdded {
            account: summary("acct-new", "Account", AccountState::NeedsSignIn, false),
            terminal_id: "term-sign-in".to_owned(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    assert!(
        shown(cx, "marketplace-sign-in-terminal"),
        "the dialog shows the sign-in terminal"
    );

    // The surface attaches to the terminal the service returned, and answers the program's queries.
    let attach = std::iter::from_fn(|| rig.script.next()).find_map(|command| match command {
        Command::Terminal(TerminalCommand::Attach { params, reply }) => Some((params, reply)),
        _ => None,
    });
    let (params, reply) = attach.expect("the sign-in terminal is attached");
    assert_eq!(params.terminal, "term-sign-in");
    assert_eq!(params.answers_queries, Some(true));

    // The terminal ends and the service keeps the account: the dialog says it is done.
    let (items, stream) = unbounded_channel();
    reply.send(Ok(stream)).expect("the surface waits");
    items
        .send(TerminalStreamItem::Ended(StreamEnd::Finished))
        .expect("the surface listens");
    rig.updates
        .send(StoreUpdate::Accounts {
            kind: "claude".to_owned(),
            result: Ok(vec![summary(
                "acct-new",
                "Account",
                AccountState::SignedIn,
                true,
            )]),
        })
        .expect("the store listens");
    cx.run_until_parked();
    assert!(
        !shown(cx, "marketplace-sign-in-terminal"),
        "a kept account closes the terminal part"
    );
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.marketplace_accounts.sign_in.is_some(),
            "Done is pressed by the person"
        );
    });
}

fn import_error(rig: &Rig, cx: &mut VisualTestContext) -> Option<Option<String>> {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .marketplace_accounts
            .import
            .as_ref()
            .map(|dialog| dialog.error().map(str::to_owned))
    })
}

#[gpui::test]
fn import_sends_the_folder_and_keeps_the_dialog_open_with_the_reason_when_nothing_was_kept(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = on_claude(cx, Vec::new());
    click(cx, "button:marketplace-import-account");
    click(cx, "dialog-action");
    assert!(rig.script.next().is_none(), "no import without a folder");
    assert_eq!(
        import_error(&rig, cx),
        Some(Some(
            "Enter the folder that holds the login files.".to_owned()
        ))
    );

    rig.shell.update_in(cx, |shell, window, cx| {
        let field = shell
            .marketplace_accounts
            .import
            .as_ref()
            .expect("the dialog is open")
            .field()
            .clone();
        field.update(cx, |field, cx| {
            field.set_text(" placeholder-folder ", window, cx)
        });
    });
    click(cx, "dialog-action");
    let Some(Command::Account(AccountCommand::Import { params, reply })) = rig.script.next() else {
        panic!("an agent.accounts.import call was expected");
    };
    assert_eq!(
        (params.kind.as_str(), params.from.as_deref(), params.label),
        ("claude", Some("placeholder-folder"), None)
    );
    reply
        .send(Ok(AccountsImported {
            result: ImportResult::NotFound,
            account: None,
            reason: Some("none of the declared files is in the source".to_owned()),
            updated: None,
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    assert_eq!(
        import_error(&rig, cx),
        Some(Some(
            "None of the declared files is in the source.".to_owned()
        ))
    );

    click(cx, "dialog-action");
    let Some(Command::Account(AccountCommand::Import { reply, .. })) = rig.script.next() else {
        panic!("a second agent.accounts.import call was expected");
    };
    reply
        .send(Ok(AccountsImported {
            result: ImportResult::Imported,
            account: Some(summary("acct-i", "Account", AccountState::SignedIn, true)),
            reason: None,
            updated: None,
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    assert_eq!(
        import_error(&rig, cx),
        None,
        "an import that worked closes the dialog"
    );
}

#[gpui::test]
fn browse_on_account_import_fills_the_folder_and_a_folder_on_another_host_has_none(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = on_claude(cx, Vec::new());
    let (asked, picker) = answering(Some("C:/Users/me/claude-logins"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    click(cx, "button:marketplace-import-account");
    click(cx, "button:account-import-browse");
    assert_eq!(*asked.borrow(), [PickKind::Folder]);
    let folder = rig.shell.read_with(cx, |shell, cx| {
        shell
            .marketplace_accounts
            .import
            .as_ref()
            .expect("the dialog is open")
            .field()
            .read(cx)
            .text()
            .to_owned()
    });
    assert_eq!(folder, "C:/Users/me/claude-logins");

    rig.shell.update(cx, |shell, _| {
        shell
            .marketplace_pick
            .aim(HostTarget::Paired("vps".to_owned()))
    });
    draw(cx);
    assert!(
        cx.debug_bounds("button:account-import-browse").is_none(),
        "a folder on another host has no Browse"
    );
}
