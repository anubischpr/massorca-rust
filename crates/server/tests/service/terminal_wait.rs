//! `terminal.wait` over the wire: the exit code of a script, a terminal that already exited, a wait that runs out, a
//! terminal closed during the wait, an unknown terminal, a condition this build does not know, and the capability.
//! Terminals come from a host-admin test method, as in `terminal_secrets`, because no workspace is needed to wait.

use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::Client;
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{CreateSpec, ShellChoice};
use massorca_protocol::methods::terminals::{
    self, TerminalState, TerminalWaitCondition, TerminalWaitParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use massorca_server::{Owner, Permission, RegistrationHandle, RequestCtx, RunningService};
use serde_json::{Value, json};

use super::common::{connect, remote_code, start, stop, temp_home};

const CREATE: &str = "ext.waittest.create";
const CAPABILITY: &str = "waittest/1";

/// A shell that runs `script` and exits.
fn script_shell(script: &str) -> ShellChoice {
    if cfg!(windows) {
        ShellChoice {
            program: "cmd.exe".to_owned(),
            args: vec!["/d".to_owned(), "/c".to_owned(), script.to_owned()],
        }
    } else {
        ShellChoice {
            program: "sh".to_owned(),
            args: vec!["-c".to_owned(), script.to_owned()],
        }
    }
}

/// Registers the test method that starts a terminal in `home`: `{ "script": ... }` runs the script and exits, and no
/// script starts the default shell. The returned handles keep the registrations alive.
fn register_create(running: &RunningService, home: &Path) -> Vec<RegistrationHandle> {
    let owner = Owner::Extension("waittest".to_owned());
    let cwd = home.to_path_buf();
    let capability = running
        .registries()
        .capabilities
        .register(CAPABILITY, owner.clone())
        .expect("the capability registers");
    let method = running
        .registries()
        .methods
        .register(
            owner,
            CREATE,
            CAPABILITY,
            Permission::HostAdmin,
            move |ctx: RequestCtx, params: Value| {
                let cwd = cwd.clone();
                async move {
                    let shell = params["script"].as_str().map(script_shell);
                    let info = ctx
                        .state
                        .terminals
                        .create(CreateSpec {
                            workspace_id: "w-wait".to_owned(),
                            cwd,
                            shell,
                            env: Default::default(),
                            size: PtySize { cols: 80, rows: 24 },
                            title: None,
                            request_id: None,
                            holds_secrets: false,
                        })
                        .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?;
                    Ok::<Value, ErrorBody>(json!({ "id": info.id }))
                }
            },
        )
        .expect("the method registers");
    vec![capability, method]
}

async fn create(client: &Client, params: Value) -> String {
    let created: Value = client
        .call(CREATE, &params)
        .await
        .expect("the terminal starts");
    created["id"].as_str().expect("the terminal id").to_owned()
}

fn wait_params(id: &str, timeout_ms: Option<u64>) -> TerminalWaitParams {
    TerminalWaitParams {
        terminal: id.to_owned(),
        condition: TerminalWaitCondition::Exit,
        timeout_ms,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_reports_the_exit_code_of_a_script_that_ends() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let _handles = register_create(&running, home.path());
    let client = connect(home.path(), "default").await;
    let id = create(&client, json!({ "script": "exit 3" })).await;

    let waited = client
        .terminal_wait(&wait_params(&id, Some(10_000)))
        .await
        .expect("the wait answers");
    assert!(waited.satisfied, "{waited:?}");
    assert_eq!(waited.status, TerminalState::Exited);
    assert_eq!(waited.exit.expect("an exit record").code, Some(3));

    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_on_a_terminal_that_already_exited_answers_at_once() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let _handles = register_create(&running, home.path());
    let client = connect(home.path(), "default").await;
    let id = create(&client, json!({ "script": "exit 3" })).await;
    client
        .terminal_wait(&wait_params(&id, Some(10_000)))
        .await
        .expect("the first wait answers");

    let started = Instant::now();
    let waited = client
        .terminal_wait(&wait_params(&id, Some(10_000)))
        .await
        .expect("the second wait answers");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "an exited terminal does not wait: {:?}",
        started.elapsed()
    );
    assert!(waited.satisfied, "{waited:?}");
    assert_eq!(waited.exit.expect("an exit record").code, Some(3));

    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_that_runs_out_answers_unsatisfied_with_the_live_state() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let _handles = register_create(&running, home.path());
    let client = connect(home.path(), "default").await;
    let id = create(&client, json!({})).await;

    let waited = client
        .terminal_wait(&wait_params(&id, Some(100)))
        .await
        .expect("the wait answers");
    assert!(!waited.satisfied, "{waited:?}");
    assert_eq!(waited.status, TerminalState::Live);
    assert!(waited.exit.is_none());

    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_terminal_closed_during_a_wait_answers_gone() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let _handles = register_create(&running, home.path());
    let client = connect(home.path(), "default").await;
    let waiter = connect(home.path(), "default").await;
    let id = create(&client, json!({})).await;

    let params = wait_params(&id, Some(30_000));
    let waiting = tokio::spawn(async move { waiter.terminal_wait(&params).await });
    // A wait that starts after the close finds no terminal, so the waiter gets a head start.
    tokio::time::sleep(Duration::from_millis(300)).await;
    client.terminal_close(&id).await.expect("the close answers");

    let answer = waiting.await.expect("the waiter ends");
    assert_eq!(
        remote_code(&answer.expect_err("a closed terminal is gone")),
        codes::TERMINAL_GONE
    );

    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_on_an_unknown_terminal_is_terminal_not_found() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let answer = client
        .terminal_wait(&wait_params("no-such-terminal", Some(100)))
        .await;
    assert_eq!(
        remote_code(&answer.expect_err("the terminal is unknown")),
        codes::TERMINAL_NOT_FOUND
    );

    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wait_with_a_condition_this_service_does_not_know_is_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let answer = client
        .call::<Value, Value>(
            terminals::WAIT,
            &json!({ "terminal": "t-1", "condition": "tui-idle" }),
        )
        .await;
    assert_eq!(
        remote_code(&answer.expect_err("the condition is unknown")),
        codes::REQUEST_INVALID_PARAMS
    );

    drop(client);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_service_offers_the_wait_capability() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    assert!(
        client.has_capability(capabilities::TERMINAL_WAIT),
        "the welcome names {}",
        capabilities::TERMINAL_WAIT
    );

    drop(client);
    stop(running).await;
}
