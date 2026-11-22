//! `massorca team ...` end to end: teams, bots and team projects in JSON and text, the Board, and the exit codes of refusals.

mod harness;

use assert_cmd::assert::Assert;
use harness::{Sandbox, single_document, stdout_text, with_json};

fn stderr_text(outcome: &Assert) -> String {
    String::from_utf8_lossy(&outcome.get_output().stderr).into_owned()
}

#[test]
fn a_team_is_created_listed_shown_and_removed_in_json_and_text() {
    let sandbox = Sandbox::new();

    let created = sandbox.json_ok(
        "default",
        &[
            "team",
            "create",
            "alpha",
            "--purpose",
            "Ships the parser",
            "--character",
            "square",
        ],
    );
    assert_eq!(created["name"], "alpha");
    assert_eq!(created["character"], "square");
    assert_eq!(created["purpose"], "Ships the parser");

    let listed = sandbox.json_ok("default", &["team", "list"]);
    assert_eq!(listed["teams"].as_array().map(Vec::len), Some(1));

    let shown = sandbox.run("default", &["team", "show", "alpha"]).success();
    assert!(stdout_text(&shown).contains("name: alpha"));

    let removed = sandbox.json_ok("default", &["team", "rm", "alpha"]);
    assert_eq!(removed["id"], created["id"]);
    sandbox
        .run("default", &["team", "show", "alpha"])
        .failure()
        .code(1);
}

#[test]
fn bots_and_projects_are_added_and_their_refusals_exit_with_code_one() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();

    let mara = sandbox.json_ok(
        "default",
        &[
            "team",
            "bot",
            "add",
            "alpha",
            "mara",
            "--agent",
            "claude",
            "--job",
            "Writes the parser",
            "--lead",
        ],
    );
    assert_eq!(mara["lead"], true);
    assert_eq!(mara["kind"], "claude");

    let second_lead = sandbox
        .run(
            "default",
            &[
                "team", "bot", "add", "alpha", "bruno", "--agent", "claude", "--job", "Reviews",
                "--lead",
            ],
        )
        .failure()
        .code(1);
    assert!(stderr_text(&second_lead).contains("bot.lead_taken"));

    let taken = sandbox
        .run("default", &["team", "create", "alpha"])
        .failure()
        .code(1);
    assert!(stderr_text(&taken).contains("team.name_in_use"));

    let project = sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "add",
            "alpha",
            "parser",
            "--goal",
            "Parse the config",
        ],
    );
    assert_eq!(project["goal"], "Parse the config");

    let board = sandbox.json_ok("default", &["team", "board", "alpha"]);
    let columns = board["columns"].as_array().expect("four columns");
    let names: Vec<&str> = columns
        .iter()
        .map(|column| column["column"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(names, ["open", "running", "waiting", "done"]);

    let text = sandbox
        .run("default", &["team", "board", "alpha"])
        .success();
    assert!(stdout_text(&text).starts_with("Open (0)"));

    sandbox
        .run("default", &["team", "bot", "rm", "alpha", "mara"])
        .success();
}

#[test]
fn a_refusal_in_json_is_an_error_document_with_its_code() {
    let sandbox = Sandbox::new();

    let refused = sandbox
        .run("default", &with_json(&["team", "show", "nothing"]))
        .failure()
        .code(1);
    let document = single_document(&refused);
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "team.not_found");
}
