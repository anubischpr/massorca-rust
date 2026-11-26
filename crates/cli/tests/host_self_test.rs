//! `host self-test NAME` through the real binary (REM-25): a server profile listens on localhost, a client profile
//! pairs with it and runs the checks while the server runs, stops, or is replaced by one with another key.

mod harness;

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, single_document, with_json};
use serde_json::Value;

/// The log command for a host installed over SSH: the same unit the install wrote, read through the same target.
const SSH_LOGS: &str = "ssh vps journalctl --user -u massorca.service -n 100 --no-pager";
const LOCAL_CHECKS: [&str; 5] = ["endpoint", "connect", "protocol", "service", "latency"];

/// Marks paired host `name` of the `client` profile as installed over SSH at `target`, the way `host add-ssh` does.
fn mark_installed_over_ssh(client: &Sandbox, name: &str, target: &str) {
    let path = client
        .profile_dir("client")
        .join("hosts")
        .join(name)
        .join("install.json");
    std::fs::write(path, format!(r#"{{"ssh_target":"{target}"}}"#)).expect("the marker is written");
}

/// A fake `ssh` first on PATH that leaves a marker file when run; returns the marker's path.
fn fake_ssh_on_path(client: &Sandbox) -> PathBuf {
    let dir = client.home().join("fake-bin");
    std::fs::create_dir_all(&dir).expect("the fake bin folder");
    let marker = client.home().join("ssh-was-run");
    #[cfg(windows)]
    {
        let script = format!("@echo ran>> \"{}\"\r\n", marker.display());
        std::fs::write(dir.join("ssh.bat"), script).expect("the fake ssh");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = format!("#!/bin/sh\necho ran >> \"{}\"\n", marker.display());
        let path = dir.join("ssh");
        std::fs::write(&path, script).expect("the fake ssh");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("the fake ssh is executable");
    }
    let mut paths = vec![dir];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let joined = std::env::join_paths(paths).expect("a PATH");
    client.set_env("PATH", &joined.to_string_lossy());
    marker
}

fn document_checks_names(checks: &[Value]) -> Vec<String> {
    checks
        .iter()
        .map(|check| check["name"].as_str().unwrap().to_owned())
        .collect()
}

const CHECKS: [&str; 8] = [
    "record", "address", "port", "identity", "protocol", "paired", "service", "latency",
];

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free local port")
        .local_addr()
        .expect("the local address")
        .port()
}

/// Starts profile `profile` of `server` listening on `listen`, and returns once its listener is up.
fn start_server(server: &Sandbox, profile: &str, listen: &str) -> ChildGuard {
    let guard =
        server.spawn_service_with(profile, &["--listen", listen], Stdio::null(), Stdio::null());
    eventually("the server listener", || {
        let outcome = server.run(profile, &with_json(&["--no-autostart", "server", "status"]));
        if !outcome.get_output().status.success() {
            return None;
        }
        let document = single_document(&outcome);
        (document["result"]["network"]["listeners"][0]["state"] == "listening").then_some(())
    });
    guard
}

fn stop_server(server: &Sandbox, profile: &str, mut guard: ChildGuard) {
    server
        .run(profile, &["--no-autostart", "service", "stop"])
        .success();
    guard.wait_for_exit();
}

/// Pairs the `client` profile with profile `profile` of `server` as host `name`.
fn pair(server: &Sandbox, profile: &str, client: &Sandbox, name: &str) {
    let offer = server.json_ok(profile, &["--no-autostart", "pair", "offer"]);
    let offer = offer["offer"].as_str().expect("the offer is a string");
    let mut command = client.command("client");
    command.args([
        "--json",
        "pair",
        "accept",
        "-",
        "--name",
        name,
        "--device-name",
        "laptop",
    ]);
    harness::execute_with_stdin(&mut command, offer).success();
}

/// Runs `host self-test <name> --json` and returns the one document it printed and its exit code.
fn self_test_json(client: &Sandbox, name: &str) -> (Value, Option<i32>) {
    let outcome = client.run("client", &["--json", "host", "self-test", name]);
    let code = outcome.get_output().status.code();
    (single_document(&outcome), code)
}

/// The checks of a self-test document, whether it passed (`result`) or failed (`error.data`).
fn checks_of(document: &Value) -> Vec<Value> {
    let report = if document["ok"] == true {
        &document["result"]
    } else {
        &document["error"]["data"]
    };
    report["checks"]
        .as_array()
        .expect("the document lists its checks")
        .clone()
}

fn check<'a>(checks: &'a [Value], name: &str) -> &'a Value {
    checks
        .iter()
        .find(|check| check["name"] == name)
        .unwrap_or_else(|| panic!("no {name} check in {checks:?}"))
}

#[test]
fn every_check_passes_on_a_healthy_host_and_the_command_exits_zero() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let _service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");

    let (document, code) = self_test_json(&client, "srv");
    assert_eq!(code, Some(0), "{document}");
    assert_eq!(document["ok"], true, "{document}");
    assert_eq!(document["result"]["name"], "srv");
    assert_eq!(document["result"]["passed"], true);

    let checks = checks_of(&document);
    let names: Vec<&str> = checks
        .iter()
        .map(|check| check["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, CHECKS, "{checks:?}");
    for check in &checks {
        assert_eq!(check["state"], "pass", "{check}");
        assert!(check["detail"].is_string(), "{check}");
    }
    assert!(
        check(&checks, "service")["detail"]
            .as_str()
            .unwrap()
            .contains("uptime"),
        "{checks:?}"
    );
    assert!(
        check(&checks, "latency")["detail"]
            .as_str()
            .unwrap()
            .ends_with(" ms"),
        "{checks:?}"
    );
}

#[test]
fn the_text_report_has_one_line_per_check() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let _service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");

    let outcome = client
        .run("client", &["host", "self-test", "srv"])
        .success();
    let stdout = String::from_utf8_lossy(&outcome.get_output().stdout).into_owned();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), CHECKS.len(), "{stdout:?}");
    for (line, name) in lines.iter().zip(CHECKS) {
        assert!(line.starts_with(&format!("{name}: pass")), "{line:?}");
    }
}

#[test]
fn a_stopped_host_fails_the_port_check_and_skips_the_later_checks_with_a_reason() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");
    stop_server(&server, "server", service);

    let (document, code) = self_test_json(&client, "srv");
    assert_ne!(code, Some(0), "{document}");
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(
        document["error"]["code"], "host.self_test_failed",
        "{document}"
    );
    assert_eq!(document["error"]["data"]["passed"], false, "{document}");

    let checks = checks_of(&document);
    assert_eq!(check(&checks, "record")["state"], "pass", "{checks:?}");
    assert_eq!(check(&checks, "address")["state"], "pass", "{checks:?}");
    let port = check(&checks, "port");
    assert_eq!(port["state"], "fail", "{checks:?}");
    assert!(port["fix"].is_string(), "{port}");
    for later in ["identity", "protocol", "paired", "service", "latency"] {
        let skipped = check(&checks, later);
        assert_eq!(skipped["state"], "skipped", "{skipped}");
        assert!(
            skipped["detail"].as_str().unwrap().contains("port"),
            "{skipped}"
        );
    }
}

#[test]
fn a_different_server_key_fails_the_identity_check_with_both_keys_and_is_not_retried() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");
    let paired_pin = client.json_ok("client", &["host", "show", "srv"])["pin"]
        .as_str()
        .expect("the paired pin")
        .to_owned();
    stop_server(&server, "server", service);
    let _impostor = start_server(&server, "other", &listen);
    let other_pin = server.json_ok("other", &["server", "status"])["network"]["server_pin"]
        .as_str()
        .expect("the other server's pin")
        .to_owned();
    assert_ne!(paired_pin, other_pin);

    let (document, code) = self_test_json(&client, "srv");
    assert_ne!(code, Some(0), "{document}");
    let checks = checks_of(&document);
    let identity = check(&checks, "identity");
    assert_eq!(identity["state"], "fail", "{identity}");
    let detail = identity["detail"].as_str().unwrap();
    assert!(detail.contains(&paired_pin), "{detail}");
    assert!(detail.contains(&other_pin), "{detail}");
    assert_eq!(check(&checks, "protocol")["state"], "skipped", "{checks:?}");
    assert_eq!(check(&checks, "paired")["state"], "skipped", "{checks:?}");
}

#[test]
fn a_name_with_no_host_fails_the_record_check() {
    let client = Sandbox::new();
    let (document, code) = self_test_json(&client, "nowhere");
    assert_ne!(code, Some(0), "{document}");
    let checks = checks_of(&document);
    let record = check(&checks, "record");
    assert_eq!(record["state"], "fail", "{record}");
    assert!(
        record["detail"].as_str().unwrap().contains("nowhere"),
        "{record}"
    );
    assert_eq!(check(&checks, "port")["state"], "skipped", "{checks:?}");
}

#[test]
fn host_status_output_is_unchanged_by_the_self_test() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let _service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");

    let outcome = client.run("client", &["host", "status", "srv"]).success();
    let stdout = String::from_utf8_lossy(&outcome.get_output().stdout).into_owned();
    assert!(stdout.starts_with("srv: connected, server "), "{stdout:?}");
    assert!(stdout.contains(", scopes "), "{stdout:?}");
}

#[test]
fn a_failed_self_test_of_an_ssh_installed_host_names_the_journal_command() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");
    mark_installed_over_ssh(&client, "srv", "vps");
    stop_server(&server, "server", service);

    let (document, code) = self_test_json(&client, "srv");
    assert_ne!(code, Some(0), "{document}");
    assert_eq!(document["error"]["data"]["logs"], SSH_LOGS, "{document}");

    let outcome = client.run("client", &["host", "self-test", "srv"]);
    let stdout = String::from_utf8_lossy(&outcome.get_output().stdout).into_owned();
    let last = stdout.lines().last().expect("a report line");
    assert_eq!(last, format!("logs: {SSH_LOGS}"), "{stdout:?}");
}

#[test]
fn a_failed_self_test_of_a_host_not_installed_over_ssh_has_no_logs_line() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");
    stop_server(&server, "server", service);

    let (document, code) = self_test_json(&client, "srv");
    assert_ne!(code, Some(0), "{document}");
    assert!(document["error"]["data"]["logs"].is_null(), "{document}");
    let outcome = client.run("client", &["host", "self-test", "srv"]);
    let stdout = String::from_utf8_lossy(&outcome.get_output().stdout).into_owned();
    assert!(!stdout.contains("logs:"), "{stdout:?}");
}

#[test]
fn the_self_test_never_starts_ssh() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");
    mark_installed_over_ssh(&client, "srv", "vps");
    let marker = fake_ssh_on_path(&client);

    let _ = self_test_json(&client, "srv");
    stop_server(&server, "server", service);
    let (document, _) = self_test_json(&client, "srv");
    assert_eq!(document["error"]["data"]["logs"], SSH_LOGS, "{document}");
    assert!(!marker.exists(), "ssh was started");
}

#[test]
fn a_self_test_without_a_name_passes_against_a_local_service_with_a_listener() {
    let server = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let _service = start_server(&server, "server", &listen);

    let outcome = server
        .run("server", &with_json(&["host", "self-test"]))
        .success();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], true, "{document}");
    let checks = document["result"]["checks"]
        .as_array()
        .expect("checks")
        .clone();
    let mut expected: Vec<String> = LOCAL_CHECKS.iter().map(|name| name.to_string()).collect();
    expected.push("network".to_owned());
    assert_eq!(document_checks_names(&checks), expected, "{checks:?}");
    for check in &checks {
        assert_eq!(check["state"], "pass", "{check}");
    }
    assert!(
        check(&checks, "network")["detail"]
            .as_str()
            .unwrap()
            .contains("listening"),
        "{checks:?}"
    );
}

#[test]
fn a_local_service_without_a_listener_has_no_network_check() {
    let server = Sandbox::new();
    let _guard = server.spawn_service("server");
    server.wait_for_running("server", true);

    let outcome = server
        .run("server", &with_json(&["host", "self-test"]))
        .success();
    let document = single_document(&outcome);
    let checks = document["result"]["checks"]
        .as_array()
        .expect("checks")
        .clone();
    assert_eq!(
        document_checks_names(&checks),
        LOCAL_CHECKS
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>(),
        "{checks:?}"
    );
}

#[test]
fn a_stopped_local_service_fails_the_connect_check_and_skips_the_rest() {
    let server = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    stop_server(&server, "server", service);

    let outcome = server.run("server", &with_json(&["host", "self-test"]));
    assert_ne!(outcome.get_output().status.code(), Some(0));
    let document = single_document(&outcome);
    let checks = document["error"]["data"]["checks"]
        .as_array()
        .expect("checks")
        .clone();
    assert_eq!(check(&checks, "endpoint")["state"], "pass", "{checks:?}");
    assert_eq!(check(&checks, "connect")["state"], "fail", "{checks:?}");
    assert!(check(&checks, "connect")["fix"].is_string(), "{checks:?}");
    assert_eq!(check(&checks, "protocol")["state"], "skipped", "{checks:?}");
    assert!(document["error"]["data"]["logs"].is_null(), "{document}");
}
