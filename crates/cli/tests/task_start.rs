//! `massorca task start` end to end (decision 0072 amendment): the team's shared folder workspace, the bot's agent started there with the
//! task as its prompt, the session link, and the refusals that keep a task Open with the reason.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, eventually, git_repo, single_document, with_json};
use massorca_core_store::Store;
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

/// Declares kind `fake`, which runs the fake agent with an argv prompt and reports its activity.
fn declare_fake(sandbox: &Sandbox) {
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

/// A sandbox whose workspaces go under `<home>/ws`, with team `alpha` and kind `fake`. Returns the sandbox and the root.
fn team_sandbox() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
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

fn create_task(sandbox: &Sandbox, title: &str, extra: &[&str]) -> String {
    let mut args = vec!["task", "create", "alpha", title];
    args.extend_from_slice(extra);
    let task = sandbox.json_ok("default", &args);
    task["id"].as_str().expect("task id").to_owned()
}

/// Runs `--json <args>`, which must fail with exit code 1; returns the error body.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure().code(1);
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

fn same_dir(left: &Path, right: &Path) -> bool {
    left.canonicalize().expect("canonical left") == right.canonicalize().expect("canonical right")
}

#[test]
fn a_task_starts_its_bot_in_the_team_folder_with_the_task_as_its_prompt() {
    let (sandbox, root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    // "hold" keeps the fake agent working until it reads "release", so the task stays Running.
    let task = create_task(
        &sandbox,
        "Parse the headers",
        &["--bot", "mara", "--body", "Start with hold, then stop."],
    );

    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    let session = started["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();
    assert_eq!(started["error"], Value::Null);

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
    assert_eq!(working["title"], "Parse the headers");

    let team = sandbox.json_ok("default", &["team", "show", "alpha"]);
    let team_id = team["id"].as_str().expect("team id");
    let workspace = sandbox.json_ok(
        "default",
        &[
            "workspace",
            "show",
            working["workspace_id"].as_str().expect("workspace id"),
        ],
    );
    let folder = root.join("bot-teams").join(team_id);
    assert!(same_dir(
        Path::new(workspace["path"].as_str().expect("workspace path")),
        &folder
    ));
    let project = sandbox.json_ok("default", &["project", "show", "Bot teams"]);
    assert_eq!(project["kind"], "folder");

    let terminal = working["terminal_id"].as_str().expect("terminal id");
    eventually("the prompt on the agent's screen", || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        let text: String = screen["lines"]
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        (text.contains("You are mara, a bot in the team alpha.")
            && text.contains("Task: Parse the headers"))
        .then_some(())
    });

    let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(shown["column"], "running");

    let again = error_of(&sandbox, &["task", "start", task.as_str()]);
    assert_eq!(again["code"], "task.started");

    // The bot works on one task at a time: a second start is refused and the card stays Open with the reason.
    let second = create_task(&sandbox, "Write the docs", &["--bot", "mara"]);
    let busy = error_of(&sandbox, &["task", "start", second.as_str()]);
    assert_eq!(busy["code"], "bot.busy");
    let board = sandbox.json_ok("default", &["team", "board", "alpha"]);
    let card = board["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .flat_map(|column| column["cards"].as_array().cloned().unwrap_or_default())
        .find(|card| card["id"] == second.as_str())
        .expect("the refused task is on the Board");
    assert_eq!(card["error_code"], "bot.busy");
    let waiting = sandbox.json_ok("default", &["task", "show", second.as_str()]);
    assert_eq!(waiting["column"], "open");
    assert_eq!(waiting["session_id"], Value::Null);
    assert!(
        waiting["error"]
            .as_str()
            .is_some_and(|reason| reason.contains("mara")),
        "{waiting}"
    );

    // Another bot of the team runs at the same time in the same team folder: a team folder has several writers.
    add_bot(&sandbox, "ada", "fake");
    let third = create_task(&sandbox, "Review the parser", &["--bot", "ada"]);
    let other = sandbox.json_ok("default", &["task", "start", third.as_str()]);
    let other_session = other["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();
    let other_record = sandbox.json_ok("default", &["agent", "status", other_session.as_str()]);
    assert_eq!(other_record["workspace_id"], working["workspace_id"]);

    for id in [&session, &other_session] {
        sandbox.json_ok("default", &["agent", "stop", id.as_str(), "--force"]);
    }
}

#[test]
fn a_start_that_fails_keeps_the_task_open_with_the_reason_and_can_be_retried() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "ghost", "no-such-agent");
    let task = create_task(&sandbox, "Parse the headers", &["--bot", "ghost"]);

    let refused = error_of(&sandbox, &["task", "start", task.as_str()]);
    assert_eq!(refused["code"], "agent.kind_not_found");
    let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(shown["column"], "open");
    assert_eq!(shown["session_id"], Value::Null);
    assert_eq!(shown["error_code"], "agent.kind_not_found");
    let reason = shown["error"].as_str().expect("the reason is kept");
    assert!(reason.contains("no-such-agent"), "{reason}");

    // The reason is kept until a start succeeds; the task can be started again.
    sandbox.json_ok(
        "default",
        &["team", "bot", "update", "alpha", "ghost", "--agent", "fake"],
    );
    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    assert_eq!(started["error"], Value::Null);
    let session = started["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();
    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
}

#[test]
fn a_task_without_a_bot_is_refused_and_stays_unchanged() {
    let (sandbox, _root) = team_sandbox();
    let task = create_task(&sandbox, "Parse the headers", &[]);
    let refused = error_of(&sandbox, &["task", "start", task.as_str()]);
    assert_eq!(refused["code"], "task.no_bot");
    let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(shown["column"], "open");
    assert_eq!(shown["error"], Value::Null);
    let unknown = error_of(&sandbox, &["task", "start", "no-such-task"]);
    assert_eq!(unknown["code"], "task.not_found");
}

#[test]
fn a_running_task_keeps_its_bot_and_its_link_and_cannot_be_removed() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    add_bot(&sandbox, "ada", "fake");
    let task = create_task(
        &sandbox,
        "Parse the headers",
        &["--bot", "mara", "--body", "Start with hold, then stop."],
    );
    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    let session = started["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();
    sandbox.json_ok(
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

    let rebot = error_of(&sandbox, &["task", "update", task.as_str(), "--bot", "ada"]);
    assert_eq!(rebot["code"], "task.running");
    let removal = error_of(&sandbox, &["task", "rm", task.as_str()]);
    assert_eq!(removal["code"], "task.running");

    // A title change touches only the title: the run stays linked.
    let renamed = sandbox.json_ok(
        "default",
        &[
            "task",
            "update",
            task.as_str(),
            "--title",
            "Parse every header",
        ],
    );
    assert_eq!(renamed["title"], "Parse every header");
    assert_eq!(renamed["session_id"], session.as_str());
    let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(shown["bot_id"], started["bot_id"]);
    assert_eq!(shown["column"], "running");

    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
}

#[test]
fn a_bot_teams_path_taken_by_a_git_project_refuses_the_start() {
    let (sandbox, root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let taken = root.join("bot-teams");
    git_repo(&taken);
    let taken_text = taken.to_string_lossy().into_owned();
    sandbox.json_ok("default", &["project", "add", taken_text.as_str()]);
    let task = create_task(&sandbox, "Parse the headers", &["--bot", "mara"]);

    let refused = error_of(&sandbox, &["task", "start", task.as_str()]);
    assert_eq!(refused["code"], "team.folder_taken");
    let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(shown["column"], "open");
    assert_eq!(shown["session_id"], Value::Null);
    assert_eq!(shown["error_code"], "team.folder_taken");
}

#[test]
fn a_run_a_failed_start_could_not_stop_keeps_its_bot_busy_until_it_ends() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let first = create_task(
        &sandbox,
        "Parse the headers",
        &["--bot", "mara", "--body", "Start with hold, then stop."],
    );
    let started = sandbox.json_ok("default", &["task", "start", first.as_str()]);
    let session = started["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();

    // Leave the run as a start that failed after its spawn and could not stop it: unlinked, attempt still pending.
    let store =
        Store::open(sandbox.profile_dir("default").join("store.sqlite3")).expect("the store opens");
    let request = store
        .get_agent_session(&session)
        .expect("read the session")
        .and_then(|row| row.request_id)
        .expect("the run's request id");
    let attempt = request
        .rsplit(':')
        .next()
        .expect("the attempt id")
        .to_owned();
    assert!(
        store
            .reopen_task(&first, Some(session.as_str()), 2)
            .expect("unlink")
    );
    store
        .begin_task_start_attempt(&first, &attempt)
        .expect("keep the attempt");

    let second = create_task(&sandbox, "Write the docs", &["--bot", "mara"]);
    let busy = error_of(&sandbox, &["task", "start", second.as_str()]);
    assert_eq!(busy["code"], "bot.busy", "{busy}");

    // Once the run is stopped, the bot is free again.
    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
    eventually("the stopped run exits", || {
        let record = sandbox.json_ok("default", &["agent", "status", session.as_str()]);
        (record["execution"]["state"] == "exited").then_some(())
    });
    let freed = sandbox.json_ok("default", &["task", "start", second.as_str()]);
    let freed_session = freed["session_id"].as_str().expect("linked session");
    sandbox.json_ok("default", &["agent", "stop", freed_session, "--force"]);
}
