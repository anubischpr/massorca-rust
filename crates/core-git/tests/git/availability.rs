//! The git version gate and the per-binary availability probe, run against stand-in git binaries.

use std::path::{Path, PathBuf};

use massorca_core_git::{Git, GitAvailability, GitError, GitVersion};

fn version(major: u32, minor: u32, patch: u32) -> GitVersion {
    GitVersion {
        major,
        minor,
        patch,
    }
}

/// A `git` that prints `version_line` for every call and appends one line to the returned log per run.
fn stand_in(dir: &Path, version_line: &str) -> (Git, PathBuf) {
    let log = dir.join("runs.log");
    let (binary, script) = if cfg!(windows) {
        (
            dir.join("git.cmd"),
            format!(
                "@echo off\r\necho run>>\"{}\"\r\necho {version_line}\r\n",
                log.display()
            ),
        )
    } else {
        (
            dir.join("git"),
            format!(
                "#!/bin/sh\necho run >> '{}'\necho '{version_line}'\n",
                log.display()
            ),
        )
    };
    std::fs::write(&binary, script).expect("write the stand-in git");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .expect("make the stand-in git executable");
    }
    (Git::with_binary(binary), log)
}

fn runs(log: &Path) -> usize {
    std::fs::read_to_string(log).map_or(0, |text| text.lines().count())
}

#[test]
fn a_git_older_than_the_2_25_baseline_is_refused_with_both_versions() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (git, _) = stand_in(dir.path(), "git version 2.24.0");
    assert_eq!(
        git.availability(),
        GitAvailability::TooOld(version(2, 24, 0))
    );
    match git.branch_exists(dir.path(), "main") {
        Err(error @ GitError::TooOld { .. }) => {
            let message = error.to_string();
            assert!(message.contains("2.24.0"), "{message}");
            assert!(message.contains("2.25.0"), "{message}");
        }
        other => panic!("expected the version refusal, got {other:?}"),
    }
}

#[test]
fn git_at_the_baseline_is_present() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (git, _) = stand_in(dir.path(), "git version 2.25.0");
    assert_eq!(
        git.availability(),
        GitAvailability::Present(version(2, 25, 0))
    );
}

#[test]
fn vendor_build_suffixes_are_read_as_the_release() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (windows, _) = stand_in(dir.path(), "git version 2.45.1.windows.1");
    assert_eq!(
        windows.availability(),
        GitAvailability::Present(version(2, 45, 1))
    );
    let apple_dir = tempfile::tempdir().expect("temp dir");
    let (apple, _) = stand_in(apple_dir.path(), "git version 2.39.3 (Apple Git-145)");
    assert_eq!(
        apple.availability(),
        GitAvailability::Present(version(2, 39, 3))
    );
}

#[test]
fn a_missing_binary_is_missing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git = Git::with_binary(dir.path().join("no-such-git"));
    assert_eq!(git.availability(), GitAvailability::Missing);
}

#[test]
fn output_that_is_not_a_version_is_unknown_and_probed_again() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (git, log) = stand_in(dir.path(), "hello");
    assert!(matches!(git.availability(), GitAvailability::Unknown(_)));
    assert!(matches!(git.availability(), GitAvailability::Unknown(_)));
    assert_eq!(runs(&log), 2);
}

#[test]
fn a_found_version_is_probed_once_per_binary_and_shared_by_clones() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (git, log) = stand_in(dir.path(), "git version 2.25.0");
    git.availability();
    git.clone().availability();
    assert_eq!(runs(&log), 1);
}
