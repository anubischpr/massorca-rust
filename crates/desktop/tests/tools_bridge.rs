//! Tool packages (decision 0151) through the bridge against a real service in a temporary profile, driven headlessly.
//! Nothing here accepts a vendor command: the install never sets `run_installer`, and the update review is only read,
//! never accepted, so no installer, update or uninstall runs on the real PATH whether or not this host has Git.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::extensions::{
    CliRemovalAction, ExtensionInstallParams, ExtensionRemoveParams,
};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// A private home below a temporary folder (ADR 0073).
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

const WAIT: Duration = Duration::from_secs(30);
const GIT: &str = "massorca.git";

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

async fn lists(updates: &mut UnboundedReceiver<StoreUpdate>) -> StoreState {
    let mut state = StoreState::default();
    let folded = timeout(WAIT, async {
        while state.extensions.is_none() || !state.program_update {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for the lists: {state:?}");
    state
}

#[tokio::test]
async fn git_is_listed_as_a_tool_and_only_a_git_massorca_installed_can_be_updated() {
    let home = temp_home();
    let service = start_service(&home.path).await;
    let (_bridge, handle, mut updates) = bridge_for(&home.path);
    let state = lists(&mut updates).await;

    let catalogue = state
        .extensions
        .as_ref()
        .and_then(|read| read.catalogue.as_ref().ok())
        .expect("the catalogue is listed");
    let git = catalogue
        .iter()
        .find(|item| item.id == GIT)
        .expect("Git is in the catalogue");
    assert_eq!(git.listing.category, "tools");
    assert_eq!(git.programs.len(), 1);
    assert_eq!(git.programs[0].id, "git");
    assert!(
        state.program_update,
        "the service offers extensions-programs/1"
    );
    let update_steps = &git.programs[0].cli.as_ref().expect("a cli block").update;
    assert!(
        update_steps
            .iter()
            .any(|step| step.os == "windows" && step.command[1] == "upgrade"),
        "the update steps reach the client: {update_steps:?}"
    );

    let refusal = handle
        .review_program_update(GIT.to_owned(), "git".to_owned())
        .await
        .expect_err("nothing is installed yet");
    assert_eq!(refusal.code(), Some("extension.not_installed"));

    let review = handle
        .inspect_extension(format!("catalogue:{GIT}"))
        .await
        .expect("the review is read");
    let installed = handle
        .install_extension(ExtensionInstallParams {
            source: format!("catalogue:{GIT}"),
            consent: Some(review.consent),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await;
    if review.runs_installer {
        // Git is not on this host: without the installer's own consent nothing runs.
        let refusal = installed.expect_err("the installer needs its own consent");
        assert_eq!(refusal.code(), Some("extension.installer_consent_required"));
    } else {
        // Git is already here: it is registered, never updated or uninstalled by MassOrca.
        installed.expect("the Git on this host is registered");
        let refusal = handle
            .review_program_update(GIT.to_owned(), "git".to_owned())
            .await
            .expect_err("MassOrca did not install it");
        assert_eq!(refusal.code(), Some("extension.program_update_refused"));
        let removed = handle
            .remove_extension(ExtensionRemoveParams {
                id: GIT.to_owned(),
                delete_agent_homes: false,
                uninstall_cli: true,
                agent_token: None,
                uninstall_consent: None,
            })
            .await
            .expect("the package is removed");
        assert_eq!(removed.cli[0].action, CliRemovalAction::LeftAlone);
    }
    stop_service(service).await;
}
