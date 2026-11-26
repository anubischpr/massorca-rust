//! A kind's standing launch settings (decision 0174) through the real binary, with the fake agent declared as kind
//! `fake`: `agent settings set|show|clear`, the standing program, arguments and variables of every run, and `agent start
//! --env`, whose values win over the standing ones. Every value here is a placeholder.

mod harness;

use std::fs;
use std::path::Path;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, massorca_in, single_document, with_json};
use serde_json::{Value, json};

/// Kept secret: given on stdin, reported by the fake agent as set or unset only, and searched for in the profile.
const SECRET_VALUE: &str = "placeholder-standing-secret-2c9a";

/// Declares kind `fake` running `program`.
fn declare_fake(sandbox: &Sandbox, program: &str) {
    sandbox.prepare_profile("default");
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let fake = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [program],
        "prompt": { "mode": "argv" }
    });
    fs::write(dir.join("fake.json"), fake.to_string()).expect("write the fake kind");
}

fn fake_agent() -> &'static str {
    env!("CARGO_BIN_EXE_massorca-fake-agent")
}

/// The service at trace level, with the fake agent told which variables to report on and which to show.
fn spawn_service(sandbox: &Sandbox) -> ChildGuard {
    sandbox.track("default");
    let child = massorca_in(sandbox.home())
        .args(["--profile", "default", "service", "run"])
        .env("MASSORCA_LOG", "trace")
        .env("FAKE_AGENT_PROBE_ENV", "STANDING_SECRET")
        .env("FAKE_AGENT_SHOW_ENV", "SHARED,STANDING_ONLY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service starts");
    let guard = ChildGuard::new(child);
    sandbox.wait_for_running("default", true);
    guard
}

fn project(sandbox: &Sandbox) {
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("the project folder");
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            notes.to_str().expect("a UTF-8 path"),
            "--folder",
        ],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
}

/// `agent start fake` in workspace `main` with `extra` arguments; returns the terminal id.
fn start_fake(sandbox: &Sandbox, extra: &[&str]) -> String {
    let mut args = vec![
        "agent",
        "start",
        "fake",
        "--workspace",
        "main",
        "--read-only",
    ];
    args.extend_from_slice(extra);
    let started = sandbox.json_ok("default", &args);
    started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned()
}

fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().map(str::trim) == Some(line))
            })
            .then_some(())
    });
}

fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn standing_settings_reach_every_run_and_a_start_wins_over_them() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox, fake_agent());
    let service = spawn_service(&sandbox);
    project(&sandbox);

    let set = sandbox.json_ok(
        "default",
        &[
            "agent",
            "settings",
            "set",
            "fake",
            "--arg",
            "--model",
            "--arg",
            "standing",
            "--env",
            "SHARED=standing",
            "--env",
            "STANDING_ONLY=only",
        ],
    );
    assert_eq!(set["kind"], "fake");
    assert_eq!(set["args"], json!(["--model", "standing"]));
    assert_eq!(set["env_names"], json!(["SHARED", "STANDING_ONLY"]));
    assert!(set.get("env").is_none(), "no value comes back: {set}");

    // A secret value comes from stdin, never from an argument.
    let secret = sandbox
        .run_with_input(
            "default",
            &with_json(&[
                "agent",
                "settings",
                "set",
                "fake",
                "--secret-env",
                "STANDING_SECRET",
            ]),
            format!("{SECRET_VALUE}\n").as_bytes(),
        )
        .success();
    let secret = single_document(&secret)["result"].clone();
    assert_eq!(
        secret["env_names"],
        json!(["SHARED", "STANDING_ONLY", "STANDING_SECRET"])
    );

    let shown = sandbox
        .run("default", &["agent", "settings", "show", "fake"])
        .success();
    let text = String::from_utf8_lossy(&shown.get_output().stdout).into_owned();
    assert!(text.contains("arguments: --model standing"), "{text}");
    assert!(text.contains("STANDING_SECRET"), "{text}");
    assert!(
        !text.contains(SECRET_VALUE) && !text.contains("only"),
        "{text}"
    );

    let plain = start_fake(&sandbox, &[]);
    wait_for_line(&sandbox, &plain, "fake-agent: model standing");
    wait_for_line(&sandbox, &plain, "fake-agent: value SHARED [standing]");
    wait_for_line(&sandbox, &plain, "fake-agent: value STANDING_ONLY [only]");
    wait_for_line(&sandbox, &plain, "fake-agent: env STANDING_SECRET set");

    // The start's own arguments come later and its variables replace the standing ones of the same name.
    let own = start_fake(
        &sandbox,
        &["--env", "SHARED=caller", "--", "--model", "caller"],
    );
    wait_for_line(&sandbox, &own, "fake-agent: model caller");
    wait_for_line(&sandbox, &own, "fake-agent: value SHARED [caller]");
    wait_for_line(&sandbox, &own, "fake-agent: value STANDING_ONLY [only]");

    // Removing a variable and the arguments leaves the rest.
    let changed = sandbox.json_ok(
        "default",
        &[
            "agent",
            "settings",
            "set",
            "fake",
            "--no-args",
            "--unset-env",
            "SHARED",
        ],
    );
    assert!(changed.get("args").is_none(), "{changed}");
    assert_eq!(
        changed["env_names"],
        json!(["STANDING_ONLY", "STANDING_SECRET"])
    );
    let after = start_fake(&sandbox, &[]);
    wait_for_line(&sandbox, &after, "fake-agent: value SHARED unset");
    wait_for_line(&sandbox, &after, "fake-agent: value STANDING_ONLY [only]");

    let cleared = sandbox.json_ok("default", &["agent", "settings", "clear", "fake"]);
    assert_eq!(cleared, json!({ "kind": "fake", "removed": true }));
    let none = sandbox.json_ok("default", &["agent", "settings", "show", "fake"]);
    assert_eq!(none, json!({ "kind": "fake" }));

    drop(service);
    sandbox.run("default", &["service", "stop"]).success();
    let mut files = Vec::new();
    files_under(&sandbox.profile_dir("default"), &mut files);
    for path in &files {
        let bytes = fs::read(path).unwrap_or_default();
        let found = bytes
            .windows(SECRET_VALUE.len())
            .any(|window| window == SECRET_VALUE.as_bytes());
        assert!(!found, "{} holds the secret value", path.display());
    }
}

#[test]
fn a_standing_program_runs_a_kind_whose_own_program_is_missing() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox, "massorca-test-no-such-agent");
    let _service = spawn_service(&sandbox);
    project(&sandbox);

    let refused = |sandbox: &Sandbox| -> Value {
        let outcome = sandbox
            .run(
                "default",
                &with_json(&[
                    "agent",
                    "start",
                    "fake",
                    "--workspace",
                    "main",
                    "--read-only",
                ]),
            )
            .failure();
        single_document(&outcome)["error"].clone()
    };
    assert_eq!(refused(&sandbox)["code"], "agent.not_installed");

    let set = sandbox.json_ok(
        "default",
        &[
            "agent",
            "settings",
            "set",
            "fake",
            "--program",
            fake_agent(),
        ],
    );
    assert_eq!(set["program"], fake_agent());
    let run = start_fake(&sandbox, &[]);
    wait_for_line(&sandbox, &run, "fake-agent: ready");

    // A program that does not resolve is refused and never repeated.
    let missing = sandbox
        .run(
            "default",
            &with_json(&[
                "agent",
                "settings",
                "set",
                "fake",
                "--program",
                "massorca-test-no-such-wrapper",
            ]),
        )
        .failure();
    let error = single_document(&missing)["error"].clone();
    assert_eq!(error["code"], "agent.not_installed", "{error}");
    assert!(
        !error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("massorca-test-no-such-wrapper"),
        "{error}"
    );

    sandbox.json_ok(
        "default",
        &["agent", "settings", "set", "fake", "--no-program"],
    );
    assert_eq!(refused(&sandbox)["code"], "agent.not_installed");
}

#[test]
fn an_unknown_kind_and_a_reserved_name_are_refused() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox, fake_agent());
    let _service = spawn_service(&sandbox);

    let error = |args: &[&str]| -> Value {
        let outcome = sandbox.run("default", &with_json(args)).failure();
        single_document(&outcome)["error"].clone()
    };
    assert_eq!(
        error(&["agent", "settings", "set", "no-such-kind", "--arg", "-x"])["code"],
        "agent.kind_not_found"
    );
    let reserved = error(&[
        "agent",
        "settings",
        "set",
        "fake",
        "--env",
        "MASSORCA_SESSION_ID=placeholder-reserved",
    ]);
    assert_eq!(reserved["code"], "request.invalid_params", "{reserved}");
    assert!(
        !reserved["message"]
            .as_str()
            .unwrap_or_default()
            .contains("placeholder-reserved"),
        "{reserved}"
    );
    // An `--env` without `=` is refused before anything is sent.
    let malformed = sandbox
        .run("default", &["agent", "start", "fake", "--env", "NOVALUE"])
        .failure();
    let stderr = String::from_utf8_lossy(&malformed.get_output().stderr).into_owned();
    assert!(stderr.contains("KEY=VALUE"), "{stderr}");
}
