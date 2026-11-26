//! What a crash or a kill during an install, refresh or adoption leaves under `pkgs/` is removed at start, and nothing
//! a lockfile entry names.

use std::fs;
use std::path::PathBuf;

use massorca_core_extensions::codes;
use serde_json::{Value, json};

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::{Setup, setup};

/// A profile with `acme.kit` installed. Returns its lockfile path.
fn installed() -> (Setup, String) {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("installs");
    let path = s.lock_json().expect("the lockfile")["packages"][0]["path"]
        .as_str()
        .expect("a path")
        .to_owned();
    (s, path)
}

/// A folder with one file under the extensions folder.
fn leftover(s: &Setup, rel: &str) -> PathBuf {
    let dir = s.service.layout().resolve(rel);
    fs::create_dir_all(&dir).expect("the leftover folder");
    fs::write(dir.join("extension.json"), "{}").expect("a leftover file");
    dir
}

fn write_lock(s: &Setup, lock: &Value) {
    fs::write(s.service.layout().lock_path(), lock.to_string()).expect("the lockfile is written");
}

#[test]
fn a_clean_profile_reports_nothing_removed() {
    let s = setup(FakeHost::new());
    assert!(s.service.sweep_leftovers().expect("sweeps").is_empty());
    let (s, path) = installed();
    assert!(s.service.sweep_leftovers().expect("sweeps").is_empty());
    assert!(s.exists(&path));
}

#[test]
fn a_staging_folder_is_removed() {
    let (s, path) = installed();
    leftover(&s, "pkgs/acme.kit/.staging-1-0");

    let removed = s.service.sweep_leftovers().expect("sweeps");

    assert_eq!(removed, ["pkgs/acme.kit/.staging-1-0"]);
    assert!(!s.exists("pkgs/acme.kit/.staging-1-0"));
    assert!(s.exists(&path));
}

#[test]
fn an_unreferenced_version_folder_is_removed_and_the_installed_one_stays_intact() {
    let (s, path) = installed();
    let manifest = s.service.layout().resolve(&path).join("extension.json");
    let before = fs::read(&manifest).expect("the installed manifest");
    leftover(&s, "pkgs/acme.kit/0.0.1-00000000");

    let removed = s.service.sweep_leftovers().expect("sweeps");

    assert_eq!(removed, ["pkgs/acme.kit/0.0.1-00000000"]);
    assert!(!s.exists("pkgs/acme.kit/0.0.1-00000000"));
    assert_eq!(fs::read(&manifest).expect("still there"), before);
}

#[test]
fn a_kept_previous_version_is_not_swept() {
    let (s, path) = installed();
    let kept = "pkgs/acme.kit/0.0.1-00000000";
    leftover(&s, kept);
    let mut lock = s.lock_json().expect("the lockfile");
    lock["packages"][0]["previous"] = json!({
        "version": "0.0.1", "source": { "kind": "dir", "ref": "x" },
        "sha256": "00000000", "path": kept, "consent": "accepted-earlier"
    });
    write_lock(&s, &lock);

    assert!(s.service.sweep_leftovers().expect("sweeps").is_empty());
    assert!(s.exists(kept), "the rollback target stays");
    assert!(s.exists(&path));
}

#[test]
fn folders_any_entry_names_are_kept_even_disabled_or_duplicated() {
    let (s, path) = installed();
    let second = "pkgs/acme.kit/0.0.2-11111111";
    leftover(&s, second);
    let mut lock = s.lock_json().expect("the lockfile");
    lock["packages"][0]["enabled"] = json!(false);
    let mut duplicate = lock["packages"][0].clone();
    duplicate["path"] = json!(second);
    lock["packages"]
        .as_array_mut()
        .expect("packages")
        .push(duplicate);
    write_lock(&s, &lock);

    assert!(s.service.sweep_leftovers().expect("sweeps").is_empty());
    assert!(s.exists(&path));
    assert!(s.exists(second));
}

#[test]
fn an_unreadable_or_corrupt_lockfile_removes_nothing() {
    let (s, _) = installed();
    leftover(&s, "pkgs/acme.kit/.staging-1-0");
    leftover(&s, "pkgs/acme.other/1.0.0-22222222");
    fs::write(s.service.layout().lock_path(), "{ not json").expect("a corrupt lockfile");

    let error = s.service.sweep_leftovers().expect_err("refuses");
    assert_eq!(error.code(), codes::INVALID_LOCKFILE);
    assert!(s.exists("pkgs/acme.kit/.staging-1-0"));
    assert!(s.exists("pkgs/acme.other/1.0.0-22222222"));

    // A folder where the lockfile should be cannot be read at all.
    fs::remove_file(s.service.layout().lock_path()).expect("the lockfile goes");
    fs::create_dir(s.service.layout().lock_path()).expect("a folder in its place");
    let error = s.service.sweep_leftovers().expect_err("refuses");
    assert_eq!(error.code(), codes::IO);
    assert!(s.exists("pkgs/acme.kit/.staging-1-0"));
    assert!(s.exists("pkgs/acme.other/1.0.0-22222222"));
}

#[test]
fn an_empty_package_folder_is_removed_after_its_leftovers() {
    let (s, _) = installed();
    leftover(&s, "pkgs/acme.other/1.0.0-22222222");
    leftover(&s, "pkgs/acme.other/.staging-3-4");
    fs::create_dir_all(s.service.layout().resolve("pkgs/acme.empty")).expect("an empty folder");

    let mut removed = s.service.sweep_leftovers().expect("sweeps");
    removed.sort();

    assert_eq!(
        removed,
        [
            "pkgs/acme.empty",
            "pkgs/acme.other",
            "pkgs/acme.other/.staging-3-4",
            "pkgs/acme.other/1.0.0-22222222",
        ]
    );
    assert!(!s.exists("pkgs/acme.other"));
    assert!(!s.exists("pkgs/acme.empty"));
    assert!(s.exists("pkgs/acme.kit"));
}

#[test]
fn files_and_unknown_names_under_a_package_folder_are_left_alone() {
    let (s, _) = installed();
    let note = s.service.layout().resolve("pkgs/acme.kit/note.txt");
    fs::write(&note, "kept").expect("a file");
    leftover(&s, "pkgs/acme.kit/.cache");
    leftover(&s, "pkgs/acme.kit/not-a-version");

    assert!(s.service.sweep_leftovers().expect("sweeps").is_empty());
    assert!(note.exists());
    assert!(s.exists("pkgs/acme.kit/.cache"));
    assert!(s.exists("pkgs/acme.kit/not-a-version"));
}

#[cfg(unix)]
#[test]
fn a_link_under_pkgs_is_not_followed() {
    let (s, _) = installed();
    let outside = s.home.path().join("outside");
    fs::create_dir_all(outside.join("1.0.0-33333333")).expect("an outside folder");
    fs::create_dir_all(outside.join(".staging-5-6")).expect("an outside staging folder");
    let package_link = s.service.layout().resolve("pkgs/acme.linked");
    std::os::unix::fs::symlink(&outside, &package_link).expect("a package link");
    let version_link = s.service.layout().resolve("pkgs/acme.kit/1.0.0-44444444");
    std::os::unix::fs::symlink(&outside, &version_link).expect("a version link");

    assert!(s.service.sweep_leftovers().expect("sweeps").is_empty());
    assert!(outside.join("1.0.0-33333333").exists());
    assert!(outside.join(".staging-5-6").exists());
    assert!(fs::symlink_metadata(&package_link).is_ok());
    assert!(fs::symlink_metadata(&version_link).is_ok());
}

#[test]
fn without_a_lockfile_only_staging_folders_are_removed() {
    let (s, path) = installed();
    leftover(&s, "pkgs/acme.kit/.staging-1-0");
    fs::remove_file(s.service.layout().lock_path()).expect("the lockfile goes");

    let removed = s.service.sweep_leftovers().expect("sweeps");

    assert_eq!(removed, ["pkgs/acme.kit/.staging-1-0"]);
    assert!(
        s.exists(&path),
        "a version folder stays when no lockfile says it is unused"
    );
}
