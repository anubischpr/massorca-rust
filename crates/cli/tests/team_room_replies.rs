//! `massorca team room post --ask` and `--reply-to` end to end (decision 0122): a conversation gets one answer from one
//! bot, posted as that bot and cut to 1,200 characters, with no task on the Board; a reply to a bot's line goes to that
//! bot and the room shows it typing while its run works; `--task` keeps a reply a task with the quoted line.

mod harness;

use harness::{Sandbox, eventually, single_document, with_json};
use serde_json::{Value, json};

/// Declares kind `fake`, which runs the fake agent with an argv prompt and reports its activity and result.
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

/// A sandbox whose workspaces go under `<home>/ws`, with team `alpha` and its fake bots mara and bruno.
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
    sandbox.json_ok("default", &["team", "create", "alpha"]);
    for name in ["mara", "bruno"] {
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
    sandbox
}

fn room(sandbox: &Sandbox) -> Value {
    sandbox.json_ok("default", &["team", "room", "list", "alpha"])
}

/// The bot line that answers `question`, once there is one.
fn answer_to(sandbox: &Sandbox, question: &str) -> Option<Value> {
    room(sandbox)["messages"]
        .as_array()?
        .iter()
        .find(|line| line["author"] == "bot" && line["reply_to"] == question)
        .cloned()
}

fn no_tasks(sandbox: &Sandbox) {
    let board = sandbox.json_ok("default", &["task", "list", "--team", "alpha"]);
    assert_eq!(
        board["tasks"].as_array().map(Vec::len),
        Some(0),
        "a reply is not a task: {board}"
    );
}

/// A line `mara` posts, and its id.
fn mara_says(sandbox: &Sandbox, text: &str) -> String {
    let posted = sandbox.json_ok(
        "default",
        &["team", "room", "post", "alpha", text, "--bot", "mara"],
    );
    posted["messages"][0]["id"]
        .as_str()
        .expect("line id")
        .to_owned()
}

#[test]
fn a_question_with_ask_gets_one_answer_from_the_bot_it_names_cut_to_1200_characters() {
    let sandbox = team_sandbox();
    let question = format!("@bruno what does the parser do {}", "and why ".repeat(200));
    let posted = sandbox.json_ok(
        "default",
        &["team", "room", "post", "alpha", question.as_str(), "--ask"],
    );
    let asked = &posted["messages"][0];
    assert_eq!(asked["author"], "person", "{posted}");
    assert!(asked.get("task").is_none(), "{posted}");
    let id = asked["id"].as_str().expect("line id").to_owned();

    let answer = eventually("bruno to answer", || answer_to(&sandbox, &id));
    assert_eq!(answer["bot_name"], "bruno", "{answer}");
    assert!(answer.get("task").is_none(), "{answer}");
    let text = answer["text"].as_str().expect("answer text");
    // The fake agent's result is "done: " and its whole prompt, which is longer than an answer may be.
    assert!(
        text.starts_with("done: You are bruno, a bot in the team alpha."),
        "{text}"
    );
    assert_eq!(text.chars().count(), 1200, "{text}");
    assert!(text.ends_with('…'));

    let answers = room(&sandbox)["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter(|line| line["reply_to"] == id.as_str())
        .count();
    assert_eq!(answers, 1, "one answer");
    no_tasks(&sandbox);
    eventually("the answered run to stop typing", || {
        room(&sandbox).get("typing").is_none().then_some(())
    });
}

#[test]
fn a_reply_to_a_bot_line_goes_to_that_bot_which_types_while_its_run_works() {
    let sandbox = team_sandbox();
    let said = mara_says(&sandbox, "Parsed the headers");

    // "hold" keeps the fake agent working, so the run is still typing below.
    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "why did it hold so long",
            "--reply-to",
            said.as_str(),
        ],
    );
    let asked = &posted["messages"][0];
    assert_eq!(asked["reply_to"], said.as_str(), "{posted}");
    assert_eq!(asked["quoted"]["bot_name"], "mara", "{posted}");
    assert_eq!(asked["quoted"]["text"], "Parsed the headers");
    eventually("mara to type", || {
        let typing = room(&sandbox)["typing"].clone();
        (typing.as_array().map(Vec::len) == Some(1) && typing[0]["bot_name"] == "mara")
            .then_some(())
    });
    sandbox
        .run("default", &["team", "room", "list", "alpha"])
        .success()
        .stdout(predicates::str::contains("mara is typing…"))
        .stdout(predicates::str::contains(
            "you\twhy did it hold so long\treplying to mara: Parsed the headers",
        ));
    no_tasks(&sandbox);
}

#[test]
fn a_reply_with_task_stays_a_task_that_keeps_the_quoted_line() {
    let sandbox = team_sandbox();
    let said = mara_says(&sandbox, "Parsed the headers");
    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            "Now cover the footers",
            "--reply-to",
            said.as_str(),
            "--task",
            "--no-start",
        ],
    );
    let answer = &posted["messages"][1];
    assert_eq!(answer["bot_name"], "mara", "{posted}");
    assert_eq!(answer["task"]["title"], "Now cover the footers");
    assert_eq!(
        answer["task"]["body"],
        "Now cover the footers\n\nIn reply to mara: Parsed the headers"
    );

    let refused = sandbox
        .run(
            "default",
            &with_json(&[
                "team",
                "room",
                "post",
                "alpha",
                "Why?",
                "--reply-to",
                "no-line",
            ]),
        )
        .failure()
        .code(1);
    let document = single_document(&refused);
    assert_eq!(
        document["error"]["code"], "team.room_line_not_found",
        "{document}"
    );
    sandbox
        .run(
            "default",
            &["team", "room", "post", "alpha", "Why?", "--ask", "--task"],
        )
        .failure()
        .code(2);
}
