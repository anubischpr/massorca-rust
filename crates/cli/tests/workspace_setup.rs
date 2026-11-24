//! `workspace create` with a setup script (decision 0086): it prints the setup terminal and how to attach to it, and
//! `--no-setup` skips the script.

mod harness;

use harness::{Sandbox, eventually, git, git_repo, stdout_text};

/// Adds `<home>/repo`, whose `orca.yaml` setup writes `setup-out.txt`, as a project with workspaces under `<home>/ws`.
fn add_project_with_setup(sandbox: &Sandbox) {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    std::fs::write(
        repo.join("orca.yaml"),
        "scripts:\n  setup: echo ok > setup-out.txt\n",
    )
    .expect("write orca.yaml");
    git(&repo, &["add", "orca.yaml"]);
    git(&repo, &["commit", "-q", "-m", "setup"]);
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = sandbox.home().join("ws").to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo_text.as_str(),
            "--workspaces-root",
            root_text.as_str(),
        ],
    );
}

#[test]
fn create_prints_the_setup_terminal_and_how_to_attach_to_it() {
    let sandbox = Sandbox::new();
    add_project_with_setup(&sandbox);

    let outcome = sandbox
        .run("default", &["workspace", "create", "repo", "feature"])
        .success();
    let text = stdout_text(&outcome);
    let terminal = text
        .lines()
        .find_map(|line| line.strip_prefix("setup running in terminal "))
        .unwrap_or_else(|| panic!("no setup line: {text}"))
        .trim()
        .to_owned();
    assert!(
        text.contains(&format!("watch it: massorca terminal attach {terminal}")),
        "{text}"
    );
    let shown = sandbox.json_ok("default", &["terminal", "show", terminal.as_str()]);
    assert_eq!(shown["title"], "Setup");
    let workspace = sandbox.json_ok("default", &["workspace", "show", "feature"]);
    let output =
        std::path::PathBuf::from(workspace["path"].as_str().expect("path")).join("setup-out.txt");
    eventually("the setup output", || output.is_file().then_some(()));
}

#[test]
fn no_setup_skips_the_script_and_json_carries_the_outcome() {
    let sandbox = Sandbox::new();
    add_project_with_setup(&sandbox);

    let created = sandbox.json_ok(
        "default",
        &["workspace", "create", "repo", "quiet", "--no-setup"],
    );
    assert_eq!(created["setup"]["outcome"], "skipped");
    let terminals = sandbox.json_ok("default", &["terminal", "list", "--workspace", "quiet"]);
    assert_eq!(terminals["terminals"].as_array().map(Vec::len), Some(0));

    let started = sandbox.json_ok("default", &["workspace", "create", "repo", "busy"]);
    assert_eq!(started["setup"]["outcome"], "started");
    assert!(started["setup"]["terminal"].is_string());
}
