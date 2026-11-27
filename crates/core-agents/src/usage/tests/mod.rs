//! The readers against fake vendor logs in a temporary folder; no real account folder is read.

use std::fs;
use std::path::Path;

use super::{read, sources, supports};

mod claude;
mod codex;

/// 2026-01-05T12:30:00Z.
pub(super) const NOW: i64 = 1_767_616_200_000;

pub(super) fn iso(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).unwrap().to_string()
}

pub(super) fn write_lines(path: &Path, lines: &[serde_json::Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(path, text).unwrap();
}

#[test]
fn only_kinds_with_a_source_have_a_reader() {
    assert!(supports("claude") && supports("codex"));
    assert!(!supports("gentle-shell"));
    for kind in ["claude", "codex"] {
        let names: Vec<_> = sources(kind).iter().map(|source| source.name()).collect();
        assert_eq!(
            names,
            [massorca_protocol::methods::usage::UsageSource::LocalLogs],
            "{kind}"
        );
    }
    assert!(sources("gentle-shell").is_empty());
}

#[test]
fn a_missing_folder_has_no_data() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(read("claude", home.path(), NOW).unwrap(), None);
    assert_eq!(read("codex", home.path(), NOW).unwrap(), None);
    assert_eq!(read("other", home.path(), NOW).unwrap(), None);
}

#[test]
fn an_oversized_line_is_skipped_and_the_next_one_still_parses() {
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("big.jsonl");
    let mut text = "x".repeat(super::MAX_LINE_BYTES + 10);
    text.push_str("\n{\"k\":1}\n");
    text.push_str(&"y".repeat(super::MAX_LINE_BYTES + 10));
    fs::write(&log, text).unwrap();
    let mut seen = Vec::new();
    super::scan(&log, "\"k\"", |value| seen.push(value));
    assert_eq!(seen, [serde_json::json!({ "k": 1 })]);
}

#[test]
fn a_linked_log_root_has_no_data() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let real = elsewhere.path().join("p");
    write_lines(
        &real.join("s.jsonl"),
        &[serde_json::json!({
            "type": "assistant", "timestamp": iso(NOW - 60_000), "requestId": "r",
            "message": { "id": "m", "usage": { "input_tokens": 5 } }
        })],
    );
    let root = home.path().join("projects");
    #[cfg(windows)]
    junction::create(elsewhere.path(), &root).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(elsewhere.path(), &root).unwrap();
    assert_eq!(read("claude", home.path(), NOW).unwrap(), None);
}
