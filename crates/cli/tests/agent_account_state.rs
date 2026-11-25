//! An account home's state file (decision 0099): built from the source's kept keys and set keys on import, never copied
//! raw, and repaired before each check and launch. Every file here is a fake one in a temporary directory; the service
//! runs with its home in the sandbox and without any vendor override variable.

mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, massorca_in};
use serde_json::{Value, json};

const FAKE_TOKEN: &str = r#"{"fake":true}"#;
/// The fake vendor's override variable, moving where both files are looked for.
const FAKE_VENDOR_ENV: &str = "FAKE_VENDOR_DIR";
/// Every override or home variable a built-in kind declares: a developer's own must never reach the service.
const BUILT_IN_VENDOR_ENV: [&str; 3] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"];

/// Declares kind `fake` with a required `token.json` in `{home}/.fakevendor` and an optional state file `state.json`
/// in `{home}`, built from `account` and `theme` with `onboarded` set to true.
fn declare_with_state(sandbox: &Sandbox) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let fake = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "credentials": {
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.json", "state.json"],
            "optional": ["state.json"],
            "sources": [
                { "file": "token.json", "env": FAKE_VENDOR_ENV, "dir": "{home}/.fakevendor" },
                { "file": "state.json", "env": FAKE_VENDOR_ENV, "dir": "{home}" }
            ],
            "state": {
                "file": "state.json",
                "keep": ["account", "theme"],
                "set": { "onboarded": true },
                "max_source_bytes": 4 * 1024 * 1024
            }
        },
        "accounts": {
            "login": ["login"],
            "check": {
                "args": ["whoami"],
                "signed_in_exit": 0,
                "expired_pattern": "expired",
                "identity_pattern": "signed in as (\\S+)"
            }
        }
    });
    fs::write(dir.join("fake.json"), fake.to_string()).expect("write the fake kind");
}

/// A fake user home for the service, with the fake vendor's folder.
fn fake_user_home(sandbox: &Sandbox) -> PathBuf {
    let home = sandbox.home().join("user");
    fs::create_dir_all(home.join(".fakevendor")).expect("create the fake user home");
    // Windows resolves the service's own known folders below `USERPROFILE`, and refuses them when they are absent.
    for known in [["AppData", "Roaming"], ["AppData", "Local"]] {
        fs::create_dir_all(home.join(known[0]).join(known[1])).expect("create a known folder");
    }
    home
}

/// Starts the service with its home at `user_home`, no vendor variable, and the fake agent told to print whether its
/// state file holds `onboarded`.
fn spawn_service_in_home(sandbox: &Sandbox, user_home: &Path) -> ChildGuard {
    sandbox.track("default");
    let mut command = massorca_in(sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env_remove("FAKE_AGENT_HOME")
        .env_remove(FAKE_VENDOR_ENV)
        .env("HOME", user_home)
        .env("USERPROFILE", user_home)
        .env("FAKE_AGENT_PRINT_STATE", "state.json:onboarded")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in BUILT_IN_VENDOR_ENV {
        command.env_remove(name);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    sandbox.wait_for_running("default", true);
    guard
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// A source folder holding a token and the state file `state`.
fn source_with_state(sandbox: &Sandbox, state: &str) -> PathBuf {
    let source = sandbox.home().join("source");
    fs::create_dir_all(&source).expect("create the source");
    fs::write(source.join("token.json"), FAKE_TOKEN).expect("write the fake token");
    fs::write(source.join("state.json"), state).expect("write the fake state");
    source
}

/// A fake source state file with keys the import must never copy.
fn rich_state() -> Value {
    json!({
        "account": { "email": "fake@example.com" },
        "theme": "dark",
        "projects": { "/fake/project": { "history": ["fake prompt"] } },
        "onboarded": false,
        "userID": "fake-user"
    })
}

fn import_from(sandbox: &Sandbox, source: &Path) -> Value {
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "import",
            "fake",
            "--from",
            path_text(source),
            "--label",
            "work",
        ],
    )
}

fn account_home(sandbox: &Sandbox, account: &str) -> PathBuf {
    sandbox
        .profile_dir("default")
        .join("agents")
        .join("accounts")
        .join("fake")
        .join(account)
}

fn state_in(home: &Path) -> Value {
    serde_json::from_slice(&fs::read(home.join("state.json")).expect("the state file exists"))
        .expect("the state file is JSON")
}

/// Imports the rich state from a folder and returns the account id and its home.
fn imported_account(sandbox: &Sandbox) -> (String, PathBuf) {
    let source = source_with_state(sandbox, &rich_state().to_string());
    let imported = import_from(sandbox, &source);
    assert_eq!(imported["result"], "imported", "{imported}");
    let id = imported["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let home = account_home(sandbox, &id);
    (id, home)
}

#[test]
fn an_import_from_a_folder_builds_the_state_file_from_its_kept_and_set_keys_only() {
    let sandbox = Sandbox::new();
    declare_with_state(&sandbox);
    let user = fake_user_home(&sandbox);
    let _service = spawn_service_in_home(&sandbox, &user);

    let (_, home) = imported_account(&sandbox);
    assert_eq!(
        state_in(&home),
        json!({ "account": { "email": "fake@example.com" }, "theme": "dark", "onboarded": true })
    );
}

#[test]
fn an_import_from_detected_sources_reads_a_state_file_over_one_mib_within_its_cap() {
    let sandbox = Sandbox::new();
    declare_with_state(&sandbox);
    let user = fake_user_home(&sandbox);
    fs::write(user.join(".fakevendor").join("token.json"), FAKE_TOKEN).unwrap();
    let mut state = rich_state();
    state["projects"]["/fake/big"] = json!("x".repeat(2 * 1024 * 1024));
    fs::write(user.join("state.json"), state.to_string()).unwrap();
    let _service = spawn_service_in_home(&sandbox, &user);

    let shown = sandbox.json_ok("default", &["agent", "accounts", "candidates", "fake"]);
    let file = &shown["candidates"][0]["files"][1];
    assert_eq!(file["name"], "state.json", "{shown}");
    assert_eq!(file["found"], true, "{shown}");

    let imported = sandbox.json_ok("default", &["agent", "accounts", "import", "fake"]);
    assert_eq!(imported["result"], "imported", "{imported}");
    assert!(imported.get("reason").is_none(), "{imported}");
    let home = account_home(&sandbox, imported["account"]["id"].as_str().unwrap());
    assert_eq!(
        state_in(&home),
        json!({ "account": { "email": "fake@example.com" }, "theme": "dark", "onboarded": true })
    );
}

#[test]
fn a_state_source_that_is_not_an_object_is_imported_with_the_set_keys_and_says_so() {
    let sandbox = Sandbox::new();
    declare_with_state(&sandbox);
    let user = fake_user_home(&sandbox);
    let _service = spawn_service_in_home(&sandbox, &user);

    let source = source_with_state(&sandbox, "[\"not an object\"]");
    let imported = import_from(&sandbox, &source);
    assert_eq!(imported["result"], "imported", "{imported}");
    let reason = imported["reason"].as_str().expect("the import says why");
    assert!(
        reason.contains("state.json") && reason.contains("not a JSON object"),
        "{reason}"
    );
    let home = account_home(&sandbox, imported["account"]["id"].as_str().unwrap());
    assert_eq!(state_in(&home), json!({ "onboarded": true }));
}

#[test]
fn a_check_repairs_a_missing_or_differing_state_file_and_keeps_its_other_keys() {
    let sandbox = Sandbox::new();
    declare_with_state(&sandbox);
    let user = fake_user_home(&sandbox);
    let _service = spawn_service_in_home(&sandbox, &user);
    let (id, home) = imported_account(&sandbox);

    fs::remove_file(home.join("state.json")).unwrap();
    let checked = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(checked["state"], "signed_in", "{checked}");
    assert_eq!(state_in(&home), json!({ "onboarded": true }));

    fs::write(
        home.join("state.json"),
        json!({ "onboarded": false, "theme": "light" }).to_string(),
    )
    .unwrap();
    sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(
        state_in(&home),
        json!({ "onboarded": true, "theme": "light" })
    );
}

#[test]
fn a_check_reports_needs_sign_in_and_leaves_a_state_file_that_is_not_an_object() {
    let sandbox = Sandbox::new();
    declare_with_state(&sandbox);
    let user = fake_user_home(&sandbox);
    let _service = spawn_service_in_home(&sandbox, &user);
    let (id, home) = imported_account(&sandbox);

    fs::write(home.join("state.json"), "[1]").unwrap();
    let checked = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(checked["state"], "needs_sign_in", "{checked}");
    assert_eq!(fs::read_to_string(home.join("state.json")).unwrap(), "[1]");
}

#[test]
fn a_session_start_repairs_the_state_file_before_the_agent_reads_it() {
    let sandbox = Sandbox::new();
    declare_with_state(&sandbox);
    let user = fake_user_home(&sandbox);
    let _service = spawn_service_in_home(&sandbox, &user);
    let (_, home) = imported_account(&sandbox);
    fs::remove_file(home.join("state.json")).unwrap();

    let folder = sandbox.home().join("notes");
    fs::create_dir_all(&folder).unwrap();
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&folder), "--folder"],
    );
    let workspace = sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            workspace["id"].as_str().unwrap(),
            "--read-only",
        ],
    );
    let terminal = started["terminal_id"].as_str().expect("a terminal id");
    eventually("the agent to see its state file", || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines.iter().any(|line| {
                    line.as_str().map(str::trim) == Some("fake-agent: state onboarded present")
                })
            })
            .then_some(())
    });
    assert_eq!(state_in(&home), json!({ "onboarded": true }));
}
