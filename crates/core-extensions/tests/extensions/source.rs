//! Loading each source kind, with the hash pinned.

use massorca_core_extensions::lockfile::SourceKind;
use massorca_core_extensions::source::{Source, canonical_sha256, load, sha256_hex};
use massorca_ext_package::codes;

use crate::fixture::{agent_package, package_files, write_dir, zip};

const WRONG: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn a_folder_hashes_canonically_and_honours_a_pin() {
    let dir = tempfile::tempdir().expect("temp dir");
    let files = package_files(&agent_package("acme", "kit", "kit"));
    write_dir(dir.path(), &files);
    let source = Source::Dir(dir.path().to_path_buf());
    let loaded = load(&source, None).expect("folder loads");
    assert_eq!(loaded.manifest.id, "acme.kit");
    assert_eq!(loaded.sha256, canonical_sha256(&files));
    assert_eq!(loaded.files, files);
    assert_eq!(loaded.source.kind, SourceKind::Dir);
    load(&source, Some(&loaded.sha256.to_uppercase())).expect("a matching pin, either case");
    let error = load(&source, Some(WRONG)).expect_err("a wrong pin is refused");
    assert_eq!(error.code(), codes::HASH_MISMATCH);
}

#[test]
fn an_archive_is_checked_against_its_pin() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bytes = zip(&package_files(&agent_package("acme", "kit", "kit")));
    let path = dir.path().join("kit.mxt");
    std::fs::write(&path, &bytes).expect("write archive");
    let source = Source::File(path);
    let loaded = load(&source, Some(&sha256_hex(&bytes))).expect("pinned archive loads");
    assert_eq!(loaded.sha256, sha256_hex(&bytes));
    let error = load(&source, Some(WRONG)).expect_err("mismatch");
    assert_eq!(error.code(), codes::HASH_MISMATCH);
}

#[test]
fn a_catalogue_entry_loads_from_the_binary() {
    let loaded = load(&Source::Catalogue("massorca.codex".to_owned()), None).expect("catalogue");
    assert_eq!(loaded.manifest.id, "massorca.codex");
    assert_eq!(loaded.sha256, canonical_sha256(&loaded.files));
    let error = load(&Source::Catalogue("acme.nope".to_owned()), None).expect_err("unknown");
    assert_eq!(error.code(), massorca_core_extensions::codes::NOT_FOUND);
}

#[test]
fn a_folder_with_a_missing_referenced_file_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut files = package_files(&agent_package("acme", "kit", "kit"));
    files.remove("agents/kit.json");
    write_dir(dir.path(), &files);
    let error = load(&Source::Dir(dir.path().to_path_buf()), None).expect_err("incomplete");
    assert_eq!(error.code(), codes::MISSING_FILE);
}
