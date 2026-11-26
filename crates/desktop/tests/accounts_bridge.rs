//! Agent accounts through the bridge (decision 0059), against a real service in a private temporary home. The kind
//! `fake` is declared in the profile: its login is this test binary waiting in `sign_in_waits`, and it declares no
//! check, so no vendor program ever runs. Every login file here is a fake file in a temporary folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::agents::{
    AccountState, AccountSummary, AccountsActivateParams, AccountsAddParams, AccountsImportParams,
    ImportResult,
};
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
const KIND: &str = "fake";
/// An obviously fake login file: the tests check it never comes back out of the bridge.
const FAKE_TOKEN: &str = r#"{"fake":true,"refresh":"accounts-bridge-fake-value-7e2a"}"#;

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    temp: TempDir,
    path: PathBuf,
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { temp, path }
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// Declares kind `fake`: its program is this test binary, and its login runs the ignored test that waits.
fn declare_fake(home: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("the kinds folder");
    let program = std::env::current_exe().expect("the test binary");
    let program = json_string(&program.to_string_lossy());
    let fake = format!(
        r#"{{"id":"{KIND}","name":"Fake agent","program":[{program}],"prompt":{{"mode":"argv"}},
"credentials":{{"home_env":"FAKE_AGENT_HOME","files":["token.json"]}},
"accounts":{{"login":["--exact","sign_in_waits","--ignored","--nocapture"]}}}}"#
    );
    fs::write(dir.join("fake.json"), fake).expect("the fake kind");
}

/// `text` as a JSON string literal.
fn json_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The fake kind's sign-in: it stays open until the service closes its terminal.
#[test]
#[ignore = "the fake kind's login; the service runs it in a sign-in terminal"]
fn sign_in_waits() {
    std::thread::sleep(Duration::from_secs(300));
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

async fn client_for(home: &Path) -> Client {
    Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo { ..base_peer_info() }),
    )
    .await
    .expect("a client connects")
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

/// The fake kind's accounts the store holds, once a list of them arrived.
fn fake_accounts(state: &StoreState) -> Option<Vec<AccountSummary>> {
    match state.accounts.get(KIND) {
        Some(Ok(accounts)) => Some(accounts.clone()),
        _ => None,
    }
}

/// A folder beside the home holding `files`, each with its content.
fn source_with(home: &TempHome, name: &str, files: &[(&str, &str)]) -> String {
    let source = home.temp.path().join(name);
    fs::create_dir_all(&source).expect("the source folder");
    for (file, content) in files {
        fs::write(source.join(file), content).expect("a fake file");
    }
    source.to_string_lossy().into_owned()
}

#[tokio::test]
async fn each_kind_is_listed_on_connect_and_relisted_on_its_event() {
    let home = temp_home();
    declare_fake(&home.path);
    let service = start_service(&home.path).await;
    let (bridge, _handle, mut updates) = bridge_for(&home.path);
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the fake kind's list", |s| {
        s.connection.is_connected() && fake_accounts(s).is_some()
    })
    .await;
    assert_eq!(fake_accounts(&state), Some(Vec::new()), "no account yet");

    // Another client starts a sign-in; the bridge learns of it from the event, not from a call of its own.
    let client = client_for(&home.path).await;
    let added = client
        .agent_accounts_add(&AccountsAddParams {
            kind: KIND.to_owned(),
            label: Some("elsewhere".to_owned()),
        })
        .await
        .expect("the sign-in starts");
    until(
        &mut updates,
        &mut state,
        "the account another client added",
        |s| {
            fake_accounts(s)
                .is_some_and(|accounts| accounts.iter().any(|a| a.id == added.account.id))
        },
    )
    .await;
    let accounts = fake_accounts(&state).expect("listed");
    assert_eq!(accounts[0].label, "elsewhere");
    assert_eq!(accounts[0].state, AccountState::NeedsSignIn);

    drop(client);
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn an_import_answers_not_found_without_login_files_and_the_list_follows_a_kept_one() {
    let home = temp_home();
    declare_fake(&home.path);
    let service = start_service(&home.path).await;
    let (bridge, handle, mut updates) = bridge_for(&home.path);
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the connection", |s| {
        s.connection.is_connected() && fake_accounts(s).is_some()
    })
    .await;

    let imported = handle
        .accounts_import(AccountsImportParams {
            kind: KIND.to_owned(),
            from: Some(source_with(&home, "empty", &[])),
            label: None,
        })
        .await
        .expect("the import answers");
    assert_eq!(imported.result, ImportResult::NotFound, "{imported:?}");
    assert!(imported.account.is_none());
    // The write is followed by a read of the kind, which still lists nothing.
    until(&mut updates, &mut state, "the read after the import", |s| {
        fake_accounts(s).is_some_and(|accounts| accounts.is_empty())
    })
    .await;

    let imported = handle
        .accounts_import(AccountsImportParams {
            kind: KIND.to_owned(),
            from: Some(source_with(&home, "login", &[("token.json", FAKE_TOKEN)])),
            label: Some("work".to_owned()),
        })
        .await
        .expect("the import answers");
    assert_eq!(imported.result, ImportResult::Imported, "{imported:?}");
    until(&mut updates, &mut state, "the imported account", |s| {
        fake_accounts(s).is_some_and(|accounts| accounts.len() == 1)
    })
    .await;
    let accounts = fake_accounts(&state).expect("listed");
    assert_eq!(accounts[0].label, "work");
    // A kind without a check cannot tell, so the account reads as unknown.
    assert_eq!(accounts[0].state, AccountState::Unknown);
    let shown = format!("{state:?} {imported:?}");
    assert!(
        !shown.contains("accounts-bridge-fake-value"),
        "a login file's content came back: {shown}"
    );

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn an_account_signing_in_refuses_activation_as_busy() {
    let home = temp_home();
    declare_fake(&home.path);
    let service = start_service(&home.path).await;
    let (bridge, handle, mut updates) = bridge_for(&home.path);
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the connection", |s| {
        s.connection.is_connected() && fake_accounts(s).is_some()
    })
    .await;

    let added = handle
        .accounts_add(AccountsAddParams {
            kind: KIND.to_owned(),
            label: Some("new".to_owned()),
        })
        .await
        .expect("the sign-in starts");
    assert!(!added.terminal_id.is_empty());
    assert_eq!(added.account.state, AccountState::NeedsSignIn);
    until(&mut updates, &mut state, "the pending account", |s| {
        fake_accounts(s).is_some_and(|accounts| accounts.iter().any(|a| a.id == added.account.id))
    })
    .await;

    let refused = handle
        .accounts_activate(AccountsActivateParams {
            kind: KIND.to_owned(),
            account: added.account.id.clone(),
            project: None,
        })
        .await
        .expect_err("a pending sign-in is not activated");
    assert_eq!(refused.code(), Some(codes::AGENT_BUSY), "{refused:?}");

    drop(bridge);
    // The service closes the sign-in terminal as it stops.
    stop_service(service).await;
}

/// Every `PeerInfo` field at its usual value; tests override the fields they vary.
fn base_peer_info() -> massorca_protocol::PeerInfo {
    massorca_protocol::PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}
