//! What a paired device below the `workspace` class sees of agent accounts: the answer, without the vendor identity
//! and without the sign-in terminal's working directory (the account's home). The owner and a `workspace` device see
//! both.

use std::fs;
use std::path::Path;
use std::time::Duration;

use massorca_client::{Client, ClientError};
use massorca_core_store::{AgentAccountRow, Store};
use massorca_protocol::methods::agents::{self, AccountsAdded};
use massorca_protocol::methods::terminals::{self, Terminal, TerminalKillParams, TerminalList};
use massorca_server::Scopes;
use serde_json::{Value, json};
use tokio::time::sleep;

use crate::common::{TempHome, connect, profile, stop, temp_home};
use crate::network::{client_side, offer, pair, peer, start_listening};

const KIND: &str = "fake";
const IDENTITY: &str = "user@example.com";
const ACCOUNT: &str = "0195c0de-0000-7000-8000-000000000002";

/// Pairs a device named `name` and, when `read_only`, holds it to `read` alone; the returned client is connected with
/// the scopes the store now holds. The client's own profile is returned with it and must outlive it.
async fn paired_device(home: &Path, name: &str, read_only: bool) -> (TempHome, Client) {
    let (client_home, hosts) = client_side();
    let standard_offer = offer(home, false, None).await;
    let (_, record) = pair(&hosts, &standard_offer, name).await.unwrap();
    assert_eq!(record.scopes, Scopes::STANDARD.names());
    if read_only {
        let store = Store::open(profile(home, "server").store_path()).unwrap();
        assert!(
            store
                .set_device_scopes(&record.device_id, &["read".to_owned()])
                .unwrap()
        );
    }
    let client = Client::connect_host(&hosts, name, &peer())
        .await
        .expect("the device reconnects");
    (client_home, client)
}

/// Records `account` as the kind's active account, signed in as [`IDENTITY`].
fn insert_active_account(home: &Path, account: &str) {
    let store = Store::open(profile(home, "server").store_path()).unwrap();
    store
        .insert_agent_account(&AgentAccountRow {
            id: account.to_owned(),
            kind: KIND.to_owned(),
            label: "work".to_owned(),
            created_at: 1,
            identity: Some(IDENTITY.to_owned()),
            state: "signed_in".to_owned(),
            origin: "import".to_owned(),
        })
        .unwrap();
    store.set_active_agent_account(KIND, account).unwrap();
}

/// `value` without the field `name`, so two answers can be compared on everything else.
fn without(mut value: Value, name: &str) -> Value {
    value.as_object_mut().expect("an object").remove(name);
    value
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_below_workspace_lists_accounts_without_their_identity() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_reader_home, reader) = paired_device(home.path(), "reader", true).await;
    let (_worker_home, worker) = paired_device(home.path(), "worker", false).await;
    let owner = connect(home.path(), "server").await;
    insert_active_account(home.path(), ACCOUNT);

    let params = json!({ "kind": KIND });
    let full: Value = owner.call(agents::ACCOUNTS_LIST, &params).await.unwrap();
    let full = full["accounts"][0].clone();
    assert_eq!(full["identity"], IDENTITY, "the owner sees the identity");
    let seen: Value = worker.call(agents::ACCOUNTS_LIST, &params).await.unwrap();
    assert_eq!(
        seen["accounts"][0], full,
        "a workspace device sees the identity"
    );

    let masked: Value = reader.call(agents::ACCOUNTS_LIST, &params).await.unwrap();
    let masked = masked["accounts"][0].clone();
    assert!(
        masked.get("identity").is_none(),
        "a read-only device sees no identity: {masked}"
    );
    assert_eq!(masked, without(full, "identity"), "nothing else changes");
    stop(running).await;
}

/// Declares kind `fake` whose sign-in is a shell that waits, so its terminal stays live while the test reads it.
fn declare_waiting_sign_in(home: &Path) {
    let dir = profile(home, "server").dir().join("agents").join("kinds");
    fs::create_dir_all(&dir).unwrap();
    let (program, login) = if cfg!(windows) {
        (
            std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned()),
            json!(["/d", "/c", "ping -n 60 127.0.0.1 >nul"]),
        )
    } else {
        ("/bin/sh".to_owned(), json!(["-c", "sleep 60"]))
    };
    let kind = json!({
        "id": KIND,
        "name": "Fake agent",
        "program": [program],
        "prompt": { "mode": "argv" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
        "accounts": { "login": login }
    });
    fs::write(dir.join("fake.json"), kind.to_string()).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_below_workspace_sees_the_sign_in_terminal_without_its_cwd() {
    let home = temp_home();
    declare_waiting_sign_in(home.path());
    let running = start_listening(home.path()).await;
    let (_reader_home, reader) = paired_device(home.path(), "reader", true).await;
    let (_worker_home, worker) = paired_device(home.path(), "worker", false).await;
    let owner = connect(home.path(), "server").await;

    let added: AccountsAdded = owner
        .call(agents::ACCOUNTS_ADD, &json!({ "kind": KIND }))
        .await
        .expect("the sign-in starts");
    let by_id = json!({ "terminal": added.terminal_id });
    let full: Value = owner.call(terminals::SHOW, &by_id).await.unwrap();
    let full_cwd = full["cwd"].as_str().expect("a cwd").to_owned();
    assert!(
        full_cwd.contains(&added.account.id),
        "the owner sees the account home: {full_cwd}"
    );
    let seen: Value = worker.call(terminals::SHOW, &by_id).await.unwrap();
    assert_eq!(seen, full, "a workspace device sees the cwd");

    let masked: Terminal = reader.call(terminals::SHOW, &by_id).await.unwrap();
    assert_eq!(masked.cwd, "", "a read-only device sees no cwd");
    let masked = serde_json::to_value(&masked).unwrap();
    assert_eq!(
        without(masked, "cwd"),
        without(full, "cwd"),
        "nothing else changes"
    );
    let listed: TerminalList = reader.call(terminals::LIST, &json!({})).await.unwrap();
    let in_list = listed
        .terminals
        .iter()
        .find(|terminal| terminal.id == added.terminal_id)
        .expect("the sign-in terminal is listed");
    assert_eq!(in_list.cwd, "", "the list masks it too");

    let _: Terminal = owner
        .call(
            terminals::KILL,
            &TerminalKillParams {
                terminal: added.terminal_id.clone(),
                force: true,
            },
        )
        .await
        .unwrap();
    // The sign-in is decided once its terminal has exited; give that a moment before the service stops.
    sleep(Duration::from_millis(500)).await;
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_below_workspace_sees_no_credential_home_in_an_answer_or_an_error() {
    let home = temp_home();
    declare_waiting_sign_in(home.path());
    let running = start_listening(home.path()).await;
    let (_reader_home, reader) = paired_device(home.path(), "reader", true).await;
    let (_worker_home, worker) = paired_device(home.path(), "worker", false).await;
    let owner = connect(home.path(), "server").await;
    insert_active_account(home.path(), ACCOUNT);

    let params = json!({ "kind": KIND });
    let full: Value = owner.call(agents::CREDENTIALS_SHOW, &params).await.unwrap();
    let full_home = full["home"].as_str().expect("a home").to_owned();
    assert!(
        full_home.contains(ACCOUNT),
        "the owner sees the account home: {full_home}"
    );
    let seen: Value = worker
        .call(agents::CREDENTIALS_SHOW, &params)
        .await
        .unwrap();
    assert_eq!(seen, full, "a workspace device sees the home");
    let masked: Value = reader
        .call(agents::CREDENTIALS_SHOW, &params)
        .await
        .unwrap();
    assert_eq!(masked["home"], "", "a read-only device sees no home");
    assert_eq!(
        without(masked, "home"),
        without(full, "home"),
        "nothing else changes"
    );

    // A file where the account home should be: the refusal names the path, but only to those who may see it.
    let accounts = profile(home.path(), "server")
        .dir()
        .join("agents")
        .join("accounts")
        .join(KIND);
    fs::create_dir_all(&accounts).unwrap();
    fs::write(accounts.join(ACCOUNT), b"not a directory").unwrap();
    let refused = owner
        .call::<_, Value>(agents::CREDENTIALS_SHOW, &params)
        .await
        .expect_err("refused");
    let ClientError::Remote(full_error) = refused else {
        panic!("expected a remote error, got {refused:?}");
    };
    assert!(
        full_error.message.contains(ACCOUNT),
        "the owner's refusal names the home: {}",
        full_error.message
    );
    let refused = reader
        .call::<_, Value>(agents::CREDENTIALS_SHOW, &params)
        .await
        .expect_err("refused");
    let ClientError::Remote(masked_error) = refused else {
        panic!("expected a remote error, got {refused:?}");
    };
    assert_eq!(masked_error.code, full_error.code, "the same code");
    assert!(
        !masked_error.message.contains(ACCOUNT)
            && !masked_error
                .message
                .contains(&*home.path().to_string_lossy())
            && masked_error.data.is_none(),
        "a read-only device's refusal names no path: {masked_error:?}"
    );
    stop(running).await;
}
