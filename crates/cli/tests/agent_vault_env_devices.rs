//! The vault's boundary for paired devices (decision 0071): a run started by a caller below host-admin scope gets no
//! vault variable and says so, and such a caller cannot read, write or resume a run that holds secrets. Every value
//! here is a placeholder.

mod harness;

use std::fs;
use std::net::{Ipv4Addr, TcpListener};
use std::process::Stdio;

use harness::{
    ChildGuard, Sandbox, eventually, execute_with_stdin, massorca_in, single_document, with_json,
};
use serde_json::{Value, json};

const SERVER: &str = "server";

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("bind a free port")
        .local_addr()
        .expect("the local address")
        .port()
}

/// The `server` profile listening on localhost, with the fake agent told to report whether `PROJECT_TOKEN` is set.
fn start_server(server: &Sandbox) -> ChildGuard {
    server.track(SERVER);
    let listen = format!("localhost:{}", free_port());
    let child = massorca_in(server.home())
        .args(["--profile", SERVER, "service", "run", "--listen", &listen])
        .env("FAKE_AGENT_PROBE_ENV", "PROJECT_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service starts");
    let guard = ChildGuard::new(child);
    eventually("the server listener", || {
        let outcome = server.run(SERVER, &with_json(&["--no-autostart", "server", "status"]));
        if !outcome.get_output().status.success() {
            return None;
        }
        let document = single_document(&outcome);
        (document["result"]["network"]["listeners"][0]["state"] == "listening").then_some(())
    });
    guard
}

/// Kind `fake`, a folder project with workspace `main`, and a project secret `PROJECT_TOKEN`, all as the owner.
fn prepare(server: &Sandbox) {
    let dir = server.profile_dir(SERVER).join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" }
    });
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
    let notes = server.home().join("notes");
    fs::create_dir_all(&notes).expect("the project folder");
    let added = server.json_ok(
        SERVER,
        &[
            "project",
            "add",
            notes.to_str().expect("a UTF-8 path"),
            "--folder",
        ],
    );
    server.json_ok(SERVER, &["workspace", "create", "notes", "main"]);
    let scope = format!("project:{}", added["id"].as_str().expect("a project id"));
    server
        .run_with_input(
            SERVER,
            &["secret", "set", &scope, "PROJECT_TOKEN"],
            b"placeholder-device-boundary-0d5c\n",
        )
        .success();
}

/// Pairs the `client` profile with standard scopes, as host `srv`.
fn pair(server: &Sandbox, client: &Sandbox) {
    let offer = server.json_ok(SERVER, &["--no-autostart", "pair", "offer"]);
    let mut command = client.command("client");
    command.args([
        "--json",
        "pair",
        "accept",
        "-",
        "--name",
        "srv",
        "--device-name",
        "laptop",
    ]);
    execute_with_stdin(&mut command, offer["offer"].as_str().expect("an offer")).success();
}

fn on_host(args: &[&str]) -> Vec<String> {
    let mut full = vec!["--host".to_owned(), "srv".to_owned()];
    full.extend(args.iter().map(|arg| (*arg).to_owned()));
    full
}

fn device_ok(client: &Sandbox, args: &[&str]) -> Value {
    let full = on_host(args);
    let refs: Vec<&str> = full.iter().map(String::as_str).collect();
    client.json_ok("client", &refs)
}

fn device_error(client: &Sandbox, args: &[&str]) -> Value {
    let full = on_host(args);
    let refs: Vec<&str> = full.iter().map(String::as_str).collect();
    let outcome = client.run("client", &with_json(&refs)).failure();
    single_document(&outcome)["error"].clone()
}

fn screen_has(sandbox: &Sandbox, profile: &str, prefix: &[&str], terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let mut args = prefix.to_vec();
        args.extend(["terminal", "screen", terminal]);
        let screen = sandbox.json_ok(profile, &args);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().map(str::trim) == Some(line))
            })
            .then_some(())
    });
}

#[test]
fn a_workspace_device_gets_no_secret_and_cannot_reach_a_run_that_holds_them() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    prepare(&server);
    pair(&server, &client);

    // A device run starts without vault variables, and says so.
    let mine = device_ok(
        &client,
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    assert_eq!(mine["holds_secrets"], Value::Null, "{mine}");
    assert!(mine["secrets_withheld"].is_string(), "{mine}");
    let my_terminal = mine["terminal_id"].as_str().expect("a terminal id");
    screen_has(
        &client,
        "client",
        &["--host", "srv"],
        my_terminal,
        "fake-agent: env PROJECT_TOKEN unset",
    );

    // The owner's run holds the secret; the device can neither read nor write it.
    let owned = server.json_ok(
        SERVER,
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    assert_eq!(owned["holds_secrets"], true, "{owned}");
    let terminal = owned["terminal_id"].as_str().expect("a terminal id");
    let session = owned["id"].as_str().expect("a session id");
    screen_has(
        &server,
        SERVER,
        &[],
        terminal,
        "fake-agent: env PROJECT_TOKEN set",
    );
    for args in [
        vec!["terminal", "screen", terminal],
        vec!["agent", "prompt", session, "hello"],
        vec!["agent", "resume", session],
    ] {
        let error = device_error(&client, &args);
        assert_eq!(error["code"], "permission.denied", "{args:?}: {error}");
    }
    let listed = device_ok(&client, &["agent", "status", session]);
    assert_eq!(listed["holds_secrets"], true, "the device may see the mark");
}
