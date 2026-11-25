//! `massorca team room` end to end (decision 0102): a person's line becomes a started task for the bot it names, the
//! bot answers in the room and posts again when the task is done, a bot posts with --bot, a team without bots
//! refuses the line, and a line no bot can be picked for gets a choice card answered once.

mod harness;

use harness::{Sandbox, eventually, single_document, with_json};
use predicates::prelude::PredicateBooleanExt;
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

/// A sandbox whose workspaces go under `<home>/ws`, with team `alpha` and kind `fake`.
fn team_sandbox() -> Sandbox {
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
    sandbox
}

fn add_bot(sandbox: &Sandbox, name: &str) {
    sandbox.json_ok(
        "default",
        &[
            "team",
            "bot",
            "add",
            "alpha",
            name,
            "--agent",
            "fake",
            "--job",
            "Writes the parser",
        ],
    );
}

fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure().code(1);
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

/// The room's lines as `author: text`.
fn room(sandbox: &Sandbox) -> Vec<String> {
    let listed = sandbox.json_ok("default", &["team", "room", "list", "alpha"]);
    listed["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|line| {
            let author = line["bot_name"].as_str().unwrap_or("you");
            format!("{author}: {}", line["text"].as_str().unwrap_or_default())
        })
        .collect()
}

#[test]
fn a_team_without_bots_refuses_a_line_and_keeps_its_room_empty() {
    let sandbox = team_sandbox();
    let refused = error_of(
        &sandbox,
        &["team", "room", "post", "alpha", "Parse the headers"],
    );
    assert_eq!(refused["code"], "task.no_bot", "{refused}");
    assert_eq!(refused["message"], "Add a bot to this team first.");
    assert!(room(&sandbox).is_empty());
    sandbox
        .run("default", &["team", "room", "list", "alpha"])
        .success()
        .stdout(predicates::str::contains("no lines"));
}

#[test]
fn a_line_becomes_a_started_task_for_the_bot_it_names_and_the_bot_answers() {
    let sandbox = team_sandbox();
    add_bot(&sandbox, "mara");
    add_bot(&sandbox, "bruno");

    // No "hold" and no lower-case "fail": the fake agent asks once, then reports done with success.
    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "@bruno Parse the headers\nthen stop.",
        ],
    );
    let lines = posted["messages"].as_array().expect("posted lines");
    assert_eq!(lines.len(), 2, "{posted}");
    assert_eq!(lines[0]["author"], "person");
    assert_eq!(lines[1]["author"], "bot");
    assert_eq!(lines[1]["bot_name"], "bruno");
    assert_eq!(lines[1]["text"], "Added to the Board: Parse the headers");
    let task = &lines[1]["task"];
    assert_eq!(task["origin"], "room", "{task}");
    assert_eq!(task["column"], "running", "{task}");
    let task_id = task["id"].as_str().expect("task id").to_owned();
    let session = task["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();

    // A bot posts its own line with --bot.
    sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "Reading the headers now",
            "--bot",
            "bruno",
        ],
    );

    // The fake agent asks once; the answer lets it finish, and the task's bot posts that it is done.
    let asking = sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            &session,
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
    eventually("the bot to post that the task is done", || {
        room(&sandbox)
            .contains(&"bruno: Done: Parse the headers".to_owned())
            .then_some(())
    });
    assert_eq!(
        room(&sandbox),
        [
            "you: @bruno Parse the headers\nthen stop.",
            "bruno: Added to the Board: Parse the headers",
            "bruno: Reading the headers now",
            "bruno: Done: Parse the headers",
        ]
    );
    let shown = sandbox.json_ok("default", &["task", "show", task_id.as_str()]);
    assert_eq!(shown["bot_id"], lines[1]["bot_id"]);
}

#[test]
fn a_line_goes_to_the_receiver_named_with_to_and_a_false_agent_token_is_refused() {
    let sandbox = team_sandbox();
    add_bot(&sandbox, "mara");
    add_bot(&sandbox, "bruno");

    // "hold" keeps the fake agent working, so the run is still live below.
    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "Start with hold",
            "--to",
            "bruno",
        ],
    );
    let answer = &posted["messages"][1];
    assert_eq!(answer["bot_name"], "bruno", "{posted}");
    let session = answer["task"]["session_id"]
        .as_str()
        .expect("linked session")
        .to_owned();

    // Inside an agent run, the CLI sends the run's session and token; a token that is not the run's is refused.
    sandbox.set_env("MASSORCA_AGENT_SESSION", &session);
    sandbox.set_env("MASSORCA_AGENT_TOKEN", "not-the-run-token");
    let refused = error_of(
        &sandbox,
        &[
            "team",
            "room",
            "post",
            "alpha",
            "I am bruno",
            "--bot",
            "mara",
        ],
    );
    assert_eq!(refused["code"], "permission.denied", "{refused}");
    assert_eq!(room(&sandbox).len(), 2, "nothing was posted");
}

#[test]
fn a_line_with_a_day_and_a_time_becomes_a_one_time_routine_instead_of_a_task() {
    let sandbox = team_sandbox();
    add_bot(&sandbox, "mara");

    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "Back up the docs on 2099-01-02 at 09:00",
        ],
    );
    let lines = posted["messages"].as_array().expect("posted lines");
    assert_eq!(lines.len(), 2, "{posted}");
    let routine = &lines[1]["routine"];
    assert_eq!(routine["cadence"], "once", "{posted}");
    assert_eq!(routine["run_date"], "2099-01-02");
    assert_eq!(routine["time"], "09:00");
    assert!(lines[1].get("task").is_none(), "no task is made: {posted}");
    assert_eq!(
        lines[1]["text"],
        "Routine created: Back up the docs on 2099-01-02 at 09:00. It runs once on 2099-01-02 at 09:00."
    );

    let routines = sandbox.json_ok("default", &["team", "routine", "list", "alpha"]);
    assert_eq!(routines["routines"][0]["id"], routine["id"], "{routines}");
    let board = sandbox.json_ok("default", &["task", "list", "--team", "alpha"]);
    assert_eq!(board["tasks"].as_array().map(Vec::len), Some(0), "{board}");
    sandbox
        .run("default", &["team", "room", "list", "alpha"])
        .success()
        .stdout(predicates::str::contains(format!(
            "routine {} (2099-01-02 09:00)",
            routine["id"].as_str().expect("routine id")
        )));
}

#[test]
fn a_refused_routine_or_a_time_only_mentioned_leaves_the_line_a_task() {
    let sandbox = team_sandbox();
    add_bot(&sandbox, "mara");

    for text in [
        "Rotate the logs on 2000-01-01 at 09:00",
        "The 5pm deploy today will break login, please fix now",
    ] {
        let posted = sandbox.json_ok("default", &["team", "room", "post", "alpha", text]);
        let answer = &posted["messages"][1];
        assert!(answer.get("routine").is_none(), "{posted}");
        assert!(
            answer["text"]
                .as_str()
                .is_some_and(|line| line.starts_with("Added to the Board")),
            "{posted}"
        );
        assert_eq!(answer["task"]["body"], text, "{posted}");
    }
    let routines = sandbox.json_ok("default", &["team", "routine", "list", "alpha"]);
    assert_eq!(routines["routines"].as_array().map(Vec::len), Some(0));
}

#[test]
fn no_start_leaves_the_task_in_todo_with_the_mention_left_out_of_its_title() {
    let sandbox = team_sandbox();
    add_bot(&sandbox, "mara");

    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "@mara, parse the headers",
            "--no-start",
        ],
    );
    let answer = &posted["messages"][1];
    assert_eq!(
        answer["text"], "Added to the Board, not started: parse the headers",
        "{posted}"
    );
    assert_eq!(answer["task"]["title"], "parse the headers");
    assert_eq!(answer["task"]["column"], "open", "it waits in Todo");
    assert!(answer["task"].get("session_id").is_none(), "{posted}");
    sandbox
        .run(
            "default",
            &[
                "team",
                "room",
                "post",
                "alpha",
                "hi",
                "--bot",
                "mara",
                "--no-start",
            ],
        )
        .failure();
}

#[test]
fn a_card_answered_with_a_bot_starts_its_task_and_a_second_answer_is_refused() {
    let sandbox = team_sandbox();
    add_bot(&sandbox, "mara");
    add_bot(&sandbox, "bruno");

    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "Parse the headers with hold",
        ],
    );
    let card = &posted["messages"][1];
    let card_id = card["id"].as_str().expect("card id").to_owned();
    let options = card["card"]["options"].as_array().expect("options");
    assert_eq!(options.len(), 3, "two bots and this team's Todo: {posted}");
    let bruno = options
        .iter()
        .find(|option| option["label"] == "bruno")
        .expect("bruno is offered")["choice"]
        .as_str()
        .expect("choice")
        .to_owned();
    let board = sandbox.json_ok("default", &["task", "list", "--team", "alpha"]);
    assert_eq!(board["tasks"].as_array().map(Vec::len), Some(0), "{board}");
    sandbox
        .run("default", &["team", "room", "list", "alpha"])
        .success()
        .stdout(
            predicates::str::contains(format!("card {card_id}: bot:"))
                .and(predicates::str::contains(format!("{bruno} (bruno)"))),
        );

    let answered = sandbox.json_ok(
        "default",
        &["team", "room", "answer", "alpha", &card_id, &bruno],
    );
    assert_eq!(
        answered["card"]["card"]["answer"],
        bruno.as_str(),
        "{answered}"
    );
    let line = &answered["messages"][0];
    assert_eq!(line["bot_name"], "bruno", "{answered}");
    assert_eq!(
        line["text"],
        "Added to the Board: Parse the headers with hold"
    );
    assert_eq!(line["task"]["column"], "running", "the fake agent started");

    let refused = error_of(
        &sandbox,
        &["team", "room", "answer", "alpha", &card_id, "todo"],
    );
    assert_eq!(refused["code"], "team.room_card_answered", "{refused}");
    sandbox
        .run("default", &["team", "room", "list", "alpha"])
        .success()
        .stdout(predicates::str::contains(format!(
            "card {card_id}: answered bruno"
        )));
    let board = sandbox.json_ok("default", &["task", "list", "--team", "alpha"]);
    assert_eq!(board["tasks"].as_array().map(Vec::len), Some(1), "{board}");
}
