//! The readers against fake vendor logs in a temporary folder; no real account folder is read.

use std::fs;
use std::path::Path;

use super::{read, sources, supports};

mod claude;

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
    assert!(supports("claude"));
    assert!(!supports("gentle-shell"));
    for kind in ["claude"] {
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
    assert_eq!(read("other", home.path(), NOW).unwrap(), None);
}
