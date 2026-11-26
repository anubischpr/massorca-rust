//! Removing a package with its agent homes takes its accounts with it, through the desktop bridge against a real service
//! that runs in a child process of this test binary (decision 0098). The profile is throwaway, the user home is fake and
//! holds fake vendor sign-in files, and a fake `claude` is alone on the service's `PATH`. Nothing is signed out.
#![cfg(windows)]

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::agents::{
    AccountState, AccountSummary, AccountsImportParams, ImportResult,
};
use massorca_protocol::methods::extensions::{
    ExtensionInstallParams, ExtensionInstalled, ExtensionRemoveParams,
};
use massorca_server::{Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
const PACKAGE: &str = "massorca.claude";
const SOURCE: &str = "catalogue:massorca.claude";
const KIND: &str = "claude";
/// The identity the fake `claude` reports: its email and organization.
const IDENTITY: &str = "fake.person@example.test / Fake Org";
const PROFILE_VAR: &str = "MASSORCA_ACCOUNT_REMOVAL_PROFILE";
const CHILD_TEST: &str = "service_child_for_account_removal";
/// Fake sign-in contents; no bridge state may ever show the secret.
const CREDENTIALS: &str = r#"{"fake":true,"refresh":"account-removal-fake-secret-4d1e"}"#;
const VENDOR_STATE: &str = r#"{"fake":true,"theme":"dark"}"#;
const SECRET: &str = "account-removal-fake-secret";

/// The fake `claude`: it logs its arguments, and `auth status --json` reports signed in while the login file is in
/// `CLAUDE_CONFIG_DIR`, the account's own home.
const FAKE_CLAUDE: &str = r#"@echo off
echo %*>>"%~dp0calls.log"
if not "%~1"=="auth" goto unexpected
if not "%~2"=="status" goto unexpected
if not "%~3"=="--json" goto unexpected
if exist "%CLAUDE_CONFIG_DIR%\.credentials.json" goto signed_in
echo {"loggedIn":false}
exit /b 1
:signed_in
echo {"loggedIn":true,"email":"fake.person@example.test","orgName":"Fake Org"}
exit /b 0
:unexpected
echo fake claude: unexpected arguments 1>&2
exit /b 2
"#;

/// A throwaway profile, a fake user home with fake vendor sign-in files, and a folder holding the fake `claude` alone.
struct Fixture {
    temp: TempDir,
    profile: PathBuf,
    user: PathBuf,
    bin: PathBuf,
    /// The vendor's `.credentials.json` and `.claude.json` in the fake home.
    vendor: [PathBuf; 2],
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = temp.path().join("profile");
    massorca_profile::create_home(&profile).expect("a private profile home");
    let user = temp.path().join("user");
    for known in [["AppData", "Roaming"], ["AppData", "Local"]] {
        fs::create_dir_all(user.join(known[0]).join(known[1])).expect("a known folder");
    }
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("the fake CLI folder");
    fs::write(bin.join("claude.cmd"), FAKE_CLAUDE.replace('\n', "\r\n")).expect("the fake CLI");
    let vendor_folder = user.join(".claude");
    fs::create_dir_all(&vendor_folder).expect("the vendor folder");
    let vendor = [
        vendor_folder.join(".credentials.json"),
        user.join(".claude.json"),
    ];
    fs::write(&vendor[0], CREDENTIALS).expect("the fake credentials");
    fs::write(&vendor[1], VENDOR_STATE).expect("the fake vendor state");
    Fixture {
        temp,
        profile,
        user,
        bin,
        vendor,
    }
}

/// The service child, started from this test binary. Closing its stdin makes it stop its service and exit.
struct ServiceChild {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl ServiceChild {
    fn stop(mut self) {
        drop(self.stdin.take());
        let started = Instant::now();
        while started.elapsed() < WAIT {
            if let Some(status) = self.child.try_wait().expect("the child can be polled") {
                assert!(status.success(), "the service child failed: {status}");
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the service child did not stop in time");
    }
}

impl Drop for ServiceChild {
    /// Kills the child when the test ended before stopping it; a finished child ignores this.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts the service child with only the environment the fake user needs: no real home, no vendor variable, and the
/// fake `claude` alone on `PATH`.
fn spawn_service(fixture: &Fixture) -> ServiceChild {
    let tmp = fixture.temp.path().join("tmp");
    fs::create_dir_all(&tmp).expect("a temporary folder for the service");
    let log = File::create(fixture.temp.path().join("service.log")).expect("the service log");
    let pathext = std::env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    let mut command = massorca_process::command(std::env::current_exe().expect("the test binary"));
    command
        .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
        .env_clear()
        .env("PATH", &fixture.bin)
        .env("PATHEXT", pathext)
        .env("HOME", &fixture.user)
        .env("USERPROFILE", &fixture.user)
        .env("APPDATA", fixture.user.join("AppData").join("Roaming"))
        .env("LOCALAPPDATA", fixture.user.join("AppData").join("Local"))
        .env("TEMP", &tmp)
        .env("TMP", &tmp)
        .env("MASSORCA_VENDOR_PATH", "process")
        .env(PROFILE_VAR, &fixture.profile)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log.try_clone().expect("the log can be shared")))
        .stderr(Stdio::from(log));
    // The Windows folders a child needs to start; none of them is a home or a profile.
    for name in ["SystemRoot", "SystemDrive", "windir", "ComSpec"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().expect("the service child starts");
    let stdin = child.stdin.take();
    ServiceChild { child, stdin }
}

/// The service the bridge talks to. It runs only when the parent test starts it, and stops when stdin closes.
#[tokio::test]
#[ignore = "the service child of removing_a_package_takes_its_accounts_through_the_bridge"]
async fn service_child_for_account_removal() {
    let home = PathBuf::from(std::env::var_os(PROFILE_VAR).expect("the profile home is named"));
    let service = Service::start(ServiceConfig {
        profile: profile(&home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts");
    // The parent closes stdin when it is done, which ends the service.
    let _ = tokio::task::spawn_blocking(|| std::io::stdin().read_to_end(&mut Vec::new())).await;
    service.shutdown_handle().shutdown();
    timeout(WAIT, service.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// The desktop's store, folded from the bridge's updates.
struct Store {
    updates: UnboundedReceiver<StoreUpdate>,
    state: StoreState,
}

impl Store {
    /// Folds the bridge's updates until `done` holds.
    async fn until(&mut self, what: &str, done: impl Fn(&StoreState) -> bool) {
        let folded = timeout(WAIT, async {
            while !done(&self.state) {
                let update = self.updates.recv().await.expect("the bridge is alive");
                self.state.apply(update, Instant::now());
            }
        })
        .await;
        assert!(
            folded.is_ok(),
            "timed out waiting for {what}: {:?}",
            self.state
        );
    }
}

fn package_listed(state: &StoreState) -> bool {
    state
        .extensions
        .as_ref()
        .and_then(|read| read.installed.as_ref().ok())
        .is_some_and(|items| items.iter().any(|item| item.id == PACKAGE))
}

/// The accounts of the kind the store holds; none when the kind is not listed.
fn claude_accounts(state: &StoreState) -> Vec<AccountSummary> {
    state
        .accounts
        .get(KIND)
        .and_then(|listed| listed.as_ref().ok())
        .cloned()
        .unwrap_or_default()
}

/// Reviews the catalogue package, then installs it with the reviewed consent, as the Marketplace does.
async fn install_claude(handle: &ServiceHandle) -> ExtensionInstalled {
    let review = handle
        .inspect_extension(SOURCE.to_owned())
        .await
        .expect("the review is read");
    assert_eq!(review.id, PACKAGE);
    handle
        .install_extension(ExtensionInstallParams {
            source: SOURCE.to_owned(),
            consent: Some(review.consent),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect("the package installs")
}

fn vendor_bytes(fixture: &Fixture) -> Vec<Vec<u8>> {
    fixture
        .vendor
        .iter()
        .map(|path| fs::read(path).expect("the vendor's sign-in file is there"))
        .collect()
}

#[tokio::test]
async fn removing_a_package_takes_its_accounts_through_the_bridge_and_leaves_the_vendor_login() {
    let fixture = fixture();
    let vendor_before = vendor_bytes(&fixture);
    let child = spawn_service(&fixture);
    let mut bridge = ServiceBridge::start(profile(&fixture.profile), connect_options(None))
        .expect("the bridge starts");
    let mut store = Store {
        updates: bridge.take_updates().expect("the updates are there"),
        state: StoreState::default(),
    };
    let handle = bridge.handle();
    store
        .until("the service and the extension lists", |s| {
            s.connection.is_connected() && s.extensions.is_some()
        })
        .await;
    let client = Client::connect(
        &profile(&fixture.profile),
        ConnectOptions::new(PeerInfo { ..base_peer_info() }),
    )
    .await
    .expect("a client connects");
    let profile_dir = profile(&fixture.profile).dir().to_path_buf();
    let accounts_dir = profile_dir.join("agents").join("accounts").join(KIND);
    let homes_dir = profile_dir.join("agents").join("homes").join(KIND);

    // Install with the fake CLI already on PATH: MassOrca detects it and does not install it.
    let installed = install_claude(&handle).await;
    let cli = installed
        .cli
        .iter()
        .find(|cli| cli.binary == "claude")
        .expect("the CLI is recorded");
    assert!(
        cli.detected_before_install && !cli.installed_by_package,
        "{cli:?}"
    );
    store.until("the package", package_listed).await;

    // Import the vendor's own sign-in files, with no folder given.
    let imported = handle
        .accounts_import(AccountsImportParams {
            kind: KIND.to_owned(),
            from: None,
            label: Some("work".to_owned()),
        })
        .await
        .expect("the import answers");
    assert_eq!(imported.result, ImportResult::Imported, "{imported:?}");
    let account = imported.account.expect("the imported account");

    // The account is listed, signed in by the kind's check, and has its folder.
    store
        .until("the account signed in", |s| {
            claude_accounts(s)
                .iter()
                .any(|listed| listed.id == account.id && listed.state == AccountState::SignedIn)
        })
        .await;
    let listed = claude_accounts(&store.state);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].label, "work");
    assert_eq!(listed[0].identity.as_deref(), Some(IDENTITY));
    assert!(accounts_dir.join(&account.id).is_dir(), "no account folder");

    // The kind's 0056 home, which the removal deletes too. The import does not make it, so this test makes it, with a
    // fake login file, and checks it exists before the removal.
    fs::create_dir_all(&homes_dir).expect("the kind's home");
    fs::write(homes_dir.join(".credentials.json"), CREDENTIALS).expect("a fake file in the home");
    assert!(homes_dir.is_dir(), "no home before removal");

    // Remove the package the way the Marketplace does, with its agent homes.
    let removed = handle
        .remove_extension(ExtensionRemoveParams {
            id: PACKAGE.to_owned(),
            delete_agent_homes: true,
            ..Default::default()
        })
        .await
        .expect("the removal answers");
    assert!(removed.failures.is_empty(), "{removed:?}");
    assert!(
        fixture.bin.join("claude.cmd").exists(),
        "the CLI MassOrca did not install stays"
    );

    // The account leaves the list, and its folders are deleted.
    store
        .until("the account to leave the list", |s| {
            claude_accounts(s).is_empty()
        })
        .await;
    assert!(!accounts_dir.exists(), "the account folder is deleted");
    assert!(!homes_dir.exists(), "the kind's home is deleted");

    // The vendor's sign-in files are byte-identical, and the vendor CLI was never asked to sign out.
    assert_eq!(
        vendor_bytes(&fixture),
        vendor_before,
        "a vendor sign-in file changed"
    );
    let calls = fs::read_to_string(fixture.bin.join("calls.log")).unwrap_or_default();
    assert!(
        calls.contains("auth status --json") && !calls.contains("logout"),
        "unexpected vendor calls: {calls}"
    );
    assert!(
        !format!("{:?}", store.state).contains(SECRET),
        "a login file's content came back"
    );

    // Reinstalling the package shows no account from before, in the store and at the service.
    install_claude(&handle).await;
    store
        .until("the reinstalled kind with no accounts", |s| {
            s.accounts
                .get(KIND)
                .is_some_and(|listed| listed.as_ref().is_ok_and(|accounts| accounts.is_empty()))
        })
        .await;
    let relisted = client
        .agent_accounts_list(KIND)
        .await
        .expect("the service lists the kind")
        .accounts;
    assert!(relisted.is_empty(), "{relisted:?}");
    assert!(!accounts_dir.exists(), "the old account folder came back");

    drop(client);
    drop(bridge);
    child.stop();
}

/// Every `PeerInfo` field at its usual value; tests override the fields they vary.
fn base_peer_info() -> massorca_protocol::PeerInfo {
    massorca_protocol::PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}
