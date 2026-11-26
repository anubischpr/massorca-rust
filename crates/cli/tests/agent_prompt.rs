//! Typed prompts and resume through the CLI (decision 0054). The fake agent is declared as kinds in the test profile;
//! the tests wait on observed states (`agent status --until`) or on the agent's screen, never on sleeps.

mod harness;

use harness::{Sandbox, eventually, single_document, with_json};
use predicates::str::contains;
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";
/// Most bytes of a prompt (0054).
const MAX_PROMPT_BYTES: usize = 8 * 1024;

/// The status table every fake kind declares: start is ready, a prompt works, a `done` carries the turn's result.
fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": {
            "start": "idle", "prompt": "working", "ask": "waiting",
            "stop": "done", "fail": "done:failure"
        },
        "result_field": "result"
    })
}

/// A status table that maps no event for a prompt: the fake agent reports `idle` at start and `done` at the end of a turn,
/// and nothing in between, so an accepted prompt leaves the session idle until the agent reports again.
fn quiet_status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "stop": "done" },
        "result_field": "result"
    })
}

/// Declares kind `id`, which runs the fake agent, with the given prompt table and status; `resume` adds its resume form.
fn declare(sandbox: &Sandbox, id: &str, prompt: Value, status: Value, resume: bool) {
    let mut kind = json!({
        "id": id,
        "name": format!("Fake agent {id}"),
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": prompt,
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "status": status
    });
    if resume {
        kind["resume"] = json!(["--resume", "{ref}"]);
    }
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with these kinds and a folder workspace `main`: `fake` (argv prompt, resumable), `fake-none` (prompt
/// `none`), `fake-silent` (prompt `none` and no status mechanism, so it can never report `idle`) and `fake-once` (argv
/// prompt, no resume form) and `fake-query` (`startup-query` prompt, resumable).
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(&sandbox, "fake", json!({ "mode": "argv" }), status(), true);
    declare(
        &sandbox,
        "fake-paste",
        json!({ "mode": "argv", "paste": "bracketed" }),
        status(),
        true,
    );
    declare(
        &sandbox,
        "fake-none",
        json!({ "mode": "none" }),
        status(),
        true,
    );
    declare(
        &sandbox,
        "fake-silent",
        json!({ "mode": "none" }),
        json!({ "mechanism": "none" }),
        true,
    );
    declare(
        &sandbox,
        "fake-once",
        json!({ "mode": "argv" }),
        status(),
        false,
    );
    declare(
        &sandbox,
        "fake-query",
        json!({ "mode": "startup-query", "flag": "--query" }),
        status(),
        true,
    );
    declare(
        &sandbox,
        "fake-quiet",
        json!({ "mode": "argv" }),
        quiet_status(),
        true,
    );
    declare(
        &sandbox,
        "fake-quiet-none",
        json!({ "mode": "none" }),
        quiet_status(),
        true,
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

/// `agent start <kind> --workspace main <extra>`, which must succeed; returns the session record.
fn start(sandbox: &Sandbox, kind: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "start", kind, "--workspace", "main"];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

fn session_of(record: &Value) -> String {
    record["id"].as_str().expect("session id").to_owned()
}

fn terminal_of(record: &Value) -> String {
    record["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned()
}

/// `agent status <session> --until <state>`, which must succeed; returns the record it printed.
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

fn activity(session: &Value) -> (&str, Option<&str>, &str) {
    let activity = &session["activity"];
    (
        activity["state"].as_str().unwrap_or_default(),
        activity["outcome"].as_str(),
        activity["source"].as_str().unwrap_or_default(),
    )
}

/// Runs `--json <args>`, which must fail, and returns the error code.
fn error_code(sandbox: &Sandbox, args: &[&str]) -> String {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"]["code"]
        .as_str()
        .expect("an error code")
        .to_owned()
}

/// Waits until the terminal's screen shows `line`.
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

#[test]
fn a_prompt_to_an_idle_agent_drives_it_to_working_then_done() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);
    until(&sandbox, &session, "idle");

    sandbox
        .run("default", &["agent", "prompt", &session, "hold this"])
        .success()
        .stdout(contains("handed to the terminal: 10 bytes"));
    let working = until(&sandbox, &session, "working");
    assert_eq!(activity(&working), ("working", None, "hook"));

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(activity(&done), ("done", Some("success"), "hook"));
    assert_eq!(done["result"], "done: hold this");
}

#[test]
fn a_startup_query_prompt_reaches_the_agent_as_its_query_flag_at_launch() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake-query", &["--prompt", "hold query"]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);
    let working = until(&sandbox, &session, "working");
    assert_eq!(activity(&working), ("working", None, "hook"));
    wait_for_line(&sandbox, &terminal, "fake-agent: working on hold query");

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: hold query");
}

#[test]
fn a_prompt_while_the_agent_works_is_refused_as_busy_unless_forced() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--prompt", "hold busy"]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);
    until(&sandbox, &session, "working");

    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, "second"]),
        "agent.busy"
    );
    let forced = sandbox.json_ok(
        "default",
        &["agent", "prompt", &session, "second", "--force"],
    );
    assert_eq!(forced["receipt"], "handed_to_terminal");
    assert_eq!(forced["bytes"], 7, "the text and its carriage return");

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: hold busy");
}

#[test]
fn a_prompt_to_an_exited_run_is_refused_as_not_live() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    until(&sandbox, &session, "idle");
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);

    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, "too late"]),
        "agent.not_live"
    );
}

#[test]
fn a_prompt_over_the_size_limit_is_refused_before_anything_is_typed() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    until(&sandbox, &session, "idle");

    let long = "a".repeat(MAX_PROMPT_BYTES + 1);
    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, &long]),
        "agent.prompt_invalid"
    );
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(activity(&shown), ("idle", None, "hook"), "still idle");
}

#[test]
fn a_first_prompt_for_a_kind_that_takes_none_waits_for_its_first_idle() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake-none", &["--prompt", "hold first"]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);

    let working = until(&sandbox, &session, "working");
    assert_eq!(activity(&working), ("working", None, "hook"));
    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: hold first");
}

#[test]
fn a_first_prompt_for_a_kind_that_reports_no_idle_is_refused_at_launch() {
    let sandbox = sandbox();
    assert_eq!(
        error_code(
            &sandbox,
            &[
                "agent",
                "start",
                "fake-silent",
                "--workspace",
                "main",
                "--prompt",
                "hi"
            ]
        ),
        "agent.prompt_invalid"
    );
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"], json!([]), "nothing was started");
}

#[test]
fn resume_starts_a_new_run_of_the_same_session_with_its_agent_ref() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--prompt", "finish up"]);
    let session = session_of(&started);
    let first_terminal = terminal_of(&started);
    let agent_ref = started["agent_ref"]
        .as_str()
        .expect("the ref is assigned at launch")
        .to_owned();
    until(&sandbox, &session, "done");
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);

    let resumed = sandbox.json_ok("default", &["agent", "resume", &session]);
    assert_eq!(resumed["id"], session.as_str(), "the session keeps its id");
    assert_eq!(resumed["agent_ref"], agent_ref.as_str());
    assert_eq!(resumed["execution"]["state"], "live");
    let terminal = terminal_of(&resumed);
    assert_ne!(terminal, first_terminal, "a new run has a new terminal");
    wait_for_line(
        &sandbox,
        &terminal,
        &format!("fake-agent: resumed {agent_ref}"),
    );

    // The new run's own hooks are accepted: its start reports idle.
    let idle = until(&sandbox, &session, "idle");
    assert_eq!(activity(&idle), ("idle", None, "hook"));
}

#[test]
fn resume_of_a_live_session_is_refused_as_busy() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    until(&sandbox, &session, "idle");

    assert_eq!(
        error_code(&sandbox, &["agent", "resume", &session]),
        "agent.busy"
    );
}

#[test]
fn resume_is_refused_for_a_kind_without_a_resume_form() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake-once", &["--prompt", "once"]);
    let session = session_of(&started);
    until(&sandbox, &session, "done");
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);

    assert_eq!(
        error_code(&sandbox, &["agent", "resume", &session]),
        "agent.resume_unavailable"
    );
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["execution"]["state"], "exited", "nothing changed");
}

#[test]
fn resume_runs_beside_another_writer_in_a_folder() {
    let sandbox = sandbox();
    let first = start(&sandbox, "fake", &[]);
    let first_session = session_of(&first);
    until(&sandbox, &first_session, "idle");
    sandbox.json_ok("default", &["agent", "stop", &first_session, "--force"]);

    // A folder takes several writers (decision 0169): the exited session resumes beside another live writer, and lists it.
    let other = start(&sandbox, "fake", &[]);
    let resumed = sandbox.json_ok("default", &["agent", "resume", &first_session]);
    assert_eq!(resumed["execution"]["state"], "live", "{resumed}");
    assert_eq!(
        resumed["shared_folder_with"],
        json!([session_of(&other)]),
        "{resumed}"
    );
}

/// After a prompt is accepted, the next one waits for a report from the agent (decision 0058): the quiet agent holds the
/// first prompt without reporting, so the session stays idle, and the second prompt is refused until a forced one or until
/// the agent reports after the first.
#[test]
fn a_second_prompt_is_refused_until_the_agent_reports_after_the_first() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake-quiet", &[]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);
    until(&sandbox, &session, "idle");

    let first = sandbox.json_ok("default", &["agent", "prompt", &session, "hold quiet"]);
    assert_eq!(first["bytes"], 11, "the text and its carriage return");
    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, "second"]),
        "agent.busy"
    );
    let forced = sandbox.json_ok(
        "default",
        &["agent", "prompt", &session, "second", "--force"],
    );
    assert_eq!(forced["bytes"], 7);

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: hold quiet");
    let third = sandbox.json_ok("default", &["agent", "prompt", &session, "third"]);
    assert_eq!(third["bytes"], 6, "accepted again after the agent reported");
}

/// A held first prompt counts as typed once it is taken at the agent's first idle, so the next prompt waits for a report
/// (decision 0058).
#[test]
fn a_held_first_prompt_also_makes_the_next_prompt_wait_for_a_report() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake-quiet-none", &["--prompt", "hold once"]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);

    wait_for_line(&sandbox, &terminal, "fake-agent: holding");
    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, "again"]),
        "agent.busy"
    );

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: hold once");
}

/// Escape sequences, DEL and the C1 controls are refused at launch and in `agent prompt`, in both paste modes; a refused
/// prompt types nothing. Tabs and line breaks are accepted (decision 0058).
#[test]
fn control_characters_are_refused_in_both_paste_modes_and_line_breaks_are_not() {
    let sandbox = sandbox();
    let refused = ["\x1b[20\x1b[201~1~", "\x1b", "\u{9b}"];
    for prompt in refused {
        assert_eq!(
            error_code(
                &sandbox,
                &[
                    "agent",
                    "start",
                    "fake",
                    "--workspace",
                    "main",
                    "--prompt",
                    prompt
                ]
            ),
            "agent.prompt_invalid"
        );
        assert_eq!(
            error_code(
                &sandbox,
                &[
                    "agent",
                    "start",
                    "fake-paste",
                    "--workspace",
                    "main",
                    "--prompt",
                    prompt
                ]
            ),
            "agent.prompt_invalid"
        );
    }
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"], json!([]), "nothing was started");

    let plain = start(&sandbox, "fake", &[]);
    let plain_session = session_of(&plain);
    let bracketed = start(&sandbox, "fake-paste", &["--read-only"]);
    let bracketed_session = session_of(&bracketed);
    until(&sandbox, &plain_session, "idle");
    until(&sandbox, &bracketed_session, "idle");
    for prompt in refused {
        assert_eq!(
            error_code(&sandbox, &["agent", "prompt", &plain_session, prompt]),
            "agent.prompt_invalid"
        );
        assert_eq!(
            error_code(&sandbox, &["agent", "prompt", &bracketed_session, prompt]),
            "agent.prompt_invalid"
        );
    }
    let shown = sandbox.json_ok("default", &["agent", "status", &plain_session]);
    assert_eq!(
        activity(&shown),
        ("idle", None, "hook"),
        "nothing was typed"
    );

    let plain_receipt = sandbox.json_ok("default", &["agent", "prompt", &plain_session, "a\tb\nc"]);
    assert_eq!(plain_receipt["bytes"], 6, "five bytes and the submit key");
    let bracketed_receipt = sandbox.json_ok(
        "default",
        &["agent", "prompt", &bracketed_session, "a\tb\nc"],
    );
    assert_eq!(
        bracketed_receipt["bytes"], 18,
        "the two paste markers, five bytes and the submit key"
    );
}
