//! First prompts that do not fit a launch argument, and session metadata (decision 0168), through
//! the CLI against the fake agent. Every wait is on an observed state, never a sleep.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, eventually, git_repo};
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";
/// The launch argument limit of a prompt (decision 0168).
const ARGV_PROMPT_MAX_BYTES: usize = 4 * 1024;

/// A status table where start is ready, a prompt works and a `done` carries the turn's result.
fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "prompt": "working", "stop": "done", "fail": "done:failure" },
        "result_field": "result"
    })
}

/// Declares kind `id` running the fake agent with `prompt`, an assigned and learned ref, a resume form, the model and
/// effort flags, and `extra` merged in.
fn declare(sandbox: &Sandbox, id: &str, prompt: Value, extra: Value) {
    let mut kind = json!({
        "id": id,
        "name": format!("Fake agent {id}"),
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": prompt,
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": status(),
        "model": { "flag": "--model" },
        "effort": { "flag": "--effort" }
    });
    if let (Some(kind), Some(extra)) = (kind.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            kind.insert(key.clone(), value.clone());
        }
    }
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with the test kinds and a folder workspace `main`.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(
        &sandbox,
        "fake",
        json!({ "mode": "argv", "paste": "bracketed" }),
        json!({}),
    );
    declare(
        &sandbox,
        "fake-flag",
        json!({ "mode": "flag", "flag": "--prompt", "paste": "bracketed" }),
        json!({}),
    );
    declare(
        &sandbox,
        "fake-query",
        json!({ "mode": "startup-query", "flag": "--query" }),
        json!({}),
    );
    declare(&sandbox, "fake-none", json!({ "mode": "none" }), json!({}));
    declare(
        &sandbox,
        "fake-once",
        json!({ "mode": "argv" }),
        json!({ "resume": null }),
    );
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// Adds `<home>/repo` as a git project and creates its worktree workspace `feature`, on branch `feature`.
fn add_feature_worktree(sandbox: &Sandbox) {
    let repo: PathBuf = sandbox.home().join("repo");
    git_repo(&repo);
    let root = sandbox.home().join("ws");
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = root.to_string_lossy().into_owned();
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
    sandbox.json_ok("default", &["workspace", "create", "repo", "feature"]);
}

fn start(sandbox: &Sandbox, kind: &str, workspace: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "start", kind, "--workspace", workspace];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} in {value}"))
}

fn until(sandbox: &Sandbox, session: &str, state: &str) -> Value {
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session,
            "--until",
            state,
            "--timeout",
            UNTIL_SECS,
        ],
    )
}

/// The lines the terminal shows now, trimmed.
fn screen(sandbox: &Sandbox, terminal: &str) -> Vec<String> {
    let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
    screen["lines"]
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| line.as_str().map(|line| line.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// Waits until the terminal shows a line that `matches`, and returns it.
fn wait_for(
    sandbox: &Sandbox,
    terminal: &str,
    what: &str,
    matches: impl Fn(&str) -> bool,
) -> String {
    eventually(what, || {
        screen(sandbox, terminal)
            .into_iter()
            .find(|line| matches(line))
    })
}

/// A first prompt longer than a launch argument may be is typed once the agent reports ready, and the agent gets it
/// whole (AG-05).
#[test]
fn a_first_prompt_longer_than_a_launch_argument_is_typed_after_ready() {
    let sandbox = sandbox();
    let long = "x".repeat(ARGV_PROMPT_MAX_BYTES + 100);
    let started = start(&sandbox, "fake", "main", &["--prompt", &long]);
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();

    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], format!("done: {long}"));
    let working = format!(
        "fake-agent: working on {}... ({} bytes)",
        "x".repeat(40),
        long.len()
    );
    wait_for(&sandbox, &terminal, &working, |line| line == working);
    assert!(
        !screen(&sandbox, &terminal)
            .iter()
            .any(|line| line == "fake-agent: prompt at launch"),
        "the prompt was not a launch argument"
    );
}

/// A multi-line first prompt for a kind with bracketed paste is typed after ready as one paste (AG-05).
#[test]
fn a_multi_line_first_prompt_is_pasted_after_ready_as_one_prompt() {
    let sandbox = sandbox();
    let started = start(
        &sandbox,
        "fake",
        "main",
        &["--prompt", "first line\nsecond line"],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();

    let done = until(&sandbox, &session, "done");
    // One turn holds both lines. The fake agent reads its terminal line by line: where the platform's terminal passes
    // the pasted line feed it joins the paste's lines with ` / `; a Windows console in line mode drops it, and the
    // lines arrive joined. Either way a second turn never starts with the second line.
    let result = done["result"].as_str().expect("a result");
    assert!(
        [
            "done: first line / second line",
            "done: first linesecond line"
        ]
        .contains(&result),
        "{result}"
    );
    let lines = screen(&sandbox, &terminal);
    assert!(
        !lines
            .iter()
            .any(|line| line == "fake-agent: working on second line"),
        "{lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line == "fake-agent: prompt at launch"),
        "the prompt was not a launch argument: {lines:?}"
    );
}

/// Each delivery form reaches the agent: an argument, a flag, a startup query and a paste after ready (AG-05).
#[test]
fn every_delivery_form_reaches_the_agent() {
    let sandbox = sandbox();
    let cases = [
        ("fake", "by argument", true),
        ("fake-flag", "by flag", true),
        ("fake-query", "by query", true),
        ("fake-none", "by paste", false),
    ];
    for (kind, prompt, at_launch) in cases {
        let started = start(&sandbox, kind, "main", &["--prompt", prompt, "--read-only"]);
        let session = text(&started, "id").to_owned();
        let terminal = text(&started, "terminal_id").to_owned();
        let done = until(&sandbox, &session, "done");
        assert_eq!(done["result"], format!("done: {prompt}"), "{kind}");
        let lines = screen(&sandbox, &terminal);
        assert_eq!(
            lines
                .iter()
                .any(|line| line == "fake-agent: prompt at launch"),
            at_launch,
            "{kind}: {lines:?}"
        );
    }
}

/// A session shows its model, effort, branch and a preview of its result; a resume keeps the model and the effort and
/// passes them again (HIST-02).
#[test]
fn a_session_shows_its_model_effort_branch_and_preview_and_a_resume_keeps_them() {
    let sandbox = sandbox();
    add_feature_worktree(&sandbox);
    let started = start(
        &sandbox,
        "fake",
        "feature",
        &["--prompt", "hello", "--model", "m-1", "--effort", "high"],
    );
    let session = text(&started, "id").to_owned();
    assert_eq!(started["model"], "m-1");
    assert_eq!(started["effort"], "high");
    assert_eq!(started["branch"], "feature");
    assert!(started.get("forked_from").is_none(), "{started}");

    let done = until(&sandbox, &session, "done");
    assert_eq!(done["preview"], "done: hello");
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    let row = listed["sessions"]
        .as_array()
        .and_then(|sessions| sessions.iter().find(|row| row["id"] == session.as_str()))
        .expect("the session is listed")
        .clone();
    assert_eq!(
        (
            &row["model"],
            &row["effort"],
            &row["branch"],
            &row["preview"]
        ),
        (
            &json!("m-1"),
            &json!("high"),
            &json!("feature"),
            &json!("done: hello")
        )
    );
    let shown = sandbox
        .run("default", &["agent", "status", &session])
        .success();
    let shown = String::from_utf8_lossy(&shown.get_output().stdout).into_owned();
    for line in [
        "branch: feature",
        "model: m-1",
        "effort: high",
        "preview: done: hello",
    ] {
        assert!(shown.lines().any(|shown| shown == line), "{line}: {shown}");
    }

    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    let resumed = sandbox.json_ok("default", &["agent", "resume", &session]);
    assert_eq!(
        (&resumed["model"], &resumed["effort"]),
        (&json!("m-1"), &json!("high"))
    );
    let terminal = text(&resumed, "terminal_id").to_owned();
    wait_for(&sandbox, &terminal, "the resumed run's model", |line| {
        line == "fake-agent: model m-1"
    });
    wait_for(&sandbox, &terminal, "the resumed run's effort", |line| {
        line == "fake-agent: effort high"
    });
}
