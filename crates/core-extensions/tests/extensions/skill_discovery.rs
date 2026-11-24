//! Skill discovery (SKL-01) through its public API, with a fake home in a temporary folder.

use std::fs;
use std::path::{Path, PathBuf};

use massorca_core_extensions::skills::{
    DiscoveredSkill, DiscoveryRequest, SkillDiscovery, SkillProvider, SkillRootEnv, SkillSource,
    SkillSourceKind, SkipReason, discover,
};
use tempfile::TempDir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a parent folder")).expect("the folder");
    fs::write(path, contents).expect("the file");
}

/// A `SKILL.md` named `name` in `home/relative`.
fn skill_at(home: &Path, relative: &str, name: &str) {
    let text = format!("---\nname: {name}\ndescription: {name} skill.\n---\n");
    write(&home.join(relative).join("SKILL.md"), &text);
}

fn scan(home: &Path) -> SkillDiscovery {
    discover(&DiscoveryRequest {
        home: home.to_path_buf(),
        ..Default::default()
    })
}

fn names(report: &SkillDiscovery) -> Vec<&str> {
    report
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect()
}

fn skill<'a>(report: &'a SkillDiscovery, name: &str) -> &'a DiscoveredSkill {
    report
        .skills
        .iter()
        .find(|skill| skill.name == name)
        .unwrap_or_else(|| panic!("no skill {name} in {:?}", names(report)))
}

fn source<'a>(report: &'a SkillDiscovery, label: &str) -> &'a SkillSource {
    report
        .sources
        .iter()
        .find(|source| source.label == label)
        .unwrap_or_else(|| panic!("no root labelled {label}"))
}

/// Every file below `root` with its bytes, in path order: the state a scan must leave unchanged.
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).expect("a folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).expect("a file");
                files.push((path, bytes));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn each_agent_tool_home_is_scanned_under_its_own_label() {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path();
    skill_at(home, ".codex/skills/alpha", "alpha");
    write(
        &home.join(".claude/skills/beta/SKILL.md"),
        "# Beta\n\nBeta does things.\n",
    );

    let report = scan(home);

    let alpha = skill(&report, "alpha");
    assert_eq!(alpha.source_label, "Codex home");
    assert_eq!(alpha.source_kind, SkillSourceKind::Home);
    assert_eq!(alpha.providers, [SkillProvider::Codex]);
    let beta = skill(&report, "Beta");
    assert_eq!(beta.source_label, "Claude home");
    assert_eq!(beta.description.as_deref(), Some("Beta does things."));
    assert!(beta.installed && beta.updated_at_ms.is_some());
    assert!(source(&report, "Codex home").exists);
    let gemini = source(&report, "Gemini home");
    assert!(!gemini.exists);
    assert_eq!(gemini.skipped_reason, Some(SkipReason::Missing));
}

#[test]
fn the_shared_agents_folder_has_no_owning_agent() {
    let temp = TempDir::new().expect("a temporary folder");
    skill_at(temp.path(), ".agents/skills/gamma", "gamma");

    let report = scan(temp.path());

    let gamma = skill(&report, "gamma");
    assert_eq!(gamma.source_label, "Agent skills home");
    assert_eq!(gamma.providers, [SkillProvider::AgentSkills]);
    assert_eq!(source(&report, "Agent skills home").owner, None);
}

#[test]
fn a_system_folder_under_a_home_root_is_bundled() {
    let temp = TempDir::new().expect("a temporary folder");
    skill_at(temp.path(), ".codex/skills/.system/imagegen", "imagegen");

    let report = scan(temp.path());

    let imagegen = skill(&report, "imagegen");
    assert_eq!(imagegen.source_kind, SkillSourceKind::Bundled);
    assert_eq!(imagegen.source_label, "Codex home bundled");
}

#[test]
fn source_kinds_keep_only_the_requested_kinds() {
    let temp = TempDir::new().expect("a temporary folder");
    skill_at(temp.path(), ".codex/skills/plain", "plain");
    skill_at(temp.path(), ".codex/skills/.system/bundled", "bundled");

    let report = discover(&DiscoveryRequest {
        home: temp.path().to_path_buf(),
        source_kinds: vec![SkillSourceKind::Bundled],
        ..Default::default()
    });

    assert_eq!(names(&report), ["bundled"]);
}

#[test]
fn a_root_that_is_not_a_folder_is_reported_unreadable() {
    let temp = TempDir::new().expect("a temporary folder");
    write(&temp.path().join(".codex/skills"), "not a folder");

    let report = scan(temp.path());

    let codex = source(&report, "Codex home");
    assert!(codex.exists);
    assert_eq!(codex.skipped_reason, Some(SkipReason::Unreadable));
}

#[test]
fn environment_folders_move_the_claude_grok_and_hermes_roots() {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path();
    skill_at(home, "cfg/skills/omega", "omega");

    let report = discover(&DiscoveryRequest {
        home: home.to_path_buf(),
        env: SkillRootEnv {
            claude_config_dir: Some(home.join("cfg")),
            grok_home: Some(home.join("grok")),
            hermes_home: Some(home.join("hermes")),
        },
        ..Default::default()
    });

    assert_eq!(source(&report, "Claude home").path, home.join("cfg/skills"));
    assert_eq!(skill(&report, "omega").root_path, home.join("cfg/skills"));
    assert_eq!(source(&report, "Grok home").path, home.join("grok/skills"));
    assert_eq!(
        source(&report, "Hermes home").path,
        home.join("hermes/skills")
    );
}

#[test]
fn skills_are_sorted_by_name_ignoring_case() {
    let temp = TempDir::new().expect("a temporary folder");
    skill_at(temp.path(), ".codex/skills/zeta", "Zeta");
    skill_at(temp.path(), ".agents/skills/alpha", "alpha");

    assert_eq!(names(&scan(temp.path())), ["alpha", "Zeta"]);
}

#[test]
fn discovery_changes_no_file_in_the_skill_folders() {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path();
    skill_at(home, ".codex/skills/read-only", "read-only");
    write(&home.join(".claude/skills/notes.txt"), "keep me");
    let before = snapshot(home);

    scan(home);

    assert_eq!(snapshot(home), before);
}
