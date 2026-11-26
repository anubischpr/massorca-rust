//! The secret vault through the bridge, against a real service whose vault key is a file in a temporary
//! profile, so no keyring is touched. The store holds names and metadata only: no value reaches it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::model::secret_requests::request_rows;
use massorca_desktop::model::transcript_secret_cards::{SecretCard, session_secret_rows};
use massorca_desktop::service::{
    Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::secrets::{
    SecretDecline, SecretLink, SecretName, SecretRemove, SecretRequest, SecretRequestParams,
    SecretSet,
};
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{RunningService, Service, ServiceConfig, VaultOptions};
use massorca_ui::components::secret_request_card::SecretState;
use massorca_ui::components::transcript::{SecretRow, TranscriptRow};
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
/// An obviously fake value: the tests check that it never comes back out of the bridge.
const VALUE: &str = "vault-bridge-test-value-0c1d";

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start_with(
        ServiceConfig {
            profile: profile(home),
            server_name: "massorca-test".to_owned(),
            server_version: "0.0.0-test".to_owned(),
        },
        VaultOptions { force_file: true },
    )
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

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
}

/// The names the store lists, once the list has arrived.
fn names(state: &StoreState) -> Vec<String> {
    match &state.secrets.list {
        Some(Ok(entries)) => entries.iter().map(|entry| entry.name.clone()).collect(),
        _ => Vec::new(),
    }
}

async fn client_for(home: &Path) -> Client {
    Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo { ..base_peer_info() }),
    )
    .await
    .expect("a client connects")
}

#[tokio::test]
async fn the_store_follows_the_vault_by_name_and_never_holds_a_value() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the vault status and list", |s| {
        s.connection.is_connected()
            && matches!(s.secrets.status, Some(Ok(_)))
            && matches!(s.secrets.list, Some(Ok(_)))
    })
    .await;
    let status = state
        .secrets
        .status
        .clone()
        .expect("a status is held")
        .expect("the status reads");
    assert_eq!(status.state, "ready");
    // The vault key is made by the first secret, so there is no protection to report before it.
    assert_eq!(status.protection, "none");
    assert_eq!(status.count, 0);
    assert!(names(&state).is_empty(), "an empty vault lists nothing");

    // Another client stores a secret; the bridge learns of it from the event, not from a request of its own.
    let client = client_for(home.path()).await;
    client
        .secret_set(&SecretSet {
            value: VALUE.to_owned(),
            ..base_secret_set()
        })
        .await
        .expect("the secret is stored");
    until(&mut updates, &mut state, "the new name and count", |s| {
        names(s) == ["BRIDGE_TOKEN"]
            && matches!(&s.secrets.status, Some(Ok(status)) if status.count == 1)
    })
    .await;
    let status = state
        .secrets
        .status
        .clone()
        .expect("a status is held")
        .expect("the status reads");
    assert_eq!(
        status.protection, "file",
        "the key is kept in the profile file"
    );
    let entries = match &state.secrets.list {
        Some(Ok(entries)) => entries.clone(),
        other => panic!("the list reads: {other:?}"),
    };
    assert_eq!(entries[0].scope, "global");
    assert_eq!(entries[0].owner_scope, "global");
    assert!(!entries[0].linked);

    let shown = format!("{state:?}");
    assert!(!shown.contains(VALUE), "the store holds a value: {shown}");

    drop(client);
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_secret_with_links_is_removed_with_them_only_when_asked() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the connection and the list",
        |s| s.connection.is_connected() && matches!(s.secrets.list, Some(Ok(_))),
    )
    .await;
    handle
        .secret_set(SecretSet {
            name: "SHARED_TOKEN".to_owned(),
            value: VALUE.to_owned(),
            ..base_secret_set()
        })
        .await
        .expect("the secret is added");
    let client = client_for(home.path()).await;
    client
        .secret_link(&SecretLink {
            ..base_secret_link()
        })
        .await
        .expect("the project links the secret");
    until(
        &mut updates,
        &mut state,
        "the owned name and its link",
        |s| names(s) == ["SHARED_TOKEN", "SHARED_TOKEN"],
    )
    .await;

    // The links would be lost, so the removal is refused with its code unless they go too.
    let remove = SecretRemove {
        ..base_secret_remove()
    };
    let refused = handle
        .secret_remove(remove.clone())
        .await
        .expect_err("the links would be lost");
    assert!(
        matches!(&refused, Refusal::Failed { code, .. } if code == codes::SECRET_HAS_LINKS),
        "{refused:?}"
    );
    assert_eq!(names(&state).len(), 2, "a refusal changes nothing");

    let removed = handle
        .secret_remove(SecretRemove {
            include_links: true,
            ..remove
        })
        .await
        .expect("the secret and its links are removed");
    assert_eq!(removed.links_removed, ["project:demo"]);
    until(
        &mut updates,
        &mut state,
        "the vault without the name",
        |s| names(s).is_empty(),
    )
    .await;

    drop(client);
    drop(handle);
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_secret_is_added_and_replaced_only_as_asked_and_each_refusal_keeps_its_code() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the connection and the list",
        |s| s.connection.is_connected() && matches!(s.secrets.list, Some(Ok(_))),
    )
    .await;

    let add = SecretSet {
        name: "ADDED_TOKEN".to_owned(),
        value: VALUE.to_owned(),
        ..base_secret_set()
    };
    let entry = handle
        .secret_set(add.clone())
        .await
        .expect("the secret is added");
    assert_eq!(
        (entry.scope.as_str(), entry.name.as_str()),
        ("global", "ADDED_TOKEN")
    );
    assert!(
        !format!("{entry:?}").contains(VALUE),
        "the answer carries no value"
    );
    until(&mut updates, &mut state, "the added name", |s| {
        names(s) == ["ADDED_TOKEN"]
    })
    .await;

    // A name already in use is refused with its code, and the store is left as it was.
    let taken = handle
        .secret_set(add.clone())
        .await
        .expect_err("the name is in use");
    assert!(
        matches!(&taken, Refusal::Failed { code, .. } if code == codes::SECRET_NAME_IN_USE),
        "{taken:?}"
    );

    // Replace rotates the owned secret; replacing a name that is not stored is refused.
    let rotated = handle
        .secret_set(SecretSet {
            replace: true,
            ..add.clone()
        })
        .await
        .expect("the secret is rotated");
    assert!(rotated.rotated_at_ms >= entry.rotated_at_ms);
    let missing = handle
        .secret_set(SecretSet {
            name: "NOT_STORED".to_owned(),
            replace: true,
            ..add.clone()
        })
        .await
        .expect_err("there is nothing to replace");
    assert!(
        matches!(&missing, Refusal::Failed { code, .. } if code == codes::SECRET_NOT_FOUND),
        "{missing:?}"
    );

    // A name that is not a variable name is refused before anything is stored.
    let invalid = handle
        .secret_set(SecretSet {
            name: "1ST".to_owned(),
            ..add
        })
        .await
        .expect_err("the name is not a variable name");
    assert!(
        matches!(&invalid, Refusal::Failed { code, .. } if code == codes::SECRET_INVALID_NAME),
        "{invalid:?}"
    );
    assert_eq!(names(&state), ["ADDED_TOKEN"], "refusals change nothing");

    drop(handle);
    drop(bridge);
    stop_service(service).await;
}

/// Every `PeerInfo` field at its usual value; tests override the fields they vary.
fn base_peer_info() -> massorca_protocol::PeerInfo {
    massorca_protocol::PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}

/// Every `SecretLink` field at its usual value; tests override the fields they vary.
fn base_secret_link() -> massorca_protocol::methods::secrets::SecretLink {
    massorca_protocol::methods::secrets::SecretLink {
        scope: "project:demo".to_owned(),
        name: "SHARED_TOKEN".to_owned(),
        from_scope: "global".to_owned(),
    }
}

/// Every `SecretRemove` field at its usual value; tests override the fields they vary.
fn base_secret_remove() -> massorca_protocol::methods::secrets::SecretRemove {
    massorca_protocol::methods::secrets::SecretRemove {
        scope: "global".to_owned(),
        name: "SHARED_TOKEN".to_owned(),
        include_links: false,
        force: false,
    }
}

/// Every `SecretSet` field at its usual value; tests override the fields they vary.
fn base_secret_set() -> massorca_protocol::methods::secrets::SecretSet {
    massorca_protocol::methods::secrets::SecretSet {
        scope: "global".to_owned(),
        name: "BRIDGE_TOKEN".to_owned(),
        value: String::new(),
        replace: false,
    }
}

#[tokio::test]
async fn a_linked_name_is_unlinked_and_the_owner_keeps_the_secret() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the connection and the list",
        |s| s.connection.is_connected() && matches!(s.secrets.list, Some(Ok(_))),
    )
    .await;
    handle
        .secret_set(SecretSet {
            name: "SHARED_TOKEN".to_owned(),
            value: VALUE.to_owned(),
            ..base_secret_set()
        })
        .await
        .expect("the secret is added");
    let client = client_for(home.path()).await;
    client
        .secret_link(&SecretLink {
            ..base_secret_link()
        })
        .await
        .expect("the project links the secret");
    until(
        &mut updates,
        &mut state,
        "the owned name and its link",
        |s| names(s) == ["SHARED_TOKEN", "SHARED_TOKEN"],
    )
    .await;

    let removed = handle
        .secret_unlink(SecretName {
            ..base_secret_name()
        })
        .await
        .expect("the link is removed");
    assert_eq!(removed.scope, "project:demo");
    until(&mut updates, &mut state, "the owned name alone", |s| {
        names(s) == ["SHARED_TOKEN"]
    })
    .await;

    drop(client);
    drop(handle);
    drop(bridge);
    stop_service(service).await;
}

/// Every `SecretName` field at its usual value; tests override the fields they vary.
fn base_secret_name() -> massorca_protocol::methods::secrets::SecretName {
    massorca_protocol::methods::secrets::SecretName {
        scope: "project:demo".to_owned(),
        name: "SHARED_TOKEN".to_owned(),
    }
}

/// The request `id` as the store holds it, once the list has arrived.
fn request_of<'a>(state: &'a StoreState, id: &str) -> Option<&'a SecretRequest> {
    match &state.secrets.requests {
        Some(Ok(requests)) => requests.iter().find(|request| request.id == id),
        _ => None,
    }
}

/// The card of request `id` among `cards`, if one is drawn.
fn secret_of<'a>(cards: &'a [SecretCard], id: &str) -> Option<&'a SecretRow> {
    cards.iter().find_map(|card| match &card.row {
        TranscriptRow::Secret { secret, .. } if secret.request_id == id => Some(secret),
        _ => None,
    })
}

fn base_secret_request_params() -> SecretRequestParams {
    SecretRequestParams {
        scope: "global".to_owned(),
        name: "OPENAI_API_KEY".to_owned(),
        reason: "Calls the OpenAI API for the project.".to_owned(),
        session: Some("s1".to_owned()),
        agent_token: None,
    }
}

fn base_secret_decline(request: &str) -> SecretDecline {
    SecretDecline {
        request: request.to_owned(),
    }
}

#[tokio::test]
async fn an_agent_request_is_drawn_in_its_session_until_it_is_answered() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the vault list", |s| {
        s.connection.is_connected() && matches!(s.secrets.list, Some(Ok(_)))
    })
    .await;

    // The agent asks through the vault's own call, with the session it runs in and no run token.
    let client = client_for(home.path()).await;
    let asked = client
        .secret_request(&base_secret_request_params())
        .await
        .expect("the agent's request is opened");
    until(&mut updates, &mut state, "the agent's request", |s| {
        request_of(s, &asked.id).is_some()
    })
    .await;

    let cards = session_secret_rows(&state.secrets, &[], "s1", "Claude Code", None, None);
    let card = secret_of(&cards, &asked.id).expect("the session's card is drawn");
    assert_eq!(card.name.as_ref(), "OPENAI_API_KEY");
    assert_eq!(card.title.as_ref(), "Claude Code needs a secret");
    assert_eq!(card.state, SecretState::Pending);
    assert_eq!(cards[0].at_ms, asked.created_at_ms);
    assert!(
        session_secret_rows(&state.secrets, &[], "s2", "Claude Code", None, None).is_empty(),
        "another session draws no card of this request"
    );

    // Declining goes through the same call the card's Decline sends; the card then settles as a row.
    handle
        .secret_decline(base_secret_decline(&asked.id))
        .await
        .expect("the request is declined");
    until(&mut updates, &mut state, "the declined request", |s| {
        request_of(s, &asked.id).is_some_and(|request| request.state == "declined")
    })
    .await;

    let cards = session_secret_rows(&state.secrets, &[], "s1", "Claude Code", None, None);
    assert_eq!(
        secret_of(&cards, &asked.id).map(|secret| secret.state.clone()),
        Some(SecretState::Declined)
    );
    assert!(
        request_rows(&state.secrets, &[]).is_empty(),
        "a declined request is no longer listed as pending"
    );

    drop(client);
    drop(handle);
    drop(bridge);
    stop_service(service).await;
}
