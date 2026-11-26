//! Element tests of the account import folder: manual without the capability, prefilled and sent when detected.

use crate::model::marketplace::{
    CandidateFile, ImportCandidate, ImportCandidates, MarketplaceItemId,
};
use crate::model::navigation::MarketplacePage;
use crate::service::fixtures::agent_kind;
use crate::service::{AccountCommand, Command, Refusal, StoreUpdate};
use crate::shell::service_tests::{Rig, click, draw, rig};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::PackageState;

/// A connected shell whose service lists Claude with no accounts, on Claude's item page.
fn on_claude(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
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
            result: Ok(Vec::new()),
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
fn import_without_the_capability_keeps_the_manual_folder_field(cx: &mut TestAppContext) {
    let (mut rig, cx) = on_claude(cx);
    click(cx, "button:marketplace-import-account");
    let (text, placeholder, detected) = rig.shell.read_with(cx, |shell, app| {
        let import = shell
            .marketplace_accounts
            .import
            .as_ref()
            .expect("the dialog is open");
        (
            import.field().read(app).text().to_owned(),
            import.placeholder().to_owned(),
            import.detection().is_some(),
        )
    });
    assert_eq!(
        (text.as_str(), placeholder.as_str(), detected),
        ("", "Folder that holds the login files", false),
        "without the capability the folder field is the manual one"
    );
    assert!(
        shown(cx, "button:account-import-browse"),
        "Browse sits with the folder field"
    );
    assert!(
        rig.script.next().is_none(),
        "opening the import asks the service nothing without the capability"
    );
}

/// What the service answers for a ready Claude: the login file is in `folder`.
fn ready_claude(folder: &str) -> ImportCandidates {
    ImportCandidates {
        candidates: vec![ImportCandidate {
            folder: Some(folder.to_owned()),
            files: vec![CandidateFile {
                ..crate::service::fixtures::base_candidate_file()
            }],
            ..crate::service::fixtures::base_import_candidate()
        }],
    }
}

/// Opens Claude's import on a service that offers detection, and answers the candidates read with `answer`.
fn open_detected(
    rig: &mut Rig,
    cx: &mut VisualTestContext,
    answer: Result<ImportCandidates, Refusal>,
) {
    rig.updates
        .send(StoreUpdate::AgentImportDetectOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "button:marketplace-import-account");
    let Some(Command::Account(AccountCommand::ImportCandidates { kind, reply })) =
        rig.script.next()
    else {
        panic!("an agent.accounts.import_candidates call was expected");
    };
    assert_eq!(kind, "claude");
    reply.send(answer).expect("the shell waits");
    cx.run_until_parked();
    draw(cx);
}

fn field_text(rig: &Rig, cx: &mut VisualTestContext) -> String {
    rig.shell.read_with(cx, |shell, app| {
        let import = shell
            .marketplace_accounts
            .import
            .as_ref()
            .expect("the dialog is open");
        import.field().read(app).text().to_owned()
    })
}

#[gpui::test]
fn a_ready_folder_prefills_the_field_and_kept_unedited_import_sends_no_folder(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = on_claude(cx);
    let folder = "C:/Users/me/.claude";
    open_detected(&mut rig, cx, Ok(ready_claude(folder)));
    assert_eq!(field_text(&rig, cx), folder);
    click(cx, "dialog-action");
    let Some(Command::Account(AccountCommand::Import { params, .. })) = rig.script.next() else {
        panic!("an agent.accounts.import call was expected");
    };
    assert_eq!(
        (params.kind.as_str(), params.from),
        ("claude", None),
        "the kept detected folder is the service's to find"
    );
}

#[gpui::test]
fn an_edited_folder_is_sent_as_typed(cx: &mut TestAppContext) {
    let (mut rig, cx) = on_claude(cx);
    open_detected(&mut rig, cx, Ok(ready_claude("C:/Users/me/.claude")));
    rig.shell.update_in(cx, |shell, window, cx| {
        let field = shell
            .marketplace_accounts
            .import
            .as_ref()
            .expect("the dialog is open")
            .field()
            .clone();
        field.update(cx, |field, cx| field.set_text(" D:/other ", window, cx));
    });
    click(cx, "dialog-action");
    let Some(Command::Account(AccountCommand::Import { params, .. })) = rig.script.next() else {
        panic!("an agent.accounts.import call was expected");
    };
    assert_eq!(params.from.as_deref(), Some("D:/other"));
}

#[gpui::test]
fn a_refused_candidates_read_leaves_the_manual_field(cx: &mut TestAppContext) {
    let (mut rig, cx) = on_claude(cx);
    open_detected(
        &mut rig,
        cx,
        Err(Refusal::unavailable("the service refused")),
    );
    assert_eq!(field_text(&rig, cx), "");
    click(cx, "dialog-action");
    assert!(
        rig.script.next().is_none(),
        "with no folder typed nothing is imported"
    );
}
