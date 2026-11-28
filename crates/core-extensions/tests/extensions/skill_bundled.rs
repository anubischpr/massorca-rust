//! Bundled skills (SKL-04) through their public API: the skill MassOrca ships is placed through the placement path into a fake
//! home, its record says which build's text is there, and a refresh brings an outdated copy to this build.

use std::fs;
use std::path::{Path, PathBuf};

use massorca_core_extensions::skills::{
    BundledState, PlacementAgent, PlacementError, PlacementStore, SkillSource, SkillSourceKind,
    Topology, bundled_status, detected_agents, find_bundled, install_bundled, managed_placements,
    place_skill, refresh_bundled, unplace_skill,
};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    staging: PathBuf,
    store: PlacementStore,
}

fn fixture() -> Fixture {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path().join("home");
    fs::create_dir_all(&home).expect("the fake home");
    let profile = temp.path().join("profile").join("skills");
    Fixture {
        staging: profile.join("bundled"),
        store: PlacementStore::new(profile.join("placements.json")),
        home,
        _temp: temp,
    }
}

fn massorca() -> &'static massorca_core_extensions::skills::BundledSkill {
    find_bundled("massorca").expect("MassOrca ships the massorca skill")
}

fn agent_folder(home: &Path, relative: &str) {
    fs::create_dir_all(home.join(relative)).expect("the agent's skill folder");
}

/// Places the skill the way an older MassOrca build did: its text, for the Claude folder, through the placement path.
fn place_older_build(fixture: &Fixture) {
    let older = fixture._temp.path().join("older-build").join("massorca");
    fs::create_dir_all(&older).expect("the older build's folder");
    fs::write(
        older.join("SKILL.md"),
        "---\nname: massorca\ndescription: An older build.\n---\nOld text\n",
    )
    .expect("the older build's skill");
    place_skill(
        &fixture.home,
        &fixture.store,
        &older,
        &[PlacementAgent::Claude],
    )
    .expect("the older build placed it");
}

#[test]
fn the_skill_ships_a_front_matter_that_names_it() {
    let skill = massorca();
    assert_eq!(skill.text().lines().next(), Some("---"));
    assert_eq!(skill.text().lines().nth(1), Some("name: massorca"));
    assert!(
        skill
            .description()
            .starts_with("Runs and controls MassOrca")
    );
}

#[test]
fn before_any_install_the_skill_is_not_placed() {
    let fixture = fixture();
    let status = bundled_status(&fixture.store, massorca()).expect("the status");
    assert_eq!(status.state, BundledState::NotPlaced);
    assert!(status.agents.is_empty());
}

#[test]
fn installing_writes_the_embedded_text_into_the_canonical_copy_and_records_its_digest() {
    let fixture = fixture();
    agent_folder(&fixture.home, ".claude/skills");
    let report = install_bundled(
        &fixture.home,
        &fixture.store,
        &fixture.staging,
        massorca(),
        &[PlacementAgent::Claude],
    )
    .expect("installed");

    let canonical = fixture.home.join(".agents/skills/massorca/SKILL.md");
    assert_eq!(
        fs::read_to_string(&canonical).expect("the canonical copy"),
        massorca().text()
    );
    assert_eq!(report.agents.len(), 1);
    let status = bundled_status(&fixture.store, massorca()).expect("the status");
    assert_eq!(status.state, BundledState::Current);
    assert_eq!(status.agents, vec![PlacementAgent::Claude]);
}

#[test]
fn every_chosen_agent_folder_reads_the_skill() {
    let fixture = fixture();
    agent_folder(&fixture.home, ".claude/skills");
    agent_folder(&fixture.home, ".cursor/skills");
    install_bundled(
        &fixture.home,
        &fixture.store,
        &fixture.staging,
        massorca(),
        &[
            PlacementAgent::Claude,
            PlacementAgent::Cursor,
            PlacementAgent::Codex,
        ],
    )
    .expect("installed");

    for folder in [".claude/skills", ".cursor/skills"] {
        let skill_file = fixture.home.join(folder).join("massorca/SKILL.md");
        assert_eq!(
            fs::read_to_string(&skill_file).expect("the agent folder reads the skill"),
            massorca().text(),
            "{folder}"
        );
    }
    let record = managed_placements(&fixture.store).expect("the record");
    let claude = record[0]
        .agents
        .iter()
        .find(|row| row.agent == PlacementAgent::Claude)
        .expect("a claude row");
    assert_eq!(claude.topology, Topology::Alias);
}

#[test]
fn a_user_owned_skill_of_the_same_name_refuses_the_install_and_is_kept() {
    let fixture = fixture();
    agent_folder(&fixture.home, ".agents/skills/massorca");
    let mine = fixture.home.join(".agents/skills/massorca/SKILL.md");
    fs::write(&mine, "my own skill").expect("the user's skill");

    let error = install_bundled(
        &fixture.home,
        &fixture.store,
        &fixture.staging,
        massorca(),
        &[PlacementAgent::Codex],
    )
    .expect_err("refused");

    assert!(matches!(error, PlacementError::NameTaken(..)), "{error:?}");
    assert_eq!(fs::read_to_string(&mine).expect("kept"), "my own skill");
}

#[test]
fn refresh_brings_an_outdated_copy_to_this_build_for_the_same_agents() {
    let fixture = fixture();
    agent_folder(&fixture.home, ".claude/skills");
    place_older_build(&fixture);
    assert_eq!(
        bundled_status(&fixture.store, massorca())
            .expect("status")
            .state,
        BundledState::Outdated
    );

    refresh_bundled(&fixture.home, &fixture.store, &fixture.staging, massorca())
        .expect("refreshed");

    let status = bundled_status(&fixture.store, massorca()).expect("status");
    assert_eq!(status.state, BundledState::Current);
    assert_eq!(status.agents, vec![PlacementAgent::Claude]);
    assert_eq!(
        fs::read_to_string(fixture.home.join(".claude/skills/massorca/SKILL.md"))
            .expect("the folder reads the skill again"),
        massorca().text()
    );
}

#[test]
fn refresh_keeps_a_copy_the_person_changed_and_places_nothing_new() {
    let fixture = fixture();
    agent_folder(&fixture.home, ".claude/skills");
    place_older_build(&fixture);
    let canonical = fixture.home.join(".agents/skills/massorca/SKILL.md");
    fs::write(&canonical, "edited by the person").expect("the change");

    let removal = refresh_bundled(&fixture.home, &fixture.store, &fixture.staging, massorca())
        .expect("answered");

    assert_eq!(
        fs::read_to_string(&canonical).expect("kept"),
        "edited by the person"
    );
    assert!(!fixture.home.join(".claude/skills/massorca").exists());
    assert_eq!(removal.agents.len(), 1);
    assert_eq!(
        bundled_status(&fixture.store, massorca())
            .expect("status")
            .state,
        BundledState::Outdated
    );
}

#[test]
fn refresh_of_a_skill_that_was_never_placed_is_refused() {
    let fixture = fixture();
    let error = refresh_bundled(&fixture.home, &fixture.store, &fixture.staging, massorca())
        .expect_err("refused");
    assert!(matches!(error, PlacementError::NotPlaced(name) if name == "massorca"));
}

#[test]
fn unplacing_the_bundled_skill_removes_its_copies() {
    let fixture = fixture();
    agent_folder(&fixture.home, ".claude/skills");
    install_bundled(
        &fixture.home,
        &fixture.store,
        &fixture.staging,
        massorca(),
        &[PlacementAgent::Claude],
    )
    .expect("installed");

    unplace_skill(&fixture.home, &fixture.store, "massorca", &[]).expect("removed");

    assert!(!fixture.home.join(".claude/skills/massorca").exists());
    assert!(!fixture.home.join(".agents/skills/massorca").exists());
    assert_eq!(
        bundled_status(&fixture.store, massorca())
            .expect("status")
            .state,
        BundledState::NotPlaced
    );
}

#[test]
fn only_agent_roots_that_exist_count_as_enabled() {
    let source = |owner: &str, exists: bool| SkillSource {
        id: format!("{owner}-root"),
        label: owner.to_owned(),
        path: PathBuf::from(owner),
        source_kind: SkillSourceKind::Home,
        providers: Vec::new(),
        owner: Some(owner.to_owned()),
        exists,
        skipped_reason: None,
    };
    let sources = [
        source("claude", true),
        source("cursor", false),
        source("gemini", true),
        source("not-an-agent", true),
    ];

    assert_eq!(
        detected_agents(&sources),
        vec![PlacementAgent::Claude, PlacementAgent::Gemini]
    );
}
