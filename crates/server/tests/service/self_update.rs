//! `host.update_*` (decision 0132) against a real in-process service: the installed binary is a fake in a temporary
//! folder, and the restart runs the fake's `restart`, which leaves a marker. Nothing here touches a real install.

use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_client::{Client, ClientError};
use massorca_core_remote::self_update::Supervisor;
use massorca_protocol::codes as core_codes;
use massorca_protocol::methods::self_update::{
    self as update, MAX_BINARY_BYTES, UpdateBeginParams, UpdateBegun, UpdateChunkAccepted,
    UpdateChunkParams, UpdateFinishParams, UpdateInstalled, UpdateRollbackParams, codes,
};
use massorca_server::{RunningService, SelfUpdateOptions, Service, ServiceConfig};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::common::{SERVER_NAME, connect, profile, remote_code, stop, temp_home};
use crate::network::{client_side, offer, pair, start_listening};

/// The fake's default `--version` is 9.9.9, so the service runs an older version than it.
const RUNNING: &str = "1.0.0";
const OLD: &[u8] = b"the installed binary";

/// The fake new binary, built on first use (see `examples/massorca-fake-update.rs`).
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
    let profile_dir = test_binary
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder");
    profile_dir.join("examples").join(format!(
        "massorca-fake-update{}",
        std::env::consts::EXE_SUFFIX
    ))
}

/// The fake's bytes, with a `--version` line of `version_line` when given.
fn fake_bytes(version_line: Option<&str>) -> Vec<u8> {
    let mut bytes = std::fs::read(fake_update()).expect("the fake binary");
    if let Some(line) = version_line {
        bytes.extend_from_slice(format!("\nMASSORCA-FAKE-VERSION:{line}\n").as_bytes());
    }
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A service whose updates replace `<dir>/bin/massorca<EXE>`, holding `OLD`, and restart through the fake.
struct Host {
    _dir: TempDir,
    home: crate::common::TempHome,
    installed: PathBuf,
    running: RunningService,
}

impl Host {
    async fn start(with_supervisor: bool) -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let installed = bin.join(format!("massorca{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&installed, OLD).expect("the installed binary");
        let home = temp_home();
        let config = ServiceConfig {
            profile: profile(home.path(), "default"),
            server_name: SERVER_NAME.to_owned(),
            server_version: RUNNING.to_owned(),
        };
        let options = SelfUpdateOptions {
            target: Some(installed.clone()),
            supervisor: with_supervisor.then(|| Supervisor::Program(fake_update())),
        };
        let running = Service::start_with_self_update(config, options)
            .await
            .expect("the service starts");
        Self {
            _dir: dir,
            home,
            installed,
            running,
        }
    }

    async fn client(&self) -> Client {
        connect(self.home.path(), "default").await
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut path = self.installed.clone().into_os_string();
        path.push(suffix);
        PathBuf::from(path)
    }

    fn previous(&self) -> PathBuf {
        let name = format!("massorca.previous{}", std::env::consts::EXE_SUFFIX);
        self.installed.with_file_name(name)
    }

    fn staged(&self) -> PathBuf {
        self.installed
            .with_file_name(format!("massorca.update{}", std::env::consts::EXE_SUFFIX))
    }
}

/// Begins with `announced` sha, uploads `bytes` in two chunks, and finishes with `finish`.
async fn upload(
    client: &Client,
    bytes: &[u8],
    announced: &str,
    finish: impl FnOnce(String) -> UpdateFinishParams,
) -> Result<UpdateInstalled, ClientError> {
    let begun: UpdateBegun = client
        .call(
            update::UPDATE_BEGIN,
            &UpdateBeginParams {
                size: bytes.len() as u64,
                sha256: announced.to_owned(),
            },
        )
        .await?;
    let half = bytes.len() / 2;
    for (offset, part) in [(0, &bytes[..half]), (half, &bytes[half..])] {
        let _: UpdateChunkAccepted = client
            .call(
                update::UPDATE_CHUNK,
                &UpdateChunkParams::new(&begun.upload_id, offset as u64, part),
            )
            .await?;
    }
    client
        .call(update::UPDATE_FINISH, &finish(begun.upload_id))
        .await
}

fn plain(upload_id: String) -> UpdateFinishParams {
    UpdateFinishParams {
        upload_id,
        expected_version: None,
        allow_downgrade: false,
        force: false,
    }
}

async fn wait_for(path: &Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{} never appeared", path.display());
}

fn assert_untouched(host: &Host) {
    assert_eq!(std::fs::read(&host.installed).expect("installed"), OLD);
    assert!(!host.previous().exists(), "no backup is made");
    assert!(!host.staged().exists(), "the staged file is removed");
    assert!(!host.sibling(".restarted").exists(), "no restart");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_update_swaps_the_binary_keeps_previous_and_restarts() {
    let host = Host::start(true).await;
    let client = host.client().await;
    let bytes = fake_bytes(None);
    let installed = upload(&client, &bytes, &sha256(&bytes), |id| UpdateFinishParams {
        expected_version: Some("9.9.9".to_owned()),
        ..plain(id)
    })
    .await
    .expect("the update is installed");
    assert_eq!(installed.version, "9.9.9");
    assert_eq!(installed.previous_version, RUNNING);
    assert_eq!(installed.supervisor, "test");
    assert_eq!(std::fs::read(&host.installed).expect("installed"), bytes);
    assert_eq!(std::fs::read(host.previous()).expect("previous"), OLD);
    wait_for(&host.sibling(".restarted")).await;
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sha_mismatch_is_refused_and_the_old_binary_stays() {
    let host = Host::start(true).await;
    let client = host.client().await;
    let bytes = fake_bytes(None);
    let error = upload(&client, &bytes, &sha256(b"something else"), plain)
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&error), codes::CHECKSUM_MISMATCH);
    assert_untouched(&host);
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversize_upload_is_refused() {
    let host = Host::start(true).await;
    let client = host.client().await;
    let begin = |size| UpdateBeginParams {
        size,
        sha256: sha256(b"x"),
    };
    let error = client
        .call::<_, UpdateBegun>(update::UPDATE_BEGIN, &begin(MAX_BINARY_BYTES + 1))
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&error), codes::TOO_LARGE);
    let begun: UpdateBegun = client
        .call(update::UPDATE_BEGIN, &begin(4))
        .await
        .expect("begun");
    let error = client
        .call::<_, UpdateChunkAccepted>(
            update::UPDATE_CHUNK,
            &UpdateChunkParams::new(&begun.upload_id, 0, b"12345"),
        )
        .await
        .expect_err("past the announced size");
    assert_eq!(remote_code(&error), codes::TOO_LARGE);
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_non_executable_or_incompatible_binary_is_refused() {
    let host = Host::start(true).await;
    let client = host.client().await;
    let not_a_program = b"#not a program at all\n".repeat(8);
    let other_protocol = fake_bytes(Some("massorca 9.9.9 (protocol 70..90)"));
    let unreadable = fake_bytes(Some("something 1.2"));
    for bytes in [not_a_program, other_protocol, unreadable] {
        let error = upload(&client, &bytes, &sha256(&bytes), plain)
            .await
            .expect_err("refused");
        assert_eq!(remote_code(&error), codes::INCOMPATIBLE, "{error:?}");
        assert_untouched(&host);
    }
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_host_refuses_a_downgrade_or_the_same_version_unless_asked() {
    let host = Host::start(true).await;
    let client = host.client().await;
    let older = fake_bytes(Some(&format!(
        "massorca 0.9.0 (protocol 1..{})",
        massorca_protocol::PROTOCOL_VERSION
    )));
    let error = upload(&client, &older, &sha256(&older), plain)
        .await
        .expect_err("older");
    assert_eq!(remote_code(&error), codes::DOWNGRADE);
    let same = fake_bytes(Some(&format!(
        "massorca {RUNNING} (protocol 1..{})",
        massorca_protocol::PROTOCOL_VERSION
    )));
    let error = upload(&client, &same, &sha256(&same), plain)
        .await
        .expect_err("same");
    assert_eq!(remote_code(&error), codes::SAME_VERSION);
    let newer = fake_bytes(None);
    let error = upload(&client, &newer, &sha256(&newer), |id| UpdateFinishParams {
        expected_version: Some("9.9.8".to_owned()),
        ..plain(id)
    })
    .await
    .expect_err("not the confirmed version");
    assert_eq!(remote_code(&error), codes::VERSION_MISMATCH);
    assert_untouched(&host);

    let installed = upload(&client, &older, &sha256(&older), |id| UpdateFinishParams {
        allow_downgrade: true,
        ..plain(id)
    })
    .await
    .expect("an explicit downgrade");
    assert_eq!(installed.version, "0.9.0");
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rollback_restores_previous_after_a_dry_run_names_it() {
    let host = Host::start(true).await;
    let client = host.client().await;
    let rollback = |dry_run| UpdateRollbackParams {
        expected_version: None,
        dry_run,
    };
    let error = client
        .call::<_, UpdateInstalled>(update::UPDATE_ROLLBACK, &rollback(true))
        .await
        .expect_err("nothing kept");
    assert_eq!(remote_code(&error), codes::NO_PREVIOUS);

    let kept = fake_bytes(Some(&format!(
        "massorca 1.0.0 (protocol 1..{})",
        massorca_protocol::PROTOCOL_VERSION
    )));
    std::fs::write(host.previous(), &kept).expect("a kept binary");
    let named: UpdateInstalled = client
        .call(update::UPDATE_ROLLBACK, &rollback(true))
        .await
        .expect("dry run");
    assert_eq!(named.version, "1.0.0");
    assert_eq!(
        std::fs::read(&host.installed).expect("installed"),
        OLD,
        "a dry run swaps nothing"
    );
    let params = UpdateRollbackParams {
        expected_version: Some(named.version),
        dry_run: false,
    };
    let _: UpdateInstalled = client
        .call(update::UPDATE_ROLLBACK, &params)
        .await
        .expect("rolled back");
    assert_eq!(std::fs::read(&host.installed).expect("installed"), kept);
    wait_for(&host.sibling(".restarted")).await;
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_host_with_no_supervisor_refuses_before_any_upload() {
    let host = Host::start(false).await;
    let client = host.client().await;
    let error = client
        .call::<_, UpdateBegun>(
            update::UPDATE_BEGIN,
            &UpdateBeginParams {
                size: 4,
                sha256: sha256(b"1234"),
            },
        )
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&error), codes::UNSUPPORTED);
    assert!(!host.staged().exists());
    stop(host.running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_without_host_admin_is_refused() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let (standard, _) = pair(&hosts, &offer(home.path(), false, None).await, "plain")
        .await
        .unwrap();
    let error = standard
        .call::<_, UpdateBegun>(
            update::UPDATE_BEGIN,
            &UpdateBeginParams {
                size: 4,
                sha256: sha256(b"1234"),
            },
        )
        .await
        .expect_err("denied");
    assert_eq!(remote_code(&error), core_codes::PERMISSION_DENIED);
    stop(running).await;
}
