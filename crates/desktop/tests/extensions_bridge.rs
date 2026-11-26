//! The Marketplace extension reads and writes of the bridge against a real service in a temporary profile, driven
//! headlessly: no window opens. The package is a folder fixture with no vendor command, so no installer runs.

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::Client;
use massorca_desktop::model::marketplace::MarketplaceSnapshot;
use massorca_desktop::service::{
    ExtensionsRead, Refusal, RollbackAnswer, ServiceBridge, ServiceHandle, StoreState, StoreUpdate,
    connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::PackageState;
use massorca_protocol::methods::extensions::{
    ExtensionInstallParams, ExtensionRemoveParams, ExtensionRollbackParams, ExtensionRunAction,
    ExtensionSetEnabledParams, ExtensionState, ExtensionUpdateParams,
};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

const WAIT: Duration = Duration::from_secs(30);
const PACKAGE: &str = "acme.notes";

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop_service(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
}

/// Folds the bridge's updates into a store state until `done` holds.
async fn until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(WAIT, async {
        while !done(state) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}: {state:?}");
}

/// The state of an installed package in the store, if the store lists it.
fn installed_state(state: &StoreState, id: &str) -> Option<ExtensionState> {
    let read = state.extensions.as_ref()?;
    let installed = read.installed.as_ref().ok()?;
    installed
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.state)
}

/// A folder package with one permission and no vendor command.
fn write_package(root: &Path, version: &str) {
    fs::create_dir_all(root).expect("a package folder");
    let manifest = format!(
        r#"{{
  "id": "{PACKAGE}", "name": "Notes", "version": "{version}", "publisher": "acme",
  "description": "Keeps notes beside the workspace.", "api": "1.0",
  "requires": {{ "massorca": ">=0.1.0" }}, "surfaces": ["desktop"], "modes": ["workspaces"],
  "permissions": [{{ "kind": "storage", "reason": "Keeps its notes." }}],
  "listing": {{ "category": "extensions" }}
}}"#
    );
    fs::write(root.join("extension.json"), manifest).expect("the manifest is written");
}

fn source_of(root: &Path) -> String {
    format!("dir:{}", root.display())
}

fn install_params(source: String, consent: Option<String>) -> ExtensionInstallParams {
    ExtensionInstallParams {
        source,
        consent,
        run_installer: false,
        sha256: None,
        agent_token: None,
    }
}

#[tokio::test]
async fn the_bridge_lists_the_catalogue_and_no_installed_package_on_connect() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;

    let read = state.extensions.as_ref().expect("the lists are read");
    let catalogue = read.catalogue.as_ref().expect("the catalogue is listed");
    assert!(
        catalogue
            .iter()
            .any(|item| item.id == "massorca.claude" && item.state == ExtensionState::Available),
        "the built-in agent packages are listed as available: {catalogue:?}"
    );
    assert_eq!(read.installed.as_ref().map(Vec::len), Ok(0));
    stop_service(service).await;
}

#[tokio::test]
async fn an_install_without_the_reviewed_consent_is_refused_and_nothing_is_listed() {
    let home = temp_home();
    let package = home.path().join("package");
    write_package(&package, "1.0.0");
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;

    let review = handle
        .inspect_extension(source_of(&package))
        .await
        .expect("the review is read");
    assert_eq!(review.id, PACKAGE);
    assert!(!review.consent.is_empty(), "the review carries a consent");

    let refusal = handle
        .install_extension(install_params(source_of(&package), None))
        .await
        .expect_err("an install without consent is refused");
    assert_eq!(refusal.code(), Some("extension.consent_required"));
    assert!(matches!(refusal, Refusal::Failed { .. }));
    stop_service(service).await;
}

#[tokio::test]
async fn a_package_changed_after_its_review_is_refused_with_consent_mismatch() {
    let home = temp_home();
    let package = home.path().join("package");
    write_package(&package, "1.0.0");
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;
    let review = handle
        .inspect_extension(source_of(&package))
        .await
        .expect("the review is read");

    write_package(&package, "1.0.1");
    let refusal = handle
        .install_extension(install_params(
            source_of(&package),
            Some(review.consent.clone()),
        ))
        .await
        .expect_err("the changed package is refused");
    assert_eq!(refusal.code(), Some("extension.consent_mismatch"));
    stop_service(service).await;
}

#[tokio::test]
async fn an_install_with_the_reviewed_consent_is_listed_and_enable_disable_and_remove_follow() {
    let home = temp_home();
    let package = home.path().join("package");
    write_package(&package, "1.0.0");
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;

    let review = handle
        .inspect_extension(source_of(&package))
        .await
        .expect("the review is read");
    let installed = handle
        .install_extension(install_params(
            source_of(&package),
            Some(review.consent.clone()),
        ))
        .await
        .expect("the reviewed package installs");
    assert_eq!(installed.id, PACKAGE);
    assert!(installed.enabled);
    until(&mut updates, &mut state, "the installed package", |state| {
        installed_state(state, PACKAGE) == Some(ExtensionState::Enabled)
    })
    .await;

    handle
        .set_extension_enabled(ExtensionSetEnabledParams {
            id: PACKAGE.to_owned(),
            enabled: false,
        })
        .await
        .expect("the package is disabled");
    until(&mut updates, &mut state, "the disabled package", |state| {
        installed_state(state, PACKAGE) == Some(ExtensionState::Disabled)
    })
    .await;

    handle
        .set_extension_enabled(ExtensionSetEnabledParams {
            id: PACKAGE.to_owned(),
            enabled: true,
        })
        .await
        .expect("the package is enabled");
    until(
        &mut updates,
        &mut state,
        "the enabled package again",
        |state| installed_state(state, PACKAGE) == Some(ExtensionState::Enabled),
    )
    .await;

    handle
        .remove_extension(ExtensionRemoveParams {
            id: PACKAGE.to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect("the package is removed");
    until(&mut updates, &mut state, "the package is gone", |state| {
        installed_state(state, PACKAGE).is_none()
    })
    .await;

    let catalogue = state
        .extensions
        .as_ref()
        .and_then(|read| read.catalogue.as_ref().ok())
        .expect("the catalogue stays listed");
    assert!(catalogue.iter().all(|item| item.id != PACKAGE));
    stop_service(service).await;
}

#[tokio::test]
async fn a_read_older_than_the_held_one_never_replaces_it() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.extensions.is_some()
    })
    .await;
    let held = state.extensions.clone().expect("a read is held");
    assert!(held.number > 0, "the bridge numbers its reads");
    state.apply(
        StoreUpdate::Extensions(ExtensionsRead {
            number: held.number - 1,
            installed: Err(Refusal::unavailable("stale")),
            catalogue: Err(Refusal::unavailable("stale")),
            history: Default::default(),
        }),
        Instant::now(),
    );
    assert_eq!(state.extensions.as_ref(), Some(&held));
    stop_service(service).await;
}

/// The kind's package state in the store's agent kinds, if the store lists the kind.
fn kind_state(state: &StoreState, id: &str) -> Option<PackageState> {
    state
        .agent_kinds
        .iter()
        .find(|kind| kind.id == id)
        .map(|kind| kind.package.state)
}

const AGENT_PACKAGE: &str = "acme.shell";
const AGENT_KIND: &str = "shell";

/// The platform shell, which every OS has: the program of the fixture agent, so no vendor CLI is needed.
fn shell_program() -> &'static str {
    if cfg!(windows) { "cmd.exe" } else { "/bin/sh" }
}

/// A folder package whose one agent kind runs the platform shell. It declares no vendor command, so no installer runs.
fn write_agent_package(root: &Path) {
    let program = shell_program();
    fs::create_dir_all(root.join("agents")).expect("an agents folder");
    let manifest = format!(
        r#"{{
  "id": "{AGENT_PACKAGE}", "name": "Shell agent", "version": "1.0.0", "publisher": "acme",
  "description": "Runs the platform shell as an agent.", "api": "1.0",
  "requires": {{ "massorca": ">=0.1.0" }}, "surfaces": ["desktop"], "modes": ["workspaces"],
  "permissions": [{{ "kind": "process", "scope": {{ "programs": ["{program}"] }}, "reason": "Runs the platform shell." }}],
  "contributes": {{ "agentAdapters": [{{ "id": "{AGENT_KIND}", "kind": "agents/{AGENT_KIND}.json" }}] }},
  "listing": {{ "category": "agents" }}
}}"#
    );
    fs::write(root.join("extension.json"), manifest).expect("the manifest is written");
    let kind = format!(r#"{{ "id": "{AGENT_KIND}", "name": "Shell", "program": ["{program}"] }}"#);
    fs::write(root.join("agents").join(format!("{AGENT_KIND}.json")), kind)
        .expect("the kind is written");
}

#[tokio::test]
async fn installing_an_agent_package_refreshes_the_agent_kinds_without_a_reconnect() {
    let home = temp_home();
    let package = home.path().join("package");
    write_agent_package(&package);
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the agent kinds and lists",
        |state| {
            kind_state(state, "claude") == Some(PackageState::Available)
                && state.extensions.is_some()
        },
    )
    .await;
    assert_eq!(
        kind_state(&state, AGENT_KIND),
        None,
        "the agent kind is not listed before its package is installed"
    );

    let source = source_of(&package);
    let review = handle
        .inspect_extension(source.clone())
        .await
        .expect("the review is read");
    handle
        .install_extension(install_params(source, Some(review.consent.clone())))
        .await
        .expect("the reviewed agent package installs");
    // The service's own answer, so a stale store is told apart from a wrong service.
    let client = Client::connect(&profile(home.path()), connect_options(None))
        .await
        .expect("a client connects");
    let direct = client
        .agent_kinds()
        .await
        .expect("the service lists its kinds");
    let shell = direct.kinds.iter().find(|kind| kind.id == AGENT_KIND);
    assert_eq!(
        shell.map(|kind| kind.package.state),
        Some(PackageState::Enabled),
        "the service lists the installed agent kind as enabled"
    );
    until(
        &mut updates,
        &mut state,
        "the installed agent's kind",
        |state| kind_state(state, AGENT_KIND) == Some(PackageState::Enabled),
    )
    .await;

    handle
        .set_extension_enabled(ExtensionSetEnabledParams {
            id: AGENT_PACKAGE.to_owned(),
            enabled: false,
        })
        .await
        .expect("the agent package is disabled");
    until(
        &mut updates,
        &mut state,
        "the disabled agent's kind",
        |state| kind_state(state, AGENT_KIND) == Some(PackageState::Disabled),
    )
    .await;

    stop_service(service).await;
}

#[tokio::test]
async fn a_not_installed_built_in_agent_shows_its_example_prompts_and_colours() {
    use massorca_desktop::model::marketplace::{MarketplaceSource, ServiceMarketplace};

    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the kinds and packages",
        |state| state.extensions.is_some() && !state.agent_kinds.is_empty(),
    )
    .await;

    let read = state.extensions.as_ref().expect("the lists are read");
    let installed = read.installed.as_ref().expect("the installed list");
    let catalogue = read.catalogue.as_ref().expect("the catalogue is listed");
    let mut source = ServiceMarketplace::default();
    source.follow_agent_kinds(&state.agent_kinds);
    source.follow_extensions(installed, catalogue);

    let snapshot = source.snapshot();
    let item = snapshot
        .items
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some("antigravity"))
        .expect("Antigravity is listed");
    assert!(!item.state.is_installed(), "a fresh profile has no install");
    assert_eq!(
        item.examples.len(),
        3,
        "its three example prompts reach the page"
    );
    let package = item.extension.as_deref().expect("it carries its package");
    let banner = package
        .listing
        .banner
        .as_ref()
        .expect("the service sends its banner");
    assert_eq!(banner.colors.len(), 2, "its colours reach the page");
    stop_service(service).await;
}

/// Writes a folder package named "Notes" whose listing carries `banner`, a JSON object.
fn write_banner_package(root: &Path, banner: &str) {
    fs::create_dir_all(root).expect("a package folder");
    let manifest = format!(
        r#"{{
  "id": "{PACKAGE}", "name": "Notes", "version": "1.0.0", "publisher": "acme",
  "description": "Keeps notes beside the workspace.", "api": "1.0",
  "requires": {{ "massorca": ">=0.1.0" }}, "surfaces": ["desktop"], "modes": ["workspaces"],
  "permissions": [{{ "kind": "storage", "reason": "Keeps its notes." }}],
  "listing": {{ "category": "extensions", "mention": "notes", "banner": {banner} }}
}}"#
    );
    fs::write(root.join("extension.json"), manifest).expect("the manifest is written");
}

/// The Marketplace model the desktop builds from the service's lists.
fn marketplace_of(state: &StoreState) -> MarketplaceSnapshot {
    use massorca_desktop::model::marketplace::{MarketplaceSource, ServiceMarketplace};
    let read = state.extensions.as_ref().expect("the lists are read");
    let mut source = ServiceMarketplace::default();
    source.follow_agent_kinds(&state.agent_kinds);
    source.follow_extensions(
        read.installed.as_ref().expect("the installed list"),
        read.catalogue.as_ref().expect("the catalogue is listed"),
    );
    source.snapshot()
}

/// Installs the folder package at `package` through its reviewed consent, as the Marketplace does.
async fn install_folder_package(
    handle: &ServiceHandle,
    package: &Path,
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
) {
    let review = handle
        .inspect_extension(source_of(package))
        .await
        .expect("the review is read");
    handle
        .install_extension(install_params(
            source_of(package),
            Some(review.consent.clone()),
        ))
        .await
        .expect("the reviewed package installs");
    until(updates, state, "the installed package", |state| {
        installed_state(state, PACKAGE) == Some(ExtensionState::Enabled)
    })
    .await;
}

#[tokio::test]
async fn a_real_services_claude_banner_has_its_colours_and_three_examples() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the kinds and packages",
        |state| state.extensions.is_some() && !state.agent_kinds.is_empty(),
    )
    .await;
    let snapshot = marketplace_of(&state);
    let item = snapshot
        .items
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some("claude"))
        .expect("Claude is listed");
    assert_eq!(
        item.examples.len(),
        3,
        "its three example prompts reach the page"
    );
    let package = item.extension.as_deref().expect("it carries its package");
    let banner = package
        .listing
        .banner
        .as_ref()
        .expect("the service sends its banner");
    assert_eq!(
        banner.colors,
        ["#D97757", "#B5532F"],
        "its two colours reach the page"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_folder_package_without_colours_takes_the_palette_slot_of_its_id() {
    use massorca_desktop::model::marketplace::banner_palette_slot;
    let home = temp_home();
    let package = home.path().join("package");
    write_banner_package(
        &package,
        r#"{ "examples": [
      { "kind": "mention", "text": "@notes", "caption": "@notes summarise my notes" },
      { "kind": "mention", "text": "@notes", "caption": "@notes list the open questions" }
    ] }"#,
    );
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;
    install_folder_package(&handle, &package, &mut updates, &mut state).await;
    let snapshot = marketplace_of(&state);
    let item = snapshot
        .items
        .iter()
        .find(|item| item.name == "Notes")
        .expect("the package is listed");
    let package = item.extension.as_deref().expect("it carries its package");
    let banner = package
        .listing
        .banner
        .as_ref()
        .expect("its banner reaches the page");
    assert!(banner.colors.is_empty(), "its listing gives no colour");
    assert_eq!(
        item.examples.len(),
        2,
        "its two example prompts reach the page"
    );
    assert_eq!(
        banner_palette_slot(item),
        1,
        "acme.notes always takes palette slot 1"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_package_with_no_examples_has_no_banner_part() {
    use massorca_desktop::model::marketplace::{Part, absent_reason};
    let home = temp_home();
    let package = home.path().join("package");
    write_package(&package, "1.0.0");
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;
    install_folder_package(&handle, &package, &mut updates, &mut state).await;
    let snapshot = marketplace_of(&state);
    let item = snapshot
        .items
        .iter()
        .find(|item| item.name == "Notes")
        .expect("the package is listed");
    assert!(item.examples.is_empty(), "it lists no example prompts");
    assert_eq!(
        absent_reason(item, Part::Banner),
        None,
        "the banner is left out, with no sentence"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_real_services_discover_lists_the_built_in_agents_and_tools_as_their_categories() {
    use massorca_desktop::model::marketplace::{
        ItemKind, MarketplaceSource, ServiceMarketplace, listed,
    };
    use massorca_desktop::model::navigation::MarketplacePage;

    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the kinds and packages",
        |state| state.extensions.is_some() && !state.agent_kinds.is_empty(),
    )
    .await;
    let read = state.extensions.as_ref().expect("the lists are read");
    let mut source = ServiceMarketplace::default();
    source.follow_agent_kinds(&state.agent_kinds);
    source.follow_extensions(
        read.installed.as_ref().expect("the installed list"),
        read.catalogue.as_ref().expect("the catalogue"),
    );
    let snapshot = source.snapshot();
    let discover = listed(&snapshot, MarketplacePage::Discover, "");
    let agents = discover
        .iter()
        .filter(|item| item.kind == ItemKind::Agent)
        .count();
    let tools = discover
        .iter()
        .filter(|item| item.kind == ItemKind::Tool)
        .count();
    assert!(
        discover
            .iter()
            .all(|item| matches!(item.kind, ItemKind::Agent | ItemKind::Tool)),
        "a fresh profile's Discover has only the Agents and Tools sections"
    );
    assert!(agents >= 4, "the built-in agents are one Agents group");
    assert_eq!(tools, 3, "Git, GitHub CLI and Node.js are one Tools group");
    stop_service(service).await;
}

/// Set in the child process that runs the built-in agent check, so the parent knows to run the check itself.
const FAKE_CLI_CHILD: &str = "MASSORCA_BRIDGE_FAKE_CLI_CHILD";

/// A stand-in for the built-in agent's CLI, named as its package runs it: it prints nothing and exits 0.
fn write_fake_cli(bin: &Path) {
    #[cfg(windows)]
    {
        fs::write(bin.join("claude.cmd"), "@echo off\r\nexit /b 0\r\n")
            .expect("the fake CLI is written");
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let cli = bin.join("claude");
        fs::write(&cli, "#!/bin/sh\nexit 0\n").expect("the fake CLI is written");
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755))
            .expect("the fake CLI is executable");
    }
}

/// The built-in Claude agent installs from the catalogue and is Enabled, and its Installed row says so. The service
/// resolves vendor programs on its own PATH (MASSORCA_VENDOR_PATH=process) and a test cannot set the process
/// environment (unsafe is denied), so the check runs in a child process whose PATH holds one fake CLI.
#[test]
fn an_installed_built_in_agent_is_an_enabled_row_in_the_installed_list() {
    if std::env::var_os(FAKE_CLI_CHILD).is_none() {
        let bin = tempfile::TempDir::new().expect("a bin folder");
        write_fake_cli(bin.path());
        let status = massorca_process::command(std::env::current_exe().expect("this test binary"))
            .args([
                "--exact",
                "an_installed_built_in_agent_is_an_enabled_row_in_the_installed_list",
                "--nocapture",
            ])
            .env(FAKE_CLI_CHILD, "1")
            .env("PATH", bin.path())
            .env("MASSORCA_VENDOR_PATH", "process")
            .status()
            .expect("the child check runs");
        assert!(status.success(), "the child check passed");
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(installed_built_in_agent_is_an_enabled_row());
}

async fn installed_built_in_agent_is_an_enabled_row() {
    use massorca_desktop::model::marketplace::{
        MarketplaceSource, ServiceMarketplace, listed, status_word,
    };
    use massorca_desktop::model::navigation::MarketplacePage;

    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the catalogue", |state| {
        state.extensions.is_some()
    })
    .await;
    let source = "catalogue:massorca.claude".to_owned();
    let review = handle
        .inspect_extension(source.clone())
        .await
        .expect("the built-in package is reviewed");
    handle
        .install_extension(install_params(source, Some(review.consent.clone())))
        .await
        .expect("the built-in agent installs without its vendor installer");
    until(
        &mut updates,
        &mut state,
        "the installed built-in agent",
        |state| installed_state(state, "massorca.claude") == Some(ExtensionState::Enabled),
    )
    .await;

    let read = state.extensions.as_ref().expect("the lists are read");
    let mut marketplace = ServiceMarketplace::default();
    marketplace.follow_agent_kinds(&state.agent_kinds);
    marketplace.follow_extensions(
        read.installed.as_ref().expect("the installed list"),
        read.catalogue.as_ref().expect("the catalogue"),
    );
    let snapshot = marketplace.snapshot();
    let installed = listed(&snapshot, MarketplacePage::Installed, "");
    let row = installed
        .iter()
        .find(|item| item.agent_kind.as_deref() == Some("claude"))
        .expect("the built-in agent is in the Installed list");
    assert_eq!(
        status_word(&row.state).as_deref(),
        Some("Enabled"),
        "its row says Enabled"
    );
    stop_service(service).await;
}

/// Writes the manifest of `package` into a zip archive at `path`, as the CLI's archive tests write theirs: one entry
/// per file.
fn write_zip_package(path: &Path, package: &Path) {
    let manifest = fs::read(package.join("extension.json")).expect("the manifest is read");
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("extension.json", zip::write::SimpleFileOptions::default())
        .expect("an archive entry");
    writer.write_all(&manifest).expect("the manifest's bytes");
    let bytes = writer.finish().expect("the archive").into_inner();
    fs::write(path, bytes).expect("the archive file");
}

/// A zip package is inspected under its `file:` source and installs from that source: the path the Add extension
/// menu's zip entry takes. The service reads a zip archive, as its archive tests show.
#[tokio::test]
async fn a_zip_package_goes_through_inspect_and_installs_from_its_file_source() {
    let home = temp_home();
    let folder = TempDir::new().expect("a folder for the package");
    let package = folder.path().join("package");
    write_package(&package, "1.0.0");
    let archive = folder.path().join("notes.zip");
    write_zip_package(&archive, &package);

    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the lists", |state| {
        state.extensions.is_some()
    })
    .await;

    let source = format!("file:{}", archive.display());
    let review = handle
        .inspect_extension(source.clone())
        .await
        .expect("the archive is reviewed");
    assert_eq!(
        review.id, PACKAGE,
        "the review names the package in the archive"
    );
    let refused = handle
        .install_extension(install_params(source.clone(), Some(review.consent.clone())))
        .await
        .expect_err("an archive without the hash its review read is refused");
    assert_eq!(refused.code(), Some("extension.hash_required"));
    let mut params = install_params(source, Some(review.consent.clone()));
    params.sha256 = Some(review.sha256.clone());
    handle
        .install_extension(params)
        .await
        .expect("the archive installs against the hash its review read");
    until(&mut updates, &mut state, "the installed archive", |state| {
        installed_state(state, PACKAGE) == Some(ExtensionState::Enabled)
    })
    .await;
    stop_service(service).await;
}

/// The installed version of a package in the store, if the store lists it.
fn installed_version(state: &StoreState, id: &str) -> Option<String> {
    let read = state.extensions.as_ref()?;
    let installed = read.installed.as_ref().ok()?;
    installed
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.version.clone())
}

#[tokio::test]
async fn a_rollback_restores_the_version_an_update_replaced_and_the_log_lists_the_runs() {
    let home = temp_home();
    let package = home.path().join("package");
    write_package(&package, "1.0.0");
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the extension lists", |state| {
        state.extensions.is_some()
    })
    .await;

    let first = handle
        .inspect_extension(source_of(&package))
        .await
        .expect("the first review is read");
    handle
        .install_extension(install_params(
            source_of(&package),
            Some(first.consent.clone()),
        ))
        .await
        .expect("the first version installs");
    let refusal = handle
        .rollback_extension(ExtensionRollbackParams {
            id: PACKAGE.to_owned(),
            consent: None,
            agent_token: None,
        })
        .await
        .expect_err("an install keeps no version to roll back to");
    assert_eq!(refusal.code(), Some(codes::EXTENSION_NO_PREVIOUS_VERSION));

    write_package(&package, "2.0.0");
    let second = handle
        .inspect_extension(source_of(&package))
        .await
        .expect("the second review is read");
    handle
        .update_extension(ExtensionUpdateParams {
            id: PACKAGE.to_owned(),
            consent: Some(second.consent.clone()),
            sha256: None,
            agent_token: None,
        })
        .await
        .expect("the update installs");
    until(&mut updates, &mut state, "the updated package", |state| {
        installed_version(state, PACKAGE).as_deref() == Some("2.0.0")
            && state
                .extensions
                .as_ref()
                .is_some_and(|read| read.history.runs.get(PACKAGE).map(Vec::len) == Some(2))
    })
    .await;
    let offered = state.extensions.as_ref().map(|read| &read.history);
    assert!(offered.is_some_and(|history| history.rollback_offered && history.log_offered));

    let log = handle
        .extension_log(PACKAGE.to_owned())
        .await
        .expect("the run log is read");
    assert_eq!(log.runs.len(), 2, "the install and the update: {log:?}");
    assert_eq!(log.runs[1].action, ExtensionRunAction::Update);
    assert_eq!(log.runs[1].from_version.as_deref(), Some("1.0.0"));
    assert_eq!(log.runs[1].to_version.as_deref(), Some("2.0.0"));

    let asked = handle
        .rollback_extension(ExtensionRollbackParams {
            id: PACKAGE.to_owned(),
            consent: None,
            agent_token: None,
        })
        .await
        .expect("the service asks for the restored version's consent");
    let RollbackAnswer::ConsentNeeded { version, consent } = asked else {
        panic!("a rollback without consent asks first: {asked:?}");
    };
    assert_eq!(version, "1.0.0");
    assert_eq!(
        consent, first.consent,
        "the restored review is the first one"
    );

    let restored = handle
        .rollback_extension(ExtensionRollbackParams {
            id: PACKAGE.to_owned(),
            consent: Some(consent),
            agent_token: None,
        })
        .await
        .expect("the reviewed version is restored");
    let RollbackAnswer::Restored(restored) = restored else {
        panic!("a rollback with the consent restores: {restored:?}");
    };
    assert_eq!(restored.version, "1.0.0");
    until(&mut updates, &mut state, "the restored version", |state| {
        installed_version(state, PACKAGE).as_deref() == Some("1.0.0")
    })
    .await;

    let log = handle
        .extension_log(PACKAGE.to_owned())
        .await
        .expect("the run log is read again");
    assert_eq!(log.runs.len(), 3, "the rollback is logged: {log:?}");
    assert_eq!(log.runs[2].action, ExtensionRunAction::Rollback);
    assert_eq!(log.runs[2].from_version.as_deref(), Some("2.0.0"));
    stop_service(service).await;
}
