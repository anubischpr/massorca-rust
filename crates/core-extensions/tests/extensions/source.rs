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

#[test]
fn a_folder_with_a_windows_short_name_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut files = package_files(&agent_package("acme", "kit", "kit"));
    files.insert("EXTENS~1.JSO".to_owned(), b"{}".to_vec());
    write_dir(dir.path(), &files);
    let error = load(&Source::Dir(dir.path().to_path_buf()), None).expect_err("an alias");
    assert_eq!(error.code(), codes::UNSAFE_PATH);
}

#[test]
fn network_and_device_paths_are_never_a_source() {
    let mut refused = vec![
        r"dir:\\host\share\kit",
        r"file:\\?\C:\kit.mxt",
        r"file:\\.\PhysicalDrive0",
        r"dir:\??\C:\kit",
    ];
    if cfg!(windows) {
        refused.extend([
            "dir://host/share/kit",
            r"file:\/host/share/kit.mxt",
            r"dir:/\host\share",
        ]);
    }
    for text in refused {
        let error = Source::parse(text).expect_err(text);
        assert_eq!(
            error.code(),
            massorca_core_extensions::codes::INVALID_SOURCE,
            "{text}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_fifo_in_a_folder_or_as_an_archive_is_refused_without_blocking() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_dir(
        dir.path(),
        &package_files(&agent_package("acme", "kit", "kit")),
    );
    let fifo = dir.path().join("pipe");
    let made = massorca_process::command("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
    let error = load(&Source::Dir(dir.path().to_path_buf()), None).expect_err("a fifo entry");
    assert_eq!(error.code(), codes::UNSAFE_ENTRY);
    // No writer ever opens the FIFO: the check must refuse it without opening it, or this test hangs.
    let error = load(&Source::File(fifo), Some(WRONG)).expect_err("a fifo archive");
    assert_eq!(error.code(), codes::INVALID_ARCHIVE);
}
