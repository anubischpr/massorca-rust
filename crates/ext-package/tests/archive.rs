//! The `.mxt` archive check, with archives built in memory.

use std::io::{Cursor, Write};

use massorca_ext_package::archive::{ArchiveLimits, open, verify_sha256};
use massorca_ext_package::{Error, codes};
use serde_json::json;
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

/// One archive entry: name (a trailing `/` makes a directory), contents, and an optional Unix mode.
type Entry = (String, Vec<u8>, Option<u32>);

fn file(name: &str, contents: &[u8]) -> Entry {
    (name.to_owned(), contents.to_vec(), None)
}

fn build(entries: &[Entry]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents, mode) in entries {
        let mut options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        if let Some(mode) = mode.filter(|mode| *mode & 0o170_000 == 0o120_000) {
            writer
                .add_symlink(
                    name.as_str(),
                    String::from_utf8_lossy(contents).into_owned(),
                    options.unix_permissions(mode & 0o777),
                )
                .expect("symlink entry");
            continue;
        }
        if let Some(mode) = mode {
            options = options.unix_permissions(*mode);
        }
        if name.ends_with('/') {
            writer
                .add_directory(name.as_str(), options)
                .expect("directory entry");
        } else {
            writer
                .start_file(name.as_str(), options)
                .expect("file entry");
            writer.write_all(contents).expect("contents");
        }
    }
    writer.finish().expect("finish").into_inner()
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn manifest_bytes() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id": "massorca.sticky-wall",
        "name": "Sticky Wall",
        "version": "1.2.0",
        "publisher": "massorca",
        "description": "Notes on a wall.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "views": 1,
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "main": "main.js",
        "contributes": {
            "views": [{ "id": "wall", "slot": "canvas-page", "title": "Wall", "document": "views/wall.json" }]
        }
    }))
    .expect("manifest serializes")
}

/// A complete package: manifest, entry point and the view it places.
fn valid_entries() -> Vec<Entry> {
    vec![
        file("extension.json", &manifest_bytes()),
        file("main.js", b"export const ready = true;"),
        file("views/wall.json", b"{}"),
    ]
}

fn expect_error(bytes: &[u8], pin: &str) -> Error {
    match open(bytes, pin, ArchiveLimits::default()) {
        Ok(_) => panic!("archive should have been refused"),
        Err(error) => error,
    }
}

#[test]
fn a_valid_archive_opens_into_a_manifest_entries_and_file_contents() {
    let mut entries = valid_entries();
    entries.push(("views/".to_owned(), Vec::new(), None));
    let bytes = build(&entries);
    let package =
        open(&bytes, &hex_sha256(&bytes), ArchiveLimits::default()).expect("valid package");
    assert_eq!(package.manifest.id, "massorca.sticky-wall");
    let names: Vec<&str> = package
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(names, ["extension.json", "main.js", "views/wall.json"]);
    assert_eq!(package.files["main.js"], b"export const ready = true;");
}

#[test]
fn the_pinned_hash_is_checked_first_and_accepts_either_case() {
    let bytes = build(&valid_entries());
    let upper = hex_sha256(&bytes).to_ascii_uppercase();
    assert!(verify_sha256(&bytes, &upper).is_ok());

    let error = expect_error(&bytes, &"0".repeat(64));
    assert_eq!(error.code(), codes::HASH_MISMATCH);
    assert!(matches!(error, Error::HashMismatch { .. }));

    let malformed = expect_error(&bytes, "not-a-hash");
    assert_eq!(malformed.code(), codes::INVALID_HASH);
}

#[test]
fn traversal_absolute_and_drive_paths_are_refused() {
    for name in [
        "../evil.js",
        "views/../../evil.js",
        "/etc/evil.js",
        "C:/evil.js",
        "a//b.js",
    ] {
        let mut entries = valid_entries();
        entries.push(file(name, b"x"));
        let bytes = build(&entries);
        let error = expect_error(&bytes, &hex_sha256(&bytes));
        assert_eq!(error.code(), codes::UNSAFE_PATH, "{name}");
    }
}

#[test]
fn a_symbolic_link_entry_is_refused() {
    let mut entries = valid_entries();
    entries.push((
        "views/link.json".to_owned(),
        b"../extension.json".to_vec(),
        Some(0o120_777),
    ));
    let bytes = build(&entries);
    let error = expect_error(&bytes, &hex_sha256(&bytes));
    assert_eq!(error.code(), codes::UNSAFE_ENTRY);
    assert!(matches!(
        error,
        Error::UnsafeEntry {
            reason: "symbolic link",
            ..
        }
    ));
}

#[test]
fn names_that_differ_only_in_case_are_refused() {
    let mut entries = valid_entries();
    entries.push(file("README.md", b"one"));
    entries.push(file("readme.md", b"two"));
    let bytes = build(&entries);
    let error = expect_error(&bytes, &hex_sha256(&bytes));
    assert!(matches!(error, Error::UnsafeEntry { reason, .. } if reason.contains("only in case")));
}

#[test]
fn a_missing_manifest_is_an_invalid_archive() {
    let entries: Vec<Entry> = valid_entries().into_iter().skip(1).collect();
    let bytes = build(&entries);
    let error = expect_error(&bytes, &hex_sha256(&bytes));
    assert_eq!(error.code(), codes::INVALID_ARCHIVE);
}

#[test]
fn a_manifest_in_a_subfolder_does_not_count_as_the_root_manifest() {
    let entries = vec![
        file("pkg/extension.json", &manifest_bytes()),
        file("main.js", b"x"),
    ];
    let bytes = build(&entries);
    assert_eq!(
        expect_error(&bytes, &hex_sha256(&bytes)).code(),
        codes::INVALID_ARCHIVE
    );
}

#[test]
fn a_manifest_that_breaks_a_rule_is_refused_with_its_problems() {
    let mut value: serde_json::Value = serde_json::from_slice(&manifest_bytes()).expect("json");
    value["bogus"] = json!(true);
    let entries = vec![file(
        "extension.json",
        &serde_json::to_vec(&value).expect("json"),
    )];
    let bytes = build(&entries);
    assert_eq!(
        expect_error(&bytes, &hex_sha256(&bytes)).code(),
        codes::INVALID_MANIFEST
    );
}

#[test]
fn a_referenced_file_missing_from_the_archive_is_refused() {
    let entries: Vec<Entry> = valid_entries()
        .into_iter()
        .filter(|(name, _, _)| name != "main.js")
        .collect();
    let bytes = build(&entries);
    let error = expect_error(&bytes, &hex_sha256(&bytes));
    assert_eq!(error.code(), codes::MISSING_FILE);
}

#[test]
fn an_archive_over_the_entry_limit_is_refused() {
    let bytes = build(&valid_entries());
    let limits = ArchiveLimits {
        max_entries: 2,
        ..ArchiveLimits::default()
    };
    match open(&bytes, &hex_sha256(&bytes), limits) {
        Err(error) => assert_eq!(error.code(), codes::LIMIT_EXCEEDED),
        Ok(_) => panic!("entry limit should refuse"),
    }
}

#[test]
fn an_archive_over_the_uncompressed_size_limit_is_refused() {
    let mut entries = valid_entries();
    entries.push(file("big.bin", &vec![0_u8; 4096]));
    let bytes = build(&entries);
    let limits = ArchiveLimits {
        max_uncompressed_bytes: 1024,
        ..ArchiveLimits::default()
    };
    match open(&bytes, &hex_sha256(&bytes), limits) {
        Err(error) => assert_eq!(error.code(), codes::LIMIT_EXCEEDED),
        Ok(_) => panic!("size limit should refuse"),
    }
}

#[test]
fn bytes_that_are_not_a_zip_are_an_invalid_archive() {
    let bytes = b"this is not a zip file".to_vec();
    assert_eq!(
        expect_error(&bytes, &hex_sha256(&bytes)).code(),
        codes::INVALID_ARCHIVE
    );
}

#[test]
fn directories_are_accepted_and_not_listed_as_entries() {
    let mut entries = valid_entries();
    entries.push(("notes/".to_owned(), Vec::new(), None));
    entries.push((
        "notes/[draft] *.md".to_owned(),
        b"glob characters are only names".to_vec(),
        None,
    ));
    let bytes = build(&entries);
    let package =
        open(&bytes, &hex_sha256(&bytes), ArchiveLimits::default()).expect("valid package");
    assert!(
        package
            .entries
            .iter()
            .all(|entry| !entry.path.ends_with('/'))
    );
    assert!(
        package
            .entries
            .iter()
            .any(|entry| entry.path == "notes/[draft] *.md")
    );
}
