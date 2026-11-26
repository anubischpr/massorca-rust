//! `massorca service run` readiness record: printed once the service accepts connections, through the real binary.

mod harness;

use std::fs;
use std::net::TcpListener;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually};
use serde_json::{Value, json};

/// A local port that nothing listens on, found by binding port 0 and releasing it.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free local port")
        .local_addr()
        .expect("the local address")
        .port()
}

/// Waits until `path` holds one complete first line and returns it. The service prints nothing before that line.
fn first_line(path: &std::path::Path) -> String {
    eventually("the readiness line on stdout", || {
        fs::read_to_string(path)
            .ok()
            .filter(|text| text.ends_with('\n'))
            .and_then(|text| text.lines().next().map(str::to_owned))
    })
}

/// Starts the `default` profile with `--json` and the given extra arguments; returns its guard and the readiness record.
fn start_with_readiness(sandbox: &Sandbox, extra: &[&str]) -> (ChildGuard, Value, String) {
    let stdout_path = sandbox.home().join("service-run.stdout");
    let stdout = fs::File::create(&stdout_path).expect("a file for stdout");
    let mut args = vec!["--json"];
    args.extend_from_slice(extra);
    let guard = sandbox.spawn_service_with("default", &args, Stdio::from(stdout), Stdio::null());
    let line = first_line(&stdout_path);
    let document: Value = serde_json::from_str(&line).expect("the readiness line is JSON");
    (guard, document["result"].clone(), line)
}

#[test]
fn readiness_record_without_a_network_listener_names_the_local_endpoint() {
    let sandbox = Sandbox::new();
    let (_service, record, _line) = start_with_readiness(&sandbox, &[]);

    assert_eq!(record["profile"], "default");
    assert!(record["pid"].as_u64().is_some_and(|pid| pid > 0));
    assert_eq!(record["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        record["protocol"]
            .as_u64()
            .is_some_and(|protocol| protocol > 0)
    );
    assert!(
        record["endpoint"]
            .as_str()
            .is_some_and(|endpoint| !endpoint.is_empty())
    );
    assert!(
        record.get("network").is_none(),
        "no network block: {record}"
    );
}

#[test]
fn readiness_record_with_a_network_listener_is_bound_and_matches_server_status() {
    let sandbox = Sandbox::new();
    let port = free_port();
    let listen = format!("localhost:{port}");
    let (_service, record, _line) = start_with_readiness(&sandbox, &["--listen", listen.as_str()]);

    let listener = &record["network"]["listeners"][0];
    assert_eq!(listener["requested"], format!("127.0.0.1:{port}"));
    assert_eq!(
        listener["state"], "listening",
        "printed only once bound: {record}"
    );
    assert!(
        listener["bound"]
            .as_str()
            .is_some_and(|bound| !bound.is_empty())
    );
    let pin = record["network"]["server_pin"]
        .as_str()
        .expect("the server pin");
    assert!(
        pin.starts_with("sha256:"),
        "a public key fingerprint: {pin}"
    );
    assert_eq!(record["network"]["pending_offers"], 0);

    let status = sandbox.json_ok("default", &["--no-autostart", "server", "status"]);
    assert_eq!(status["network"]["server_pin"], pin);
    assert_eq!(
        status["network"]["listeners"][0]["bound"],
        listener["bound"]
    );
}

#[test]
fn readiness_record_never_carries_a_pairing_secret() {
    let sandbox = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let (_service, record, line) = start_with_readiness(&sandbox, &["--listen", listen.as_str()]);
    let offer = sandbox.json_ok("default", &["--no-autostart", "pair", "offer"]);
    let offer = offer["offer"]
        .as_str()
        .expect("the offer is a string")
        .to_owned();

    let printed = fs::read_to_string(sandbox.home().join("service-run.stdout")).expect("stdout");
    assert!(
        !printed.contains(&offer),
        "the service stdout holds no offer"
    );
    assert!(!line.contains(&offer));
    let keys = record.as_object().expect("an object");
    assert!(
        keys.keys()
            .all(|key| !key.contains("offer") && !key.contains("secret"))
    );
}

#[test]
fn text_readiness_line_names_the_profile_pid_and_endpoint() {
    let sandbox = Sandbox::new();
    let stdout_path = sandbox.home().join("service-run-text.stdout");
    let stdout = fs::File::create(&stdout_path).expect("a file for stdout");
    let _service = sandbox.spawn_service_with("default", &[], Stdio::from(stdout), Stdio::null());

    let line = first_line(&stdout_path);
    assert!(line.contains("default"), "profile in: {line}");
    assert!(line.contains("pid"), "pid in: {line}");
    assert!(line.contains("ready"), "readiness in: {line}");
    assert!(
        !line.trim_start().starts_with('{'),
        "text mode is not JSON: {line}"
    );
}

#[test]
fn readiness_record_names_its_schema_and_the_bound_and_advertised_endpoints() {
    let sandbox = Sandbox::new();
    let port = free_port();
    let listen = format!("localhost:{port}");
    let (_service, record, _line) = start_with_readiness(&sandbox, &["--listen", listen.as_str()]);

    assert_eq!(record["schema"], "orca_server_ready");
    assert_eq!(record["schema_version"], 1);
    let bound = record["network"]["listeners"][0]["bound"].clone();
    assert_eq!(record["bound_endpoints"], json!([bound]));
    // The advertised list is what offers list: the bound listener, with no MagicDNS name on localhost.
    assert_eq!(record["advertised_endpoints"], json!([bound]));
}

#[test]
fn readiness_record_without_a_listener_has_no_endpoints_and_says_pairing_needs_one() {
    let sandbox = Sandbox::new();
    let (_service, record, _line) = start_with_readiness(&sandbox, &[]);

    assert_eq!(record["schema"], "orca_server_ready");
    assert_eq!(record["bound_endpoints"], json!([]));
    assert_eq!(record["advertised_endpoints"], json!([]));
    assert_eq!(record["pairing"]["available"], false);
    let reason = record["pairing"]["reason"].as_str().expect("a reason");
    assert!(reason.contains("--listen"), "guidance in: {reason}");
    assert_eq!(record["pairing"]["pending_offers"], json!([]));
}

#[test]
fn readiness_pairing_object_is_available_with_a_listener_and_offers_no_secret() {
    let sandbox = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let (_service, record, _line) = start_with_readiness(&sandbox, &["--listen", listen.as_str()]);

    assert_eq!(record["pairing"]["available"], true);
    assert!(record["pairing"].get("reason").is_none());
    assert_eq!(record["pairing"]["pending_offers"], json!([]));
}
