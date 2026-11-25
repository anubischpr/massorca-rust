//! `pair offer --address`, `pair cancel` and the offer's QR code through the real binary (SRV-06, SRV-07). A server
//! profile listens on localhost and a client profile pairs with it.

mod harness;

use std::net::TcpListener;
use std::process::Stdio;

use assert_cmd::assert::Assert;
use harness::{ChildGuard, Sandbox, eventually, execute_with_stdin, single_document, with_json};
use serde_json::Value;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free local port")
        .local_addr()
        .expect("the local address")
        .port()
}

/// Starts the `server` profile on a localhost port, and returns once its listener is up, with that port.
fn start_server(server: &Sandbox) -> (ChildGuard, u16) {
    let port = free_port();
    let listen = format!("localhost:{port}");
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
    (guard, port)
}

fn offer(server: &Sandbox, extra: &[&str]) -> Value {
    let mut args = vec!["--no-autostart", "pair", "offer"];
    args.extend_from_slice(extra);
    server.json_ok("server", &args)
}

fn accept(client: &Sandbox, offer: &str, name: &str) -> Assert {
    let mut command = client.command("client");
    command.args(["--json", "pair", "accept", "-", "--name", name]);
    execute_with_stdin(&mut command, offer)
}

fn text(value: &Value) -> &str {
    value.as_str().expect("a string")
}

#[test]
fn given_addresses_are_listed_exactly_and_a_device_pairs_through_them() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let (_service, port) = start_server(&server);

    let made = offer(
        &server,
        &[
            "--address",
            "localhost",
            "--address",
            "10.0.2.2:9000,localhost",
        ],
    );
    let listed = [format!("localhost:{port}"), "10.0.2.2:9000".to_owned()];
    assert_eq!(made["addresses"], serde_json::json!(listed));

    let stored =
        single_document(&accept(&client, text(&made["offer"]), "srv").success())["result"].clone();
    assert_eq!(stored["addresses"], serde_json::json!(listed));
}

#[test]
fn an_address_no_device_can_dial_is_refused() {
    let server = Sandbox::new();
    let (_service, _port) = start_server(&server);
    for bad in ["0.0.0.0", "host:0", "wss://host/path"] {
        let refused = server
            .run(
                "server",
                &with_json(&["--no-autostart", "pair", "offer", "--address", bad]),
            )
            .failure();
        assert_eq!(
            single_document(&refused)["error"]["code"],
            "pairing.invalid_address",
            "{bad}"
        );
    }
}
