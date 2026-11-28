//! Dialing a paired host through SSH (decision 0133) through the real binary. A server profile listens on localhost, a
//! client pairs with it, and `host route` sends its connections through `massorca-fake-ssh`, which stands in for
//! `ssh -W` (named by `MASSORCA_SSH`) and never contacts any machine but this one.

mod harness;

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use harness::{ChildGuard, Sandbox, eventually, execute_with_stdin, single_document, with_json};
use serde_json::Value;
use tempfile::TempDir;

const ROUTE_TARGET: &str = "deploy@vps.example.ts.net";

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free local port")
        .local_addr()
        .expect("the local address")
        .port()
}

/// Starts the `server` profile listening on localhost; the guard kills it when the test ends.
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

/// Pairs the `client` profile with the server as host `srv`.
fn pair(server: &Sandbox, client: &Sandbox) -> Value {
    let offer = server.json_ok("server", &["--no-autostart", "pair", "offer"]);
    let offer = offer["offer"].as_str().expect("the offer is a string");
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
    single_document(&execute_with_stdin(&mut command, offer).success())["result"].clone()
}

/// The example `massorca-fake-ssh`. `cargo test -p massorca-cli` builds it with the tests; a filtered run builds it
/// here once. An existing build is used as it is: rebuilding with other features would relink the `massorca` binary
/// another test's service is running.
fn fake_ssh_program() -> PathBuf {
    let test_binary = std::env::current_exe().expect("the test binary");
    let program = test_binary
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder")
        .join("examples")
        .join(format!("massorca-fake-ssh{}", std::env::consts::EXE_SUFFIX));
    static BUILT: std::sync::Once = std::sync::Once::new();
    BUILT.call_once(|| {
        if program.is_file() {
            return;
        }
        let status = massorca_process::command(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "-p",
                "massorca-cli",
                "--example",
                "massorca-fake-ssh",
            ])
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the fake ssh example builds");
    });
    program
}

/// Points the client's dial at the fake through `MASSORCA_SSH`, so the real ssh is never run. A copy named `ssh` on
/// `PATH` would do too, but real-time antivirus holds a fresh `ssh.exe` in a temporary folder suspended. Returns the
/// folder of the log the fake appends to, and the log.
fn use_fake_ssh(client: &Sandbox) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a log folder");
    let program = fake_ssh_program();
    client.set_env("MASSORCA_SSH", program.to_str().expect("a UTF-8 path"));
    let log = dir.path().join("ssh-runs.jsonl");
    client.set_env("FAKE_SSH_LOG", log.to_str().expect("a UTF-8 path"));
    (dir, log)
}

/// Each run of the fake: its pid and arguments.
fn ssh_runs(log: &Path) -> Vec<(u32, Vec<String>)> {
    let Ok(text) = fs::read_to_string(log) else {
        return Vec::new();
    };
    text.lines()
        .map(|line| {
            let run: Value = serde_json::from_str(line).expect("a JSON line");
            let pid = run["pid"].as_u64().and_then(|pid| u32::try_from(pid).ok());
            let args = run["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|arg| arg.as_str().expect("a string").to_owned())
                .collect();
            (pid.expect("a pid"), args)
        })
        .collect()
}

/// Waits until process `pid` is gone; the fake lingers a minute unless it is killed.
fn wait_until_gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !is_gone(pid) {
        assert!(
            Instant::now() < deadline,
            "the ssh process {pid} outlived its connection"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(unix)]
fn is_gone(pid: u32) -> bool {
    let alive = massorca_process::command("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    !alive
}

#[cfg(windows)]
fn is_gone(pid: u32) -> bool {
    let output = massorca_process::command("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .output()
        .expect("run tasklist");
    !String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
}

#[test]
fn a_host_with_an_ssh_route_is_dialed_through_ssh_and_its_key_is_still_checked() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    let stored = pair(&server, &client);
    let address = stored["addresses"][0]
        .as_str()
        .expect("an address")
        .to_owned();
    let (_bin, log) = use_fake_ssh(&client);

    let set = client.json_ok(
        "client",
        &[
            "host",
            "route",
            "srv",
            "--ssh",
            ROUTE_TARGET,
            "--ssh-arg",
            "-J",
            "--ssh-arg",
            "bastion",
        ],
    );
    assert_eq!(set["route"]["ssh_target"], ROUTE_TARGET);
    assert_eq!(
        set["route"]["ssh_args"],
        serde_json::json!(["-J", "bastion"])
    );
    let shown = client.json_ok("client", &["host", "show", "srv"]);
    assert_eq!(
        shown["route"]["ssh_target"], ROUTE_TARGET,
        "the route is saved"
    );

    let me = client.json_ok("client", &["--host", "srv", "device", "whoami"]);
    assert_eq!(me["principal"], "device");
    let runs = ssh_runs(&log);
    assert_eq!(runs.len(), 1, "one connection, through ssh: {runs:?}");
    let (pid, args) = &runs[0];
    let expected_tail = ["-J", "bastion", "-W", address.as_str(), "--", ROUTE_TARGET];
    assert!(
        args.ends_with(&expected_tail.map(str::to_owned)),
        "{args:?}"
    );
    assert!(args.iter().any(|arg| arg == "BatchMode=yes"), "{args:?}");
    wait_until_gone(*pid);

    // Another server key behind the same route is refused as on a direct connection.
    let host_file = client
        .profile_dir("client")
        .join("hosts")
        .join("srv")
        .join("host.json");
    let mut record: Value =
        serde_json::from_slice(&fs::read(&host_file).expect("the host file")).expect("JSON");
    let other_pin = format!("sha256:{}", "A".repeat(43));
    assert_ne!(record["pin"], other_pin.as_str());
    record["pin"] = Value::String(other_pin);
    fs::write(&host_file, serde_json::to_vec(&record).expect("JSON"))
        .expect("the host file is written");
    let refused = client
        .run("client", &with_json(&["--host", "srv", "device", "whoami"]))
        .failure();
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "host.identity_mismatch"
    );
    let runs = ssh_runs(&log);
    assert_eq!(
        runs.len(),
        2,
        "the refused connection went through ssh too: {runs:?}"
    );
    wait_until_gone(runs[1].0);
}

#[test]
fn a_failing_route_is_unreachable_and_names_it_and_direct_clears_it() {
    let server = Sandbox::new();
    let client = Sandbox::new();
    let _service = start_server(&server);
    pair(&server, &client);
    let (_bin, log) = use_fake_ssh(&client);

    client.json_ok(
        "client",
        &[
            "host",
            "route",
            "srv",
            "--ssh",
            "denied@vps.example.ts.net",
            "--ssh-arg",
            "-J",
            "--ssh-arg",
            "bastion",
        ],
    );
    let failed = client
        .run("client", &with_json(&["--host", "srv", "device", "whoami"]))
        .code(4);
    let error = &single_document(&failed)["error"];
    assert_eq!(error["code"], "host.unreachable");
    let message = error["message"].as_str().expect("a message");
    assert!(
        message.contains("ssh -J bastion denied@vps.example.ts.net")
            && message.contains("Permission denied"),
        "{message}"
    );
    assert_eq!(ssh_runs(&log).len(), 1);

    let cleared = client.json_ok("client", &["host", "route", "srv", "--direct"]);
    assert!(cleared["route"].is_null(), "{cleared}");
    let shown = client.json_ok("client", &["host", "show", "srv"]);
    assert!(
        shown.get("route").is_none(),
        "the cleared route is saved: {shown}"
    );
    client.json_ok("client", &["--host", "srv", "device", "whoami"]);
    assert_eq!(ssh_runs(&log).len(), 1, "a direct host runs no ssh");
}
