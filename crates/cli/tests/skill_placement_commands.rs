//! `massorca skills preview|place|unplace|managed`, through the CLI and a service that reads and writes a fake home folder.

mod harness;

use std::fs;
use std::path::Path;

use harness::Sandbox;
use massorca_server::SKILL_HOME_ENV;
use predicates::str::contains;
use serde_json::Value;

/// A `SKILL.md` named `name` in `user_home/relative`.
fn write_skill(user_home: &Path, relative: &str, name: &str) {
    let folder = user_home.join(relative);
    fs::create_dir_all(&folder).expect("the skill folder");
    let text = format!("---\nname: {name}\ndescription: {name} skill.\n---\n");
    fs::write(folder.join("SKILL.md"), text).expect("the skill file");
}

/// A sandbox whose service reads and writes `user-home` as the home folder, with one Claude skill named `notes`.
fn sandbox_with_notes() -> Sandbox {
    let sandbox = Sandbox::new();
    let user_home = sandbox.home().join("user-home");
    write_skill(&user_home, ".claude/skills/notes", "notes");
    sandbox.set_env(SKILL_HOME_ENV, &user_home.to_string_lossy());
    sandbox
}

fn notes_id(sandbox: &Sandbox) -> String {
    let installed = sandbox.json_ok("default", &["skills", "installed"]);
    installed["skills"][0]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("no id in {installed}"))
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
fn skills_place_copies_the_skill_for_each_chosen_agent_and_reports_each_as_json() {
    let sandbox = sandbox_with_notes();
    let id = notes_id(&sandbox);
    let user_home = sandbox.home().join("user-home");

    let placed = sandbox.json_ok(
        "default",
        &[
            "skills",
            "place",
            id.as_str(),
            "--agent",
            "cursor",
            "--agent",
            "codex",
        ],
    );

    assert_eq!(state_of(&placed, "cursor"), "placed", "{placed}");
    assert_eq!(state_of(&placed, "codex"), "canonical", "{placed}");
    assert!(user_home.join(".cursor/skills/notes/SKILL.md").is_file());
    assert!(user_home.join(".agents/skills/notes/SKILL.md").is_file());
}

#[test]
fn skills_preview_says_what_would_happen_and_writes_nothing() {
    let sandbox = sandbox_with_notes();
    let id = notes_id(&sandbox);
    let user_home = sandbox.home().join("user-home");

    let preview = sandbox.json_ok(
        "default",
        &[
            "skills",
            "preview",
            id.as_str(),
            "--agent",
            "cursor",
            "--agent",
            "claude",
        ],
    );

    assert_eq!(state_of(&preview, "cursor"), "create", "{preview}");
    assert_eq!(
        state_of(&preview, "claude"),
        "skipped_existing",
        "{preview}"
    );
    assert!(!user_home.join(".agents").exists());
    assert!(!user_home.join(".cursor").exists());
}

#[test]
fn skills_place_prints_one_line_per_agent_without_json() {
    let sandbox = sandbox_with_notes();
    let id = notes_id(&sandbox);

    sandbox
        .run(
            "default",
            &["skills", "place", id.as_str(), "--agent", "cursor"],
        )
        .success()
        .stdout(contains("notes"))
        .stdout(contains("cursor  placed"));
}

#[test]
fn skills_managed_lists_what_was_placed_and_unplace_removes_it() {
    let sandbox = sandbox_with_notes();
    let id = notes_id(&sandbox);
    let user_home = sandbox.home().join("user-home");
    sandbox.json_ok(
        "default",
        &["skills", "place", id.as_str(), "--agent", "gemini"],
    );

    let managed = sandbox.json_ok("default", &["skills", "managed"]);
    assert_eq!(managed["skills"][0]["name"], "notes", "{managed}");

    let removed = sandbox.json_ok("default", &["skills", "unplace", "notes"]);

    assert_eq!(state_of(&removed, "gemini"), "removed", "{removed}");
    assert!(!user_home.join(".gemini/skills/notes").exists());
    assert!(!user_home.join(".agents/skills/notes").exists());
    assert!(
        user_home.join(".claude/skills/notes/SKILL.md").is_file(),
        "the skill the user had stays"
    );
    sandbox
        .run("default", &["skills", "managed"])
        .success()
        .stdout(contains("no skills placed by MassOrca"));
}

#[test]
fn skills_place_refuses_an_agent_the_cli_does_not_know() {
    let sandbox = sandbox_with_notes();
    let id = notes_id(&sandbox);

    sandbox
        .run(
            "default",
            &["skills", "place", id.as_str(), "--agent", "future-tool"],
        )
        .failure();
}
