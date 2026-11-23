//! Team access end to end (decision 0072 amendment "team access", decision 0083): a full-access team's bot runs in its
//! kind's bypass mode, a default-access team's bot in the vendor's default, and the create and start paths say so.

mod harness;

use harness::{Sandbox, stdout_text};
use serde_json::{Value, json};

/// Declares kind `id`, running the fake agent with `modes`.
fn declare(sandbox: &Sandbox, id: &str, modes: Value) {
    sandbox.prepare_profile("default");
    let kind = json!({
        "id": id,
        "name": format!("Fake agent {id}"),
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": {
            "mechanism": "env-command",
            "events": { "start": "idle", "prompt": "working", "stop": "done" },
            "result_field": "result"
        },
        "permission_modes": modes,
        "default_permission_mode": "careful"
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kind `fake` (modes careful and the bypass mode wild) and kind `tame` (careful only).
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    let careful = json!({ "id": "careful", "label": "Careful", "args": ["--mode", "careful"] });
    let wild = json!({ "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true });
    declare(&sandbox, "fake", json!([careful, wild]));
    declare(&sandbox, "tame", json!([careful]));
    let root = sandbox.home().join("ws");
    sandbox.json_ok(
        "default",
        &[
            "config",
            "set",
            "workspaces.default_root",
            root.to_string_lossy().as_ref(),
        ],
    );
    sandbox
}

/// Adds bot `name` of `kind` to `team`, creates a task for it, starts it and returns the started task's session record.
fn start_task(sandbox: &Sandbox, team: &str, name: &str, kind: &str) -> Value {
    sandbox.json_ok(
        "default",
        &[
            "team", "bot", "add", team, name, "--agent", kind, "--job", "Checks",
        ],
    );
    let task = sandbox.json_ok(
        "default",
        &["task", "create", team, "Check the build", "--bot", name],
    );
    let task_id = task["id"].as_str().expect("a task id");
    let started = sandbox.json_ok("default", &["task", "start", task_id]);
    let session = started["session_id"].as_str().expect("linked session");
    let record = sandbox.json_ok("default", &["agent", "status", session]);
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
    record
}

#[test]
fn a_new_team_has_full_access_and_says_its_bots_run_without_asking() {
    let sandbox = sandbox();

    let created = stdout_text(
        &sandbox
            .run("default", &["team", "create", "alpha"])
            .success(),
    );

    assert!(
        created.contains("Full access: bots run without asking for permission."),
        "{created}"
    );
    assert!(
        created.contains("massorca team update alpha --access default"),
        "{created}"
    );
    let shown = sandbox.json_ok("default", &["team", "show", "alpha"]);
    assert_eq!(shown["access"], "full", "{shown}");
}

#[test]
fn a_full_access_team_runs_its_bot_in_the_kinds_bypass_mode() {
    let sandbox = sandbox();
    sandbox.json_ok("default", &["team", "create", "alpha"]);

    let record = start_task(&sandbox, "alpha", "mara", "fake");

    assert_eq!(record["permission_mode"], "wild", "{record}");
}

#[test]
fn a_default_access_team_runs_its_bot_in_the_vendor_default() {
    let sandbox = sandbox();
    let created = sandbox.json_ok(
        "default",
        &["team", "create", "beta", "--access", "default"],
    );
    assert_eq!(created["access"], "default", "{created}");

    let record = start_task(&sandbox, "beta", "mara", "fake");

    assert_eq!(record["permission_mode"], Value::Null, "{record}");
    assert_eq!(record["mode_source"], "vendor_default", "{record}");

    // Turning full access on later is one step, and says what it does.
    let updated = stdout_text(
        &sandbox
            .run("default", &["team", "update", "beta", "--access", "full"])
            .success(),
    );
    assert!(
        updated.contains("Full access: bots run without asking for permission."),
        "{updated}"
    );
}

#[test]
fn a_full_access_bot_whose_kind_has_no_bypass_mode_runs_its_default_and_the_start_says_so() {
    let sandbox = sandbox();
    sandbox.json_ok("default", &["team", "create", "alpha"]);
    sandbox.json_ok(
        "default",
        &[
            "team", "bot", "add", "alpha", "ada", "--agent", "tame", "--job", "Checks",
        ],
    );
    let task = sandbox.json_ok(
        "default",
        &["task", "create", "alpha", "Check the build", "--bot", "ada"],
    );
    let task_id = task["id"].as_str().expect("a task id");

    let started = stdout_text(
        &sandbox
            .run("default", &["task", "start", task_id])
            .success(),
    );

    assert!(
        started.contains("the tame agent has no mode that skips permission prompts"),
        "{started}"
    );
    let shown = sandbox.json_ok("default", &["task", "show", task_id]);
    let session = shown["session_id"].as_str().expect("linked session");
    let record = sandbox.json_ok("default", &["agent", "status", session]);
    assert_eq!(record["permission_mode"], Value::Null, "{record}");
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}
