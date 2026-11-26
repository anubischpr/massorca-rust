//! `service tools` lists the tools of the service's host, one line each, or as JSON.

mod harness;

use std::path::{Path, PathBuf};

use harness::Sandbox;
use massorca_process::test_executable::write_executable;
use predicates::str::contains;

/// A stand-in program `name` that prints `line`; a `.cmd` batch on Windows.
fn stand_in(dir: &Path, name: &str, line: &str) -> PathBuf {
    if cfg!(windows) {
        let binary = dir.join(format!("{name}.cmd"));
        std::fs::write(&binary, format!("@echo off\r\necho {line}\r\n"))
            .expect("write the stand-in");
        binary
    } else {
        let binary = dir.join(name);
        write_executable(&binary, &format!("#!/bin/sh\necho '{line}'\n"));
        binary
    }
}

/// The `PATH` with `dir` first, so a stand-in `gh` is the one found.
fn path_with_first(dir: &Path) -> String {
    let mut entries = vec![dir.to_path_buf()];
    entries.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(entries)
        .expect("join the PATH")
        .into_string()
        .expect("utf-8 PATH")
}

/// Sandbox with a stand-in git (through `MASSORCA_GIT`) and a stand-in `gh` first on the `PATH`.
fn sandbox_with_tools(dir: &Path) -> Sandbox {
    let sandbox = Sandbox::new();
    let git = stand_in(dir, "git", "git version 2.45.1");
    sandbox.set_env("MASSORCA_GIT", git.to_str().expect("utf-8 path"));
    let tools = dir.join("tools");
    std::fs::create_dir_all(&tools).expect("create the tools folder");
    stand_in(&tools, "gh", "gh version 2.40.1 (2023-12-13)");
    sandbox.set_env("PATH", &path_with_first(&tools));
    sandbox
}

#[test]
fn tools_reports_git_and_a_forge_cli_found_on_the_path_as_json() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sandbox = sandbox_with_tools(dir.path());

    let tools = sandbox.json_ok("default", &["service", "tools", "--refresh"]);

    let list = tools["tools"].as_array().expect("a tools array");
    let find = |id: &str| {
        list.iter()
            .find(|tool| tool["id"] == id)
            .unwrap_or_else(|| panic!("{id} is listed: {tools}"))
            .clone()
    };
    let git = find("git");
    assert_eq!(git["kind"], "vcs");
    assert_eq!(git["state"], "present");
    assert_eq!(git["version"], "2.45.1");
    let gh = find("gh");
    assert_eq!(gh["kind"], "forge");
    assert_eq!(gh["state"], "present");
    assert_eq!(gh["version"], "2.40.1");
}

#[test]
fn tools_prints_one_line_per_tool() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sandbox = sandbox_with_tools(dir.path());

    sandbox
        .run("default", &["service", "tools"])
        .success()
        .stdout(contains("git: present 2.45.1"))
        .stdout(contains("gh: present 2.40.1"));
}
