//! Agent status through the CLI (decisions 0052, 0053): the fake agent reports through `massorca agent hook`, and
//! the tests wait on reported states with `agent status --until`, never on fixed sleeps.

mod harness;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::Stdio;
use std::time::{Duration, Instant};

use harness::{Sandbox, capture, eventually, single_document, stdout_text, with_json};
use predicates::str::contains;
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

/// Declares kind `fake` with the full event table, or with `status` replaced by `status_override`.
fn declare_fake(sandbox: &Sandbox, status_override: Option<Value>) {
    let status = status_override.unwrap_or_else(|| {
        json!({
            "mechanism": "env-command",
            "events": {
                "start": "idle", "prompt": "working", "ask": "waiting",
                "stop": "done", "fail": "done:failure"
            },
            "result_field": "result"
        })
    });
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "status": status
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write the kind file");
}

/// A sandbox with kind `fake` and a folder workspace `main`.
fn sandbox_with_workspace(status_override: Option<Value>) -> Sandbox {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox, status_override);
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

/// Starts the fake agent with `prompt`; returns the session id and its terminal id.
fn start(sandbox: &Sandbox, prompt: &str, extra: &[&str]) -> (String, String) {
    let mut args = vec![
        "agent",
        "start",
        "fake",
        "--workspace",
        "main",
        "--prompt",
        prompt,
    ];
    args.extend_from_slice(extra);
    let started = sandbox.json_ok("default", &args);
    (
        started["id"].as_str().expect("session id").to_owned(),
        started["terminal_id"]
            .as_str()
            .expect("terminal id")
            .to_owned(),
    )
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

#[test]
fn a_prompted_agent_reports_working_then_done_through_its_hooks() {
    let sandbox = sandbox_with_workspace(None);
    let (session, terminal) = start(&sandbox, "hold this", &[]);

    let working = until(&sandbox, &session, "working");
    assert_eq!(activity(&working), ("working", None, "hook"));
    assert_eq!(working["execution"]["state"], "live");
    assert!(working["activity"]["at_ms"].as_i64().unwrap_or(0) > 0);

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(activity(&done), ("done", Some("success"), "hook"));
    assert_eq!(done["result"], "done: hold this");

    sandbox
        .run("default", &["agent", "status", &session])
        .success()
        .stdout(contains("execution: live"))
        .stdout(contains("activity: done (success) from hook"));
    let listed = stdout_text(&sandbox.run("default", &["agent", "list"]).success());
    assert!(
        listed.contains("done (success) from hook"),
        "list shows activity and its source: {listed}"
    );
}

#[test]
fn a_prompt_that_asks_leaves_the_agent_waiting() {
    let sandbox = sandbox_with_workspace(None);
    let (session, _terminal) = start(&sandbox, "ask: may I", &[]);

    let waiting = until(&sandbox, &session, "waiting");
    assert_eq!(activity(&waiting), ("waiting", None, "hook"));
    sandbox
        .run("default", &["agent", "status", &session])
        .success()
        .stdout(contains("activity: waiting from hook"));
}

#[test]
fn a_run_that_ends_while_working_is_done_and_interrupted_by_the_process() {
    let sandbox = sandbox_with_workspace(None);

    // Stopped through the agent: the stop itself applies the exit.
    let (stopped, _terminal) = start(&sandbox, "hold one", &[]);
    until(&sandbox, &stopped, "working");
    let after_stop = sandbox.json_ok("default", &["agent", "stop", &stopped, "--force"]);
    assert_eq!(after_stop["execution"]["state"], "exited");
    assert_eq!(
        activity(&after_stop),
        ("done", Some("interrupted"), "process")
    );
    sandbox
        .run("default", &["agent", "status", &stopped])
        .success()
        .stdout(contains("execution: exited (killed"))
        .stdout(contains("activity: done (interrupted) from process"));

    // Ended behind the agent's back: the sweep sees the terminal's exit.
    let (swept, terminal) = start(&sandbox, "hold two", &[]);
    until(&sandbox, &swept, "working");
    sandbox
        .run("default", &["terminal", "kill", &terminal, "--force"])
        .success();
    let exited = until(&sandbox, &swept, "exited");
    assert_eq!(exited["execution"]["state"], "exited");
    assert_eq!(activity(&exited), ("done", Some("interrupted"), "process"));
}

#[test]
fn a_report_with_a_wrong_token_is_refused_and_the_hook_still_exits_zero() {
    let sandbox = sandbox_with_workspace(None);
    let (session, _terminal) = start(&sandbox, "hold on", &[]);
    until(&sandbox, &session, "working");

    let mut hook = sandbox.command("default");
    hook.args(["agent", "hook", "stop", r#"{"result":"forged"}"#])
        .env("MASSORCA_AGENT_SESSION", &session)
        .env("MASSORCA_AGENT_TOKEN", "not-the-token");
    let outcome = harness::execute(&mut hook).success();
    let output = outcome.get_output();
    assert!(output.stdout.is_empty(), "a hook prints nothing on stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("agent.hook_refused"), "{stderr}");

    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(
        activity(&shown),
        ("working", None, "hook"),
        "a refused report changes nothing"
    );
    assert!(shown.get("result").is_none(), "{shown}");
}

#[test]
fn a_hook_outside_an_agent_run_exits_zero_promptly() {
    let sandbox = Sandbox::new();
    let mut hook = sandbox.command("default");
    hook.args(["agent", "hook", "start"])
        .env_remove("MASSORCA_AGENT_SESSION")
        .env_remove("MASSORCA_AGENT_TOKEN");
    let began = Instant::now();
    let output = capture(&mut hook, Duration::from_secs(30)).expect("the hook finishes in time");
    assert!(output.status.success(), "{output:?}");
    assert!(
        began.elapsed() < Duration::from_secs(30),
        "within the bound"
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("MASSORCA_AGENT_SESSION"), "{stderr}");
    let status = sandbox.json_ok("default", &["service", "status"]);
    assert_eq!(
        status["running"], false,
        "a hook never starts the service: {status}"
    );
}

#[test]
fn waiting_for_a_state_that_never_comes_times_out_with_an_error() {
    let sandbox = sandbox_with_workspace(None);
    let (session, _terminal) = start(&sandbox, "hold forever", &[]);
    until(&sandbox, &session, "working");

    let outcome = sandbox
        .run(
            "default",
            &with_json(&[
                "agent",
                "status",
                &session,
                "--until",
                "idle",
                "--timeout",
                "1",
            ]),
        )
        .failure();
    let document = single_document(&outcome);
    assert_eq!(
        document["error"]["code"], "agent.status_timeout",
        "{document}"
    );
    sandbox
        .run(
            "default",
            &[
                "agent",
                "status",
                &session,
                "--until",
                "idle",
                "--timeout",
                "1",
            ],
        )
        .failure()
        .stderr(contains("did not reach idle within 1 s"));
}

/// Reads what a child has written to `file` so far.
fn written(file: &mut File) -> String {
    file.seek(SeekFrom::Start(0)).expect("rewind");
    let mut text = String::new();
    file.read_to_string(&mut text).expect("read");
    text
}

#[test]
fn a_watch_sends_a_snapshot_first_and_then_each_change() {
    let sandbox = sandbox_with_workspace(None);
    let (session, terminal) = start(&sandbox, "hold watched", &[]);
    until(&sandbox, &session, "working");

    let mut out = tempfile::tempfile().expect("a temporary file");
    let mut watcher = sandbox.command("default");
    watcher
        .args(["--json", "agent", "status", &session, "--watch"])
        .stdin(Stdio::null())
        .stdout(out.try_clone().expect("duplicate"))
        .stderr(Stdio::null());
    let mut child = harness::ChildGuard::new(watcher.spawn().expect("the watcher starts"));

    let first = eventually("the snapshot", || {
        let text = written(&mut out);
        text.lines().next().map(str::to_owned)
    });
    let first: Value = serde_json::from_str(&first).expect("a JSON line");
    assert_eq!(first["result"]["kind"], "snapshot", "{first}");
    assert_eq!(first["result"]["sessions"][0]["id"], session.as_str());
    assert_eq!(
        first["result"]["sessions"][0]["activity"]["state"],
        "working"
    );

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let changed = eventually("a change to done", || {
        written(&mut out)
            .lines()
            .skip(1)
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|item| item["result"]["session"]["activity"]["state"] == "done")
    });
    assert_eq!(changed["result"]["kind"], "changed");
    assert_eq!(changed["result"]["session"]["id"], session.as_str());
    assert!(child.is_running(), "a watch runs until interrupted");
}

#[test]
fn a_session_with_no_reports_shows_no_status() {
    let sandbox = sandbox_with_workspace(Some(json!({ "mechanism": "none" })));
    let (session, terminal) = start(&sandbox, "hello", &[]);
    eventually("the agent's answer", || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", &terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|line| line.as_str().map(str::trim) == Some("fake-agent: done hello"))
            })
            .then_some(())
    });

    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert!(shown.get("activity").is_none(), "{shown}");
    sandbox
        .run("default", &["agent", "status", &session])
        .success()
        .stdout(contains("execution: live"))
        .stdout(contains("activity: no status"));
    let listed = stdout_text(&sandbox.run("default", &["agent", "list"]).success());
    assert!(listed.contains("no status"), "{listed}");
    assert!(listed.contains("live"), "{listed}");
}

#[test]
fn a_done_report_with_a_result_survives_the_run_exiting() {
    let sandbox = sandbox_with_workspace(None);
    let (session, terminal) = start(&sandbox, "finish up", &[]);
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: finish up");

    sandbox
        .run("default", &["terminal", "send", &terminal, "exit"])
        .success();
    let exited = until(&sandbox, &session, "exited");
    assert_eq!(activity(&exited), ("done", Some("success"), "hook"));
    assert_eq!(exited["result"], "done: finish up");
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"][0]["id"], session.as_str());
    assert_eq!(
        activity(&listed["sessions"][0]),
        ("done", Some("success"), "hook")
    );
    assert_eq!(listed["sessions"][0]["result"], "done: finish up");
}

#[test]
fn the_first_report_of_a_run_is_never_refused() {
    let sandbox = sandbox_with_workspace(None);
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let session = started["id"].as_str().expect("session id");
    let idle = until(&sandbox, session, "idle");
    assert_eq!(activity(&idle), ("idle", None, "hook"));
}

#[test]
fn a_hook_with_a_command_line_it_does_not_expect_still_exits_zero() {
    let sandbox = Sandbox::new();
    for args in [
        &["agent", "hook"][..],
        &["agent", "hook", "start", "{}", "extra", "--unknown-flag"][..],
        &["agent", "hook", "--odd-event"][..],
    ] {
        let mut hook = sandbox.command("default");
        hook.args(args)
            .env_remove("MASSORCA_AGENT_SESSION")
            .env_remove("MASSORCA_AGENT_TOKEN");
        let output =
            capture(&mut hook, Duration::from_secs(30)).expect("the hook finishes in time");
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            !output.stderr.is_empty(),
            "{args:?}: the problem is on stderr"
        );
    }
}
