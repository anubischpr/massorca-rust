//! `massorca host update` (decision 0132) through the real binary: a throwaway server profile listens on localhost, a
//! client profile pairs with it, and the server's "installed binary" is a file in a temporary folder whose restart is the
//! fake's `restart`. Nothing reaches a real host or a real install.

mod harness;

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, execute_with_stdin, single_document, with_json};
use massorca_cli::host::update::require_self_update;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const OLD: &[u8] = b"the installed binary";

fn fake_update() -> PathBuf {
    static BUILT: std::sync::Once = std::sync::Once::new();
    BUILT.call_once(|| {
        let status = massorca_process::command(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "-p",
                "massorca-server",
                "--example",
                "massorca-fake-update",
            ])
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the fake update example builds");
    });
    let test_binary = std::env::current_exe().expect("the test binary");
    let profile = test_binary
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder");
    profile.join("examples").join(format!(
        "massorca-fake-update{}",
        std::env::consts::EXE_SUFFIX
    ))
}

/// A copy of the fake reporting `version_line`, in its own folder, and its sha256.
fn candidate(dir: &Path, version_line: Option<&str>) -> (PathBuf, String) {
    let mut bytes = std::fs::read(fake_update()).expect("the fake");
    if let Some(line) = version_line {
        bytes.extend_from_slice(format!("\nMASSORCA-FAKE-VERSION:{line}\n").as_bytes());
    }
    let path = dir.join(format!("candidate{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&path, &bytes).expect("the candidate");
    let sha = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    (path, sha)
}

/// The server profile listening on localhost, updating `<dir>/massorca<EXE>`, and a client paired as host `srv`.
struct Pair {
    _server: Sandbox,
    client: Sandbox,
    dir: TempDir,
    installed: PathBuf,
    _service: ChildGuard,
}

impl Pair {
    fn start(standard: bool) -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let installed = dir
            .path()
            .join(format!("massorca{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&installed, OLD).expect("the installed binary");
        let server = Sandbox::new();
        server.set_env(
            "MASSORCA_TEST_SELF_UPDATE_TARGET",
            installed.to_str().expect("utf-8"),
        );
        server.set_env(
            "MASSORCA_TEST_SELF_UPDATE_RESTART",
            fake_update().to_str().expect("utf-8"),
        );
        let port = TcpListener::bind("127.0.0.1:0")
            .expect("a port")
            .local_addr()
            .expect("addr")
            .port();
        let listen = format!("localhost:{port}");
        let child = server
            .command("server")
            .args(["service", "run", "--listen", listen.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server starts");
        let service = ChildGuard::new(child);
        eventually("the server listener", || {
            let outcome = server.run(
                "server",
                &with_json(&["--no-autostart", "server", "status"]),
            );
            let up = outcome.get_output().status.success()
                && single_document(&outcome)["result"]["network"]["listeners"][0]["state"]
                    == "listening";
            up.then_some(())
        });
        let mut offer_args = vec!["--no-autostart", "pair", "offer"];
        if standard {
            offer_args.push("--standard");
        }
        let offer = server.json_ok("server", &offer_args)["offer"]
            .as_str()
            .expect("offer")
            .to_owned();
        let client = Sandbox::new();
        client.set_env("MASSORCA_TEST_UPDATE_WAIT_MS", "1500");
        let mut accept = client.command("client");
        accept.args([
            "--json",
            "pair",
            "accept",
            "-",
            "--name",
            "srv",
            "--device-name",
            "laptop",
        ]);
        execute_with_stdin(&mut accept, &offer).success();
        Self {
            _server: server,
            client,
            dir,
            installed,
            _service: service,
        }
    }

    fn update(&self, extra: &[&str]) -> serde_json::Value {
        let mut args = vec!["--json", "host", "update", "srv"];
        args.extend_from_slice(extra);
        let outcome = self.client.run("client", &args);
        single_document(&outcome)
    }

    fn sibling(&self, name: &str) -> PathBuf {
        self.installed
            .with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }

    fn assert_untouched(&self) {
        assert_eq!(std::fs::read(&self.installed).expect("installed"), OLD);
        assert!(!self.sibling("massorca.previous").exists());
        assert!(
            !self.sibling("massorca.update").exists(),
            "nothing was uploaded"
        );
    }
}

#[test]
fn an_older_host_is_refused_before_any_upload() {
    let error =
        require_self_update(&["service/1".to_owned(), "host/1".to_owned()]).expect_err("refused");
    assert_eq!(error.code, "service.capability_missing");
    assert!(error.message.contains("self-update/1"), "{}", error.message);
    assert!(require_self_update(&["self-update/1".to_owned()]).is_ok());
}

#[test]
fn an_update_is_confirmed_checked_against_downgrades_then_swapped_and_restarted() {
    let pair = Pair::start(false);
    let (binary, sha) = candidate(pair.dir.path(), None);
    let binary = binary.to_str().expect("utf-8");

    let missing_sha = pair.update(&["--binary", binary, "--yes"]);
    assert_eq!(
        missing_sha["error"]["code"], "usage.sha256_required",
        "{missing_sha}"
    );

    let unconfirmed = pair.update(&["--binary", binary, "--sha256", &sha]);
    assert_eq!(
        unconfirmed["error"]["code"], "update.confirmation_required",
        "{unconfirmed}"
    );
    assert_eq!(unconfirmed["error"]["data"]["name"], "srv");
    assert_eq!(unconfirmed["error"]["data"]["version"], "9.9.9");
    assert!(unconfirmed["error"]["data"]["current_version"].is_string());
    pair.assert_untouched();

    let old_dir = tempfile::tempdir().expect("a folder");
    let (older, older_sha) = candidate(old_dir.path(), Some("massorca 0.0.1 (protocol 1..1)"));
    let older = older.to_str().expect("utf-8");
    let downgrade = pair.update(&["--binary", older, "--sha256", &older_sha, "--yes"]);
    assert_eq!(
        downgrade["error"]["code"], "update.downgrade",
        "{downgrade}"
    );
    pair.assert_untouched();

    // The fake restart leaves the old service running, so the CLI reports it never came back as 9.9.9.
    let updated = pair.update(&["--binary", binary, "--sha256", &sha, "--yes"]);
    assert_eq!(
        updated["error"]["code"], "update.not_confirmed",
        "{updated}"
    );
    assert_eq!(
        std::fs::read(&pair.installed).expect("installed"),
        std::fs::read(binary).expect("binary")
    );
    assert_eq!(
        std::fs::read(pair.sibling("massorca.previous")).expect("previous"),
        OLD
    );
    eventually("the restart marker", || {
        let mut marker = pair.installed.clone().into_os_string();
        marker.push(".restarted");
        PathBuf::from(marker).exists().then_some(())
    });
}

#[test]
fn a_device_without_host_admin_is_refused() {
    let pair = Pair::start(true);
    let (binary, sha) = candidate(pair.dir.path(), None);
    let denied = pair.update(&[
        "--binary",
        binary.to_str().expect("utf-8"),
        "--sha256",
        &sha,
        "--yes",
    ]);
    assert_eq!(denied["error"]["code"], "permission.denied", "{denied}");
    pair.assert_untouched();
}
