//! The skill files a scan reads: what each one says about itself, and the bounded walk that finds them.

use std::fs;
use std::path::{Path, PathBuf};

use massorca_core_extensions::skills::{SkillSummary, find_skill_files, read_skill_summary};
use tempfile::TempDir;

/// Writes `markdown` as a `SKILL.md` in a fresh folder and reads it back.
fn summary_of(markdown: &str) -> SkillSummary {
    let temp = TempDir::new().expect("a temporary folder");
    let path = temp.path().join("SKILL.md");
    fs::write(&path, markdown).expect("the skill file");
    read_skill_summary(&path).expect("a readable skill file")
}

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().expect("a parent folder")).expect("the folder");
    fs::write(path, "x").expect("the file");
}

/// The skill files a walk of `root` finds with no budget to run out.
fn files_of(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut budget = usize::MAX;
    find_skill_files(root, depth, &mut budget)
        .expect("a readable root")
        .files
}

#[test]
fn frontmatter_gives_the_name_description_and_slash_flag() {
    let summary = summary_of(
        "---\nname: \"delta-kit\"\ndescription: >\n  Folds these\n  lines.\n\
         disable-slash-command: true\n---\nBody\n",
    );

    assert_eq!(summary.name.as_deref(), Some("delta-kit"));
    assert_eq!(summary.description.as_deref(), Some("Folds these lines."));
    assert!(summary.disable_slash_command);
}

#[test]
fn without_frontmatter_the_first_heading_and_paragraph_stand_in() {
    let summary = summary_of("# Epsilon\n\nFirst line.\nSecond line.\n\nLater text.\n");

    assert_eq!(summary.name.as_deref(), Some("Epsilon"));
    assert_eq!(
        summary.description.as_deref(),
        Some("First line. Second line.")
    );
    assert!(!summary.disable_slash_command);
}

#[test]
fn a_first_paragraph_stops_after_the_line_that_passes_240_units() {
    let lines = vec!["abcdefghij"; 30].join("\n");

    let summary = summary_of(&format!("# Zeta\n\n{lines}\n"));

    // Twenty-two lines of ten units with spaces between come to 241 units, the first count past 240.
    assert_eq!(summary.description, Some(vec!["abcdefghij"; 22].join(" ")));
}

#[test]
fn a_utf8_byte_order_mark_and_crlf_line_endings_are_read() {
    let summary = summary_of(
        "\u{feff}---\r\nname: crlf-kit\r\ndescription: Windows lines.\r\n---\r\nBody\r\n",
    );

    assert_eq!(summary.name.as_deref(), Some("crlf-kit"));
    assert_eq!(summary.description.as_deref(), Some("Windows lines."));
}

#[test]
fn only_the_first_256_kib_are_read() {
    // The closing delimiter lies past the read window, so the frontmatter never closes and no name is read.
    let padding = "x".repeat(300 * 1024);

    let summary = summary_of(&format!("---\nname: too-late\n{padding}\n---\n"));

    assert_eq!(summary.name, None);
}

#[test]
fn the_walk_finds_skill_files_at_most_five_folders_deep() {
    let temp = TempDir::new().expect("a temporary folder");
    let root = temp.path().join("skills");
    touch(&root.join("a/b/c/d/SKILL.md"));
    touch(&root.join("a/b/c/d/e/SKILL.md"));

    let found = files_of(&root, 5);

    assert_eq!(found, [root.join("a/b/c/d/SKILL.md")]);
}

#[test]
fn the_walk_of_a_plugin_cache_reaches_ten_folders_deep() {
    let temp = TempDir::new().expect("a temporary folder");
    let root = temp.path().join("cache");
    let nine = (1..=9).map(|index| format!("p{index}")).collect::<Vec<_>>();
    let ten = nine.join("/");
    touch(&root.join(format!("{ten}/SKILL.md")));
    touch(&root.join(format!("{ten}/p10/SKILL.md")));

    let found = files_of(&root, 10);

    assert_eq!(found, [root.join(format!("{ten}/SKILL.md"))]);
}

#[test]
fn a_root_that_is_missing_or_not_a_folder_is_an_error() {
    let temp = TempDir::new().expect("a temporary folder");
    let file = temp.path().join("file");
    fs::write(&file, "x").expect("a file");

    let mut budget = usize::MAX;
    assert!(find_skill_files(&temp.path().join("missing"), 5, &mut budget).is_err());
    assert!(find_skill_files(&file, 5, &mut budget).is_err());
}

#[test]
fn the_walk_stops_when_its_entry_budget_runs_out() {
    let temp = TempDir::new().expect("a temporary folder");
    let root = temp.path().join("skills");
    for index in 0..6 {
        touch(&root.join(format!("s{index}/SKILL.md")));
    }
    let mut budget = 3;

    let walk = find_skill_files(&root, 5, &mut budget).expect("a readable root");

    assert!(walk.truncated);
    assert!(walk.files.len() < 6);
    assert_eq!(budget, 0);
}

#[test]
fn a_walk_within_its_budget_is_not_truncated() {
    let temp = TempDir::new().expect("a temporary folder");
    let root = temp.path().join("skills");
    touch(&root.join("one/SKILL.md"));
    let mut budget = 10;

    let walk = find_skill_files(&root, 5, &mut budget).expect("a readable root");

    assert!(!walk.truncated);
    assert_eq!(walk.files, [root.join("one/SKILL.md")]);
}

#[test]
fn a_frontmatter_description_is_cut_to_240_units() {
    let long = "x".repeat(300);

    let summary = summary_of(&format!("---\nname: long-one\ndescription: {long}\n---\n"));

    let description = summary.description.expect("a description");
    assert_eq!(description.encode_utf16().count(), 240);
    assert!(description.chars().all(|character| character == 'x'));
}

#[test]
fn a_frontmatter_description_is_never_cut_inside_a_character() {
    // Each emoji is two UTF-16 units, so 240 units hold exactly 120 of them.
    let emoji = "\u{1F600}".repeat(200);

    let summary = summary_of(&format!("---\ndescription: {emoji}\n---\n"));

    let description = summary.description.expect("a description");
    assert_eq!(description.encode_utf16().count(), 240);
    assert_eq!(description.chars().count(), 120);
}

#[test]
fn a_frontmatter_description_of_exactly_240_units_is_kept_whole() {
    let exact = "y".repeat(240);

    let summary = summary_of(&format!("---\ndescription: {exact}\n---\n"));

    assert_eq!(summary.description, Some(exact));
}
