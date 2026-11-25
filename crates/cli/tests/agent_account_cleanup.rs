//! A package removed with `--delete-agent-homes` takes the accounts of its agent kinds with it (decision 0098): their
//! homes and rows go, nothing is signed out, and another package's kind keeps its accounts. Every package here is a test
//! package from a temporary folder whose agent is the fake agent; every credential is a fake file.

mod extension_support;
mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use extension_support::{Host, Package};
use harness::{ChildGuard, eventually, massorca_in};
use serde_json::{Value, json};

const FAKE_TOKEN: &str = r#"{"fake":true,"refresh":"fake-secret-value"}"#;
/// The same login with other contents: the fake agent's check reports the same identity.
/// A token whose identity differs from the fake login's default one.
const IMPORTED_TOKEN: &str =
    r#"{"fake":true,"email":"imported@example.invalid","refresh":"fake-imported-value"}"#;
const FAKE_TOKEN_AGAIN: &str = r#"{"fake":true,"refresh":"another-fake-value"}"#;

/// A package whose agent kind is `acctagent`, run as the CLI `acctcli`.
const ACCOUNTS: Package = Package::new("acme.accounts", "acctagent", "acctcli");
/// A second package with its own kind, which a removal of [`ACCOUNTS`] must not touch.
const OTHER: Package = Package::new("acme.otheraccounts", "otheragent", "othercli");

/// Every override or home variable a built-in kind declares: a developer's own must never reach the service.
const BUILT_IN_VENDOR_ENV: [&str; 3] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"];

/// The declaration of `package`'s kind: its CLI is the fake agent, with a sign-in, a sign-out and a check that names
/// the login's identity.
fn kind_of(package: Package) -> Value {
    json!({
        "id": package.agent,
        "name": "Fake account agent",
        "program": [package.binary],
        "prompt": { "mode": "argv" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
        "accounts": {
            "login": ["login"],
            "logout": ["logout"],
            "check": {
                "args": ["whoami"],
                "signed_in_exit": 0,
                "identity_pattern": "signed in as (\\S+)"
            }
        }
    })
}

/// Puts the fake agent on the service's `PATH` as `package`'s CLI, and returns the package's `dir:` source.
fn write_package(host: &Host, package: Package) -> String {
    fs::copy(
        env!("CARGO_BIN_EXE_massorca-fake-agent"),
        host.cli_path(package.binary),
    )
    .expect("the fake agent is the package's CLI");
    host.write_dir_with_kind(package, &kind_of(package))
}

/// A user home inside the sandbox, with the known folders Windows resolves below `USERPROFILE`.
fn user_home(host: &Host) -> PathBuf {
    let home = host.sandbox.home().join("user");
    for known in [["AppData", "Roaming"], ["AppData", "Local"]] {
        fs::create_dir_all(home.join(known[0]).join(known[1])).expect("create a known folder");
    }
    home
}

/// Starts the service with the host's `PATH`, its home at a sandboxed user home, and no vendor variable; `marker` is where
/// the fake agent's sign-out would write.
fn spawn_service(host: &Host, marker: &Path) -> ChildGuard {
    let user = user_home(host);
    host.sandbox.prepare_profile("default");
    host.sandbox.track("default");
    let mut command = massorca_in(host.sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env("PATH", host.bin.path())
        .env("MASSORCA_VENDOR_PATH", "process")
        .env("HOME", &user)
        .env("USERPROFILE", &user)
        .env("FAKE_AGENT_LOGOUT_MARKER", marker)
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in BUILT_IN_VENDOR_ENV {
        command.env_remove(name);
    }
    // The per-user folders a vendor CLI or the service could resolve: all inside the sandboxed home.
    for (name, path) in [
        ("APPDATA", user.join("AppData").join("Roaming")),
        ("LOCALAPPDATA", user.join("AppData").join("Local")),
        ("XDG_CONFIG_HOME", user.join(".config")),
        ("XDG_DATA_HOME", user.join(".local").join("share")),
    ] {
        command.env(name, path);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    host.sandbox.wait_for_running("default", true);
    guard
}

/// A folder holding the fake token file.
fn source(host: &Host, name: &str, token: &str) -> PathBuf {
    let folder = host.packages.path().join(name);
    fs::create_dir_all(&folder).expect("create the source");
    fs::write(folder.join("token.json"), token).expect("write the fake token");
    folder
}

/// Imports `folder` into `kind`, and returns the result.
fn import(host: &Host, kind: &str, folder: &Path) -> Value {
    host.json(&[
        "agent",
        "accounts",
        "import",
        kind,
        "--from",
        folder.to_str().expect("a UTF-8 path"),
    ])
}

/// The accounts of `kind`, as listed.
fn accounts(host: &Host, kind: &str) -> Vec<Value> {
    host.json(&["agent", "accounts", "list", kind])["accounts"]
        .as_array()
        .expect("an accounts array")
        .clone()
}

/// `<profile>/agents/accounts/<kind>`.
fn accounts_dir(host: &Host, kind: &str) -> PathBuf {
    host.sandbox
        .profile_dir("default")
        .join("agents")
        .join("accounts")
        .join(kind)
}

/// The names in a folder; none when it is gone.
fn names_in(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn removing_a_package_with_its_agent_homes_deletes_its_accounts_and_a_new_import_makes_one() {
    let host = Host::new();
    let marker = host.packages.path().join("logout-marker");
    let _service = spawn_service(&host, &marker);
    let package = write_package(&host, ACCOUNTS);
    host.json(&["extension", "install", &package, "--yes"]);

    // Its own identity, so the sign-in below (the fake login's default identity) stays a second account.
    let imported = import(&host, "acctagent", &source(&host, "first", IMPORTED_TOKEN));
    assert_eq!(imported["result"], "imported", "{imported}");
    let old = imported["account"]["id"].as_str().unwrap().to_owned();
    // A second account, made by a MassOrca sign-in: its removal would run the sign-out, which this cleanup never runs.
    host.json(&[
        "agent",
        "accounts",
        "add",
        "acctagent",
        "--label",
        "signed",
        "--no-attach",
    ]);
    // Listed is not decided: the sign-in's account stays busy until its check settles it, so wait for both to be signed in.
    eventually("the sign-in to be decided", || {
        let listed = accounts(&host, "acctagent");
        (listed.len() == 2 && listed.iter().all(|account| account["state"] == "signed_in"))
            .then_some(())
    });
    let homes = accounts_dir(&host, "acctagent");
    assert!(homes.join(&old).is_dir(), "the imported account has a home");
    // What an interrupted replace or import leaves: hidden folders that may hold a copy of a token.
    let aside = homes.join(format!(".{old}.old-0123"));
    fs::create_dir_all(&aside).unwrap();
    fs::write(aside.join("token.json"), FAKE_TOKEN).unwrap();
    fs::create_dir_all(homes.join(format!(".{old}.import-0456"))).unwrap();

    let removed = host.json(&[
        "extension",
        "remove",
        "acme.accounts",
        "--delete-agent-homes",
    ]);
    assert_eq!(removed["failures"], json!([]), "{removed}");
    assert!(
        !homes.exists(),
        "the kind's account folder and its leftovers are gone: {:?}",
        names_in(&homes)
    );
    assert!(
        accounts(&host, "acctagent").is_empty(),
        "no account row is left"
    );
    assert!(!marker.exists(), "nothing is signed out");

    host.json(&["extension", "install", &package, "--yes"]);
    let again = import(
        &host,
        "acctagent",
        &source(&host, "again", FAKE_TOKEN_AGAIN),
    );
    assert_eq!(again["result"], "imported", "{again}");
    let listed = accounts(&host, "acctagent");
    assert_eq!(listed.len(), 1, "exactly one account: {listed:?}");
    assert_ne!(listed[0]["id"], old.as_str(), "a new account");
    assert!(!homes.join(&old).exists(), "the old account's home is gone");
}

#[test]
fn removing_a_package_without_its_agent_homes_keeps_its_accounts() {
    let host = Host::new();
    let marker = host.packages.path().join("logout-marker");
    let _service = spawn_service(&host, &marker);
    let package = write_package(&host, ACCOUNTS);
    host.json(&["extension", "install", &package, "--yes"]);
    let imported = import(&host, "acctagent", &source(&host, "first", FAKE_TOKEN));
    let id = imported["account"]["id"].as_str().unwrap().to_owned();

    host.json(&["extension", "remove", "acme.accounts"]);

    assert!(accounts_dir(&host, "acctagent").join(&id).is_dir());
    assert_eq!(accounts(&host, "acctagent").len(), 1);
}

#[test]
fn the_cleanup_leaves_another_packages_kind_and_its_accounts_alone() {
    let host = Host::new();
    let marker = host.packages.path().join("logout-marker");
    let _service = spawn_service(&host, &marker);
    let first = write_package(&host, ACCOUNTS);
    let second = write_package(&host, OTHER);
    host.json(&["extension", "install", &first, "--yes"]);
    host.json(&["extension", "install", &second, "--yes"]);
    import(&host, "acctagent", &source(&host, "first", FAKE_TOKEN));
    let kept = import(&host, "otheragent", &source(&host, "other", FAKE_TOKEN));
    let kept_id = kept["account"]["id"].as_str().unwrap().to_owned();

    let removed = host.json(&[
        "extension",
        "remove",
        "acme.accounts",
        "--delete-agent-homes",
    ]);

    assert_eq!(removed["failures"], json!([]), "{removed}");
    assert!(accounts(&host, "acctagent").is_empty());
    let other = accounts(&host, "otheragent");
    assert_eq!(other.len(), 1, "{other:?}");
    assert_eq!(other[0]["id"], kept_id.as_str());
    assert_eq!(other[0]["active"], true, "it stays active");
    assert!(accounts_dir(&host, "otheragent").join(&kept_id).is_dir());
}
