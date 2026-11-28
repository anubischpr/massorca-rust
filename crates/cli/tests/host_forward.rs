//! Remote ports and port forwards through the real binary (REM-16, REM-17, decision 0131): a server profile listens on
//! localhost and runs a dev server in a terminal; a client profile paired with it lists the host's ports with their URLs
//! and forwards one to a local port.

mod harness;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{ChildStdout, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use harness::{ChildGuard, Sandbox, eventually, execute_with_stdin, single_document, with_json};
use serde_json::Value;

const LINE_WAIT: Duration = Duration::from_secs(60);

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free local port")
        .local_addr()
        .expect("the local address")
        .port()
}

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

/// Pairs `client` with the server as host `name`; `extra` options go to the offer.
fn pair(server: &Sandbox, client: &Sandbox, name: &str, extra: &[&str]) {
    let mut args = vec!["--no-autostart", "pair", "offer"];
    args.extend_from_slice(extra);
    let offer = server.json_ok("server", &args);
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
        name,
    ]);
    execute_with_stdin(&mut command, offer).success();
}

/// Starts the fake dev server in a terminal of a folder workspace on the server, and returns its port once `host ports`
/// on the paired host lists it.
fn dev_server_port(server: &Sandbox, client: &Sandbox) -> Value {
    let folder = server.home().join("site");
    std::fs::create_dir_all(&folder).expect("create folder");
    let folder = folder.to_string_lossy().into_owned();
    server.json_ok("server", &["project", "add", folder.as_str(), "--folder"]);
    server.json_ok("server", &["workspace", "create", "site", "main"]);
    server.json_ok(
        "server",
        &[
            "terminal",
            "create",
            "main",
            "--shell",
            env!("CARGO_BIN_EXE_massorca-fake-agent"),
            "--arg",
            "dev-server",
        ],
    );
    eventually("the dev server's port on the host", || {
        let ports = client.json_ok("client", &["--host", "srv", "host", "ports"]);
        ports["ports"]
            .as_array()?
            .iter()
            .find(|port| port["url_printed"] == true)
            .cloned()
    })
}

/// A running `host forward` and the lines it prints.
struct Forward {
    child: ChildGuard,
    lines: Receiver<String>,
}

impl Forward {
    fn start(client: &Sandbox, args: &[&str]) -> Self {
        let mut child = client
            .command("client")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the forward starts");
        let stdout = child.stdout.take().expect("stdout");
        Self {
            child: ChildGuard::new(child),
            lines: read_lines(stdout),
        }
    }

    fn next_document(&self) -> Value {
        let line = self
            .lines
            .recv_timeout(LINE_WAIT)
            .expect("the forward prints a line");
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("{error}: {line}"))
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

/// Sends `message` to the local port, ends its data and returns what came back before the forward closed.
fn round_trip(port: u16, message: &[u8]) -> Vec<u8> {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).expect("connect to the forward");
    socket.set_read_timeout(Some(LINE_WAIT)).expect("timeout");
    socket.write_all(message).expect("send");
    socket.shutdown(Shutdown::Write).expect("end of data");
    let mut back = Vec::new();
    socket.read_to_end(&mut back).expect("read the echo");
    back
}

#[test]
fn a_hosts_dev_server_is_listed_with_its_url_and_forwarded_to_a_local_port() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    pair(&server, &client, "srv", &[]);

    let listed = dev_server_port(&server, &client);
    let port = listed["port"].as_u64().expect("a port");
    assert_eq!(
        listed["url"],
        format!("http://localhost:{port}/app/"),
        "{listed}"
    );
    assert!(listed.get("forwarded").is_none(), "{listed}");
    let port_text = port.to_string();
    let text = client
        .run("client", &["--host", "srv", "host", "ports"])
        .success();
    let text = String::from_utf8_lossy(&text.get_output().stdout).into_owned();
    assert!(
        text.contains("URL") && text.contains(&format!("http://localhost:{port}/app/")),
        "{text}"
    );

    let forward = Forward::start(
        &client,
        &[
            "--json",
            "host",
            "forward",
            "srv",
            port_text.as_str(),
            "--local",
            "0",
        ],
    );
    let started = forward.next_document();
    assert_eq!(started["ok"], true, "{started}");
    let started = &started["result"];
    assert_eq!(started["state"], "forwarding", "{started}");
    assert_eq!(started["local_address"], "127.0.0.1", "{started}");
    let local =
        u16::try_from(started["local_port"].as_u64().expect("the bound port")).expect("a port");
    assert_ne!(local, 0);
    assert_eq!(
        started["url"],
        format!("http://127.0.0.1:{local}/app/"),
        "{started}"
    );
    let id = started["forward"]
        .as_str()
        .expect("the forward id")
        .to_owned();

    let marked = client.json_ok("client", &["--host", "srv", "host", "ports"]);
    assert!(
        marked["ports"]
            .as_array()
            .expect("ports")
            .iter()
            .any(|entry| entry["port"] == port && entry["forwarded"] == true),
        "{marked}"
    );
    let forwards = client.json_ok("client", &["host", "forward", "srv", "--list"]);
    assert_eq!(forwards["forwards"][0]["id"], id.as_str(), "{forwards}");
    assert_eq!(forwards["forwards"][0]["local_port"], local, "{forwards}");

    for round in 0..15 {
        let message = format!("request {round}").into_bytes();
        assert_eq!(round_trip(local, &message), message);
    }
    let lanes: Vec<_> = (0u8..3)
        .map(|lane| {
            thread::spawn(move || {
                let message: Vec<u8> = (0..200 * 1024).map(|at| (at % 251) as u8 ^ lane).collect();
                assert!(
                    round_trip(local, &message) == message,
                    "lane {lane} came back changed"
                );
            })
        })
        .collect();
    for lane in lanes {
        lane.join().expect("a concurrent connection");
    }

    let stopped = client.json_ok("client", &["host", "forward", "srv", "--stop", id.as_str()]);
    assert_eq!(stopped["forward"]["id"], id.as_str(), "{stopped}");
    assert_eq!(stopped["forward"]["total_connections"], 18, "{stopped}");
    let ended = forward.next_document();
    assert_eq!(ended["result"]["state"], "stopped", "{ended}");
    assert_eq!(ended["result"]["reason"], "stopped on the host", "{ended}");
    let mut child = forward.child;
    assert!(child.wait_for_exit().success());
    let after = client.json_ok("client", &["host", "forward", "srv", "--list"]);
    assert_eq!(after["forwards"], Value::Array(Vec::new()), "{after}");
}

/// A forward reaches loopback ports on the host, so it needs host-admin: a standard device is refused, but may list.
#[test]
fn a_device_without_host_admin_is_refused_and_bad_command_lines_exit_2() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    pair(&server, &client, "small", &["--standard"]);

    let denied = client
        .run(
            "client",
            &with_json(&["host", "forward", "small", "3000", "--local", "0"]),
        )
        .code(1);
    assert_eq!(
        single_document(&denied)["error"]["code"],
        "permission.denied"
    );
    let listed = client.json_ok("client", &["host", "forward", "small", "--list"]);
    assert_eq!(listed["forwards"], Value::Array(Vec::new()), "{listed}");

    let refused = client
        .run(
            "client",
            &with_json(&["--host", "small", "host", "forward", "small", "3000"]),
        )
        .code(2);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "cli.host_not_supported"
    );
    client.run("client", &["host", "forward", "small"]).code(2);
    client
        .run("client", &["host", "forward", "small", "3000", "--list"])
        .code(2);
}
