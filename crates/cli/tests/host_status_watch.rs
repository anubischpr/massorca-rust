//! `host status --watch` through the real binary (REM-07): a server profile listens on localhost, a client profile pairs
//! with it and watches it while the server stops, comes back, or is replaced by one with another key.

mod harness;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{ChildStdout, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use harness::{ChildGuard, Sandbox, eventually, execute_with_stdin, single_document, with_json};
use serde_json::Value;

/// How long the watch may take to print the next change; a retry waits at most 30 s.
const LINE_WAIT: Duration = Duration::from_secs(60);

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
    execute_with_stdin(&mut command, offer).success();
}

/// A running watch and the lines it printed, read on their own thread.
struct Watch {
    child: ChildGuard,
    lines: Receiver<String>,
}

impl Watch {
    fn start(client: &Sandbox, args: &[&str]) -> Self {
        let mut child = client
            .command("client")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the watch starts");
        let stdout = child.stdout.take().expect("stdout");
        Self {
            child: ChildGuard::new(child),
            lines: read_lines(stdout),
        }
    }

    /// The next line; fails when none comes in time.
    fn next(&self) -> String {
        self.lines
            .recv_timeout(LINE_WAIT)
            .expect("the watch prints the next change")
    }

    /// Lines up to and including the first that `done` accepts.
    fn until(&self, done: impl Fn(&str) -> bool) -> Vec<String> {
        let mut seen = Vec::new();
        loop {
            let line = self.next();
            seen.push(line.clone());
            if done(&line) {
                return seen;
            }
        }
    }

    /// JSON documents up to and including the first whose result has state `state`.
    fn until_state(&self, state: &str) -> Vec<Value> {
        self.until(|line| document(line)["result"]["state"] == state)
            .iter()
            .map(|line| document(line))
            .collect()
    }
}

fn read_lines(stdout: ChildStdout) -> Receiver<String> {
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { return };
            if sender.send(line).is_err() {
                return;
            }
        }
    });
    lines
}

fn document(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{line:?} is not JSON: {error}"))
}

#[test]
fn a_watched_host_goes_unreachable_when_it_stops_and_connected_when_it_returns() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");

    let watch = Watch::start(&client, &["--json", "host", "status", "--watch", "srv"]);
    let first = watch.until_state("connected");
    assert_eq!(first[0]["result"]["state"], "connecting", "{first:?}");
    let connected = &first[first.len() - 1]["result"];
    assert_eq!(connected["name"], "srv");
    assert!(connected["server_version"].is_string(), "{connected}");

    stop_server(&server, "server", service);
    let lost = watch.until_state("unreachable");
    let unreachable = &lost[lost.len() - 1]["result"];
    assert_eq!(unreachable["name"], "srv");
    assert_eq!(unreachable["attempt"], 1, "{unreachable}");
    assert!(unreachable["retry_in_ms"].is_u64(), "{unreachable}");
    assert!(unreachable["detail"].is_string(), "{unreachable}");

    let _service = start_server(&server, "server", &listen);
    let back = watch.until_state("connected");
    assert!(
        back.iter()
            .all(|line| line["ok"] == true && line["result"]["name"] == "srv"),
        "{back:?}"
    );
}

#[test]
fn a_host_with_another_server_key_is_reported_once_and_not_retried() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");
    stop_server(&server, "server", service);
    // Another computer now answers at the paired address, with a key of its own.
    let _impostor = start_server(&server, "other", &listen);

    let watch = Watch::start(&client, &["host", "status", "--watch", "srv"]);
    let seen = watch.until(|line| line.starts_with("srv: identity mismatch: "));
    assert_eq!(seen[0], "srv: connecting", "{seen:?}");
    let mut watch = watch;
    // Nothing more can change: the watch ends, with the exit code of the mismatch.
    assert_eq!(watch.child.wait_for_exit().code(), Some(4));
    assert!(
        watch.lines.recv_timeout(Duration::from_secs(1)).is_err(),
        "nothing more is printed"
    );
}

#[test]
fn a_watch_of_a_device_revoked_on_its_host_ends_once_nothing_can_change() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let listen = format!("localhost:{}", free_port());
    let _service = start_server(&server, "server", &listen);
    pair(&server, "server", &client, "srv");

    let mut watch = Watch::start(&client, &["--json", "host", "status", "--watch", "srv"]);
    watch.until_state("connected");
    server.json_ok("server", &["--no-autostart", "device", "revoke", "laptop"]);
    let refused = watch.until_state("refused");
    let last = &refused[refused.len() - 1]["result"];
    assert_eq!(last["detail"], "auth.device_revoked", "{last}");
    assert_eq!(watch.child.wait_for_exit().code(), Some(4));
    let failure = watch.next();
    assert_eq!(document(&failure)["ok"], false, "{failure}");
}

#[test]
fn watching_with_no_hosts_says_so_and_ends() {
    let client = Sandbox::new();
    let outcome = client
        .run("client", &["host", "status", "--watch"])
        .success();
    assert_eq!(
        String::from_utf8_lossy(&outcome.get_output().stdout).trim(),
        "no hosts"
    );
}
