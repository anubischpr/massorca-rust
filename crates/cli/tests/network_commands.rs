//! Pairing, hosts and the `service run` rules through the real binary. A server profile listens on localhost, client
//! profiles pair with it, and decisions 0041, 0045 and 0046 are checked from the outside.

mod harness;

use std::net::TcpListener;
use std::process::Stdio;

use assert_cmd::assert::Assert;
use harness::{
    ChildGuard, Sandbox, eventually, execute_with_stdin, single_document, stdout_text, with_json,
};
use serde_json::{Value, json};

/// A local port that nothing listens on, found by binding port 0 and releasing it.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free local port")
        .local_addr()
        .expect("the local address")
        .port()
}

/// Starts the `server` profile listening on localhost, and returns once its listener is up.
/// The guard kills the service when the test ends, even when it fails.
fn start_server(server: &Sandbox) -> ChildGuard {
    let listen = format!("localhost:{}", free_port());
    let guard = server.spawn_service_with(
        "server",
        &["--listen", listen.as_str()],
        Stdio::null(),
        Stdio::null(),
    );
    eventually("the server listener", || {
        let outcome = server.run(
            "server",
            &with_json(&["--no-autostart", "server", "status"]),
        );
        if !outcome.get_output().status.success() {
            return None;
        }
        let document = single_document(&outcome);
        (document["result"]["network"]["listeners"][0]["state"] == "listening").then_some(())
    });
    guard
}

/// Makes a pairing offer on the `server` profile, as its owner, with the extra options given.
fn pair_offer(server: &Sandbox, extra: &[&str]) -> String {
    let mut args = vec!["--no-autostart", "pair", "offer"];
    args.extend_from_slice(extra);
    let result = server.json_ok("server", &args);
    result["offer"]
        .as_str()
        .expect("the offer is a string")
        .to_owned()
}

/// Pairs the `client` profile with the server of `offer`, storing it as host `name` for device `device`.
fn accept(client: &Sandbox, offer: &str, name: &str, device: &str) -> Assert {
    let mut command = client.command("client");
    command.args([
        "--json",
        "pair",
        "accept",
        "-",
        "--name",
        name,
        "--device-name",
        device,
    ]);
    execute_with_stdin(&mut command, offer)
}

/// `accept`, which must succeed. Returns the host it stored.
fn accept_ok(client: &Sandbox, offer: &str, name: &str, device: &str) -> Value {
    let outcome = accept(client, offer, name, device).success();
    single_document(&outcome)["result"].clone()
}

#[test]
fn a_paired_device_runs_commands_on_its_host_and_cannot_list_devices() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);

    let stored = accept_ok(&client, &pair_offer(&server, &[]), "srv", "laptop");
    assert_eq!(stored["name"], "srv");
    assert_eq!(stored["device_name"], "laptop");

    client.json_ok("client", &["--host", "srv", "workspace", "list"]);
    let me = client.json_ok("client", &["--host", "srv", "device", "whoami"]);
    assert_eq!(me["principal"], "device");

    let denied = client
        .run("client", &with_json(&["--host", "srv", "device", "list"]))
        .code(1);
    assert_eq!(
        single_document(&denied)["error"]["code"],
        "permission.denied"
    );

    let hosts = client.json_ok("client", &["host", "list"]);
    assert_eq!(hosts["hosts"][0]["name"], "srv");

    let remote = client.json_ok("client", &["--host", "srv", "server", "status"]);
    assert_eq!(remote["network"]["listeners"][0]["state"], "listening");
    assert!(
        remote.get("tailnet").is_none(),
        "a host reports no local tailnet"
    );
    let local = server.json_ok("server", &["--no-autostart", "server", "status"]);
    assert!(
        local["tailnet"].is_object(),
        "the local status reports the tailnet"
    );
}

#[test]
fn an_admin_device_revokes_another_device_which_is_then_refused() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    accept_ok(&client, &pair_offer(&server, &[]), "srv", "laptop");
    accept_ok(&client, &pair_offer(&server, &["--admin"]), "adm", "desk");

    let devices = client.json_ok("client", &["--host", "adm", "device", "list"]);
    assert_eq!(devices["devices"].as_array().map(Vec::len), Some(2));

    let revoked = client.json_ok("client", &["--host", "adm", "device", "revoke", "laptop"]);
    assert!(revoked["revoked_at_ms"].is_i64());

    let refused = client
        .run(
            "client",
            &with_json(&["--host", "srv", "workspace", "list"]),
        )
        .code(4);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "auth.device_revoked"
    );
}

#[test]
fn damaged_and_spent_offers_are_refused_and_the_input_is_never_echoed() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);

    let damaged = client
        .run(
            "client",
            &with_json(&["pair", "accept", "mop1.xxx", "--name", "bad"]),
        )
        .code(1);
    assert_eq!(
        single_document(&damaged)["error"]["code"],
        "pairing.damaged"
    );
    assert!(
        !stdout_text(&damaged).contains("mop1.xxx"),
        "the input is never echoed"
    );

    let made = pair_offer(&server, &[]);
    accept_ok(&client, &made, "first", "laptop");
    let spent = accept(&client, &made, "second", "laptop").code(4);
    assert_eq!(single_document(&spent)["error"]["code"], "pairing.invalid");
}

#[test]
fn an_unreachable_host_is_reported_and_can_be_forgotten_without_revoking() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    accept_ok(&client, &pair_offer(&server, &[]), "srv", "laptop");
    let connected = client.json_ok("client", &["host", "status", "srv"]);
    assert_eq!(connected["state"], "connected");

    server
        .run("server", &["--no-autostart", "service", "stop"])
        .success();
    let unreachable = client
        .run("client", &with_json(&["host", "status", "srv"]))
        .code(4);
    assert_eq!(
        single_document(&unreachable)["error"]["code"],
        "host.unreachable"
    );

    let kept = client.json_ok("client", &["host", "remove", "srv", "--keep-device"]);
    assert_eq!(kept["device"]["state"], "kept");
    assert_eq!(
        client.json_ok("client", &["host", "list"])["hosts"],
        json!([])
    );
}

#[test]
fn removing_a_reachable_host_revokes_this_device_on_the_server() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    accept_ok(&client, &pair_offer(&server, &[]), "srv", "laptop");

    let removed = client.json_ok("client", &["host", "remove", "srv"]);
    assert_eq!(removed["device"]["state"], "revoked");

    let devices = server.json_ok("server", &["--no-autostart", "device", "list"]);
    let laptop = devices["devices"]
        .as_array()
        .expect("a device list")
        .iter()
        .find(|device| device["name"] == "laptop")
        .expect("the device is still listed");
    assert!(laptop["revoked_at_ms"].is_i64());
}

#[test]
fn listen_addresses_the_unit_and_the_local_only_commands_follow_their_rules() {
    let home = Sandbox::new();
    let all_interfaces = home
        .run(
            "default",
            &with_json(&["service", "run", "--listen", "0.0.0.0:1234"]),
        )
        .code(2);
    assert_eq!(
        single_document(&all_interfaces)["error"]["code"],
        "usage.invalid_listen"
    );
    home.run(
        "default",
        &with_json(&["service", "run", "--listen", "tailscale:"]),
    )
    .code(2);

    let unit = home
        .run(
            "default",
            &["service", "unit", "--listen", "tailscale:7400"],
        )
        .success();
    let text = stdout_text(&unit);
    assert!(text.contains("--listen tailscale:7400"), "{text}");
    assert!(text.contains("MASSORCA_HOME=%h/.massorca/home"), "{text}");

    let refused = home
        .run("default", &with_json(&["--host", "srv", "service", "stop"]))
        .code(2);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "cli.host_not_supported"
    );
}

#[test]
fn host_commands_refuse_bad_input_before_they_contact_anything() {
    let home = Sandbox::new();
    assert_eq!(
        home.json_ok("default", &["host", "list"])["hosts"],
        json!([])
    );

    let unknown = home
        .run("default", &with_json(&["host", "remove", "nobody"]))
        .code(1);
    assert_eq!(single_document(&unknown)["error"]["code"], "host.not_found");

    let missing = home.home().join("no-such-massorca").display().to_string();
    let bad_listen = home
        .run(
            "default",
            &with_json(&[
                "host",
                "add-ssh",
                "example.invalid",
                "--binary",
                missing.as_str(),
                "--listen",
                "0.0.0.0:7347",
            ]),
        )
        .code(2);
    assert_eq!(
        single_document(&bad_listen)["error"]["code"],
        "usage.invalid_listen"
    );

    let discovery = home.json_ok("default", &["host", "discover"]);
    assert!(discovery["peers"].is_array());
}

#[test]
fn paths_for_a_host_are_sent_as_typed_not_resolved_on_this_computer() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    accept_ok(&client, &pair_offer(&server, &["--admin"]), "srv", "laptop");

    // A relative path means nothing on the host, so the service refuses it as typed.
    let refused = client
        .run(
            "client",
            &with_json(&["--host", "srv", "project", "add", "some/dir", "--folder"]),
        )
        .code(1);
    let error = &single_document(&refused)["error"];
    assert_eq!(error["code"], "request.invalid_params");
    let message = error["message"].as_str().expect("a message");
    assert!(message.contains("some"), "{message}");
    let here = std::env::current_dir().expect("the working directory");
    assert!(
        !message.contains(here.to_str().expect("utf-8")),
        "the host saw a path of this computer: {message}"
    );
}
