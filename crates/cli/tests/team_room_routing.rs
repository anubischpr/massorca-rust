//! `massorca team room post --route` end to end (AUTO-13): one routing query, run with the agent Settings names as
//! default, sends a line that names no bot to a bot, another team or a conversation, and an unclear answer gets a choice
//! card. The fake agent's answer is its whole prompt, so the request's own last `route=` decides here.

mod harness;

use harness::{Sandbox, eventually};
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

/// Team alpha (fake bots mara and bruno, no lead) and team beta (its fake bot iris), with `fake` as the default agent.
fn sandbox() -> Sandbox {
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
    sandbox.json_ok("default", &["config", "set", "agents.default_kind", "fake"]);
    for (team, bots) in [("alpha", &["mara", "bruno"][..]), ("beta", &["iris"][..])] {
        sandbox.json_ok("default", &["team", "create", team]);
        for bot in bots {
            sandbox.json_ok(
                "default",
                &[
                    "team",
                    "bot",
                    "add",
                    team,
                    bot,
                    "--agent",
                    "fake",
                    "--job",
                    "Writes the parser",
                ],
            );
        }
    }
    sandbox
}

fn room(sandbox: &Sandbox, team: &str) -> Vec<Value> {
    sandbox.json_ok("default", &["team", "room", "list", team])["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Posts `text` with `--route` and `--no-start`, and returns the person's line id.
fn routed(sandbox: &Sandbox, text: &str) -> String {
    let posted = sandbox.json_ok(
        "default",
        &[
            "team",
            "room",
            "post",
            "alpha",
            text,
            "--route",
            "--no-start",
        ],
    );
    let lines = posted["messages"].as_array().expect("lines");
    assert_eq!(lines[0]["author"], "person", "{posted}");
    lines[0]["id"].as_str().expect("id").to_owned()
}

/// The room's lines after the person's line `id`, once there are some.
fn after(sandbox: &Sandbox, id: &str) -> Vec<Value> {
    eventually("the routed line to be placed", || {
        let lines = room(sandbox, "alpha");
        let at = lines.iter().position(|line| line["id"] == id)?;
        let rest: Vec<Value> = lines[at + 1..].to_vec();
        (!rest.is_empty()).then_some(rest)
    })
}

#[test]
fn the_routing_query_gives_a_line_to_the_bot_it_names() {
    let sandbox = sandbox();
    let id = routed(&sandbox, "Tidy the parser please route=bot:bruno");
    let placed = after(&sandbox, &id);
    assert_eq!(placed[0]["bot_name"], "bruno", "{placed:?}");
    assert_eq!(
        placed[0]["text"],
        "Added to the Board, not started: Tidy the parser please route=bot:bruno"
    );
    assert_eq!(placed[0]["task"]["column"], "open", "{placed:?}");
}

#[test]
fn the_routing_query_sends_a_line_to_another_team_or_answers_it_as_a_conversation() {
    let sandbox = sandbox();
    let id = routed(&sandbox, "Redraw the diagram route=team:beta");
    let placed = after(&sandbox, &id);
    assert_eq!(
        placed[0]["text"],
        "Sent to team beta, in its Todo: Redraw the diagram route=team:beta"
    );
    let beta = sandbox.json_ok("default", &["task", "list", "--team", "beta"]);
    assert_eq!(beta["tasks"].as_array().map(Vec::len), Some(1), "{beta}");

    let asked = routed(&sandbox, "What is the parser for? route=conversation");
    let answer = eventually("a bot to answer", || {
        room(&sandbox, "alpha")
            .into_iter()
            .find(|line| line["author"] == "bot" && line["reply_to"] == asked.as_str())
    });
    assert_eq!(
        answer["bot_name"], "mara",
        "the first bot answers: {answer}"
    );
    let board = sandbox.json_ok("default", &["task", "list", "--team", "alpha"]);
    assert_eq!(board["tasks"].as_array().map(Vec::len), Some(0), "{board}");
}

#[test]
fn an_unclear_route_gets_a_choice_card() {
    let sandbox = sandbox();
    let id = routed(&sandbox, "Something about the docs");
    let placed = after(&sandbox, &id);
    let card = &placed[0]["card"];
    assert_eq!(card["request_id"], id.as_str(), "{placed:?}");
    assert!(
        placed[0]["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("The room could not tell who should take this.")),
        "{placed:?}"
    );
    let choices: Vec<&str> = card["options"]
        .as_array()
        .expect("options")
        .iter()
        .filter_map(|option| option["label"].as_str())
        .collect();
    assert_eq!(
        choices,
        ["mara", "bruno", "beta", "Keep in this team's Todo"]
    );
}
