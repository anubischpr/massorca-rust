//! `service info` reports the host's git: its version, a refusal for one below the 2.25 baseline, or its absence.

mod harness;

use std::path::{Path, PathBuf};

use harness::Sandbox;
use massorca_process::test_executable::write_executable;
use predicates::str::contains;

/// A stand-in `git` that prints `version_line`; the service runs it through `MASSORCA_GIT`.
fn stand_in_git(dir: &Path, version_line: &str) -> PathBuf {
    if cfg!(windows) {
        let binary = dir.join("git.cmd");
        std::fs::write(&binary, format!("@echo off\r\necho {version_line}\r\n"))
            .expect("write the stand-in git");
        binary
    } else {
        let binary = dir.join("git");
        write_executable(&binary, &format!("#!/bin/sh\necho '{version_line}'\n"));
        binary
    }
}

#[test]
fn info_refuses_a_git_below_the_baseline_by_name() {
    let sandbox = Sandbox::new();
    let dir = tempfile::tempdir().expect("temp dir");
    sandbox.set_env(
        "MASSORCA_GIT",
        stand_in_git(dir.path(), "git version 2.24.0")
            .to_str()
            .expect("utf-8 path"),
    );

    let info = sandbox.json_ok("default", &["service", "info"]);

    assert_eq!(info["git"]["state"], "too_old");
    assert_eq!(info["git"]["version"], "2.24.0");
    assert_eq!(info["git"]["required"], "2.25.0");
}

#[test]
fn info_prints_the_version_of_a_present_git_with_a_vendor_suffix() {
    let sandbox = Sandbox::new();
    let dir = tempfile::tempdir().expect("temp dir");
    sandbox.set_env(
        "MASSORCA_GIT",
        stand_in_git(dir.path(), "git version 2.45.1.windows.1")
            .to_str()
            .expect("utf-8 path"),
    );

    sandbox
        .run("default", &["service", "info"])
        .success()
        .stdout(contains("git: present 2.45.1"));
}

#[test]
fn info_reports_a_missing_git_as_missing() {
    let sandbox = Sandbox::new();
    let dir = tempfile::tempdir().expect("temp dir");
    sandbox.set_env(
        "MASSORCA_GIT",
        dir.path().join("no-such-git").to_str().expect("utf-8 path"),
    );

    let info = sandbox.json_ok("default", &["service", "info"]);

    assert_eq!(info["git"]["state"], "missing");
}
