//! `massorca team routine ...` end to end (decision 0072 amendment, decision 0082): a routine run now into a task for its
//! bot, which starts and links its session; a routine whose bot was removed records why and starts nothing; pause and
//! resume; the refusals of an invalid schedule; and the listings.

mod harness;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use harness::{Sandbox, stdout_text, with_json};
use serde_json::{Value, json};

/// Generous upper bound for `--timeout`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

/// Declares kind `fake`, which runs the fake agent with an argv prompt and reports its activity.
fn declare_fake(sandbox: &Sandbox) {
    sandbox.prepare_profile("default");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": {
            "mechanism": "env-command",
            "events": {
                "start": "idle", "prompt": "working", "ask": "waiting",
                "stop": "done", "fail": "done:failure"
            },
            "result_field": "result"
        }
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
}

/// The host zone the service reads (`TZ`), so a routine without a zone has a known zone on every machine.
const HOST_ZONE: &str = "America/New_York";

/// A sandbox whose workspaces go under `<home>/ws`, with team `alpha` and kind `fake`, on a host in [`HOST_ZONE`].
fn team_sandbox() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
    sandbox.set_env("TZ", HOST_ZONE);
    declare_fake(&sandbox);
    let root = sandbox.home().join("ws");
    let root_text = root.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "config",
            "set",
            "workspaces.default_root",
            root_text.as_str(),
        ],
    );
    sandbox.json_ok(
        "default",
        &["team", "create", "alpha", "--purpose", "Ships the parser"],
    );
    (sandbox, root)
}

fn add_bot(sandbox: &Sandbox, name: &str, kind: &str) {
    sandbox.json_ok(
        "default",
        &[
            "team",
            "bot",
            "add",
            "alpha",
            name,
            "--agent",
            kind,
            "--job",
            "Writes the parser",
        ],
    );
}

/// A daily 09:00 routine named `morning` for bot `mara`, whose prompt holds the fake agent until it is released.
fn add_morning_routine(sandbox: &Sandbox, extra: &[&str]) -> Value {
    let mut args = vec![
        "team",
        "routine",
        "add",
        "alpha",
        "morning",
        "--bot",
        "mara",
        "--prompt",
        "Start with hold, then stop.",
        "--time",
        "09:00",
        "--cadence",
        "daily",
    ];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

/// Runs `--json <args>`, which must fail with exit code 1; returns the error body.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure().code(1);
    let document = harness::single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

fn now_ms() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is after the epoch")
            .as_millis(),
    )
    .expect("the time fits")
}

#[test]
fn a_routine_run_now_makes_a_task_for_its_bot_that_starts_and_links_its_session() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let routine = add_morning_routine(&sandbox, &["--zone", "Europe/Madrid"]);
    assert_eq!(routine["cadence"], "daily");
    assert_eq!(routine["starts_as_admin"], true, "the owner runs as admin");
    assert!(routine["next_run_at_ms"].is_i64(), "a daily routine is due");

    let run = sandbox.json_ok(
        "default",
        &["team", "routine", "run", "alpha", "morning", "--now"],
    );
    assert_eq!(run["error"], Value::Null, "{run}");
    let task_id = run["task_id"].as_str().expect("the run made a task");

    let task = sandbox.json_ok("default", &["task", "show", task_id]);
    assert_eq!(task["title"], "morning");
    assert!(
        task["body"]
            .as_str()
            .expect("the task body")
            .contains("Clock: this run is for "),
        "{task}"
    );
    let session = task["session_id"]
        .as_str()
        .expect("the start linked a session")
        .to_owned();

    let working = sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session.as_str(),
            "--until",
            "working",
            "--timeout",
            UNTIL_SECS,
        ],
    );
    assert_eq!(working["kind"], "fake");
    assert_eq!(working["title"], "morning");

    let runs = sandbox.json_ok("default", &["team", "routine", "runs", "alpha", "morning"]);
    assert_eq!(runs["runs"][0]["task_id"], task_id, "{runs}");
    assert_eq!(runs["runs"][0]["id"], run["id"], "{runs}");

    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
}

#[test]
fn a_routine_whose_bot_was_removed_records_why_and_starts_no_agent() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    add_morning_routine(&sandbox, &[]);
    sandbox.json_ok("default", &["team", "bot", "rm", "alpha", "mara"]);

    let run = sandbox.json_ok(
        "default",
        &["team", "routine", "run", "alpha", "morning", "--now"],
    );
    assert!(run["task_id"].is_null(), "{run}");
    assert_eq!(run["error_code"], "bot.not_found", "{run}");
    assert!(
        run["error"]
            .as_str()
            .expect("the run records why")
            .contains("bot removed"),
        "{run}"
    );

    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"], json!([]), "nothing was started");
}

#[test]
fn pausing_clears_the_next_run_and_resuming_schedules_one_from_now() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    add_morning_routine(&sandbox, &["--zone", "Europe/Madrid"]);

    let paused = sandbox.json_ok("default", &["team", "routine", "pause", "alpha", "morning"]);
    assert_eq!(paused["paused"], true, "{paused}");
    assert!(paused["next_run_at_ms"].is_null(), "{paused}");

    let before = now_ms();
    let resumed = sandbox.json_ok(
        "default",
        &["team", "routine", "resume", "alpha", "morning"],
    );
    assert_eq!(resumed["paused"], false, "{resumed}");
    let next = resumed["next_run_at_ms"]
        .as_i64()
        .expect("a resumed daily routine has a next run");
    assert!(
        next > before,
        "nothing missed is made up: {next} vs {before}"
    );
    assert!(
        next - before <= 25 * 60 * 60 * 1000,
        "the next run is the next 09:00: {next} vs {before}"
    );
}

#[test]
fn a_weekly_routine_without_a_weekday_is_refused_as_invalid_params() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");

    let error = error_of(
        &sandbox,
        &[
            "team",
            "routine",
            "add",
            "alpha",
            "weekly-check",
            "--bot",
            "mara",
            "--prompt",
            "Read the open issues.",
            "--time",
            "09:00",
            "--cadence",
            "weekly",
        ],
    );

    assert_eq!(error["code"], "request.invalid_params", "{error}");
    assert!(
        error["message"]
            .as_str()
            .expect("a message")
            .contains("weekday"),
        "{error}"
    );
}

#[test]
fn a_run_without_the_now_flag_is_refused_and_moves_nothing() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    add_morning_routine(&sandbox, &[]);

    let error = error_of(&sandbox, &["team", "routine", "run", "alpha", "morning"]);
    assert_eq!(error["code"], "request.invalid_params", "{error}");

    let runs = sandbox.json_ok("default", &["team", "routine", "runs", "alpha", "morning"]);
    assert_eq!(runs["runs"], json!([]), "{runs}");
}

#[test]
fn an_update_changes_only_the_fields_given_and_removal_leaves_no_routine() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    add_morning_routine(&sandbox, &[]);

    let updated = sandbox.json_ok(
        "default",
        &[
            "team", "routine", "update", "alpha", "morning", "--time", "10:30",
        ],
    );
    assert_eq!(updated["time"], "10:30", "{updated}");
    assert_eq!(updated["name"], "morning", "{updated}");
    assert_eq!(updated["cadence"], "daily", "{updated}");

    let refused = error_of(
        &sandbox,
        &[
            "team",
            "routine",
            "update",
            "alpha",
            "morning",
            "--cadence",
            "weekly",
        ],
    );
    assert_eq!(refused["code"], "request.invalid_params", "{refused}");

    sandbox.json_ok("default", &["team", "routine", "rm", "alpha", "morning"]);
    let listed = sandbox.json_ok("default", &["team", "routine", "list", "alpha"]);
    assert_eq!(listed["routines"], json!([]), "{listed}");
}

#[test]
fn the_routine_list_and_runs_print_one_row_each() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    add_morning_routine(&sandbox, &["--zone", "Europe/Madrid"]);
    sandbox.json_ok("default", &["team", "bot", "rm", "alpha", "mara"]);
    sandbox.json_ok(
        "default",
        &["team", "routine", "run", "alpha", "morning", "--now"],
    );

    let listed = stdout_text(
        &sandbox
            .run("default", &["team", "routine", "list", "alpha"])
            .success(),
    );
    assert!(listed.contains("morning"), "{listed}");
    assert!(listed.contains("daily 09:00 Europe/Madrid"), "{listed}");

    let runs = stdout_text(
        &sandbox
            .run("default", &["team", "routine", "runs", "alpha", "morning"])
            .success(),
    );
    assert!(runs.contains("bot removed"), "{runs}");
    assert!(runs.contains("bot.not_found"), "{runs}");
    assert_eq!(runs.lines().count(), 1, "one run, one row: {runs}");
}

#[test]
fn a_routine_without_a_zone_shows_the_zone_it_runs_in() {
    let (sandbox, _) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let routine = add_morning_routine(&sandbox, &[]);
    assert!(routine["zone"].is_null(), "{routine}");
    assert_eq!(routine["zone_used"], HOST_ZONE, "{routine}");

    let listed = stdout_text(
        &sandbox
            .run("default", &["team", "routine", "list", "alpha"])
            .success(),
    );
    assert!(
        listed.contains(&format!("daily 09:00 host zone ({HOST_ZONE})")),
        "{listed}"
    );
}

#[test]
fn a_run_left_pending_by_a_stopped_service_is_settled_when_the_service_starts_again() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let routine = add_morning_routine(&sandbox, &[]);
    let routine_id = routine["id"].as_str().expect("a routine id").to_owned();
    sandbox.run("default", &["service", "stop"]).success();

    // A run the stopped service claimed and never settled, made before the next service starts.
    let store =
        massorca_core_store::Store::open(sandbox.profile_dir("default").join("store.sqlite3"))
            .expect("the store opens");
    let pending = massorca_core_store::TeamRoutineRunRow {
        id: massorca_core_store::new_id(),
        routine_id,
        scheduled_for: 1,
        task_id: None,
        error: None,
        error_code: None,
        settled_at: None,
        created_at: 1,
    };
    store
        .insert_routine_run(&pending, None)
        .expect("store the pending run");
    drop(store);

    // Any command starts the service again, and its startup settles the run.
    let deadline = now_ms() + 60_000;
    let run = loop {
        let runs = sandbox.json_ok("default", &["team", "routine", "runs", "alpha", "morning"]);
        let run = runs["runs"][0].clone();
        if !run["error_code"].is_null() || now_ms() > deadline {
            break run;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    assert_eq!(run["id"], pending.id.as_str(), "{run}");
    assert_eq!(run["error_code"], "team.routine_run_interrupted", "{run}");
    assert_eq!(
        run["error"], "the service stopped before this run started",
        "{run}"
    );
}
