//! Element tests of the install review: what the service reported is what the dialog holds, and the install is sent
//! only from the person's press, with the consent of the review they read. A scripted service answers each call.

use super::InstallReview;
use crate::model::marketplace::MarketplaceItemId;
use crate::model::navigation::MarketplacePage;
use crate::service::{Command, ExtensionCommand, ExtensionsRead, Refusal, StoreUpdate};
use crate::shell::service_tests::{Rig, click, draw, rig};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::extensions::{
    CliPlan, CliPlanState, ExtensionInstalled, ExtensionItem, ExtensionReview, ExtensionState,
    Listing, Permission, Risk, RiskLevel, SourceKind, SourceRef, UninstallEffects,
};
use tokio::sync::oneshot;

const CONSENT: &str = "fingerprint-of-the-review";

fn catalogue_package() -> ExtensionItem {
    ExtensionItem {
        id: "acme.notes".to_owned(),
        name: "Notes".to_owned(),
        description: "Keeps notes beside the workspace.".to_owned(),
        version: "1.0.0".to_owned(),
        publisher: "acme".to_owned(),
        state: ExtensionState::Available,
        installed: None,
        listing: Listing {
            logo: None,
            category: "extensions".to_owned(),
            developer: None,
            mention: None,
            banner: None,
            connections: Vec::new(),
        },
        permissions: Vec::new(),
        risk: Risk {
            level: RiskLevel::Low,
            reasons: Vec::new(),
        },
        readme: None,
        agents: Vec::new(),
        detected: None,
        settings: Vec::new(),
        uninstall: UninstallEffects {
            package_files: true,
            storage: false,
            settings: Vec::new(),
            secret_names: Vec::new(),
            secrets_unscoped: false,
            connections: Vec::new(),
            cli_binaries: Vec::new(),
            offered_agent_homes: Vec::new(),
        },
        problem: None,
    }
}

/// What `extensions.inspect` answers for the package; a vendor installer is listed when `runs_installer`.
fn review(runs_installer: bool) -> ExtensionReview {
    ExtensionReview {
        id: "acme.notes".to_owned(),
        name: "Notes".to_owned(),
        version: "1.0.0".to_owned(),
        publisher: "acme".to_owned(),
        description: "Keeps notes beside the workspace.".to_owned(),
        source: SourceRef {
            kind: SourceKind::Catalogue,
            reference: "acme.notes".to_owned(),
        },
        sha256: "0123abcd".to_owned(),
        permissions: vec![Permission {
            kind: "files:read".to_owned(),
            reason: "Reads the notes.".to_owned(),
            scope: None,
        }],
        risk: Risk {
            level: RiskLevel::Medium,
            reasons: Vec::new(),
        },
        listing: catalogue_package().listing,
        kinds: Vec::new(),
        mcp_servers: Vec::new(),
        cli: if runs_installer {
            vec![CliPlan {
                agent: "notes".to_owned(),
                binary: Some("notes".to_owned()),
                verify: None,
                uninstall: None,
                state: CliPlanState::WillRun {
                    command: vec![
                        "npm".to_owned(),
                        "install".to_owned(),
                        "-g".to_owned(),
                        "notes".to_owned(),
                    ],
                },
            }]
        } else {
            Vec::new()
        },
        runs_installer,
        consent: CONSENT.to_owned(),
    }
}

/// A connected shell with the catalogue listing one available package, and its page open.
fn open_page(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext, MarketplaceItemId) {
    let (rig, cx) = rig(cx, &[]);
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 1,
            installed: Ok(Vec::new()),
            catalogue: Ok(vec![catalogue_package()]),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    let id = rig
        .shell
        .read_with(cx, |shell, _| {
            shell
                .model()
                .marketplace()
                .items
                .iter()
                .find(|item| item.name == "Notes")
                .map(|item| item.id)
        })
        .expect("the package is listed");
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Item(id), cx)
    });
    draw(cx);
    (rig, cx, id)
}

/// Takes the inspect the review sent and returns its reply.
fn next_inspect(rig: &mut Rig) -> oneshot::Sender<Result<ExtensionReview, Refusal>> {
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { source, reply })) => {
            assert_eq!(source, "catalogue:acme.notes");
            reply
        }
        _ => panic!("the review asks the service what installing does"),
    }
}

fn installed() -> ExtensionInstalled {
    ExtensionInstalled {
        id: "acme.notes".to_owned(),
        version: "1.0.0".to_owned(),
        source: SourceRef {
            kind: SourceKind::Catalogue,
            reference: "acme.notes".to_owned(),
        },
        sha256: "0123abcd".to_owned(),
        enabled: true,
        installed_at_ms: 0,
        cli: Vec::new(),
    }
}

fn review_on_screen(rig: &Rig, cx: &mut VisualTestContext) -> Option<InstallReview> {
    rig.shell
        .read_with(cx, |shell, _| shell.marketplace_review.clone())
}

#[gpui::test]
fn the_review_holds_what_inspect_returned_and_install_is_sent_only_after_the_press(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, _) = open_page(cx);
    click(cx, "button:marketplace-install");
    let reply = next_inspect(&mut rig);
    assert!(
        rig.script.next().is_none(),
        "nothing installs before the press"
    );
    let answer = review(false);
    let _ = reply.send(Ok(answer.clone()));
    cx.run_until_parked();
    draw(cx);

    let held = review_on_screen(&rig, cx).expect("the review is on screen");
    assert_eq!(held.review.as_ref(), Some(&answer));
    assert!(
        rig.script.next().is_none(),
        "reading the review sends nothing else"
    );

    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { params, reply })) => {
            assert_eq!(params.source, "catalogue:acme.notes");
            assert_eq!(params.consent.as_deref(), Some(CONSENT));
            assert!(
                !params.run_installer,
                "no vendor installer is run without its tick"
            );
            let _ = reply.send(Ok(installed()));
        }
        _ => panic!("the press sends the install with the reviewed consent"),
    }
    cx.run_until_parked();
    assert!(
        review_on_screen(&rig, cx).is_none(),
        "an install that worked closes the review"
    );
}

#[gpui::test]
fn a_review_that_runs_a_vendor_installer_needs_its_own_tick_before_install(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, _) = open_page(cx);
    click(cx, "button:marketplace-install");
    let reply = next_inspect(&mut rig);
    let _ = reply.send(Ok(review(true)));
    cx.run_until_parked();
    draw(cx);

    click(cx, "dialog-action");
    assert!(
        rig.script.next().is_none(),
        "Install is off until the installer is ticked"
    );

    click(cx, "marketplace-install-run-installer");
    assert!(review_on_screen(&rig, cx).is_some_and(|held| held.run_installer));
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { params, reply })) => {
            assert!(params.run_installer, "the ticked installer is accepted");
            assert_eq!(params.consent.as_deref(), Some(CONSENT));
            let _ = reply.send(Ok(installed()));
        }
        _ => panic!("the ticked review sends the install"),
    }
}

#[gpui::test]
fn a_package_changed_after_its_review_says_so_and_keeps_the_review_open(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_page(cx);
    click(cx, "button:marketplace-install");
    let reply = next_inspect(&mut rig);
    let _ = reply.send(Ok(review(false)));
    cx.run_until_parked();
    draw(cx);
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { reply, .. })) => {
            let _ = reply.send(Err(Refusal::Failed {
                code: "extension.consent_mismatch".to_owned(),
                message: "the package or its install plan changed since the review".to_owned(),
            }));
        }
        _ => panic!("the press sends the install"),
    }
    cx.run_until_parked();
    let held = review_on_screen(&rig, cx).expect("the review stays open");
    assert_eq!(
        held.error.as_deref(),
        Some("The package changed after you reviewed it. Review it again before installing.")
    );
    assert!(!held.sending);
}
