//! `massorca skills bundled list|install|refresh|remove|show` (SKL-04), through the CLI and a service that writes a fake home
//! folder. `show` answers from this build alone and prints the checked-in skill text.

mod harness;

use std::fs;

use harness::{Sandbox, stdout_text};
use massorca_server::SKILL_HOME_ENV;
use predicates::str::contains;
use serde_json::Value;

/// The text this build ships, as the checked-in file.
const SHIPPED: &str = include_str!("../../../packages/skills/massorca/SKILL.md");

/// A sandbox whose service writes `user-home` as the home folder, with a Claude skill folder so Claude is enabled.
fn sandbox_with_claude() -> Sandbox {
    let sandbox = Sandbox::new();
    let user_home = sandbox.home().join("user-home");
    fs::create_dir_all(user_home.join(".claude/skills")).expect("the claude folder");
    sandbox.set_env(SKILL_HOME_ENV, &user_home.to_string_lossy());
    sandbox
}

/// The state the list shows for the MassOrca skill.
fn massorca_state(listed: &Value) -> String {
    listed["skills"]
        .as_array()
        .expect("a skills array")
        .iter()
        .find(|skill| skill["name"] == "massorca")
        .unwrap_or_else(|| panic!("no massorca skill in {listed}"))["state"]
        .as_str()
        .expect("a state")
        .to_owned()
}

fn state_of(result: &Value, agent: &str) -> String {
    result["agents"]
        .as_array()
        .expect("an agents array")
        .iter()
        .find(|row| row["agent"] == agent)
        .unwrap_or_else(|| panic!("no row for {agent} in {result}"))["state"]
        .as_str()
        .expect("a state")
        .to_owned()
}

#[test]
fn bundled_install_places_the_skill_for_each_enabled_agent_and_list_reports_it() {
    let sandbox = sandbox_with_claude();
    let user_home = sandbox.home().join("user-home");

    let listed = sandbox.json_ok("default", &["skills", "bundled", "list"]);
    assert_eq!(massorca_state(&listed), "not_placed", "{listed}");

    let installed = sandbox.json_ok("default", &["skills", "bundled", "install", "massorca"]);
    assert_eq!(state_of(&installed, "claude"), "placed", "{installed}");
    assert_eq!(
        fs::read_to_string(user_home.join(".claude/skills/massorca/SKILL.md"))
            .expect("the Claude folder reads the skill"),
        SHIPPED
    );

    let listed = sandbox.json_ok("default", &["skills", "bundled", "list"]);
    assert_eq!(massorca_state(&listed), "current", "{listed}");
    sandbox
        .run("default", &["skills", "bundled", "list"])
        .success()
        .stdout(contains("massorca  installed for claude"));
}

#[test]
fn bundled_refresh_reports_copies_already_current_and_remove_takes_them_away() {
    let sandbox = sandbox_with_claude();
    let user_home = sandbox.home().join("user-home");
    sandbox.json_ok("default", &["skills", "bundled", "install", "massorca"]);

    let refreshed = sandbox.json_ok("default", &["skills", "bundled", "refresh", "massorca"]);
    assert_eq!(
        state_of(&refreshed, "claude"),
        "already_placed",
        "{refreshed}"
    );

    let removed = sandbox.json_ok("default", &["skills", "unplace", "massorca"]);
    assert_eq!(state_of(&removed, "claude"), "removed", "{removed}");
    assert!(!user_home.join(".claude/skills/massorca").exists());
    assert!(!user_home.join(".agents/skills/massorca").exists());
}

#[test]
fn bundled_install_refuses_a_name_this_build_does_not_ship() {
    let sandbox = sandbox_with_claude();
    sandbox
        .run("default", &["skills", "bundled", "install", "not-shipped"])
        .failure()
        .stderr(contains("MassOrca ships no skill with that name"));
}

#[test]
fn bundled_show_prints_the_checked_in_text_without_a_service() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run("default", &["skills", "bundled", "show", "massorca"]);
    assert_eq!(stdout_text(&outcome), SHIPPED);
    assert!(
        !sandbox.profile_dir("default").join("skills").exists(),
        "show writes nothing"
    );
}
