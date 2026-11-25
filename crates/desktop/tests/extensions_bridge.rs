//! The Marketplace extension reads and writes of the bridge against a real service in a temporary profile, driven
//! headlessly: no window opens. The package is a folder fixture with no vendor command, so no installer runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{
    ExtensionsRead, Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::extensions::{
    ExtensionInstallParams, ExtensionRemoveParams, ExtensionSetEnabledParams, ExtensionState,
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
        }),
        Instant::now(),
    );
    assert_eq!(state.extensions.as_ref(), Some(&held));
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

#[tokio::test]
async fn a_real_services_discover_lists_the_built_in_agents_as_one_category() {
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
    assert!(
        discover.iter().all(|item| item.kind == ItemKind::Agent),
        "a fresh profile's Discover has no other category, so no other section"
    );
    assert!(agents >= 4, "the built-in agents are one Agents group");
    stop_service(service).await;
}
