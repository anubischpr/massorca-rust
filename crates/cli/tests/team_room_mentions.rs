//! `massorca team room mentions` and `team room resolve` end to end (AUTO-13): an `@` suggests the team's bots, the
//! other teams and `@file:`, a line's mentions resolve to what they name, and an unknown one is refused with its code.

mod harness;

use harness::{Sandbox, single_document, with_json};

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    for team in ["alpha", "beta"] {
        sandbox.json_ok("default", &["team", "create", team]);
    }
    sandbox.json_ok(
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
    sandbox
}

#[test]
fn an_at_suggests_bots_teams_and_files_and_a_line_resolves_its_mentions() {
    let sandbox = sandbox();
    let all = sandbox.json_ok("default", &["team", "room", "mentions", "alpha"]);
    let inserts: Vec<&str> = all["mentions"]
        .as_array()
        .expect("mentions")
        .iter()
        .filter_map(|mention| mention["insert"].as_str())
        .collect();
    assert_eq!(inserts, ["@mara", "@team:beta", "@file:"], "{all}");
    sandbox
        .run("default", &["team", "room", "mentions", "alpha", "team:"])
        .success()
        .stdout("team\t@team:beta\n");

    let resolved = sandbox.json_ok(
        "default",
        &["team", "room", "resolve", "alpha", "@mara ask @team:beta"],
    );
    assert_eq!(resolved["mentions"][0]["kind"], "bot", "{resolved}");
    assert_eq!(resolved["mentions"][1]["kind"], "team", "{resolved}");

    let refused = sandbox
        .run(
            "default",
            &with_json(&["team", "room", "resolve", "alpha", "ask @team:gamma"]),
        )
        .failure()
        .code(1);
    let document = single_document(&refused);
    assert_eq!(
        document["error"]["code"], "team.room_mention_unknown",
        "{document}"
    );
}
