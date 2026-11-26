//! Element tests of the install review: what the service reported is what the dialog holds, and the install is sent
//! only from the person's press, with the consent of the review they read. A scripted service answers each call.

use super::InstallReview;
use crate::model::marketplace::{ItemState, MarketplaceItemId, ProblemTone, problem_alert};
use crate::model::navigation::MarketplacePage;
use crate::service::fixtures::agent_kind;
use crate::service::{
    AccountCommand, Command, ExtensionCommand, ExtensionHistory, ExtensionsRead, Refusal,
    RollbackAnswer, StoreUpdate,
};
use crate::shell::service_tests::{Rig, click, draw, rig};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::PackageState;
use massorca_protocol::methods::extensions::{
    CliPlan, CliPlanState, ExtensionEnabled, ExtensionInstalled, ExtensionItem, ExtensionLog,
    ExtensionRemoved, ExtensionReview, ExtensionRun, ExtensionRunAction, ExtensionRunOutcome,
    ExtensionState, InstalledInfo, ItemProblem, Listing, Permission, Risk, RiskLevel, SourceKind,
    SourceRef, UninstallEffects,
};
use tokio::sync::oneshot;

const CONSENT: &str = "fingerprint-of-the-review";

pub(super) fn catalogue_package() -> ExtensionItem {
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
            website: None,
            privacy: None,
            terms: None,
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
pub(super) fn review(runs_installer: bool) -> ExtensionReview {
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
                    runs_package_scripts: true,
                },
            }]
        } else {
            Vec::new()
        },
        runs_installer,
        consent: CONSENT.to_owned(),
    }
}

/// A connected shell whose lists hold these packages, with the page of the catalogue's first one open.
pub(super) fn open_listed(
    cx: &mut TestAppContext,
    installed: Vec<ExtensionItem>,
    catalogue: Vec<ExtensionItem>,
) -> (Rig, &mut VisualTestContext, MarketplaceItemId) {
    let (rig, cx) = rig(cx, &[]);
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 1,
            installed: Ok(installed),
            catalogue: Ok(catalogue),
            history: Default::default(),
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

/// A connected shell with the catalogue listing one available package, and its page open.
fn open_page(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext, MarketplaceItemId) {
    open_listed(cx, Vec::new(), vec![catalogue_package()])
}

/// The package as the service lists it once installed, on or off.
fn installed_package(enabled: bool) -> ExtensionItem {
    let mut item = catalogue_package();
    item.state = if enabled {
        ExtensionState::Enabled
    } else {
        ExtensionState::Disabled
    };
    item.installed = Some(InstalledInfo {
        version: "1.0.0".to_owned(),
        source: SourceRef {
            kind: SourceKind::Catalogue,
            reference: "acme.notes".to_owned(),
        },
        sha256: "0123abcd".to_owned(),
        installed_at_ms: 0,
        cli: Vec::new(),
    });
    item
}

fn item_state(rig: &Rig, cx: &mut VisualTestContext, id: MarketplaceItemId) -> Option<ItemState> {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .model()
            .marketplace()
            .item(id)
            .map(|item| item.state.clone())
    })
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

#[gpui::test]
fn the_enabled_switch_sends_the_change_and_shows_only_what_the_service_then_reports(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, id) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    click(cx, "marketplace-enabled-switch");
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::SetEnabled { params, reply })) => {
            assert_eq!(params.id, "acme.notes");
            assert!(
                !params.enabled,
                "the switch sends the state it was turned to"
            );
            reply
        }
        _ => panic!("the switch sends the change to the service"),
    };
    cx.run_until_parked();
    assert_eq!(
        item_state(&rig, cx, id),
        Some(ItemState::Installed {
            enabled: true,
            update: None
        }),
        "nothing moves before the service answers"
    );

    let _ = reply.send(Ok(ExtensionEnabled {
        id: "acme.notes".to_owned(),
        enabled: false,
        changed: true,
    }));
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 2,
            installed: Ok(vec![installed_package(false)]),
            catalogue: Ok(vec![installed_package(false)]),
            history: Default::default(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    assert_eq!(
        item_state(&rig, cx, id),
        Some(ItemState::Installed {
            enabled: false,
            update: None
        })
    );
}

#[gpui::test]
fn a_refused_switch_says_why_under_the_item(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    click(cx, "marketplace-enabled-switch");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::SetEnabled { reply, .. })) => {
            let _ = reply.send(Err(Refusal::Failed {
                code: "extension.busy".to_owned(),
                message: "extension acme.notes is busy".to_owned(),
            }));
        }
        _ => panic!("the switch sends the change to the service"),
    }
    cx.run_until_parked();
    let shown = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_item_error.clone());
    assert_eq!(
        shown,
        Some((
            id,
            "This package is busy. Wait for its install or removal to finish.".to_owned()
        ))
    );
}

#[gpui::test]
fn uninstall_sends_the_removal_and_closes_the_confirm_when_the_service_answers(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, id) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { params, reply })) => {
            assert_eq!(params.id, "acme.notes");
            assert!(!params.delete_agent_homes && !params.uninstall_cli);
            assert_eq!(params.uninstall_consent, None);
            let _ = reply.send(Ok(ExtensionRemoved {
                id: "acme.notes".to_owned(),
                version: "1.0.0".to_owned(),
                failures: Vec::new(),
                cli: Vec::new(),
            }));
        }
        _ => panic!("the confirm sends the removal to the service"),
    }
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 2,
            installed: Ok(Vec::new()),
            catalogue: Ok(vec![catalogue_package()]),
            history: Default::default(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_uninstall.is_none()),
        "a removal that worked closes the confirm"
    );
    assert_eq!(item_state(&rig, cx, id), Some(ItemState::NotInstalled));
}

#[gpui::test]
fn a_refused_removal_keeps_the_confirm_open_with_the_reason(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { reply, .. })) => {
            let _ = reply.send(Err(Refusal::Failed {
                code: "agent.busy".to_owned(),
                message: "a session uses this agent".to_owned(),
            }));
        }
        _ => panic!("the confirm sends the removal to the service"),
    }
    cx.run_until_parked();
    let confirm = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_uninstall.clone())
        .expect("the confirm stays open");
    assert!(!confirm.sending);
    assert_eq!(confirm.error.as_deref(), Some("A session uses this agent"));
}

/// An installed package whose install record says it installed its CLI.
fn installed_with_cli() -> ExtensionItem {
    let mut item = installed_package(true);
    if let Some(info) = item.installed.as_mut() {
        info.cli = vec![massorca_protocol::methods::extensions::CliRecord {
            agent: "notes".to_owned(),
            binary: "notes".to_owned(),
            detected_before_install: false,
            installer: None,
            installed_by_package: true,
        }];
    }
    item
}

#[gpui::test]
fn a_removal_that_takes_the_cli_sends_uninstall_cli_once_the_box_is_ticked(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, _) = open_listed(cx, vec![installed_with_cli()], vec![installed_with_cli()]);
    click(cx, "button:marketplace-uninstall");
    click(cx, "marketplace-uninstall-cli");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { params, reply })) => {
            assert_eq!(params.id, "acme.notes");
            assert!(params.uninstall_cli, "the ticked CLI removal is asked for");
            let _ = reply.send(Ok(ExtensionRemoved {
                id: "acme.notes".to_owned(),
                version: "1.0.0".to_owned(),
                failures: Vec::new(),
                cli: Vec::new(),
            }));
        }
        _ => panic!("the confirm sends the removal to the service"),
    }
}

#[gpui::test]
fn an_unticked_cli_removal_keeps_the_cli_as_today(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(cx, vec![installed_with_cli()], vec![installed_with_cli()]);
    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { params, reply })) => {
            assert!(
                !params.uninstall_cli,
                "the CLI stays unless the box is ticked"
            );
            let _ = reply.send(Ok(ExtensionRemoved {
                id: "acme.notes".to_owned(),
                version: "1.0.0".to_owned(),
                failures: Vec::new(),
                cli: Vec::new(),
            }));
        }
        _ => panic!("the confirm sends the removal to the service"),
    }
}

#[gpui::test]
fn a_cli_that_was_not_removed_is_said_under_the_item(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(cx, vec![installed_with_cli()], vec![installed_with_cli()]);
    click(cx, "button:marketplace-uninstall");
    click(cx, "marketplace-uninstall-cli");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { reply, .. })) => {
            let _ = reply.send(Ok(ExtensionRemoved {
                id: "acme.notes".to_owned(),
                version: "1.0.0".to_owned(),
                failures: Vec::new(),
                cli: vec![massorca_protocol::methods::extensions::CliRemoval {
                    agent: "notes".to_owned(),
                    binary: "notes".to_owned(),
                    action: massorca_protocol::methods::extensions::CliRemovalAction::Failed {
                        command: vec!["npm".to_owned(), "uninstall".to_owned()],
                        message: "the uninstall exited with code 1".to_owned(),
                    },
                }],
            }));
        }
        _ => panic!("the confirm sends the removal to the service"),
    }
    cx.run_until_parked();
    let shown = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_item_error.clone());
    assert_eq!(
        shown,
        Some((
            id,
            "Removed, but not every step finished. The CLI was not removed: the uninstall exited with code 1"
                .to_owned()
        ))
    );
}

/// A built-in agent's package as the catalogue lists it: it names the agent kind it provides.
fn agent_package(kind: &str, state: ExtensionState) -> ExtensionItem {
    let mut item = catalogue_package();
    item.id = format!("massorca.{kind}");
    item.publisher = "massorca".to_owned();
    item.state = state;
    item.listing.category = "agents".to_owned();
    item.agents = vec![massorca_protocol::methods::extensions::AgentItem {
        id: kind.to_owned(),
        detected: false,
        program_path: None,
        path_source: None,
        cli_installed_by_package: false,
        cli: None,
    }];
    item
}

/// A built-in agent's package once installed, on or off.
fn installed_agent(kind: &str, enabled: bool) -> ExtensionItem {
    let mut item = agent_package(
        kind,
        if enabled {
            ExtensionState::Enabled
        } else {
            ExtensionState::Disabled
        },
    );
    item.installed = Some(InstalledInfo {
        version: "1.0.0".to_owned(),
        source: SourceRef {
            kind: SourceKind::Catalogue,
            reference: format!("massorca.{kind}"),
        },
        sha256: "0123abcd".to_owned(),
        installed_at_ms: 0,
        cli: Vec::new(),
    });
    item
}

/// A connected shell that lists one agent kind and its package, with the agent's page open.
fn open_agent<'a>(
    cx: &'a mut TestAppContext,
    kind: &str,
    name: &str,
    installed: Vec<ExtensionItem>,
    catalogue: Vec<ExtensionItem>,
) -> (Rig, &'a mut VisualTestContext, MarketplaceItemId) {
    let (rig, cx) = rig(cx, &[]);
    let state = match installed.first() {
        None => PackageState::Available,
        Some(package) if package.state == ExtensionState::Enabled => PackageState::Enabled,
        Some(_) => PackageState::Disabled,
    };
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            kind, name, state,
        )]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 1,
            installed: Ok(installed),
            catalogue: Ok(catalogue),
            history: Default::default(),
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
                .find(|item| item.name == name)
                .map(|item| item.id)
        })
        .expect("the agent is listed once, with its package");
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Item(id), cx)
    });
    draw(cx);
    (rig, cx, id)
}

#[gpui::test]
fn a_built_in_agent_installs_through_the_review_like_any_package(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        Vec::new(),
        vec![agent_package("codex", ExtensionState::Available)],
    );
    click(cx, "button:marketplace-install");
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { source, reply })) => {
            assert_eq!(source, "catalogue:massorca.codex");
            reply
        }
        _ => panic!("the review asks the service what installing the agent does"),
    };
    let _ = reply.send(Ok(review(false)));
    cx.run_until_parked();
    draw(cx);

    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { params, reply })) => {
            assert_eq!(params.source, "catalogue:massorca.codex");
            assert_eq!(params.consent.as_deref(), Some(CONSENT));
            let _ = reply.send(Ok(installed()));
        }
        _ => panic!("the press sends the agent's install with the reviewed consent"),
    }
}

#[gpui::test]
fn a_built_in_agent_switch_turns_its_package_on_and_off(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        vec![installed_agent("codex", false)],
        vec![installed_agent("codex", false)],
    );
    click(cx, "marketplace-enabled-switch");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::SetEnabled { params, reply })) => {
            assert_eq!(params.id, "massorca.codex");
            assert!(params.enabled, "the switch turns the agent on");
            let _ = reply.send(Ok(ExtensionEnabled {
                id: "massorca.codex".to_owned(),
                enabled: true,
                changed: true,
            }));
        }
        _ => panic!("the switch sends the agent's package to the service"),
    }
}

#[gpui::test]
fn a_built_in_agent_uninstall_removes_its_package_and_leaves_its_cli(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        vec![installed_agent("codex", true)],
        vec![installed_agent("codex", true)],
    );
    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { params, reply })) => {
            assert_eq!(params.id, "massorca.codex");
            assert!(
                !params.uninstall_cli,
                "the CLI stays unless the person asks"
            );
            let _ = reply.send(Ok(ExtensionRemoved {
                id: "massorca.codex".to_owned(),
                version: "1.0.0".to_owned(),
                failures: Vec::new(),
                cli: Vec::new(),
            }));
        }
        _ => panic!("the confirm sends the agent's removal to the service"),
    }
}

/// An installed package whose files the service found changed (`extension.tampered`).
fn tampered_package() -> ExtensionItem {
    let mut item = installed_package(true);
    item.problem = Some(ItemProblem {
        code: "extension.tampered".to_owned(),
        message: "the files changed".to_owned(),
    });
    item
}

fn shown(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    draw(cx);
    cx.debug_bounds(selector).is_some()
}

#[gpui::test]
fn a_tampered_package_shows_its_problem_on_its_page_and_on_its_catalogue_card(
    cx: &mut TestAppContext,
) {
    let (rig, cx, id) = open_listed(cx, vec![tampered_package()], vec![tampered_package()]);
    assert!(
        shown(cx, "marketplace-problem"),
        "the item page shows the alert"
    );
    let tone = rig.shell.read_with(cx, |shell, _| {
        shell
            .model()
            .marketplace()
            .item(id)
            .and_then(problem_alert)
            .map(|alert| alert.tone)
    });
    assert_eq!(tone, Some(ProblemTone::Danger));

    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Discover, cx)
    });
    assert!(
        shown(cx, "marketplace-problem"),
        "the catalogue card shows the alert too"
    );
}

#[gpui::test]
fn a_tampered_package_refuses_enable_and_can_still_be_uninstalled(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(cx, vec![tampered_package()], vec![tampered_package()]);
    click(cx, "marketplace-enabled-switch");
    assert!(
        rig.script.next().is_none(),
        "the refused switch sends nothing to the service"
    );
    let refused = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_item_error.clone());
    assert_eq!(
        refused
            .as_ref()
            .map(|(item, text)| (*item, text.contains("cannot be turned on"))),
        Some((id, true))
    );

    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { params, reply })) => {
            assert_eq!(params.id, "acme.notes");
            let _ = reply.send(Ok(ExtensionRemoved {
                id: "acme.notes".to_owned(),
                version: "1.0.0".to_owned(),
                failures: Vec::new(),
                cli: Vec::new(),
            }));
        }
        _ => panic!("uninstall is still sent for a tampered package"),
    }
}

#[gpui::test]
fn the_information_part_links_each_page_the_package_lists(cx: &mut TestAppContext) {
    let mut package = catalogue_package();
    package.listing.website = Some("https://acme.example".to_owned());
    package.listing.terms = Some("https://acme.example/terms".to_owned());
    let (_rig, cx, _) = open_listed(cx, Vec::new(), vec![package]);
    assert!(
        shown(cx, "button:marketplace-link-website"),
        "the website is a link"
    );
    assert!(
        shown(cx, "button:marketplace-link-terms-of-service"),
        "the terms are a link"
    );
    assert!(
        !shown(cx, "button:marketplace-link-privacy-policy"),
        "a page the package does not list has no row"
    );
}

/// An installed package from a folder: not built in, so the update check inspects it.
#[gpui::test]
fn add_extension_reviews_a_folder_package_and_installs_it_only_after_the_press(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[]);
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Discover, cx)
    });
    click(cx, "button:marketplace-add-extension");
    let folder = if cfg!(windows) {
        "C:/Users/me/kit"
    } else {
        "/Users/me/kit"
    };
    rig.shell.update_in(cx, |shell, window, cx| {
        let field = shell
            .marketplace_add
            .as_ref()
            .expect("the add dialog is open")
            .field()
            .clone();
        field.update(cx, |field, cx| field.set_text(folder, window, cx));
    });
    click(cx, "dialog-action");
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { source, reply })) => {
            assert_eq!(source, format!("dir:{folder}"));
            reply
        }
        _ => panic!("the review asks the service what the folder's package does"),
    };
    let answer = review(false);
    let _ = reply.send(Ok(answer.clone()));
    cx.run_until_parked();
    draw(cx);

    let held = review_on_screen(&rig, cx).expect("the review is on screen");
    assert_eq!(held.item, None, "a folder review is not a catalogue item's");
    assert_eq!(held.review.as_ref(), Some(&answer));
    assert!(
        rig.script.next().is_none(),
        "reading the review sends nothing else"
    );

    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { params, reply })) => {
            assert_eq!(params.source, format!("dir:{folder}"));
            assert_eq!(params.consent.as_deref(), Some(CONSENT));
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

fn folder_package() -> ExtensionItem {
    let mut item = installed_package(true);
    if let Some(installed) = item.installed.as_mut() {
        installed.source = SourceRef {
            kind: SourceKind::Dir,
            reference: "/packages/notes".to_owned(),
        };
    }
    item
}

/// The folder package as a later read lists it, once an update installed version 2.0.0.
fn updated_folder_package() -> ExtensionItem {
    let mut item = folder_package();
    if let Some(installed) = item.installed.as_mut() {
        installed.version = "2.0.0".to_owned();
        installed.sha256 = "fresh".to_owned();
    }
    item
}

/// Takes the inspect the update check of a folder package sent and returns its reply.
fn next_update_check(rig: &mut Rig) -> oneshot::Sender<Result<ExtensionReview, Refusal>> {
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { source, reply })) => {
            assert_eq!(source, "dir:/packages/notes");
            reply
        }
        _ => panic!("the update check inspects the folder package"),
    }
}

#[gpui::test]
fn a_newer_review_lists_the_folder_package_on_updates_with_its_version(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(cx, vec![folder_package()], Vec::new());
    let reply = next_update_check(&mut rig);
    let _ = reply.send(Ok(ExtensionReview {
        version: "2.0.0".to_owned(),
        sha256: "fresh".to_owned(),
        ..review(false)
    }));
    cx.run_until_parked();
    assert_eq!(
        item_state(&rig, cx, id),
        Some(ItemState::Installed {
            enabled: true,
            update: Some("2.0.0".to_owned())
        })
    );
    draw(cx);
    draw(cx);
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "drawing the page does not inspect the package again"
    );
}

#[gpui::test]
fn a_review_with_the_installed_sha256_is_not_an_update(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(cx, vec![folder_package()], Vec::new());
    let reply = next_update_check(&mut rig);
    let _ = reply.send(Ok(review(false)));
    cx.run_until_parked();
    assert_eq!(
        item_state(&rig, cx, id),
        Some(ItemState::Installed {
            enabled: true,
            update: None
        })
    );
}

#[gpui::test]
fn a_built_in_package_is_not_inspected(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(cx, vec![installed_package(true)], Vec::new());
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "a built-in package is refreshed by the service, so it is not inspected"
    );
}

#[gpui::test]
fn an_inspect_refusal_leaves_the_package_off_updates_and_is_not_asked_again(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, id) = open_listed(cx, vec![folder_package()], Vec::new());
    let reply = next_update_check(&mut rig);
    let _ = reply.send(Err(Refusal::unavailable("the service is busy")));
    cx.run_until_parked();
    assert_eq!(
        item_state(&rig, cx, id),
        Some(ItemState::Installed {
            enabled: true,
            update: None
        })
    );
    draw(cx);
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "a refused check is not retried"
    );
}

#[gpui::test]
fn update_sends_the_consent_of_the_checked_review_and_the_list_refreshes(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(cx, vec![folder_package()], Vec::new());
    let reply = next_update_check(&mut rig);
    let _ = reply.send(Ok(ExtensionReview {
        version: "2.0.0".to_owned(),
        sha256: "fresh".to_owned(),
        consent: "consent-2".to_owned(),
        ..review(false)
    }));
    cx.run_until_parked();
    assert_eq!(
        id,
        MarketplaceItemId(1000),
        "the first package is item 1000"
    );
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Updates, cx)
    });
    click(cx, "button:marketplace-card-1000-update");
    cx.run_until_parked();
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Update { params, reply })) => {
            assert_eq!(params.id, "acme.notes");
            assert_eq!(params.consent.as_deref(), Some("consent-2"));
            assert_eq!(params.sha256, None, "a folder is not pinned by hash");
            let _ = reply.send(Ok(installed()));
        }
        _ => panic!("the Update button sends extensions.update"),
    }
    cx.run_until_parked();
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 2,
            installed: Ok(vec![updated_folder_package()]),
            catalogue: Ok(Vec::new()),
            history: Default::default(),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    assert_eq!(
        item_state(&rig, cx, id),
        Some(ItemState::Installed {
            enabled: true,
            update: None
        }),
        "the refreshed list no longer offers the update"
    );
}

#[gpui::test]
fn a_refused_update_opens_the_item_page_with_the_refusal(cx: &mut TestAppContext) {
    let (mut rig, cx, id) = open_listed(cx, vec![folder_package()], Vec::new());
    let reply = next_update_check(&mut rig);
    let _ = reply.send(Ok(ExtensionReview {
        version: "2.0.0".to_owned(),
        sha256: "fresh".to_owned(),
        ..review(false)
    }));
    cx.run_until_parked();
    assert_eq!(
        id,
        MarketplaceItemId(1000),
        "the first package is item 1000"
    );
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Updates, cx)
    });
    click(cx, "button:marketplace-card-1000-update");
    cx.run_until_parked();
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Update { reply, .. })) => {
            let _ = reply.send(Err(Refusal::Failed {
                code: "extension.consent_mismatch".to_owned(),
                message: "the review changed".to_owned(),
            }));
        }
        _ => panic!("the Update button sends extensions.update"),
    }
    cx.run_until_parked();
    assert!(
        shown(cx, "marketplace-connection-alert"),
        "the refusal is shown on the item page"
    );
}

/// While the agent's package is not installed, Add account sends nothing and Import does not open.
#[gpui::test]
fn add_account_and_import_wait_while_the_agents_package_is_not_installed(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        Vec::new(),
        vec![agent_package("codex", ExtensionState::Available)],
    );
    click(cx, "button:marketplace-add-account");
    assert!(rig.script.next().is_none(), "no account add is sent");
    click(cx, "button:marketplace-import-account");
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_accounts.import.is_none()),
        "no import dialog opens"
    );
}

/// While the agent's package is installed but off, Add account sends nothing.
#[gpui::test]
fn add_account_waits_while_the_agents_package_is_turned_off(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        vec![installed_agent("codex", false)],
        vec![installed_agent("codex", false)],
    );
    click(cx, "button:marketplace-add-account");
    assert!(rig.script.next().is_none(), "no account add is sent");
}

/// Once the install lands, the page re-reads the package and Add account works without a restart.
#[gpui::test]
fn add_account_turns_on_after_the_install_without_a_restart(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        Vec::new(),
        vec![agent_package("codex", ExtensionState::Available)],
    );
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            "codex",
            "Codex",
            PackageState::Enabled,
        )]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 2,
            installed: Ok(vec![installed_agent("codex", true)]),
            catalogue: Ok(vec![installed_agent("codex", true)]),
            history: Default::default(),
        }))
        .expect("the store listens");
    cx.run_until_parked();

    click(cx, "button:marketplace-add-account");
    match rig.script.next() {
        Some(Command::Account(AccountCommand::Add { params, .. })) => {
            assert_eq!(params.kind, "codex");
        }
        _ => panic!("Add account sends the agent's account add once its package is on"),
    }
}

/// A refused install shows what was looked for and where, and Copy details puts all of it on the clipboard.
#[gpui::test]
fn copy_details_puts_the_refused_install_on_the_clipboard(cx: &mut TestAppContext) {
    let refusal = "the installer for codex failed: `npm` was not found on PATH\nLooked for npm on the registry PATH.";
    let (mut rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        Vec::new(),
        vec![agent_package("codex", ExtensionState::Available)],
    );
    click(cx, "button:marketplace-install");
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { source, reply })) => {
            assert_eq!(source, "catalogue:massorca.codex");
            reply
        }
        _ => panic!("the review asks the service what installing does"),
    };
    let _ = reply.send(Ok(review(false)));
    cx.run_until_parked();
    draw(cx);
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { reply, .. })) => {
            let _ = reply.send(Err(Refusal::Failed {
                code: "extension.installer_failed".to_owned(),
                message: refusal.to_owned(),
            }));
        }
        _ => panic!("the press sends the agent's install"),
    }
    cx.run_until_parked();
    draw(cx);
    click(cx, "button:marketplace-install-copy");
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert!(
        copied
            .as_deref()
            .is_some_and(|text| text.contains("Looked for npm on the registry PATH.")),
        "{copied:?}"
    );
}

#[gpui::test]
fn details_start_collapsed_open_on_show_details_and_leave_the_install_request_unchanged(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, _) = open_page(cx);
    click(cx, "button:marketplace-install");
    let _ = next_inspect(&mut rig).send(Ok(review(false)));
    cx.run_until_parked();
    draw(cx);
    let open = |rig: &Rig, cx: &mut VisualTestContext| {
        review_on_screen(rig, cx).is_some_and(|review| review.details_open)
    };
    assert!(!open(&rig, cx), "details start collapsed");
    assert!(
        cx.debug_bounds("marketplace-install-details-list")
            .is_none(),
        "collapsed, the dialog shows only the sentence"
    );

    click(cx, "button:marketplace-install-details");
    assert!(open(&rig, cx));
    draw(cx);
    assert!(
        cx.debug_bounds("marketplace-install-details-list")
            .is_some(),
        "Show details draws the technical lines"
    );

    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { params, reply })) => {
            assert_eq!(params.source, "catalogue:acme.notes");
            assert_eq!(params.consent.as_deref(), Some(CONSENT));
            assert!(!params.run_installer);
            let _ = reply.send(Ok(installed()));
        }
        _ => panic!("opening the details changes nothing the install sends"),
    }
}

#[gpui::test]
fn removing_a_package_with_connections_deletes_its_agent_homes(cx: &mut TestAppContext) {
    let mut package = installed_package(true);
    package.listing.connections = vec![massorca_protocol::methods::extensions::Connection {
        id: "notes-login".to_owned(),
        kind: "agent-login".to_owned(),
        title: "Notes account".to_owned(),
        multiple: true,
    }];
    let (mut rig, cx, _) = open_listed(cx, vec![package.clone()], vec![package]);
    click(cx, "button:marketplace-uninstall");
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Remove { params, .. })) => {
            assert!(
                params.delete_agent_homes,
                "accounts the package connected go with it"
            );
            assert!(!params.uninstall_cli);
        }
        _ => panic!("the confirm sends the removal to the service"),
    }
}

#[gpui::test]
fn an_agent_page_header_draws_the_agents_own_mark(cx: &mut TestAppContext) {
    let (_rig, cx, _) = open_agent(
        cx,
        "codex",
        "Codex",
        Vec::new(),
        vec![agent_package("codex", ExtensionState::Available)],
    );
    assert!(cx.debug_bounds("marketplace-header-agent").is_some());
    assert!(cx.debug_bounds("marketplace-header-icon").is_none());
}

/// The service's history of the installed package: rollback and the log offered, and an update that replaced 0.9.0.
fn kept_history() -> ExtensionHistory {
    let update = ExtensionRun {
        at_ms: 1_700_000_000_000,
        action: ExtensionRunAction::Update,
        from_version: Some("0.9.0".to_owned()),
        to_version: Some("1.0.0".to_owned()),
        outcome: ExtensionRunOutcome::Ok,
        output: Vec::new(),
    };
    ExtensionHistory {
        rollback_offered: true,
        log_offered: true,
        runs: [("acme.notes".to_owned(), vec![update])]
            .into_iter()
            .collect(),
    }
}

/// The package as a rollback answers it: the version it restores, installed.
fn restored_package() -> ExtensionInstalled {
    ExtensionInstalled {
        id: "acme.notes".to_owned(),
        version: "0.9.0".to_owned(),
        source: SourceRef {
            kind: SourceKind::Catalogue,
            reference: "acme.notes".to_owned(),
        },
        sha256: "0123abcd".to_owned(),
        enabled: true,
        installed_at_ms: 1_700_000_000_000,
        cli: Vec::new(),
    }
}

/// Opens the More menu of the installed package's row, after the service's next read sends `history`.
fn open_installed_menu(rig: &mut Rig, cx: &mut VisualTestContext, history: ExtensionHistory) {
    rig.updates
        .send(StoreUpdate::Extensions(ExtensionsRead {
            number: 2,
            installed: Ok(vec![installed_package(true)]),
            catalogue: Ok(vec![installed_package(true)]),
            history,
        }))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "rail-marketplace");
    click(cx, "marketplace-nav-installed");
    click(cx, "button:marketplace-row-1000-more");
}

#[gpui::test]
fn roll_back_asks_without_consent_then_sends_the_consent_the_confirm_shows(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    open_installed_menu(&mut rig, cx, kept_history());
    assert!(
        shown(cx, "menu-item-0"),
        "the menu offers Roll back to 0.9.0 first"
    );
    click(cx, "menu-item-0");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Rollback { params, reply })) => {
            assert_eq!(params.id, "acme.notes");
            assert_eq!(params.consent, None, "the first call names no consent");
            let _ = reply.send(Ok(RollbackAnswer::ConsentNeeded {
                version: "0.9.0".to_owned(),
                consent: CONSENT.to_owned(),
            }));
        }
        _ => panic!("Roll back sends the rollback to the service"),
    }
    cx.run_until_parked();
    let confirm = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_rollback.clone())
        .expect("the confirm opens with the consent the service named");
    assert_eq!(
        (confirm.from.as_str(), confirm.version.as_str()),
        ("1.0.0", "0.9.0")
    );
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Rollback { params, reply })) => {
            assert_eq!(
                params.consent.as_deref(),
                Some(CONSENT),
                "the confirm sends the consent it shows"
            );
            let _ = reply.send(Ok(RollbackAnswer::Restored(restored_package())));
        }
        _ => panic!("the confirm sends the rollback again"),
    }
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_rollback.is_none()),
        "a rollback that worked closes the confirm"
    );
}

#[gpui::test]
fn a_rollback_with_no_version_kept_says_so_and_opens_no_confirm(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    open_installed_menu(&mut rig, cx, kept_history());
    click(cx, "menu-item-0");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Rollback { reply, .. })) => {
            let _ = reply.send(Err(Refusal::Failed {
                code: "extension.no_previous_version".to_owned(),
                message: "nothing is kept".to_owned(),
            }));
        }
        _ => panic!("Roll back sends the rollback to the service"),
    }
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_rollback.is_none()),
        "nothing kept opens no confirm"
    );
    let error = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_item_error.clone())
        .map(|(_, text)| text);
    assert_eq!(
        error.as_deref(),
        Some("There is no earlier version of this package to roll back to.")
    );
}

#[gpui::test]
fn without_the_offer_the_row_menu_has_no_roll_back(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    open_installed_menu(&mut rig, cx, ExtensionHistory::default());
    assert!(!shown(cx, "menu-item-1"), "Remove is the only entry");
    click(cx, "menu-item-0");
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_uninstall.is_some()),
        "the first entry is Remove, which asks the uninstall confirm"
    );
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_rollback.is_none()),
        "no roll back is offered without the service's offer"
    );
}

/// A run of the installed package, as the service lists it.
fn logged_run(
    action: ExtensionRunAction,
    from: Option<&str>,
    to: Option<&str>,
    output: Vec<String>,
) -> ExtensionRun {
    ExtensionRun {
        at_ms: 1_700_000_000_000,
        action,
        from_version: from.map(str::to_owned),
        to_version: to.map(str::to_owned),
        outcome: ExtensionRunOutcome::Ok,
        output,
    }
}

/// Chooses View log from the open More menu, and answers the service's read of the package's runs with `runs`.
fn answer_run_log(rig: &mut Rig, cx: &mut VisualTestContext, runs: Vec<ExtensionRun>) {
    click(cx, "menu-item-1");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Log { id, reply })) => {
            assert_eq!(id, "acme.notes");
            let _ = reply.send(Ok(ExtensionLog { id, runs }));
        }
        _ => panic!("View log asks the service for the package's runs"),
    }
    cx.run_until_parked();
}

#[gpui::test]
fn view_log_lists_the_runs_and_opens_output_only_for_a_run_that_wrote_some(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    open_installed_menu(&mut rig, cx, kept_history());
    answer_run_log(
        &mut rig,
        cx,
        vec![
            logged_run(ExtensionRunAction::Install, None, Some("0.9.0"), Vec::new()),
            logged_run(
                ExtensionRunAction::Update,
                Some("0.9.0"),
                Some("1.0.0"),
                vec!["npm WARN deprecated".to_owned()],
            ),
        ],
    );
    assert!(
        !shown(cx, "button:marketplace-run-0-output"),
        "a run that wrote no output has no details"
    );
    assert!(
        shown(cx, "button:marketplace-run-1-output"),
        "a run that wrote output opens its details"
    );
}

#[gpui::test]
fn a_package_with_no_runs_says_so(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    open_installed_menu(&mut rig, cx, kept_history());
    answer_run_log(&mut rig, cx, Vec::new());
    assert!(
        shown(cx, "marketplace-run-log-empty"),
        "an empty log says no runs are recorded yet"
    );
}

#[gpui::test]
fn copy_details_puts_the_run_log_on_the_clipboard(cx: &mut TestAppContext) {
    let (mut rig, cx, _) = open_listed(
        cx,
        vec![installed_package(true)],
        vec![installed_package(true)],
    );
    open_installed_menu(&mut rig, cx, kept_history());
    answer_run_log(
        &mut rig,
        cx,
        vec![logged_run(
            ExtensionRunAction::Update,
            Some("0.9.0"),
            Some("1.0.0"),
            vec!["npm WARN deprecated".to_owned()],
        )],
    );
    draw(cx);
    click(cx, "button:marketplace-run-log-copy");
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert!(
        copied.as_deref().is_some_and(|text| {
            text.contains("Updated · 0.9.0 to 1.0.0 · Done\nnpm WARN deprecated")
        }),
        "{copied:?}"
    );
}
