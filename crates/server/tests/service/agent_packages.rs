//! `agent.kinds` reports each built-in kind's package state as the profile's lockfile records it, and the service
//! adopts the built-ins a profile already used when it starts (decisions 0059 and 0063).

use massorca_protocol::methods::agents::{AgentKinds, PackageState};
use massorca_protocol::methods::extensions::ExtensionSetEnabledParams;

use super::common::{connect, profile, start, stop, temp_home};

fn state(kinds: &AgentKinds, id: &str) -> PackageState {
    kinds
        .kinds
        .iter()
        .find(|kind| kind.id == id && kind.owner == "core")
        .unwrap_or_else(|| panic!("{id} is listed: {kinds:?}"))
        .package
        .state
}

#[tokio::test]
async fn a_new_profile_lists_the_built_in_kinds_as_available() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let kinds = client.agent_kinds().await.expect("the kinds");
    for id in ["claude", "codex", "antigravity", "gentle-shell"] {
        assert_eq!(state(&kinds, id), PackageState::Available, "{id}");
    }

    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_kind_the_profile_used_is_adopted_at_start_and_follows_its_package() {
    let home = temp_home();
    let accounts = profile(home.path(), "default")
        .dir()
        .join("agents")
        .join("accounts")
        .join("claude");
    std::fs::create_dir_all(&accounts).expect("a claude account folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let kinds = client.agent_kinds().await.expect("the kinds");
    assert_eq!(state(&kinds, "claude"), PackageState::Enabled);
    assert_eq!(state(&kinds, "codex"), PackageState::Available);

    client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "massorca.claude".to_owned(),
            enabled: false,
        })
        .await
        .expect("disables");
    let kinds = client.agent_kinds().await.expect("the kinds");
    assert_eq!(state(&kinds, "claude"), PackageState::Disabled);

    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn adoption_runs_once_so_a_disabled_built_in_stays_disabled_after_a_restart() {
    let home = temp_home();
    let accounts = profile(home.path(), "default")
        .dir()
        .join("agents")
        .join("accounts")
        .join("claude");
    std::fs::create_dir_all(&accounts).expect("a claude account folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "massorca.claude".to_owned(),
            enabled: false,
        })
        .await
        .expect("disables");
    drop(client);
    stop(running).await;

    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let kinds = client.agent_kinds().await.expect("the kinds");
    assert_eq!(state(&kinds, "claude"), PackageState::Disabled);

    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_staging_folder_left_by_a_crash_is_gone_after_the_service_starts() {
    let home = temp_home();
    let staging = profile(home.path(), "default")
        .extensions_dir()
        .join("pkgs")
        .join("massorca.claude")
        .join(".staging-1-0");
    std::fs::create_dir_all(&staging).expect("a staging folder");
    std::fs::write(staging.join("extension.json"), "{}").expect("a staged file");

    let running = start(home.path(), "default").await;
    assert!(!staging.exists());

    stop(running).await;
}
