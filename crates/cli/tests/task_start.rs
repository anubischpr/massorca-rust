//! `massorca task start` end to end (decision 0072 amendment): the team's shared folder workspace, the bot's agent started there with the
//! task as its prompt, the session link, and the refusals that keep a task Open with the reason.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, eventually, git_repo, single_document, with_json};
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
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

    // A bot working on one task starts another in its own concurrent session, in the same team folder.
    let second = create_task(&sandbox, "Write the docs", &["--bot", "mara"]);
    let concurrent = sandbox.json_ok("default", &["task", "start", second.as_str()]);
    let concurrent_session = concurrent["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();
    assert_ne!(concurrent_session, session);
    assert_eq!(concurrent["column"], "running", "{concurrent}");
    let concurrent_record =
        sandbox.json_ok("default", &["agent", "status", concurrent_session.as_str()]);
    assert_eq!(concurrent_record["workspace_id"], working["workspace_id"]);
    let first = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(first["session_id"], session.as_str());

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

    for id in [&session, &concurrent_session, &other_session] {
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
fn three_failed_starts_in_a_row_break_the_circuit_until_the_task_is_reopened() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "ghost", "no-such-agent");
    let task = create_task(&sandbox, "Parse the headers", &["--bot", "ghost"]);

    for _ in 0..2 {
        let refused = error_of(&sandbox, &["task", "start", task.as_str()]);
        assert_eq!(refused["code"], "agent.kind_not_found", "{refused}");
    }
    let third = error_of(&sandbox, &["task", "start", task.as_str()]);
    assert_eq!(third["code"], "task.circuit_broken", "{third}");
    let reason = third["message"].as_str().expect("a reason");
    assert!(reason.starts_with("Circuit broken: "), "{reason}");
    assert!(reason.contains("no-such-agent"), "{reason}");
    let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(shown["error_code"], "task.circuit_broken", "{shown}");
    assert_eq!(shown["error"], reason, "{shown}");

    // A broken task is refused even once its cause is fixed, and no agent starts.
    sandbox.json_ok(
        "default",
        &["team", "bot", "update", "alpha", "ghost", "--agent", "fake"],
    );
    let refused = error_of(&sandbox, &["task", "start", task.as_str()]);
    assert_eq!(refused["code"], "task.circuit_broken", "{refused}");
    let sessions = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(sessions["sessions"], json!([]), "{sessions}");

    // A reopen resets the breaker.
    sandbox.json_ok("default", &["task", "reopen", task.as_str()]);
    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    let session = started["session_id"].as_str().expect("linked session");
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
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

/// Answers the question the fake agent asks, then waits until its turn is done. Every task prompt asks: its "Task:" line
/// contains the fake agent's "ask:" trigger.
fn answer_and_wait_for_done(sandbox: &Sandbox, session: &str) {
    let asking = sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session,
            "--until",
            "waiting",
            "--timeout",
            UNTIL_SECS,
        ],
    );
    let terminal = asking["terminal_id"].as_str().expect("terminal id");
    sandbox
        .run("default", &["terminal", "send", terminal, "yes"])
        .success();
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session,
            "--until",
            "done",
            "--timeout",
            UNTIL_SECS,
        ],
    );
}

#[test]
fn a_finished_agent_latches_success_into_its_task_and_the_task_keeps_it_once_forgotten() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    // No "hold" and no lower-case "fail" in the prompt: the fake agent reports done with success.
    let task = create_task(
        &sandbox,
        "Parse the headers",
        &["--bot", "mara", "--body", "Write the parser, then stop."],
    );
    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    let session = started["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();

    answer_and_wait_for_done(&sandbox, &session);
    let latched = eventually("the task to latch success", || {
        let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
        (shown["outcome"] == "success").then_some(shown)
    });
    assert_eq!(latched["column"], "done");

    // The session is forgotten, and the Board still shows the task's own outcome.
    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
    sandbox.json_ok("default", &["agent", "forget", session.as_str()]);
    let kept = sandbox.json_ok("default", &["task", "show", task.as_str()]);
    assert_eq!(kept["outcome"], "success");
    assert_eq!(kept["column"], "done");
}

#[test]
fn a_finished_agent_that_reports_failure_latches_failure_into_its_task() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    // The lower-case "fail" in the body makes the fake agent report done with failure.
    let task = create_task(
        &sandbox,
        "Parse the headers",
        &["--bot", "mara", "--body", "Write the parser, then fail."],
    );
    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    let session = started["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();

    answer_and_wait_for_done(&sandbox, &session);
    let latched = eventually("the task to latch failure", || {
        let shown = sandbox.json_ok("default", &["task", "show", task.as_str()]);
        (shown["outcome"] == "failure").then_some(shown)
    });
    assert_eq!(latched["column"], "waiting");

    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
}

/// Adds `path` as a Workspaces project (`--folder` for a plain folder) and team project `name` linked to it.
fn link_source(sandbox: &Sandbox, path: &Path, folder: bool, name: &str) {
    let text = path.to_string_lossy().into_owned();
    let mut args = vec!["project", "add", text.as_str()];
    if folder {
        args.push("--folder");
    }
    sandbox.json_ok("default", &args);
    sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "add",
            "alpha",
            name,
            "--source",
            text.as_str(),
        ],
    );
}

#[test]
fn a_task_on_a_linked_git_project_runs_in_its_own_worktree_and_names_its_branch() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let repo = sandbox.home().join("parser-repo");
    git_repo(&repo);
    link_source(&sandbox, &repo, false, "parser");
    let task = create_task(
        &sandbox,
        "Parse the Headers",
        &[
            "--bot",
            "mara",
            "--project",
            "parser",
            "--body",
            "Start with hold, then stop.",
        ],
    );

    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    let branch = started["branch"]
        .as_str()
        .expect("the task's branch")
        .to_owned();
    assert!(branch.starts_with("parse-the-headers-"), "{branch}");
    assert_eq!(branch.len(), "parse-the-headers-".len() + 6, "{branch}");
    let workspace_id = started["workspace_id"].as_str().expect("its workspace");
    let workspace = sandbox.json_ok("default", &["workspace", "show", workspace_id]);
    assert_eq!(workspace["kind"], "worktree");
    assert_eq!(workspace["branch"], branch.as_str());

    // The agent runs in that worktree, and its prompt says what the worktree does not carry.
    let session = started["session_id"].as_str().expect("linked session");
    let record = sandbox.json_ok("default", &["agent", "status", session]);
    assert_eq!(record["workspace_id"], workspace_id);
    let terminal = record["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    eventually("the branch note in the prompt", || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal.as_str()]);
        let text = screen["lines"]
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        text.contains("uncommitted changes").then_some(())
    });

    // The Board card names the branch, so the result is findable.
    let board = sandbox.json_ok("default", &["team", "board", "alpha"]);
    let card = board["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .flat_map(|column| column["cards"].as_array().cloned().unwrap_or_default())
        .find(|card| card["id"] == task.as_str())
        .expect("the task is on the Board");
    assert_eq!(card["branch"], branch.as_str());
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

#[test]
fn a_task_on_a_linked_folder_runs_in_that_folder_and_has_no_branch() {
    let (sandbox, _root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create the folder");
    link_source(&sandbox, &notes, true, "notes");
    let task = create_task(
        &sandbox,
        "Tidy the notes",
        &[
            "--bot",
            "mara",
            "--project",
            "notes",
            "--body",
            "Start with hold, then stop.",
        ],
    );

    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    assert_eq!(started["branch"], Value::Null);
    let workspace_id = started["workspace_id"].as_str().expect("its workspace");
    let workspace = sandbox.json_ok("default", &["workspace", "show", workspace_id]);
    assert_eq!(workspace["kind"], "folder");
    let shown = PathBuf::from(workspace["path"].as_str().expect("workspace path"));
    assert!(same_dir(&shown, &notes), "{workspace}");
    let session = started["session_id"].as_str().expect("linked session");
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

#[test]
fn a_task_whose_project_links_no_source_runs_in_the_team_folder() {
    let (sandbox, root) = team_sandbox();
    add_bot(&sandbox, "mara", "fake");
    sandbox.json_ok(
        "default",
        &[
            "team", "project", "add", "alpha", "loose", "--goal", "Ship it",
        ],
    );
    let task = create_task(
        &sandbox,
        "Parse the headers",
        &[
            "--bot",
            "mara",
            "--project",
            "loose",
            "--body",
            "Start with hold, then stop.",
        ],
    );

    let started = sandbox.json_ok("default", &["task", "start", task.as_str()]);
    assert_eq!(started["branch"], Value::Null);
    let workspace_id = started["workspace_id"].as_str().expect("its workspace");
    let workspace = sandbox.json_ok("default", &["workspace", "show", workspace_id]);
    let shown = PathBuf::from(workspace["path"].as_str().expect("workspace path"));
    let team = sandbox.json_ok("default", &["team", "show", "alpha"]);
    let folder = root
        .join("bot-teams")
        .join(team["id"].as_str().expect("team id"));
    assert!(same_dir(&shown, &folder), "{workspace}");
    let session = started["session_id"].as_str().expect("linked session");
    // The prompt names the task's project and its goal.
    let record = sandbox.json_ok("default", &["agent", "status", session]);
    let terminal = record["terminal_id"].as_str().expect("terminal id");
    eventually("the project on the agent's screen", || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        let text: String = screen["lines"]
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        (text.contains("part of the project \"loose\"") && text.contains("Goal: Ship it"))
            .then_some(())
    });
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}
