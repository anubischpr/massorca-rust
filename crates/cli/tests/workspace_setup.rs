//! `workspace create` with a setup script (decision 0086): it prints the setup terminal and how to attach to it, and
//! `--no-setup` skips the script. `project trust-scripts` shows the scripts with their hashes and trusts one by its
//! hash; an untrusted script is skipped (decision 0092).

mod harness;

use harness::{Sandbox, eventually, git, git_repo, stdout_text};

/// Adds `<home>/repo`, whose `orca.yaml` setup writes `setup-out.txt`, as a project with workspaces under `<home>/ws`.
fn add_project_with_setup(sandbox: &Sandbox) {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    std::fs::write(
        repo.join("orca.yaml"),
        "scripts:\n  setup: echo ok > setup-out.txt\n  archive: echo archived\n",
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

/// Trusts the setup script `project trust-scripts repo` shows, and returns its hash.
fn trust_setup(sandbox: &Sandbox) -> String {
    let shown = sandbox.json_ok("default", &["project", "trust-scripts", "repo"]);
    let hash = shown["scripts"][0]["hash"]
        .as_str()
        .expect("the setup script's hash")
        .to_owned();
    let trusted = sandbox.json_ok(
        "default",
        &["project", "trust-scripts", "repo", "--hash", hash.as_str()],
    );
    assert_eq!(trusted["scripts"][0]["kind"], "setup");
    assert_eq!(trusted["scripts"][0]["trusted"], true);
    assert_eq!(trusted["scripts"][1]["trusted"], false);
    hash
}

#[test]
fn trust_scripts_shows_each_script_with_its_hash_and_how_to_trust_it() {
    let sandbox = Sandbox::new();
    add_project_with_setup(&sandbox);

    let outcome = sandbox
        .run("default", &["project", "trust-scripts", "repo"])
        .success();
    let text = stdout_text(&outcome);
    let shown = sandbox.json_ok("default", &["project", "trust-scripts", "repo"]);
    let setup = shown["scripts"][0]["hash"].as_str().expect("setup hash");
    let archive = shown["scripts"][1]["hash"].as_str().expect("archive hash");
    assert_eq!(setup.len(), 64);
    assert!(
        text.contains(&format!("setup script (not trusted), sha256 {setup}:")),
        "{text}"
    );
    assert!(text.contains("    echo ok > setup-out.txt"), "{text}");
    assert!(
        text.contains(&format!("archive script (not trusted), sha256 {archive}:")),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "trust it: massorca project trust-scripts repo --hash {setup}"
        )),
        "{text}"
    );

    let unknown = sandbox
        .run(
            "default",
            &["project", "trust-scripts", "repo", "--hash", "0123abcd"],
        )
        .failure();
    let stderr = String::from_utf8_lossy(&unknown.get_output().stderr).into_owned();
    assert!(stderr.contains("has hash 0123abcd"), "{stderr}");
}

#[test]
fn an_untrusted_setup_is_skipped_until_its_hash_is_trusted() {
    let sandbox = Sandbox::new();
    add_project_with_setup(&sandbox);

    let outcome = sandbox
        .run("default", &["workspace", "create", "repo", "early"])
        .success();
    let text = stdout_text(&outcome);
    assert!(text.contains("setup skipped: the setup script"), "{text}");
    let terminals = sandbox.json_ok("default", &["terminal", "list", "--workspace", "early"]);
    assert_eq!(terminals["terminals"].as_array().map(Vec::len), Some(0));

    let hash = trust_setup(&sandbox);
    let started = sandbox.json_ok("default", &["workspace", "create", "repo", "later"]);
    assert_eq!(started["setup"]["outcome"], "started");
    assert_ne!(hash, "");

    let removed = sandbox.json_ok("default", &["workspace", "rm", "early"]);
    assert_eq!(removed["archive"]["outcome"], "untrusted");
}

#[test]
fn create_prints_the_setup_terminal_and_how_to_attach_to_it() {
    let sandbox = Sandbox::new();
    add_project_with_setup(&sandbox);
    trust_setup(&sandbox);

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
    trust_setup(&sandbox);
    let terminals = sandbox.json_ok("default", &["terminal", "list", "--workspace", "quiet"]);
    assert_eq!(terminals["terminals"].as_array().map(Vec::len), Some(0));

    let started = sandbox.json_ok("default", &["workspace", "create", "repo", "busy"]);
    assert_eq!(started["setup"]["outcome"], "started");
    assert!(started["setup"]["terminal"].is_string());
}
