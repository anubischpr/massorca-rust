//! Skill placement (SKL-02) against a record or a folder layout it did not write: a tampered record deletes nothing, a
//! staging folder of another process stays, and the canonical copy an agent still reads is reported as such.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_core_extensions::skills::{
    AgentOutcome, LinkMode, Outcome, PlacementAgent, PlacementError, PlacementReport,
    PlacementStore, SkipCause, managed_placements, place_skill, place_skill_with, unplace_skill,
};
use serde_json::Value;
use tempfile::TempDir;

struct Fixture {
    temp: TempDir,
    home: PathBuf,
    store: PlacementStore,
}

fn fixture() -> Fixture {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path().join("home");
    fs::create_dir_all(&home).expect("the fake home");
    let store = PlacementStore::new(temp.path().join("profile/skills/placements.json"));
    Fixture { temp, home, store }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("a parent folder")).expect("the folder");
    fs::write(path, contents).expect("the file");
}

/// A skill `name` at `dir`, with the same files every test skill holds, so two of them share a digest.
fn skill_files(dir: &Path, name: &str) {
    write(
        &dir.join("SKILL.md"),
        &format!("---\nname: {name}\ndescription: Notes.\n---\nBody\n"),
    );
    write(&dir.join("notes/extra.md"), "extra\n");
}

fn claude_skill(home: &Path, name: &str) -> PathBuf {
    let dir = home.join(".claude/skills").join(name);
    skill_files(&dir, name);
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

/// Rewrites the record file with `change` applied to its JSON.
fn tamper(store: &PlacementStore, change: impl FnOnce(&mut Value)) {
    let text = fs::read_to_string(store.path()).expect("the record");
    let mut record: Value = serde_json::from_str(&text).expect("the record parses");
    change(&mut record);
    fs::write(
        store.path(),
        serde_json::to_vec_pretty(&record).expect("json"),
    )
    .expect("the record is rewritten");
}

#[test]
fn a_record_name_that_climbs_out_of_the_skill_folder_is_refused_and_deletes_nothing() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Codex]).expect("placed");
    // The same skill outside the home folder, so its digest matches the record.
    let victim = fx.temp.path().join("victim");
    skill_files(&victim, "release-notes");
    tamper(&fx.store, |record| {
        record["skills"][0]["name"] = Value::from("../../../victim");
    });

    let result = unplace_skill(&fx.home, &fx.store, "../../../victim", &[]);

    assert!(
        matches!(result, Err(PlacementError::InvalidRecord(_))),
        "the tampered record is refused: {result:?}"
    );
    assert!(
        victim.join("SKILL.md").is_file(),
        "the folder outside stays"
    );
    assert!(fx.home.join(".agents/skills/release-notes").is_dir());
}

#[test]
fn an_agent_row_pointing_at_an_unrelated_folder_is_kept_and_that_folder_is_untouched() {
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
    let unrelated = fx.home.join("projects/keep-me");
    skill_files(&unrelated, "release-notes");
    let unrelated_text = unrelated.to_string_lossy().into_owned();
    tamper(&fx.store, |record| {
        record["skills"][0]["agents"][0]["path"] = Value::from(unrelated_text);
    });

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
    assert!(
        unrelated.join("SKILL.md").is_file(),
        "the unrelated folder stays"
    );
    assert!(fx.home.join(".cursor/skills/release-notes").is_dir());
    assert!(fx.home.join(".agents/skills/release-notes").is_dir());
    let left = managed_placements(&fx.store).expect("the record reads");
    assert_eq!(left.len(), 1, "the row stays recorded");
    assert_eq!(left[0].agents.len(), 1);
}

#[test]
fn a_staging_folder_of_another_running_placement_is_not_swept() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_nanos();
    let fresh = fx
        .home
        .join(format!(".cursor/skills/.release-notes.placing-4242-{now}"));
    write(&fresh.join("SKILL.md"), "being copied");
    let odd = fx.home.join(".cursor/skills/.release-notes.placing-mine");
    write(&odd.join("SKILL.md"), "the user's");
    let stale = fx.home.join(".cursor/skills/.release-notes.placing-7-9");
    write(&stale.join("SKILL.md"), "half copied");

    place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");

    assert!(
        fresh.exists(),
        "a staging folder younger than ten minutes stays"
    );
    assert!(odd.exists(), "a name off the staging pattern stays");
    assert!(!stale.exists(), "an old leftover goes");
}

#[test]
fn unplacing_a_canonical_agent_while_another_agent_still_reads_the_copy_says_so() {
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
        &[PlacementAgent::Codex],
    )
    .expect("the request runs");

    assert_eq!(
        outcome_of(&report, PlacementAgent::Codex),
        Outcome::Canonical,
        "Codex still reads the canonical copy Cursor needs"
    );
    assert!(
        fx.home
            .join(".agents/skills/release-notes/SKILL.md")
            .is_file()
    );
}

/// A stow-managed agent folder: `.cursor` is itself a link to a folder elsewhere in the home.
#[cfg(unix)]
#[test]
fn an_alias_below_a_linked_agent_folder_reaches_the_canonical_copy() {
    let fx = fixture();
    let source = claude_skill(&fx.home, "release-notes");
    let dotfiles = fx.home.join("dotfiles/cursor");
    fs::create_dir_all(dotfiles.join("skills")).expect("the stow folder");
    std::os::unix::fs::symlink(&dotfiles, fx.home.join(".cursor")).expect("the stow link");

    let report =
        place_skill(&fx.home, &fx.store, &source, &[PlacementAgent::Cursor]).expect("placed");

    assert!(matches!(
        outcome_of(&report, PlacementAgent::Cursor),
        Outcome::Placed(_)
    ));
    let placed = fx.home.join(".cursor/skills/release-notes/SKILL.md");
    assert_eq!(
        fs::read_to_string(&placed).expect("the placed skill reads"),
        fs::read_to_string(source.join("SKILL.md")).expect("the source")
    );

    let removed = unplace_skill(&fx.home, &fx.store, "release-notes", &[]).expect("removed");
    assert_eq!(
        outcome_of(&removed, PlacementAgent::Cursor),
        Outcome::Removed
    );
    assert!(!fx.home.join(".agents/skills/release-notes").exists());
}
