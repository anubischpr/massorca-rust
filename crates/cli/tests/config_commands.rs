//! Settings through the CLI: the set, get and list round trip, and the exit codes of rejected changes.

mod harness;

use harness::{Sandbox, stdout_text};
use predicates::str::contains;

#[test]
fn config_set_then_get_and_list_round_trip_a_path_setting() {
    let sandbox = Sandbox::new();
    let root = sandbox
        .home()
        .join("workspaces")
        .to_string_lossy()
        .into_owned();

    sandbox
        .run(
            "default",
            &["config", "set", "workspaces.default_root", root.as_str()],
        )
        .success()
        .stdout(contains(root.as_str()));

    let got = sandbox.json_ok("default", &["config", "get", "workspaces.default_root"]);
    assert_eq!(got["owner"], "core");
    assert_eq!(got["key"], "workspaces.default_root");
    assert_eq!(got["value"], root.as_str());
    assert_eq!(got["is_default"], false);

    let listed = sandbox.json_ok("default", &["config", "list"]);
    let entry = listed["values"]
        .as_array()
        .and_then(|values| {
            values
                .iter()
                .find(|value| value["key"] == "workspaces.default_root")
        })
        .expect("the changed setting is listed");
    assert_eq!(entry["value"], root.as_str());

    let text = sandbox
        .run("default", &["config", "get", "workspaces.default_root"])
        .success();
    assert_eq!(stdout_text(&text), format!("{root}\n"));
}

#[test]
fn config_get_with_owner_core_reads_the_core_setting() {
    let sandbox = Sandbox::new();

    let got = sandbox.json_ok(
        "default",
        &[
            "config",
            "get",
            "workspaces.default_root",
            "--owner",
            "core",
        ],
    );

    assert_eq!(got["owner"], "core");
    assert_eq!(got["is_default"], true);
}

#[test]
fn config_set_rejects_a_relative_path_with_exit_1_and_keeps_the_default() {
    let sandbox = Sandbox::new();

    sandbox
        .run(
            "default",
            &["config", "set", "workspaces.default_root", "relative/path"],
        )
        .code(1)
        .stderr(contains("config.invalid_value"));

    let got = sandbox.json_ok("default", &["config", "get", "workspaces.default_root"]);
    assert_eq!(got["is_default"], true);
}

#[test]
fn config_get_of_an_unknown_key_exits_1_with_its_code() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["config", "get", "no.such.key"])
        .code(1)
        .stderr(contains("config.unknown_key"));
}

#[test]
fn settings_set_reset_and_list_a_choice_setting() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["settings", "set", "appearance.theme", "dark"])
        .success()
        .stdout(contains("appearance.theme = dark"));
    let got = sandbox.json_ok("default", &["settings", "get", "appearance.theme"]);
    assert_eq!(got["value"], "dark");
    assert_eq!(got["kind"], "choice");
    assert_eq!(
        got["choices"],
        serde_json::json!(["system", "dark", "light"])
    );

    sandbox
        .run(
            "default",
            &["settings", "set", "appearance.theme", "purple"],
        )
        .code(1)
        .stderr(contains("config.invalid_value"))
        .stderr(contains("system, dark, light"));

    sandbox
        .run("default", &["settings", "reset", "appearance.theme"])
        .success()
        .stdout(contains("appearance.theme = system (default)"));
    let listed = sandbox.json_ok("default", &["settings", "list"]);
    let theme = listed["values"]
        .as_array()
        .and_then(|values| {
            values
                .iter()
                .find(|value| value["key"] == "appearance.theme")
        })
        .expect("the theme is listed");
    assert_eq!(theme["value"], "system");
    assert_eq!(theme["is_default"], true);
}
