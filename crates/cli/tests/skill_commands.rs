//! `massorca skills installed` and `massorca skills roots`, through the CLI and a service that reads a fake home folder.

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

/// A sandbox whose service reads `user-home` as the home folder, with one Codex skill named `alpha`.
fn sandbox_with_alpha() -> Sandbox {
    let sandbox = Sandbox::new();
    let user_home = sandbox.home().join("user-home");
    write_skill(&user_home, ".codex/skills/alpha", "alpha");
    sandbox.set_env(SKILL_HOME_ENV, &user_home.to_string_lossy());
    sandbox
}

#[test]
fn skills_installed_lists_the_skills_found_in_the_home_folder_as_json() {
    let sandbox = sandbox_with_alpha();

    let installed = sandbox.json_ok("default", &["skills", "installed"]);

    let skills = installed["skills"].as_array().expect("a skills array");
    assert_eq!(skills.len(), 1, "{installed}");
    assert_eq!(skills[0]["name"], "alpha");
    assert_eq!(skills[0]["source_label"], "Codex home");
    assert_eq!(skills[0]["installed"], Value::Bool(true));
}

#[test]
fn skills_roots_reports_each_root_with_whether_it_was_read() {
    let sandbox = sandbox_with_alpha();

    let listed = sandbox.json_ok("default", &["skills", "roots"]);

    let sources = listed["sources"].as_array().expect("a sources array");
    let codex = sources
        .iter()
        .find(|source| source["label"] == "Codex home")
        .unwrap_or_else(|| panic!("no Codex home in {listed}"));
    assert_eq!(codex["exists"], Value::Bool(true));
    let gemini = sources
        .iter()
        .find(|source| source["label"] == "Gemini home")
        .unwrap_or_else(|| panic!("no Gemini home in {listed}"));
    assert_eq!(gemini["exists"], Value::Bool(false));
    assert_eq!(gemini["skipped_reason"], "missing");
}

#[test]
fn skills_installed_prints_one_line_per_skill_without_json() {
    let sandbox = sandbox_with_alpha();

    sandbox
        .run("default", &["skills", "installed"])
        .success()
        .stdout(contains("alpha"))
        .stdout(contains("Codex home"));
}
