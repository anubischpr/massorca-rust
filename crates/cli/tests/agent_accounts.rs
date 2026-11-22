//! `massorca agent accounts ...` (decision 0059): sign-in, import, the active account and per-project choices, checks,
//! removal, and the migration of 0056's single home. Every credential here is a fake file in a temporary directory, and
//! the fake agent only ever reads the account home the service gives it.

mod harness;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness::{
    ChildGuard, Sandbox, eventually, massorca_in, single_document, stdout_text, with_json,
};
use serde_json::{Value, json};

const FAKE_TOKEN: &str = r#"{"fake":true,"refresh":"fake-secret-value"}"#;
/// A token the fake agent's check reports as expired.
const EXPIRED_TOKEN: &str = r#"{"fake":true,"expired":true}"#;

/// Declares kind `fake` with its sign-in, its sign-out, and the check `check` runs (`None` declares no check).
fn declare_kind(sandbox: &Sandbox, check: Option<&[&str]>) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let mut accounts = json!({ "login": ["login"], "logout": ["logout"] });
    if let Some(args) = check {
        accounts["check"] = json!({
            "args": args,
            "signed_in_exit": 0,
            "expired_pattern": "expired",
            "identity_pattern": "signed in as (\\S+)"
        });
    }
    let fake = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
        "accounts": accounts
    });
    fs::write(dir.join("fake.json"), fake.to_string()).expect("write the fake kind");
}

/// Declares kind `fake` with the check the fake agent's `whoami` runs.
fn declare_fake(sandbox: &Sandbox) {
    declare_kind(sandbox, Some(&["whoami"]));
}

/// Starts the service with `env` set on it, and waits until it runs.
fn spawn_service_with(sandbox: &Sandbox, env: &[(&str, &str)]) -> ChildGuard {
    sandbox.track("default");
    let mut command = massorca_in(sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    sandbox.wait_for_running("default", true);
    guard
}

/// A directory beside the profile with the given files, each written with its content.
fn source_with(sandbox: &Sandbox, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let source = sandbox.home().join(name);
    fs::create_dir_all(&source).expect("create the source");
    for (file, content) in files {
        fs::write(source.join(file), content).expect("write a fake file");
    }
    source
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// Runs `--json <args>`, which must fail, and returns the error document's `error` object.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

/// The kind's accounts as listed.
fn accounts(sandbox: &Sandbox) -> Vec<Value> {
    sandbox.json_ok("default", &["agent", "accounts", "list", "fake"])["accounts"]
        .as_array()
        .expect("an accounts array")
        .clone()
}

/// The account with `label`, as listed.
fn account_labelled(sandbox: &Sandbox, label: &str) -> Value {
    accounts(sandbox)
        .into_iter()
        .find(|account| account["label"] == label)
        .unwrap_or_else(|| panic!("no account labelled {label}"))
}

/// The credential home of an account, under the profile.
fn account_home(sandbox: &Sandbox, account: &str) -> PathBuf {
    sandbox
        .profile_dir("default")
        .join("agents")
        .join("accounts")
        .join("fake")
        .join(account)
}

/// Imports `source` as a new account labelled `label`, and returns the result document.
fn import(sandbox: &Sandbox, source: &Path, label: &str) -> Value {
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
            label,
        ],
    )
}

/// A project named `name` with a workspace `main`; returns the project id and the workspace id.
fn project_with_main(sandbox: &Sandbox, name: &str) -> (String, String) {
    let folder = sandbox.home().join(name);
    fs::create_dir_all(&folder).expect("create the project folder");
    let added = sandbox.json_ok(
        "default",
        &["project", "add", path_text(&folder), "--folder"],
    );
    let workspace = sandbox.json_ok("default", &["workspace", "create", name, "main"]);
    (
        added["id"].as_str().expect("a project id").to_owned(),
        workspace["id"].as_str().expect("a workspace id").to_owned(),
    )
}

/// Starts a read-only fake agent in the workspace `workspace`, and returns its terminal id.
fn start_in(sandbox: &Sandbox, workspace: &str) -> String {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            workspace,
            "--read-only",
        ],
    );
    started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned()
}

/// Waits until the terminal's screen shows `line`.
fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().map(str::trim) == Some(line))
            })
            .then_some(())
    });
}

#[test]
fn add_with_a_good_login_keeps_a_signed_in_account_and_lists_it() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);

    let added = sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "work",
            "--no-attach",
        ],
    );
    assert!(added["terminal_id"].as_str().is_some(), "{added}");
    assert_eq!(added["account"]["label"], "work");

    eventually("the sign-in to keep the account", || {
        (account_labelled_or_none(&sandbox, "work")?["state"] == "signed_in").then_some(())
    });
    let kept = account_labelled(&sandbox, "work");
    assert_eq!(kept["identity"], "fake@example.com");
    assert_eq!(
        kept["active"], true,
        "the first kept account becomes active"
    );
}

/// The account labelled `label`, when it is listed.
fn account_labelled_or_none(sandbox: &Sandbox, label: &str) -> Option<Value> {
    accounts(sandbox)
        .into_iter()
        .find(|account| account["label"] == label)
}

#[test]
fn a_failed_login_leaves_no_account() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service_with(&sandbox, &[("FAKE_AGENT_LOGIN_FAIL", "1")]);

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "doomed",
            "--no-attach",
        ],
    );
    eventually("the failed sign-in to be discarded", || {
        accounts(&sandbox).is_empty().then_some(())
    });
    let homes = sandbox
        .profile_dir("default")
        .join("agents")
        .join("accounts")
        .join("fake");
    let left: Vec<String> = fs::read_dir(&homes)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(left.is_empty(), "no home is left behind: {left:?}");
}

#[test]
fn import_reports_imported_failed_or_not_found() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);

    let good = source_with(&sandbox, "good", &[("token.json", FAKE_TOKEN)]);
    let imported = import(&sandbox, &good, "good");
    assert_eq!(imported["result"], "imported", "{imported}");
    assert_eq!(imported["account"]["state"], "signed_in");

    let expired = source_with(&sandbox, "expired", &[("token.json", EXPIRED_TOKEN)]);
    let failed = import(&sandbox, &expired, "expired");
    assert_eq!(failed["result"], "failed", "{failed}");
    assert!(
        failed["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("not signed in")),
        "{failed}"
    );
    let text = failed.to_string();
    assert!(
        !text.contains("expired\":true"),
        "no contents in the result: {text}"
    );

    let missing = source_with(&sandbox, "missing", &[("history.jsonl", "not declared\n")]);
    let none = import(&sandbox, &missing, "none");
    assert_eq!(none["result"], "not_found", "{none}");

    assert_eq!(accounts(&sandbox).len(), 1, "only the good account is kept");
}

#[test]
fn activate_per_project_changes_which_account_a_started_agent_uses() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service_with(&sandbox, &[("FAKE_AGENT_PRINT_HOME", "1")]);
    let (_, notes_workspace) = project_with_main(&sandbox, "notes");
    let (docs, docs_workspace) = project_with_main(&sandbox, "docs");
    let first = source_with(&sandbox, "first", &[("token.json", FAKE_TOKEN)]);
    let second = source_with(&sandbox, "second", &[("token.json", FAKE_TOKEN)]);
    let first_id = import(&sandbox, &first, "first")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let second_id = import(&sandbox, &second, "second")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();

    sandbox.json_ok(
        "default",
        &["agent", "accounts", "activate", "fake", &first_id],
    );
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "activate",
            "fake",
            &second_id,
            "--project",
            &docs,
        ],
    );

    let in_notes = start_in(&sandbox, &notes_workspace);
    wait_for_line(&sandbox, &in_notes, &format!("fake-agent: home {first_id}"));
    let in_docs = start_in(&sandbox, &docs_workspace);
    wait_for_line(&sandbox, &in_docs, &format!("fake-agent: home {second_id}"));
}

#[test]
fn remove_is_refused_while_a_run_uses_the_account() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let (_, notes_workspace) = project_with_main(&sandbox, "notes");
    let source = source_with(&sandbox, "source", &[("token.json", FAKE_TOKEN)]);
    let id = import(&sandbox, &source, "work")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();

    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            &notes_workspace,
            "--read-only",
            "--prompt",
            "hold",
        ],
    );
    let terminal = started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned();
    wait_for_line(&sandbox, &terminal, "fake-agent: holding");

    let error = error_of(&sandbox, &["agent", "accounts", "remove", "fake", &id]);
    assert_eq!(error["code"], "agent.busy", "{error}");
    assert_eq!(accounts(&sandbox).len(), 1, "the account is kept");
}

#[test]
fn check_reports_expired_and_a_different_identity_as_conflict() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let source = source_with(
        &sandbox,
        "source",
        &[("token.json", r#"{"email":"fake@example.com"}"#)],
    );
    let id = import(&sandbox, &source, "work")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let token = account_home(&sandbox, &id).join("token.json");

    let signed_in = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(signed_in["state"], "signed_in", "{signed_in}");
    assert_eq!(signed_in["identity"], "fake@example.com");

    fs::write(&token, r#"{"email":"other@example.com"}"#).expect("write another identity");
    let conflict = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(conflict["state"], "conflict", "{conflict}");
    assert_eq!(
        conflict["identity"], "fake@example.com",
        "the recorded identity stays"
    );

    fs::write(&token, EXPIRED_TOKEN).expect("write an expired token");
    let expired = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(expired["state"], "expired", "{expired}");
}

#[test]
fn migration_turns_the_old_home_into_an_active_default_account() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let legacy = sandbox
        .profile_dir("default")
        .join("agents")
        .join("homes")
        .join("fake");
    fs::create_dir_all(&legacy).expect("create the 0056 home");
    fs::write(legacy.join("token.json"), FAKE_TOKEN).expect("write the fake token");
    // The move happens at a service start (decision 0059), so the home is in place before the service starts.
    let _service = spawn_service_with(&sandbox, &[("FAKE_AGENT_PRINT_HOME", "1")]);

    let listed = accounts(&sandbox);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["label"], "default");
    assert_eq!(listed[0]["state"], "migrated");
    assert_eq!(listed[0]["active"], true);
    assert!(!legacy.exists(), "nothing is left in homes/");

    let (_, workspace) = project_with_main(&sandbox, "notes");
    let terminal = start_in(&sandbox, &workspace);
    wait_for_line(&sandbox, &terminal, "fake-agent: credentials found");
    wait_for_line(
        &sandbox,
        &terminal,
        &format!("fake-agent: home {}", listed[0]["id"].as_str().unwrap()),
    );
}

#[test]
fn a_started_run_with_a_linked_account_home_is_refused() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let (_, workspace) = project_with_main(&sandbox, "notes");
    let source = source_with(&sandbox, "source", &[("token.json", FAKE_TOKEN)]);
    let id = import(&sandbox, &source, "work")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let home = account_home(&sandbox, &id);
    let elsewhere = sandbox.home().join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("create the other directory");
    fs::remove_dir_all(&home).expect("remove the account home");
    if let Err(error) = make_link(&elsewhere, &home) {
        let _ = writeln!(
            std::io::stderr(),
            "skipped: links cannot be made here ({error})"
        );
        return;
    }

    let error = error_of(
        &sandbox,
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            &workspace,
            "--read-only",
        ],
    );
    assert_eq!(error["code"], "agent.credentials_invalid", "{error}");
}

#[cfg(windows)]
fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(windows))]
fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[test]
fn the_accounts_output_names_no_secret() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let source = source_with(&sandbox, "source", &[("token.json", FAKE_TOKEN)]);
    let outcome = sandbox.run(
        "default",
        &[
            "agent",
            "accounts",
            "import",
            "fake",
            "--from",
            path_text(&source),
        ],
    );
    let text = stdout_text(&outcome.success());
    assert!(!text.contains("fake-secret-value"), "{text}");
}

/// Waits until the kind lists at least one account and returns the first one.
fn first_account(sandbox: &Sandbox) -> Value {
    eventually("an account to be listed", || {
        accounts(sandbox).into_iter().next()
    })
}

/// Waits until no account is listed.
fn no_accounts(sandbox: &Sandbox) {
    eventually("no account to be listed", || {
        accounts(sandbox).is_empty().then_some(())
    });
}

#[test]
fn a_hung_check_is_stopped_with_its_child_and_reports_unknown() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, Some(&["hang"]));
    let source = source_with(&sandbox, "source", &[("token.json", FAKE_TOKEN)]);

    let started = std::time::Instant::now();
    let imported = import(&sandbox, &source, "work");
    assert_eq!(imported["result"], "imported", "{imported}");
    let id = imported["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let checked = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(checked["state"], "unknown", "{checked}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(40),
        "two checks of 10 seconds each must not hang: {:?}",
        started.elapsed()
    );
}

#[test]
fn removing_an_imported_account_does_not_run_the_sign_out() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let marker = sandbox.home().join("logout-marker");
    let _service = spawn_service_with(
        &sandbox,
        &[("FAKE_AGENT_LOGOUT_MARKER", path_text(&marker))],
    );
    let source = source_with(&sandbox, "source", &[("token.json", FAKE_TOKEN)]);
    let imported = import(&sandbox, &source, "imported");
    let id = imported["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();

    sandbox.json_ok("default", &["agent", "accounts", "remove", "fake", &id]);
    assert!(!marker.exists(), "an imported login is not signed out");

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "signed",
            "--no-attach",
        ],
    );
    eventually("the signed-in account", || {
        (account_labelled_or_none(&sandbox, "signed")?["state"] == "signed_in").then_some(())
    });
    let signed = account_labelled(&sandbox, "signed");
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "remove",
            "fake",
            signed["id"].as_str().unwrap(),
        ],
    );
    assert!(
        marker.exists(),
        "a sign-in account is signed out when removed"
    );
}

#[test]
fn a_sign_in_whose_check_says_expired_is_discarded() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service_with(&sandbox, &[("FAKE_AGENT_LOGIN_EXPIRED", "1")]);

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "expired",
            "--no-attach",
        ],
    );
    no_accounts(&sandbox);
}

#[test]
fn a_sign_in_without_a_check_is_kept_as_unknown_not_signed_in() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, None);

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "plain",
            "--no-attach",
        ],
    );
    eventually("the sign-in to finish", || {
        (account_labelled_or_none(&sandbox, "plain")?["state"] != "needs_sign_in").then_some(())
    });
    assert_eq!(account_labelled(&sandbox, "plain")["state"], "unknown");
}

#[test]
fn a_sign_in_whose_check_hangs_is_kept_as_unknown_when_its_files_exist() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, Some(&["hang"]));

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "slow",
            "--no-attach",
        ],
    );
    eventually("the sign-in to finish after its check times out", || {
        (account_labelled_or_none(&sandbox, "slow")?["state"] == "unknown").then_some(())
    });
}

#[test]
fn a_pending_sign_in_refuses_removal_and_activation() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service_with(&sandbox, &[("FAKE_AGENT_LOGIN_HANG", "1")]);
    let added = sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "pending",
            "--no-attach",
        ],
    );
    let id = added["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();

    let removal = error_of(&sandbox, &["agent", "accounts", "remove", "fake", &id]);
    assert_eq!(removal["code"], "agent.busy", "{removal}");
    let activation = error_of(&sandbox, &["agent", "accounts", "activate", "fake", &id]);
    assert_eq!(activation["code"], "agent.busy", "{activation}");
    assert_eq!(accounts(&sandbox).len(), 1, "the pending account is kept");
}

#[test]
fn a_sign_in_left_unfinished_by_a_restart_is_decided_at_startup() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let first = spawn_service_with(&sandbox, &[("FAKE_AGENT_LOGIN_HANG", "1")]);
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            "fake",
            "--label",
            "orphan",
            "--no-attach",
        ],
    );
    assert_eq!(accounts(&sandbox).len(), 1);
    drop(first);
    sandbox.wait_for_running("default", false);

    let _service = spawn_service_with(&sandbox, &[]);
    no_accounts(&sandbox);
}

#[test]
fn an_orphan_account_home_is_adopted_as_migrated_at_startup() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let orphan = "0195c0de-0000-7000-8000-0000000000aa";
    let home = account_home(&sandbox, orphan);
    fs::create_dir_all(&home).expect("create the orphan home");
    fs::write(home.join("token.json"), FAKE_TOKEN).expect("write the orphan token");

    let _service = spawn_service_with(&sandbox, &[]);
    let adopted = first_account(&sandbox);
    assert_eq!(adopted["id"], orphan);
    assert_eq!(adopted["state"], "migrated");
    assert_eq!(adopted["active"], true, "no other account is active");
}

#[test]
fn listing_accounts_and_kinds_does_not_migrate_the_legacy_home() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service_with(&sandbox, &[]);
    let legacy = sandbox
        .profile_dir("default")
        .join("agents")
        .join("homes")
        .join("fake");
    fs::create_dir_all(&legacy).expect("create the 0056 home");
    fs::write(legacy.join("token.json"), FAKE_TOKEN).expect("write the fake token");

    sandbox.json_ok("default", &["agent", "kinds"]);
    assert!(
        accounts(&sandbox).is_empty(),
        "a read does not move the home"
    );
    assert!(
        legacy.exists(),
        "the 0056 home stays until a start or the next service"
    );
}

#[test]
fn a_label_with_control_characters_or_too_long_is_refused_as_invalid_params() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let long = "a".repeat(257);
    let control = "bad\u{7}label";
    for label in [long.as_str(), control] {
        let error = error_of(
            &sandbox,
            &[
                "agent",
                "accounts",
                "add",
                "fake",
                "--label",
                label,
                "--no-attach",
            ],
        );
        assert_eq!(error["code"], "request.invalid_params", "{error}");
    }
    assert!(
        accounts(&sandbox).is_empty(),
        "nothing is created for a refused label"
    );
}

/// Stops the running service and starts it again with `env`, as a restart does.
fn restart_service(sandbox: &Sandbox, env: &[(&str, &str)]) -> ChildGuard {
    sandbox.run("default", &["service", "stop"]).success();
    sandbox.wait_for_running("default", false);
    spawn_service_with(sandbox, env)
}

/// A login the vendor cannot be reached to confirm: the check exits without saying the login expired.
const OFFLINE_TOKEN: &str = r#"{"fake":true,"offline":true}"#;

#[test]
fn the_sweep_leaves_an_imported_account_in_needs_sign_in_alone() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let source = source_with(&sandbox, "source", &[("token.json", FAKE_TOKEN)]);
    let id = import(&sandbox, &source, "work")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let token = account_home(&sandbox, &id).join("token.json");
    fs::write(&token, OFFLINE_TOKEN).expect("write an offline token");
    let checked = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(checked["state"], "needs_sign_in", "{checked}");

    let _service = restart_service(&sandbox, &[]);
    let listed = account_labelled(&sandbox, "work");
    assert_eq!(listed["id"], id.as_str());
    assert_eq!(
        listed["state"], "needs_sign_in",
        "a state only, not a sign-in to decide"
    );
    assert!(
        token.is_file(),
        "a restart does not delete an imported login"
    );
}

#[test]
fn the_sweep_leaves_a_migrated_account_in_needs_sign_in_alone() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let legacy = sandbox
        .profile_dir("default")
        .join("agents")
        .join("homes")
        .join("fake");
    fs::create_dir_all(&legacy).expect("create the 0056 home");
    fs::write(legacy.join("token.json"), FAKE_TOKEN).expect("write the fake token");
    let _service = spawn_service_with(&sandbox, &[]);
    let migrated = first_account(&sandbox);
    assert_eq!(migrated["state"], "migrated");
    let id = migrated["id"].as_str().expect("an account id").to_owned();
    let token = account_home(&sandbox, &id).join("token.json");
    fs::write(&token, OFFLINE_TOKEN).expect("write an offline token");
    let checked = sandbox.json_ok("default", &["agent", "accounts", "check", "fake", &id]);
    assert_eq!(checked["state"], "needs_sign_in", "{checked}");

    let _service = restart_service(&sandbox, &[]);
    let listed = account_labelled(&sandbox, "default");
    assert_eq!(listed["state"], "needs_sign_in");
    assert!(
        token.is_file(),
        "a restart does not delete a migrated login"
    );
}

#[test]
fn a_removed_active_account_refuses_the_next_start_until_another_is_activated() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let (_, workspace) = project_with_main(&sandbox, "notes");
    let first = source_with(&sandbox, "first", &[("token.json", FAKE_TOKEN)]);
    let second = source_with(&sandbox, "second", &[("token.json", FAKE_TOKEN)]);
    let first_id = import(&sandbox, &first, "first")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    let second_id = import(&sandbox, &second, "second")["account"]["id"]
        .as_str()
        .expect("an account id")
        .to_owned();
    sandbox.json_ok(
        "default",
        &["agent", "accounts", "remove", "fake", &first_id],
    );

    let error = error_of(
        &sandbox,
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            &workspace,
            "--read-only",
        ],
    );
    assert_eq!(error["code"], "agent.credentials_invalid", "{error}");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|text| text.contains("was removed")),
        "{error}"
    );

    sandbox.json_ok(
        "default",
        &["agent", "accounts", "activate", "fake", &second_id],
    );
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            &workspace,
            "--read-only",
        ],
    );
    assert!(started["terminal_id"].as_str().is_some(), "{started}");
}

#[test]
fn a_kind_that_never_had_an_account_starts_on_the_usual_home() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let (_, workspace) = project_with_main(&sandbox, "notes");
    let terminal = start_in(&sandbox, &workspace);
    wait_for_line(&sandbox, &terminal, "fake-agent: credentials missing");
}
