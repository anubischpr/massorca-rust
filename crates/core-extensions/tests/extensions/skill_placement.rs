//! Skill placement (SKL-02) through its public API, with a fake home and a managed-record file in a temporary folder.

use std::fs;
use std::path::{Path, PathBuf};

use massorca_core_extensions::skills::{
    AgentOutcome, LinkMode, ManagedSkill, Outcome, PlacementAgent, PlacementError, PlacementReport,
    PlacementScope, PlacementStore, Plan, SkipCause, Topology, managed_placements, place_skill,
    place_skill_in, place_skill_with, preview_placement, switch_agents_in, unplace_skill,
    unplace_skill_in,
};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    store: PlacementStore,
}

fn fixture() -> Fixture {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path().join("home");
    fs::create_dir_all(&home).expect("the fake home");
    let store = PlacementStore::new(
        temp.path()
            .join("profile")
            .join("skills")
            .join("placements.json"),
    );
    Fixture {
        _temp: temp,
        home,
        store,
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a parent folder")).expect("the folder");
    fs::write(path, contents).expect("the file");
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("the file")
}

/// A skill `name` in the Claude folder of the fake home, with one extra file below it.
fn claude_skill(home: &Path, name: &str) -> PathBuf {
    let dir = home.join(".claude/skills").join(name);
    write(
        &dir.join("SKILL.md"),
        &format!("---\nname: {name}\ndescription: Notes.\n---\nBody\n"),
    );
    write(&dir.join("notes/extra.md"), "extra\n");
    dir
}

fn outcome_of(report: &PlacementReport, agent: PlacementAgent) -> Outcome {
    report
        .agents
        .iter()
        .find(|row: &&AgentOutcome| row.agent == agent)
        .expect("a row for the agent")
        .outcome
        .clone()
}

fn managed_named(store: &PlacementStore, name: &str) -> Option<ManagedSkill> {
    managed_placements(store)
        .expect("the record reads")
        .into_iter()
        .find(|skill| skill.name == name)
}

#[test]
fn placing_writes_one_canonical_copy_and_an_alias_for_each_chosen_folder_agent() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let agents = [
        PlacementAgent::Codex,
        PlacementAgent::Cursor,
        PlacementAgent::Gemini,
    ];
    let report = place_skill(&fx.home, &fx.store, &source, &agents).expect("placed");

    let canonical = fx.home.join(".agents/skills/release-notes");
    assert_eq!(report.canonical, canonical);
    assert_eq!(
        read(&canonical.join("SKILL.md")),
        read(&source.join("SKILL.md"))
    );
    assert_eq!(read(&canonical.join("notes/extra.md")), "extra\n");
    assert_eq!(
        read(&fx.home.join(".cursor/skills/release-notes/SKILL.md")),
        read(&source.join("SKILL.md"))
    );
    assert_eq!(
        read(&fx.home.join(".gemini/skills/release-notes/notes/extra.md")),
        "extra\n"
    );
    assert!(
        !fx.home.join(".codex").exists(),
        "codex reads the canonical root and gets no folder of its own"
    );
    assert_eq!(
        outcome_of(&report, PlacementAgent::Codex),
        Outcome::Canonical
    );
    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Placed(Topology::Alias)
    );
    assert_eq!(
        outcome_of(&report, PlacementAgent::Gemini),
        Outcome::Placed(Topology::Alias)
    );
}

#[test]
fn a_folder_the_user_already_has_is_skipped_and_never_overwritten() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    // The source itself sits in the Claude folder, so Claude's copy is already there.
    let report = place_skill(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Claude, PlacementAgent::Cursor],
    )
    .expect("placed");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Claude),
        Outcome::Skipped(SkipCause::Existing)
    );
    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Placed(Topology::Alias)
    );
    assert!(source.join("SKILL.md").exists());
}

#[test]
fn an_existing_canonical_copy_the_user_made_refuses_the_whole_placement() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    write(
        &fx.home.join(".agents/skills/release-notes/SKILL.md"),
        "the user's own copy",
    );

    let err =
        place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect_err("refused");

    assert!(matches!(err, PlacementError::NameTaken(ref name, _) if name == "release-notes"));
    assert_eq!(
        read(&fx.home.join(".agents/skills/release-notes/SKILL.md")),
        "the user's own copy"
    );
    assert!(!fx.home.join(".cursor").exists());
    assert!(managed_named(&fx.store, "release-notes").is_none());
}

#[test]
fn unplacing_all_removes_the_aliases_and_the_canonical_copy_but_only_the_copies_massorca_placed() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Codex, PlacementAgent::Cursor],
    )
    .expect("placed");
    write(&fx.home.join(".cursor/skills/own-skill/SKILL.md"), "mine");

    let report = unplace_skill(&fx.home, &fx.store, "release-notes", &[]).expect("removed");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Removed
    );
    assert_eq!(outcome_of(&report, PlacementAgent::Codex), Outcome::Removed);
    assert!(!fx.home.join(".cursor/skills/release-notes").exists());
    assert!(!fx.home.join(".agents/skills/release-notes").exists());
    assert_eq!(
        read(&fx.home.join(".cursor/skills/own-skill/SKILL.md")),
        "mine"
    );
    assert!(
        source.join("SKILL.md").exists(),
        "the source is not MassOrca's to remove"
    );
    assert!(managed_named(&fx.store, "release-notes").is_none());
}

#[test]
fn unplacing_one_alias_removes_only_the_link_and_keeps_the_canonical_copy() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Codex, PlacementAgent::Cursor],
    )
    .expect("placed");

    let report = unplace_skill(
        &fx.home,
        &fx.store,
        "release-notes",
        &[PlacementAgent::Cursor],
    )
    .expect("removed");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Removed
    );
    assert!(!fx.home.join(".cursor/skills/release-notes").exists());
    assert_eq!(
        read(&fx.home.join(".agents/skills/release-notes/SKILL.md")),
        read(&source.join("SKILL.md")),
        "the canonical copy Codex reads is intact"
    );
    assert!(managed_named(&fx.store, "release-notes").is_some());
}

#[test]
fn a_copy_the_user_edited_is_kept_and_reported_as_modified() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill_with(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Cursor],
        LinkMode::Unavailable,
    )
    .expect("placed");
    write(
        &fx.home.join(".cursor/skills/release-notes/SKILL.md"),
        "edited by the user",
    );

    let report = unplace_skill(
        &fx.home,
        &fx.store,
        "release-notes",
        &[PlacementAgent::Cursor],
    )
    .expect("the request runs");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Skipped(SkipCause::Modified)
    );
    assert_eq!(
        read(&fx.home.join(".cursor/skills/release-notes/SKILL.md")),
        "edited by the user"
    );
    let left = managed_named(&fx.store, "release-notes").expect("still managed");
    assert!(
        left.agents
            .iter()
            .any(|row| row.agent == PlacementAgent::Cursor)
    );
}

#[test]
fn placing_falls_back_to_a_verified_copy_when_links_are_unavailable() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");

    let report = place_skill_with(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Cursor],
        LinkMode::Unavailable,
    )
    .expect("placed");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Placed(Topology::Copy)
    );
    assert_eq!(
        read(&fx.home.join(".cursor/skills/release-notes/notes/extra.md")),
        "extra\n"
    );
    let managed = managed_named(&fx.store, "release-notes").expect("managed");
    assert_eq!(managed.agents[0].topology, Topology::Copy);
}

#[test]
fn placing_again_leaves_placed_folders_alone_and_adds_new_agents() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");

    let again = place_skill(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Cursor, PlacementAgent::Gemini],
    )
    .expect("placed again");

    assert_eq!(
        outcome_of(&again, PlacementAgent::Cursor),
        Outcome::AlreadyPlaced(Topology::Alias)
    );
    assert_eq!(
        outcome_of(&again, PlacementAgent::Gemini),
        Outcome::Placed(Topology::Alias)
    );
}

#[test]
fn a_source_that_changed_since_it_was_placed_is_refused() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");
    write(&source.join("SKILL.md"), "a newer version");

    let err =
        place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Gemini]).expect_err("refused");

    assert!(matches!(err, PlacementError::ContentChanged(ref name) if name == "release-notes"));
    assert!(!fx.home.join(".gemini").exists());
}

#[test]
fn a_name_that_is_not_a_plain_skill_name_is_refused() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "Bad Name");

    let err =
        place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect_err("refused");

    assert!(matches!(err, PlacementError::InvalidName(_)));
    assert!(!fx.home.join(".agents").exists());
}

#[test]
fn placing_needs_at_least_one_agent() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");

    let err = place_skill(&fx.home, &fx.store, &source, &[]).expect_err("refused");

    assert!(matches!(err, PlacementError::NoAgents));
}

#[test]
fn no_staging_folder_is_left_next_to_the_placed_folders() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");

    for folder in [".agents/skills", ".cursor/skills"] {
        for entry in fs::read_dir(fx.home.join(folder)).expect("the folder") {
            let name = entry.expect("an entry").file_name();
            assert!(
                !name.to_string_lossy().starts_with('.'),
                "{name:?} is a staging folder left behind"
            );
        }
    }
}

#[test]
fn a_crash_leftover_staging_folder_is_swept_by_the_next_placement() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let stale = fx.home.join(".cursor/skills/.release-notes.placing-7-9");
    write(&stale.join("SKILL.md"), "half copied");
    let untouched = fx.home.join(".cursor/skills/.hidden-other.placing-1-1");
    write(&untouched.join("SKILL.md"), "not ours");

    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");

    assert!(!stale.exists(), "the leftover is removed");
    assert!(
        untouched.exists(),
        "a folder with another skill's name stays"
    );
}

#[test]
fn a_preview_plans_the_placement_and_writes_nothing() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");

    let preview = preview_placement(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Cursor, PlacementAgent::Claude],
    )
    .expect("previewed");

    let cursor = preview
        .agents
        .iter()
        .find(|row| row.agent == PlacementAgent::Cursor)
        .expect("a row for cursor");
    assert_eq!(cursor.plan, Plan::Create);
    let claude = preview
        .agents
        .iter()
        .find(|row| row.agent == PlacementAgent::Claude)
        .expect("a row for claude");
    assert_eq!(claude.plan, Plan::Skipped(SkipCause::Existing));
    assert!(!fx.home.join(".agents").exists());
    assert!(!fx.store.path().exists());
}

#[test]
fn the_record_names_each_placed_agent_its_folder_and_its_topology() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(
        &fx.home,
        &fx.store,
        &source,
        &[PlacementAgent::Codex, PlacementAgent::Cursor],
    )
    .expect("placed");

    let managed = managed_named(&fx.store, "release-notes").expect("managed");

    assert_eq!(
        managed.canonical,
        fx.home.join(".agents/skills/release-notes")
    );
    let codex = managed
        .agents
        .iter()
        .find(|row| row.agent == PlacementAgent::Codex)
        .expect("codex");
    assert_eq!(codex.path, None);
    assert_eq!(codex.topology, Topology::Canonical);
    let cursor = managed
        .agents
        .iter()
        .find(|row| row.agent == PlacementAgent::Cursor)
        .expect("cursor");
    assert_eq!(
        cursor.path,
        Some(fx.home.join(".cursor/skills/release-notes"))
    );
    assert_eq!(cursor.topology, Topology::Alias);
}

#[test]
fn unplacing_a_skill_massorca_did_not_place_is_refused() {
    let fx = fixture();

    let err = unplace_skill(&fx.home, &fx.store, "release-notes", &[]).expect_err("refused");

    assert!(matches!(err, PlacementError::NotPlaced(ref name) if name == "release-notes"));
}

#[cfg(unix)]
#[test]
fn an_alias_the_user_pointed_elsewhere_is_kept_and_its_target_is_untouched() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");
    let elsewhere = fx.home.join("elsewhere");
    write(&elsewhere.join("SKILL.md"), "the user's own");
    let link = fx.home.join(".cursor/skills/release-notes");
    fs::remove_file(&link).expect("the alias");
    std::os::unix::fs::symlink(&elsewhere, &link).expect("a new link");

    let report = unplace_skill(
        &fx.home,
        &fx.store,
        "release-notes",
        &[PlacementAgent::Cursor],
    )
    .expect("the request runs");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Skipped(SkipCause::Modified)
    );
    assert!(link.exists(), "the user's link stays");
    assert_eq!(read(&elsewhere.join("SKILL.md")), "the user's own");
}

#[cfg(unix)]
#[test]
fn a_source_holding_a_link_is_refused_and_nothing_is_written() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "linked");
    std::os::unix::fs::symlink(fx.home.join(".ssh"), source.join("keys")).expect("a link");

    let err =
        place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect_err("refused");

    assert!(matches!(err, PlacementError::Unsupported { .. }));
    assert!(!fx.home.join(".agents").exists());
}

/// A folder workspace under the fake home, where a workspace placement is written.
fn workspace_folder(fx: &Fixture) -> PathBuf {
    let workspace = fx.home.join("work").join("app");
    fs::create_dir_all(&workspace).expect("the workspace folder");
    workspace
}

#[test]
fn a_workspace_placement_puts_the_canonical_copy_and_the_agent_folders_under_the_workspace_root() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let workspace = workspace_folder(&fx);
    let scope = PlacementScope::Workspace("ws-app".to_owned());
    let agents = [PlacementAgent::Claude, PlacementAgent::Cursor];

    let report = place_skill_in(&workspace, &scope, &fx.store, &source, &agents).expect("placed");

    assert_eq!(
        report.canonical,
        workspace.join(".agents/skills/release-notes")
    );
    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Placed(Topology::Alias)
    );
    assert_eq!(
        read(&workspace.join(".cursor/skills/release-notes/SKILL.md")),
        read(&source.join("SKILL.md"))
    );
    assert!(
        !fx.home.join(".agents/skills/release-notes").exists(),
        "the global canonical copy is not written"
    );
    assert!(
        !fx.home.join(".cursor").exists(),
        "no agent folder under the home folder is written"
    );
}

#[test]
fn a_global_and_a_workspace_placement_of_one_name_are_recorded_and_removed_apart() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let workspace = workspace_folder(&fx);
    let scope = PlacementScope::Workspace("ws-app".to_owned());
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed globally");
    place_skill_in(
        &workspace,
        &scope,
        &fx.store,
        &source,
        &[PlacementAgent::Cursor],
    )
    .expect("placed for the workspace");

    let records = managed_placements(&fx.store).expect("the record reads");
    assert_eq!(records.len(), 2, "one record per scope: {records:?}");

    unplace_skill_in(&workspace, &scope, &fx.store, "release-notes", &[]).expect("removed");

    assert!(!workspace.join(".agents/skills/release-notes").exists());
    assert!(
        fx.home
            .join(".agents/skills/release-notes/SKILL.md")
            .is_file()
    );
    assert!(fx.home.join(".cursor/skills/release-notes").exists());
    let left = managed_placements(&fx.store).expect("the record reads");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].scope, PlacementScope::Global);
}

#[test]
fn unplacing_a_workspace_that_holds_no_placement_is_refused_even_when_global_has_one() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let workspace = workspace_folder(&fx);
    let scope = PlacementScope::Workspace("ws-app".to_owned());
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed globally");

    let result = unplace_skill_in(&workspace, &scope, &fx.store, "release-notes", &[]);

    assert!(matches!(result, Err(PlacementError::NotPlaced(_))));
    assert!(fx.home.join(".cursor/skills/release-notes").exists());
}

#[test]
fn a_record_written_before_scopes_existed_reads_as_a_global_placement() {
    let fx = fixture();
    write(
        fx.store.path(),
        r#"{"skills":[{"name":"release-notes","canonical":"/x","digest":"d","agents":[]}]}"#,
    );

    let skills = managed_placements(&fx.store).expect("the record reads");

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].scope, PlacementScope::Global);
}

#[test]
fn switching_agents_adds_one_folder_and_drops_one_alias_within_its_scope_only() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let workspace = workspace_folder(&fx);
    let scope = PlacementScope::Workspace("ws-app".to_owned());
    place_skill_in(
        &workspace,
        &scope,
        &fx.store,
        &source,
        &[PlacementAgent::Claude],
    )
    .expect("placed for the workspace");
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Gemini]).expect("placed globally");

    let report = switch_agents_in(
        &workspace,
        &scope,
        &fx.store,
        "release-notes",
        &[PlacementAgent::Cursor],
        &[PlacementAgent::Claude],
    )
    .expect("switched");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Placed(Topology::Alias)
    );
    assert_eq!(
        outcome_of(&report, PlacementAgent::Claude),
        Outcome::Removed
    );
    assert!(
        workspace
            .join(".cursor/skills/release-notes/SKILL.md")
            .is_file()
    );
    assert!(!workspace.join(".claude/skills/release-notes").exists());
    assert!(
        workspace.join(".agents/skills/release-notes").is_dir(),
        "the canonical copy stays while the cursor alias reaches it"
    );
    assert!(
        !workspace.join(".gemini").exists(),
        "no other folder is made"
    );
    assert!(
        fx.home.join(".gemini/skills/release-notes").exists(),
        "the global placement is untouched"
    );
    let record = managed_placements(&fx.store).expect("the record reads");
    let workspace_row = record
        .iter()
        .find(|skill| skill.scope == scope)
        .expect("the workspace record");
    let agents: Vec<PlacementAgent> = workspace_row.agents.iter().map(|row| row.agent).collect();
    assert_eq!(agents, [PlacementAgent::Cursor]);
}

#[test]
fn switching_a_skill_not_placed_in_that_scope_is_refused() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let workspace = workspace_folder(&fx);
    let scope = PlacementScope::Workspace("ws-app".to_owned());
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed globally");

    let result = switch_agents_in(
        &workspace,
        &scope,
        &fx.store,
        "release-notes",
        &[PlacementAgent::Gemini],
        &[],
    );

    assert!(matches!(result, Err(PlacementError::NotPlaced(_))));
    assert!(!workspace.join(".gemini").exists());
}
