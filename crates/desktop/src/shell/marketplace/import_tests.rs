//! Element tests of the account import folder: manual without the capability, prefilled and sent when detected.

use crate::model::marketplace::{
    CandidateFile, ImportCandidate, ImportCandidates, MarketplaceItemId,
};
use crate::model::navigation::MarketplacePage;
use crate::service::fixtures::agent_kind;
use crate::service::{AccountCommand, Command, StoreUpdate};
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
        shown(cx, "button:marketplace-import-browse"),
        "Browse sits with the folder field"
    );
    assert!(
        rig.script.next().is_none(),
        "opening the import asks the service nothing without the capability"
    );
}

#[gpui::test]
fn a_detected_folder_prefills_the_import_field_and_import_sends_it_as_before(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = on_claude(cx);
    click(cx, "button:marketplace-import-account");
    let folder = "C:/Users/me/.claude";
    let detected = ImportCandidates {
        candidates: vec![ImportCandidate {
            kind: "claude".to_owned(),
            ready: true,
            folder: Some(folder.to_owned()),
            files: vec![CandidateFile {
                name: ".credentials.json".to_owned(),
                required: true,
                found: true,
                reason: None,
                path: None,
                env: None,
            }],
        }],
    };
    rig.shell.update(cx, |shell, cx| {
        shell.finish_account_detection("claude", Some(detected), cx)
    });
    draw(cx);
    let (text, placeholder) = rig.shell.read_with(cx, |shell, app| {
        let import = shell
            .marketplace_accounts
            .import
            .as_ref()
            .expect("the dialog is open");
        (
            import.field().read(app).text().to_owned(),
            import.placeholder().to_owned(),
        )
    });
    assert_eq!((text.as_str(), placeholder.as_str()), (folder, folder));
    click(cx, "dialog-action");
    let Some(Command::Account(AccountCommand::Import { params, .. })) = rig.script.next() else {
        panic!("an agent.accounts.import call was expected");
    };
    assert_eq!(
        params.from, folder,
        "Import sends the detected folder until the protocol makes from optional"
    );
}
