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
