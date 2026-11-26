//! The first-prompt rename (decision 0118): the service names a generated workspace after the first prompt of its first
//! session, with the title the kind's one-shot form writes (decision 0167), and renames its branch through
//! `workspace.rename` (decision 0111). The fake agent answers the one-shot runs with the `--fake-mode` its kind declares;
//! the report file its run writes shows whether a title was asked for. The service logs each outcome, and the tests wait
//! for those lines instead of sleeping.

mod harness;

use std::fs;
use std::path::PathBuf;

use harness::{Sandbox, eventually, git_repo, with_json};
use serde_json::{Value, json};

/// A fake token: the fake agent's check finds it signed in.
const FAKE_TOKEN: &str = r#"{"fake":true,"refresh":"fake-secret-value"}"#;
/// Where the fake one-shot run writes its report.
const REPORT_ENV: &str = "FAKE_AGENT_ONESHOT_REPORT";
/// The setting that turns the rename off (decision 0118).
const SETTING: &str = "workspaces.rename_from_first_prompt";
/// Generous upper bound for `--until` and the waits on the log.
const UNTIL_SECS: &str = "60";

/// The status table of the fake kind: start is idle, a prompt works, a stop is done.
fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": {
            "start": "idle", "prompt": "working", "ask": "waiting",
            "stop": "done", "fail": "done:failure"
        },
        "result_field": "result"
    })
}

/// Declares kind `fake`, which runs the fake agent, with its one-shot form answering with `mode`.
fn declare_kind(sandbox: &Sandbox, mode: &str) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": status(),
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
        "accounts": {
            "login": ["login"],
            "logout": ["logout"],
            "check": {
                "args": ["whoami"],
                "signed_in_exit": 0,
                "expired_pattern": "expired",
                "identity_pattern": "signed in as (\\S+)"
            }
        },
        "oneshot": {
            "args": ["-p", "{prompt}", "--output-format", "text", "--fake-mode", mode]
        }
    });
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
}

/// Imports one fake account of kind `fake` and makes it the kind's active account, which a title run uses.
fn signed_in_account(sandbox: &Sandbox) {
    let source = sandbox.home().join("token-source");
    fs::create_dir_all(&source).expect("create the token folder");
    fs::write(source.join("token.json"), FAKE_TOKEN).expect("write the fake token");
    let source = source.to_str().expect("a UTF-8 path");
    let imported = sandbox.json_ok(
        "default",
        &[
            "agent", "accounts", "import", "fake", "--from", source, "--label", "work",
        ],
    );
    let id = imported["account"]["id"]
        .as_str()
        .expect("the import names the account")
        .to_owned();
    sandbox.json_ok("default", &["agent", "accounts", "activate", "fake", &id]);
}

/// A git project with a worktree root, the one a generated workspace is created in. Returns the repository's folder.
fn repo_project(sandbox: &Sandbox) -> PathBuf {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let root = sandbox.home().join("ws");
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = root.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo_text.as_str(),
            "--workspaces-root",
            root_text.as_str(),
        ],
    );
    repo
}

/// A sandbox with the fake kind answering its one-shot form with `mode`, a signed-in account, a git project and the
/// one-shot report in the sandbox's home.
fn sandbox(mode: &str) -> Sandbox {
    let sandbox = Sandbox::new();
    let report = sandbox.home().join("oneshot-report.json");
    sandbox.set_env(REPORT_ENV, report.to_str().expect("a UTF-8 path"));
    declare_kind(&sandbox, mode);
    signed_in_account(&sandbox);
    repo_project(&sandbox);
    sandbox
}

/// The one-shot report's path in `sandbox`.
fn report_of(sandbox: &Sandbox) -> PathBuf {
    sandbox.home().join("oneshot-report.json")
}

/// Starts a session of kind `fake` in workspace `workspace` and waits until its agent is ready for a prompt.
fn start_session(sandbox: &Sandbox, workspace: &str) -> String {
    start_session_with(sandbox, workspace, &[])
}

/// [`start_session`] with `extra` arguments after the workspace.
fn start_session_with(sandbox: &Sandbox, workspace: &str, extra: &[&str]) -> String {
    let mut args = vec!["agent", "start", "fake", "--workspace", workspace];
    args.extend_from_slice(extra);
    let record = sandbox.json_ok("default", &args);
    let session = record["id"].as_str().expect("session id").to_owned();
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session.as_str(),
            "--until",
            "idle",
            "--timeout",
            UNTIL_SECS,
        ],
    );
    session
}

/// Types `text` into `session` at once, ahead of the agent's readiness.
fn prompt(sandbox: &Sandbox, session: &str, text: &str) {
    sandbox.json_ok("default", &["agent", "prompt", session, text, "--force"]);
}

/// The service log's tail, once it holds `line`.
fn log_with(sandbox: &Sandbox, line: &str) -> String {
    eventually(&format!("`{line}` in the service log"), || {
        let log = sandbox.service_log_tail("default");
        log.contains(line).then_some(log)
    })
}

/// The workspace `id` as the service shows it now.
fn workspace(sandbox: &Sandbox, id: &str) -> Value {
    sandbox.json_ok("default", &["workspace", "show", id])
}

/// Creates a worktree of the repository project with `args` after its project and returns the workspace.
fn create(sandbox: &Sandbox, args: &[&str]) -> Value {
    let mut all = vec!["workspace", "create", "repo"];
    all.extend_from_slice(args);
    sandbox.json_ok("default", &all)
}

#[test]
fn the_first_prompt_names_a_generated_workspace_and_renames_its_branch() {
    let sandbox = sandbox("say:Fix login");
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    assert_eq!(created["name_generated"], true);
    let session = start_session(&sandbox, &id);

    prompt(&sandbox, &session, "Fix the login bug");

    let renamed = eventually("the generated workspace renamed", || {
        let shown = workspace(&sandbox, &id);
        (shown["name"] == "fix-login").then_some(shown)
    });
    assert_eq!(renamed["branch"], "fix-login", "{renamed}");
    assert_eq!(renamed["path"], created["path"], "the folder is not moved");
    assert!(
        renamed.get("name_generated").is_none(),
        "the trigger's rename makes the name the person's: {renamed}"
    );
    let report = fs::read_to_string(report_of(&sandbox)).expect("the title run wrote its report");
    assert!(report.contains("Fix the login bug"), "{report}");
    log_with(
        &sandbox,
        &format!("first-prompt rename of workspace {id} named it fix-login"),
    );
}

#[test]
fn a_typed_name_is_never_renamed_and_no_title_is_asked_for() {
    let sandbox = sandbox("say:Fix login");
    let typed = create(&sandbox, &["typed"]);
    let id = typed["id"].as_str().expect("id").to_owned();
    assert!(typed.get("name_generated").is_none(), "{typed}");
    let session = start_session(&sandbox, &id);

    prompt(&sandbox, &session, "Fix the login bug");

    log_with(
        &sandbox,
        &format!("first-prompt rename of workspace {id} left its name: a name a person gave"),
    );
    assert_eq!(workspace(&sandbox, &id)["name"], "typed");
    assert!(
        !report_of(&sandbox).exists(),
        "a typed name never reaches the one-shot form"
    );
}

#[test]
fn the_setting_turned_off_keeps_the_generated_name() {
    let sandbox = sandbox("say:Fix login");
    sandbox.json_ok("default", &["config", "set", SETTING, "false"]);
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    let session = start_session(&sandbox, &id);

    prompt(&sandbox, &session, "Fix the login bug");

    log_with(
        &sandbox,
        &format!("first-prompt rename of workspace {id} left its name: the setting is off"),
    );
    let shown = workspace(&sandbox, &id);
    assert_eq!(shown["name"], created["name"], "{shown}");
    assert!(
        !report_of(&sandbox).exists(),
        "the setting stops the title run"
    );
}

#[test]
fn a_failed_title_keeps_the_name_and_the_next_prompt_of_the_session_retries_once() {
    let sandbox = sandbox("fail-then-say:Fix login");
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    let session = start_session(&sandbox, &id);

    prompt(&sandbox, &session, "Fix the login bug");

    log_with(
        &sandbox,
        &format!(
            "first-prompt rename of workspace {id} failed with agent.oneshot_failed; the next prompt of session {session} retries once"
        ),
    );
    assert_eq!(workspace(&sandbox, &id)["name"], created["name"]);

    prompt(&sandbox, &session, "Also check the logout");

    eventually("the retried title renamed the workspace", || {
        (workspace(&sandbox, &id)["name"] == "fix-login").then_some(())
    });
}

#[test]
fn a_second_failed_title_is_final() {
    let sandbox = sandbox("exit:1");
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    let session = start_session(&sandbox, &id);

    prompt(&sandbox, &session, "Fix the login bug");
    log_with(
        &sandbox,
        &format!(
            "first-prompt rename of workspace {id} failed with agent.oneshot_failed; the next prompt"
        ),
    );

    prompt(&sandbox, &session, "Also check the logout");
    log_with(
        &sandbox,
        &format!(
            "first-prompt rename of workspace {id} failed again with agent.oneshot_failed; it is not retried"
        ),
    );
    assert_eq!(workspace(&sandbox, &id)["name"], created["name"]);
}

#[test]
fn a_second_session_does_not_rename_a_workspace_that_is_already_named() {
    let sandbox = sandbox("say:Fix login");
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    let first = start_session(&sandbox, &id);
    prompt(&sandbox, &first, "Fix the login bug");
    eventually("the first session renamed the workspace", || {
        (workspace(&sandbox, &id)["name"] == "fix-login").then_some(())
    });

    // A read-only session does not count as a writer, so the workspace takes a second one (the one-writer rule).
    let second = start_session_with(&sandbox, &id, &["--read-only"]);
    prompt(&sandbox, &second, "Write the changelog");

    log_with(
        &sandbox,
        &format!(
            "first-prompt rename of workspace {id} left its name: an earlier prompt already decided it"
        ),
    );
    assert_eq!(workspace(&sandbox, &id)["name"], "fix-login");
}

#[test]
fn the_setting_is_declared_on_by_default_and_settable_through_config() {
    let sandbox = sandbox("say:Fix login");
    let listed = sandbox.json_ok("default", &["config", "get", SETTING]);
    assert_eq!(listed["value"], true, "{listed}");
    sandbox
        .run("default", &with_json(&["config", "set", SETTING, "false"]))
        .success();
}

#[test]
fn a_first_prompt_the_queue_delivers_names_the_workspace_too() {
    let sandbox = sandbox("say:Fix login");
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    let session = start_session(&sandbox, &id);

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "queue",
            "add",
            session.as_str(),
            "Fix the login bug",
        ],
    );

    eventually("the queued first prompt renamed the workspace", || {
        (workspace(&sandbox, &id)["name"] == "fix-login").then_some(())
    });
    assert!(
        report_of(&sandbox).exists(),
        "the queued prompt asked for a title"
    );
}

#[test]
fn a_rename_by_the_trigger_is_final_after_a_service_restart() {
    let sandbox = sandbox("say:Fix login");
    let created = create(&sandbox, &[]);
    let id = created["id"].as_str().expect("id").to_owned();
    let session = start_session(&sandbox, &id);
    prompt(&sandbox, &session, "Fix the login bug");
    eventually("the first prompt renamed the workspace", || {
        (workspace(&sandbox, &id)["name"] == "fix-login").then_some(())
    });
    let renamed = workspace(&sandbox, &id);
    assert!(
        renamed.get("name_generated").is_none(),
        "the trigger's rename ends the generated state: {renamed}"
    );

    sandbox.run("default", &["service", "stop"]);
    fs::remove_file(report_of(&sandbox)).expect("the first run wrote its report");
    let later = start_session_with(&sandbox, &id, &["--read-only"]);
    prompt(&sandbox, &later, "Write the changelog");

    log_with(
        &sandbox,
        &format!("first-prompt rename of workspace {id} left its name: a name a person gave"),
    );
    assert_eq!(workspace(&sandbox, &id)["name"], "fix-login");
    assert!(
        !report_of(&sandbox).exists(),
        "a later run asks no title for a name the trigger already settled"
    );
}
