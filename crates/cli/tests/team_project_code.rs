//! `massorca team project` code sources end to end (decision 0126): `show` prints the code source, `none` clears it with
//! Workspaces off, and recording a kind is refused there.

mod harness;

use assert_cmd::assert::Assert;
use harness::{Sandbox, stdout_text};

fn stderr_text(outcome: &Assert) -> String {
    String::from_utf8_lossy(&outcome.get_output().stderr).into_owned()
}

#[test]
fn show_prints_the_code_source_and_none_clears_it_with_workspaces_off() {
    let sandbox = Sandbox::new();
    sandbox.json_ok("default", &["team", "create", "alpha"]);
    sandbox.json_ok(
        "default",
        &["team", "update", "alpha", "--workspaces", "true"],
    );
    sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "add",
            "alpha",
            "parser",
            "--code-source",
            "existing-repository",
        ],
    );

    let shown = sandbox.json_ok("default", &["team", "project", "show", "alpha", "parser"]);
    assert_eq!(shown["code_source"], "existing_repository");
    let text = stdout_text(
        &sandbox
            .run("default", &["team", "project", "show", "alpha", "parser"])
            .success(),
    );
    assert!(text.contains("code source: Existing repository"), "{text}");

    sandbox.json_ok(
        "default",
        &["team", "update", "alpha", "--workspaces", "false"],
    );
    let refused = sandbox
        .run(
            "default",
            &[
                "team",
                "project",
                "update",
                "alpha",
                "parser",
                "--code-source",
                "new-folder-with-git",
            ],
        )
        .failure()
        .code(1);
    assert!(stderr_text(&refused).contains("team.workspaces_off"));

    let cleared = sandbox.json_ok(
        "default",
        &[
            "team",
            "project",
            "update",
            "alpha",
            "parser",
            "--code-source",
            "none",
        ],
    );
    assert!(cleared["code_source"].is_null());
    let after = stdout_text(
        &sandbox
            .run("default", &["team", "project", "show", "alpha", "parser"])
            .success(),
    );
    assert!(after.contains("code source: -"), "{after}");

    let missing = sandbox
        .run("default", &["team", "project", "show", "alpha", "nope"])
        .failure()
        .code(1);
    assert!(stderr_text(&missing).contains("team.project_not_found"));
}
