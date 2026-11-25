//! The account Import prefill through the bridge, against a real service (decision 0097). The service runs in a child
//! process, this test binary run again, so that its home is a fake one that only it sees; the real home and the real
//! profile are never read. Nothing is imported: only the detection read is asked.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use massorca_desktop::service::{ServiceBridge, StoreState, StoreUpdate, connect_options};
use massorca_profile::Profile;
use massorca_server::{Service, ServiceConfig};
use tempfile::TempDir;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(60);
/// The profile home the child serves; unset in every other run of the child's test.
const SERVE_HOME: &str = "MASSORCA_IMPORT_DETECT_SERVE_HOME";
/// Every override or home variable the built-in kinds declare: a developer's own must never reach detection.
const VENDOR_ENV: [&str; 3] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"];
/// An obviously fake login file; detection reads metadata only, so it is never opened.
const FAKE_LOGIN: &str = r#"{"fake":true}"#;

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// The service, in a child process. It serves until its input closes, then stops cleanly.
#[tokio::test]
#[ignore = "the service the bridge test starts in a child process"]
async fn serve_until_the_parent_lets_go() {
    let Some(home) = std::env::var_os(SERVE_HOME).map(PathBuf::from) else {
        return;
    };
    let running = Service::start(ServiceConfig {
        profile: profile(&home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts");
    fs::write(home.join("serving"), "").expect("the ready marker");
    let _ = tokio::task::spawn_blocking(|| {
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut Vec::new())
    })
    .await;
    running.shutdown_handle().shutdown();
    let _ = timeout(WAIT, running.wait()).await;
}

/// A fake user home holding Claude's login files where detection looks, and the folders Windows insists on.
fn fake_user_home(root: &Path) -> PathBuf {
    let user = root.join("user");
    fs::create_dir_all(user.join(".claude")).expect("the fake Claude folder");
    fs::write(user.join(".claude").join(".credentials.json"), FAKE_LOGIN).expect("a fake login");
    fs::write(user.join(".claude.json"), FAKE_LOGIN).expect("a fake state file");
    for known in [["AppData", "Roaming"], ["AppData", "Local"]] {
        fs::create_dir_all(user.join(known[0]).join(known[1])).expect("a known folder");
    }
    user
}

/// The service child, given `user` as its only home.
fn spawn_service(profile_home: &Path, user: &Path) -> Child {
    let mut command = Command::new(std::env::current_exe().expect("the test binary"));
    command
        .args(["--exact", "serve_until_the_parent_lets_go", "--ignored"])
        .env(SERVE_HOME, profile_home)
        .env("HOME", user)
        .env("USERPROFILE", user)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in VENDOR_ENV {
        command.env_remove(name);
    }
    command.spawn().expect("the service child starts")
}

async fn serving(profile_home: &Path) {
    let marker = profile_home.join("serving");
    let started = Instant::now();
    while !marker.exists() {
        assert!(
            started.elapsed() < WAIT,
            "the service child did not start in time"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn the_service_detects_the_claude_folder_ready_in_its_own_home() {
    let root: TempDir = tempfile::tempdir().expect("a temporary folder");
    let user = fake_user_home(root.path());
    let profile_home = root.path().join("home");
    massorca_profile::create_home(&profile_home).expect("a private home");
    let mut service = spawn_service(&profile_home, &user);
    serving(&profile_home).await;

    let mut bridge = ServiceBridge::start(profile(&profile_home), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    let mut state = StoreState::default();
    let connected = timeout(WAIT, async {
        while !state.connection.is_connected() {
            let update: StoreUpdate = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(connected.is_ok(), "the bridge did not connect: {state:?}");

    let answer = handle
        .accounts_import_candidates("claude")
        .await
        .expect("the service offers detection");
    let claude = answer
        .candidates
        .iter()
        .find(|candidate| candidate.kind == "claude")
        .expect("Claude is detected");
    assert!(claude.ready, "the fake login files are found: {claude:?}");
    let folder = claude.folder.as_deref().expect("the detected folder");
    assert_eq!(
        Path::new(folder),
        user.join(".claude"),
        "the folder is below the service's fake home"
    );
    assert!(
        claude
            .files
            .iter()
            .any(|file| file.name == ".credentials.json" && file.required && file.found),
        "the required login file is found: {claude:?}"
    );

    drop(bridge);
    drop(service.stdin.take());
    let stopped = service.wait();
    assert!(stopped.is_ok(), "the service child stops once let go");
}
