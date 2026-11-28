//! A kind's standing variables (decision 0174) may hold secrets, so they follow the vault's boundary for paired devices
//! (decision 0071): only a run started by a host-admin caller gets them, and such a run is marked as holding secrets. A
//! device run gets none and starts without the mark. Every value here is a placeholder.

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

/// The `server` profile listening on localhost, with the fake agent told to report whether `STANDING_TOKEN` is set.
fn start_server(server: &Sandbox) -> ChildGuard {
    server.track(SERVER);
    let listen = format!("localhost:{}", free_port());
    let child = massorca_in(server.home())
        .args(["--profile", SERVER, "service", "run", "--listen", &listen])
        .env("FAKE_AGENT_PROBE_ENV", "STANDING_TOKEN")
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

/// Kind `fake` with a standing variable `STANDING_TOKEN`, and a folder project with workspace `main`, all as the
/// owner. The project has no vault secret, so only the standing variable can mark a run.
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
    server.json_ok(
        SERVER,
        &[
            "project",
            "add",
            notes.to_str().expect("a UTF-8 path"),
            "--folder",
        ],
    );
    server.json_ok(SERVER, &["workspace", "create", "notes", "main"]);
    let set = server.json_ok(
        SERVER,
        &[
            "agent",
            "settings",
            "set",
            "fake",
            "--env",
            "STANDING_TOKEN=placeholder-standing-device-4b7e",
        ],
    );
    assert_eq!(set["env_names"], json!(["STANDING_TOKEN"]), "{set}");
}

/// Pairs the `client` profile with standard scopes, as host `srv`.
fn pair(server: &Sandbox, client: &Sandbox) {
    let offer = server.json_ok(SERVER, &["--no-autostart", "pair", "offer", "--standard"]);
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

const START: [&str; 6] = [
    "agent",
    "start",
    "fake",
    "--workspace",
    "main",
    "--read-only",
];

#[test]
fn standing_variables_reach_only_a_host_admin_run_which_then_holds_secrets() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    prepare(&server);
    pair(&server, &client);

    // A device run gets no standing variable and is not marked.
    let mut device_start = vec!["--host", "srv"];
    device_start.extend(START);
    let mine: Value = client.json_ok("client", &device_start);
    assert_eq!(mine["holds_secrets"], Value::Null, "{mine}");
    screen_has(
        &client,
        "client",
        &["--host", "srv"],
        mine["terminal_id"].as_str().expect("a terminal id"),
        "fake-agent: env STANDING_TOKEN unset",
    );

    // The owner's run gets it, and holds secrets because of it.
    let owned = server.json_ok(SERVER, &START);
    assert_eq!(owned["holds_secrets"], true, "{owned}");
    screen_has(
        &server,
        SERVER,
        &[],
        owned["terminal_id"].as_str().expect("a terminal id"),
        "fake-agent: env STANDING_TOKEN set",
    );
}
