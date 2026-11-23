//! `task.start` and the vault (decisions 0071 and 0072): the bot's run gets vault variables only when the caller who
//! starts the task holds host-admin scope. A paired device with standard scopes starts a run without them, and the task
//! never starts as the service. Every value here is a placeholder.

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

/// The `server` profile listening on localhost, with the fake agent told to report whether `TEAM_TOKEN` is set.
fn start_server(server: &Sandbox) -> ChildGuard {
    server.track(SERVER);
    let listen = format!("localhost:{}", free_port());
    let child = massorca_in(server.home())
        .args(["--profile", SERVER, "service", "run", "--listen", &listen])
        .env("FAKE_AGENT_PROBE_ENV", "TEAM_TOKEN")
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

/// Kind `fake`, team `alpha` with bots `mara` and `tomas`, and a global secret `TEAM_TOKEN`, all as the owner.
fn prepare(server: &Sandbox) {
    server.prepare_profile(SERVER);
    let dir = server.profile_dir(SERVER).join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" }
    });
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
    let root = server.home().join("ws");
    server.json_ok(
        SERVER,
        &[
            "config",
            "set",
            "workspaces.default_root",
            root.to_str().expect("a UTF-8 path"),
        ],
    );
    server.json_ok(SERVER, &["team", "create", "alpha"]);
    for bot in ["mara", "tomas"] {
        server.json_ok(
            SERVER,
            &[
                "team",
                "bot",
                "add",
                "alpha",
                bot,
                "--agent",
                "fake",
                "--job",
                "Writes the parser",
            ],
        );
    }
    server
        .run_with_input(
            SERVER,
            &["secret", "set", "global", "TEAM_TOKEN"],
            b"placeholder-task-start-boundary-41e7\n",
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

fn device_ok(client: &Sandbox, args: &[&str]) -> Value {
    let mut full = vec!["--host", "srv"];
    full.extend_from_slice(args);
    client.json_ok("client", &full)
}

/// Creates a task for `bot` as the owner and returns its id.
fn create_task(server: &Sandbox, title: &str, bot: &str) -> String {
    let task = server.json_ok(SERVER, &["task", "create", "alpha", title, "--bot", bot]);
    task["id"].as_str().expect("a task id").to_owned()
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
fn a_task_started_by_a_workspace_device_runs_without_vault_variables() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    prepare(&server);
    pair(&server, &client);

    // The device's start runs with the device's scope, not the service's: no vault variable reaches the run.
    let device_task = create_task(&server, "Parse the headers", "mara");
    let started = device_ok(&client, &["task", "start", device_task.as_str()]);
    let session = started["session_id"].as_str().expect("a linked session");
    let run = device_ok(&client, &["agent", "status", session]);
    assert_eq!(run["holds_secrets"], Value::Null, "{run}");
    let terminal = run["terminal_id"].as_str().expect("a terminal id");
    screen_has(
        &client,
        "client",
        &["--host", "srv"],
        terminal,
        "fake-agent: env TEAM_TOKEN unset",
    );

    // The owner's start of the same kind of task does pass them, so the device's run lacked them by scope alone.
    let owner_task = create_task(&server, "Parse the body", "tomas");
    let started = server.json_ok(SERVER, &["task", "start", owner_task.as_str()]);
    let session = started["session_id"].as_str().expect("a linked session");
    let run = server.json_ok(SERVER, &["agent", "status", session]);
    assert_eq!(run["holds_secrets"], true, "{run}");
    let terminal = run["terminal_id"].as_str().expect("a terminal id");
    screen_has(
        &server,
        SERVER,
        &[],
        terminal,
        "fake-agent: env TEAM_TOKEN set",
    );
}
