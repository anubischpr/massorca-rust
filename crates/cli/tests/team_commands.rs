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

#[test]
fn the_board_round_trips_through_create_done_and_reopen_in_json_and_text() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();
    sandbox
        .run(
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
            ],
        )
        .success();
    sandbox
        .run("default", &["team", "project", "add", "alpha", "parser"])
        .success();

    let task = sandbox.json_ok(
        "default",
        &[
            "task",
            "create",
            "alpha",
            "parse the header",
            "--bot",
            "mara",
            "--project",
            "parser",
            "--body",
            "Magic bytes first.",
        ],
    );
    assert_eq!(task["column"], "open");
    assert_eq!(task["state"], "not_started");
    let id = task["id"].as_str().expect("the task id").to_owned();

    let board = sandbox.json_ok("default", &["team", "board", "alpha"]);
    let open_cards = board["columns"][0]["cards"].as_array().expect("cards");
    assert_eq!(open_cards.len(), 1);
    assert_eq!(open_cards[0]["bot"], "mara");
    let text = sandbox
        .run("default", &["team", "board", "alpha"])
        .success();
    assert!(stdout_text(&text).contains("parse the header"));
    assert!(stdout_text(&text).contains("Not started"));

    let done = sandbox.json_ok("default", &["task", "done", id.as_str()]);
    assert_eq!(done["column"], "done");
    let done_board = sandbox.json_ok("default", &["team", "board", "alpha"]);
    assert_eq!(
        done_board["columns"][3]["cards"].as_array().map(Vec::len),
        Some(1)
    );

    let reopened = sandbox.json_ok("default", &["task", "reopen", id.as_str()]);
    assert_eq!(reopened["column"], "open");
    let listed = sandbox.json_ok(
        "default",
        &["task", "list", "--team", "alpha", "--column", "open"],
    );
    assert_eq!(listed["tasks"].as_array().map(Vec::len), Some(1));

    let shown = sandbox
        .run("default", &["task", "show", id.as_str()])
        .success();
    assert!(stdout_text(&shown).contains("parse the header"));

    sandbox.json_ok("default", &["task", "rm", id.as_str()]);
    sandbox
        .run("default", &["task", "show", id.as_str()])
        .failure()
        .code(1);
}

#[test]
fn task_refusals_exit_with_code_one_and_name_their_code() {
    let sandbox = Sandbox::new();

    let no_team = sandbox
        .run("default", &["task", "create", "nothing", "orphan"])
        .failure()
        .code(1);
    assert!(stderr_text(&no_team).contains("team.not_found"));

    let missing = sandbox
        .run("default", &["task", "show", "missing"])
        .failure()
        .code(1);
    assert!(stderr_text(&missing).contains("task.not_found"));
}

#[test]
fn a_team_is_updated_and_only_the_fields_given_change() {
    let sandbox = Sandbox::new();
    sandbox
        .run(
            "default",
            &["team", "create", "alpha", "--purpose", "old purpose"],
        )
        .success();

    let updated = sandbox.json_ok(
        "default",
        &[
            "team",
            "update",
            "alpha",
            "--name",
            "beta",
            "--character",
            "star",
            "--workspaces",
            "true",
        ],
    );
    assert_eq!(updated["name"], "beta");
    assert_eq!(updated["character"], "star");
    assert_eq!(updated["workspaces"], true);
    assert_eq!(updated["purpose"], "old purpose");

    let cleared = sandbox.json_ok("default", &["team", "update", "beta", "--purpose", ""]);
    assert_eq!(cleared["purpose"], "");

    let text = sandbox
        .run(
            "default",
            &["team", "update", "beta", "--workspaces", "false"],
        )
        .success();
    assert!(stdout_text(&text).starts_with("updated team beta"));
}

#[test]
fn a_bot_moves_its_lead_only_by_clearing_it_and_a_project_is_updated_and_removed() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();
    sandbox
        .run(
            "default",
            &[
                "team", "bot", "add", "alpha", "mara", "--agent", "claude", "--job", "Writes",
                "--lead",
            ],
        )
        .success();
    sandbox
        .run(
            "default",
            &[
                "team", "bot", "add", "alpha", "bruno", "--agent", "claude", "--job", "Reviews",
            ],
        )
        .success();

    let refused = sandbox
        .run(
            "default",
            &["team", "bot", "update", "alpha", "bruno", "--lead", "true"],
        )
        .failure()
        .code(1);
    assert!(stderr_text(&refused).contains("bot.lead_taken"));

    let stepped_down = sandbox.json_ok(
        "default",
        &[
            "team", "bot", "update", "alpha", "mara", "--lead", "false", "--model", "opus",
            "--effort", "high",
        ],
    );
    assert_eq!(stepped_down["lead"], false);
    assert_eq!(stepped_down["model"], "opus");
    let promoted = sandbox.json_ok(
        "default",
        &[
            "team",
            "bot",
            "update",
            "alpha",
            "bruno",
            "--lead",
            "true",
            "--job",
            "Reviews the parser",
        ],
    );
    assert_eq!(promoted["lead"], true);
    assert_eq!(promoted["job"], "Reviews the parser");
    let cleared = sandbox.json_ok(
        "default",
        &["team", "bot", "update", "alpha", "mara", "--model", ""],
    );
    assert!(cleared["model"].is_null());

    sandbox.json_ok("default", &["team", "project", "add", "alpha", "parser"]);
    let project = sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "update",
            "alpha",
            "parser",
            "--goal",
            "Parse the config",
            "--brief",
            "Keep it small.",
        ],
    );
    assert_eq!(project["goal"], "Parse the config");
    assert_eq!(project["brief"], "Keep it small.");
    let removed = sandbox.json_ok("default", &["team", "project", "rm", "alpha", "parser"]);
    assert_eq!(removed["name"], "parser");
    let gone = sandbox
        .run("default", &["team", "project", "rm", "alpha", "parser"])
        .failure()
        .code(1);
    assert!(stderr_text(&gone).contains("team.project_not_found"));
}

#[test]
fn a_task_is_updated_and_an_empty_bot_clears_it() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();
    sandbox
        .run(
            "default",
            &[
                "team", "bot", "add", "alpha", "mara", "--agent", "claude", "--job", "Writes",
            ],
        )
        .success();
    let task = sandbox.json_ok(
        "default",
        &["task", "create", "alpha", "parse", "--bot", "mara"],
    );
    let id = task["id"].as_str().expect("the task id").to_owned();
    assert!(!task["bot_id"].is_null());

    let updated = sandbox.json_ok(
        "default",
        &[
            "task",
            "update",
            id.as_str(),
            "--title",
            "parse the body",
            "--body",
            "Magic bytes first.",
            "--bot",
            "",
        ],
    );
    assert_eq!(updated["title"], "parse the body");
    assert_eq!(updated["body"], "Magic bytes first.");
    assert!(updated["bot_id"].is_null());

    let missing = sandbox
        .run("default", &["task", "update", "missing", "--title", "x"])
        .failure()
        .code(1);
    assert!(stderr_text(&missing).contains("task.not_found"));
}

#[test]
fn a_teams_bots_and_projects_are_listed_in_json_and_text() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();

    let none = sandbox
        .run("default", &["team", "bot", "list", "alpha"])
        .success();
    assert_eq!(stdout_text(&none).trim(), "no bots");
    let none = sandbox
        .run("default", &["team", "project", "list", "alpha"])
        .success();
    assert_eq!(stdout_text(&none).trim(), "no projects");

    sandbox
        .run(
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
        )
        .success();
    sandbox
        .run(
            "default",
            &[
                "team", "bot", "add", "alpha", "ada", "--agent", "codex", "--job", "Reviews",
            ],
        )
        .success();
    sandbox
        .run(
            "default",
            &[
                "team", "project", "add", "alpha", "parser", "--goal", "Ship it",
            ],
        )
        .success();

    let bots = sandbox.json_ok("default", &["team", "bot", "list", "alpha"]);
    let names: Vec<&str> = bots["bots"]
        .as_array()
        .expect("bots array")
        .iter()
        .filter_map(|bot| bot["name"].as_str())
        .collect();
    assert_eq!(names, ["mara", "ada"]);
    assert_eq!(bots["bots"][0]["lead"], true);

    let text = sandbox
        .run("default", &["team", "bot", "list", "alpha"])
        .success();
    let lines: Vec<String> = stdout_text(&text).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].starts_with("mara	claude	lead	Writes the parser	"),
        "{lines:?}"
    );
    assert!(lines[1].starts_with("ada	codex	-	Reviews	"), "{lines:?}");

    let projects = sandbox.json_ok("default", &["team", "project", "list", "alpha"]);
    assert_eq!(projects["projects"][0]["name"], "parser");
    assert_eq!(projects["projects"][0]["goal"], "Ship it");
    let text = sandbox
        .run("default", &["team", "project", "list", "alpha"])
        .success();
    assert!(stdout_text(&text).starts_with("parser	Ship it	"));

    let missing = sandbox
        .run("default", &["team", "bot", "list", "gamma"])
        .failure()
        .code(1);
    assert!(stderr_text(&missing).contains("gamma"));
    sandbox
        .run("default", &["team", "project", "list", "gamma"])
        .failure()
        .code(1);
}

#[test]
fn team_memory_lines_are_added_listed_and_removed_in_json_and_text() {
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
        ],
    );
    let task = sandbox.json_ok(
        "default",
        &[
            "task",
            "create",
            "alpha",
            "Parse the config",
            "--bot",
            "mara",
        ],
    );
    let task_id = task["id"].as_str().expect("the task id");

    let rule = sandbox.json_ok(
        "default",
        &["team", "memory", "add", "alpha", "Keep the parser small."],
    );
    assert_eq!(rule["source"], "person");
    assert!(rule.get("bot_id").is_none());
    let rule_id = rule["id"].as_str().expect("the line id").to_owned();

    let note = sandbox.json_ok(
        "default",
        &[
            "team",
            "memory",
            "add",
            "alpha",
            "Mara checks the tables.",
            "--bot",
            "mara",
        ],
    );
    assert_eq!(note["bot_id"], mara["id"]);

    let from_task = sandbox.json_ok(
        "default",
        &[
            "team",
            "memory",
            "add",
            "alpha",
            "The config keeps its keys.",
            "--bot",
            "mara",
            "--task",
            task_id,
        ],
    );
    assert_eq!(from_task["source"], "task");
    assert_eq!(from_task["task_id"], task["id"]);

    let listed = sandbox.json_ok("default", &["team", "memory", "list", "alpha"]);
    let texts: Vec<&str> = listed["lines"]
        .as_array()
        .expect("the lines")
        .iter()
        .map(|line| line["text"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        texts,
        [
            "The config keeps its keys.",
            "Mara checks the tables.",
            "Keep the parser small."
        ]
    );
    let bot_only = sandbox.json_ok(
        "default",
        &["team", "memory", "list", "alpha", "--bot", "mara"],
    );
    assert_eq!(bot_only["lines"].as_array().map(Vec::len), Some(2));

    let text = sandbox
        .run("default", &["team", "memory", "list", "alpha"])
        .success();
    assert!(stdout_text(&text).contains("Keep the parser small."));
    let added_text = sandbox
        .run(
            "default",
            &["team", "memory", "add", "alpha", "Short line."],
        )
        .success();
    assert!(stdout_text(&added_text).starts_with("added memory line"));

    let removed = sandbox.json_ok("default", &["team", "memory", "rm", "alpha", &rule_id]);
    assert_eq!(removed["id"], rule["id"]);
    let gone = sandbox
        .run("default", &["team", "memory", "rm", "alpha", &rule_id])
        .failure()
        .code(1);
    assert!(stderr_text(&gone).contains("team.memory_not_found"));
}

#[test]
fn team_memory_refusals_exit_with_code_one_and_name_their_code() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();

    let too_long = "a".repeat(501);
    let refused_long = sandbox
        .run(
            "default",
            &["team", "memory", "add", "alpha", too_long.as_str()],
        )
        .failure()
        .code(1);
    assert!(stderr_text(&refused_long).contains("request.invalid_params"));
    let refused_blank = sandbox
        .run("default", &["team", "memory", "add", "alpha", "   "])
        .failure()
        .code(1);
    assert!(stderr_text(&refused_blank).contains("request.invalid_params"));

    let unknown = sandbox
        .run("default", &["team", "memory", "list", "nowhere"])
        .failure()
        .code(1);
    assert!(stderr_text(&unknown).contains("team.not_found"));

    let refused = sandbox
        .run(
            "default",
            &with_json(&["team", "memory", "list", "nowhere"]),
        )
        .failure()
        .code(1);
    let document = single_document(&refused);
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "team.not_found");
}

#[test]
fn a_team_project_takes_a_workspaces_source_on_add_and_update_and_names_it_in_the_list() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_str().expect("utf-8 path");
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();
    let folder = sandbox.json_ok("default", &["project", "add", notes_text, "--folder"]);
    let folder_id = folder["id"].as_str().expect("project id").to_owned();

    let linked = sandbox.json_ok(
        "default",
        &[
            "team", "project", "add", "alpha", "parser", "--source", "notes",
        ],
    );
    assert_eq!(linked["source_project"], folder_id);
    assert_eq!(linked["source_project_name"], "notes");
    assert_eq!(linked["source_kind"], "folder");

    let plain = sandbox.json_ok("default", &["team", "project", "add", "alpha", "lexer"]);
    assert!(plain["source_project"].is_null());

    let moved = sandbox.json_ok(
        "default",
        &[
            "team", "project", "update", "alpha", "lexer", "--source", notes_text,
        ],
    );
    assert_eq!(moved["source_project"], folder_id);
    assert_eq!(moved["source_kind"], "folder");

    let listed = stdout_text(
        &sandbox
            .run("default", &["team", "project", "list", "alpha"])
            .success(),
    );
    let parser_line = listed
        .lines()
        .find(|line| line.starts_with("parser"))
        .expect("parser is listed");
    assert_eq!(parser_line.rsplit('\t').next(), Some("notes"), "{listed}");

    let cleared = sandbox.json_ok(
        "default",
        &[
            "team", "project", "update", "alpha", "parser", "--source", "",
        ],
    );
    assert!(cleared["source_project"].is_null());
    assert!(cleared["source_project_name"].is_null());
    let relisted = stdout_text(
        &sandbox
            .run("default", &["team", "project", "list", "alpha"])
            .success(),
    );
    let parser_line = relisted
        .lines()
        .find(|line| line.starts_with("parser"))
        .expect("parser is listed");
    assert_eq!(parser_line.rsplit('\t').next(), Some("-"), "{relisted}");

    let refused_add = sandbox
        .run(
            "default",
            &[
                "team", "project", "add", "alpha", "vanished", "--source", "nothing",
            ],
        )
        .failure()
        .code(1);
    assert!(stderr_text(&refused_add).contains("project.not_found"));
    let refused_update = sandbox
        .run(
            "default",
            &[
                "team", "project", "update", "alpha", "lexer", "--source", "nothing",
            ],
        )
        .failure()
        .code(1);
    assert!(stderr_text(&refused_update).contains("project.not_found"));
}

#[test]
fn a_team_project_records_its_code_source_only_when_the_team_has_workspaces() {
    let sandbox = Sandbox::new();
    sandbox.json_ok("default", &["team", "create", "alpha"]);

    let refused = sandbox
        .run(
            "default",
            &with_json(&[
                "team",
                "project",
                "add",
                "alpha",
                "parser",
                "--code-source",
                "new-folder-with-git",
            ]),
        )
        .failure()
        .code(1);
    let document = single_document(&refused);
    assert_eq!(
        document["error"]["code"], "request.invalid_params",
        "{document}"
    );

    sandbox.json_ok(
        "default",
        &["team", "update", "alpha", "--workspaces", "true"],
    );
    let added = sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "add",
            "alpha",
            "parser",
            "--code-source",
            "new-folder-with-git",
        ],
    );
    assert_eq!(added["code_source"], "new_folder_with_git", "{added}");
    let listed = stdout_text(
        &sandbox
            .run("default", &["team", "project", "list", "alpha"])
            .success(),
    );
    assert!(listed.contains("\tNew folder with git\t-"), "{listed}");

    let updated = sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "update",
            "alpha",
            "parser",
            "--code-source",
            "existing-repository",
        ],
    );
    assert_eq!(updated["code_source"], "existing_repository", "{updated}");
    // Recorded only: no Workspaces project is made or linked.
    assert!(updated["source_project"].is_null(), "{updated}");
    let projects = sandbox.json_ok("default", &["project", "list"]);
    assert_eq!(projects["projects"], serde_json::json!([]), "{projects}");
}

/// The titles of every card on a Board, column by column.
fn board_titles(board: &serde_json::Value) -> Vec<String> {
    board["columns"]
        .as_array()
        .expect("four columns")
        .iter()
        .flat_map(|column| column["cards"].as_array().into_iter().flatten())
        .filter_map(|card| card["title"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn the_board_without_a_project_lists_only_the_cards_no_project_holds() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["team", "create", "alpha"])
        .success();
    sandbox
        .run("default", &["team", "project", "add", "alpha", "parser"])
        .success();
    sandbox
        .run(
            "default",
            &[
                "task",
                "create",
                "alpha",
                "parse the header",
                "--project",
                "parser",
            ],
        )
        .success();
    sandbox
        .run("default", &["task", "create", "alpha", "write the readme"])
        .success();

    let all = sandbox.json_ok("default", &["team", "board", "alpha"]);
    assert_eq!(board_titles(&all), ["parse the header", "write the readme"]);
    let parser = sandbox.json_ok(
        "default",
        &["team", "board", "alpha", "--project", "parser"],
    );
    assert_eq!(board_titles(&parser), ["parse the header"]);
    let loose = sandbox.json_ok("default", &["team", "board", "alpha", "--no-project"]);
    assert_eq!(board_titles(&loose), ["write the readme"]);
    assert_eq!(loose["project_id"], serde_json::Value::Null);

    sandbox
        .run(
            "default",
            &[
                "team",
                "board",
                "alpha",
                "--project",
                "parser",
                "--no-project",
            ],
        )
        .failure();
}
