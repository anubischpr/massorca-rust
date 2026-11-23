//! Permission modes through the CLI (decision 0083). The fake agent is declared with two modes, `careful` (the default,
//! `--mode careful`) and `wild` (a bypass, `--mode wild`, live switch `/mode wild`); it prints the mode it was started
//! in and every live switch it reads, so each test reads the agent's screen to see which mode reached it.

mod harness;

use harness::{Sandbox, eventually, single_document, with_json};
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "prompt": "working", "stop": "done" },
        "result_field": "result"
    })
}

/// The two fake modes, plus `extra`.
fn modes(extra: &[Value]) -> Value {
    let mut modes = vec![
        json!({ "id": "careful", "label": "Careful", "args": ["--mode", "careful"] }),
        json!({
            "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true,
            "live_switch": { "type": "input", "text": "/mode wild" }
        }),
    ];
    modes.extend_from_slice(extra);
    Value::Array(modes)
}

/// Declares kind `id`, running the fake agent with `modes` (default `careful`), the prompt table and the status table.
fn declare(sandbox: &Sandbox, id: &str, modes: Value, prompt: Value, status: Value) {
    let kind = json!({
        "id": id,
        "name": format!("Fake agent {id}"),
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": prompt,
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": status,
        "permission_modes": modes,
        "default_permission_mode": "careful"
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with a folder workspace `main` and these kinds: `fake` (the two modes), `fake-live` (also `quiet`, a mode
/// with only a live switch; prompts wait for the first idle), and `fake-silent` (the same modes and no status mechanism,
/// so it never reports idle).
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    let quiet = json!({
        "id": "quiet", "label": "Quiet", "live_switch": { "type": "input", "text": "/mode quiet" }
    });
    declare(
        &sandbox,
        "fake",
        modes(&[]),
        json!({ "mode": "argv" }),
        status(),
    );
    declare(
        &sandbox,
        "fake-live",
        modes(std::slice::from_ref(&quiet)),
        json!({ "mode": "none" }),
        status(),
    );
    declare(
        &sandbox,
        "fake-silent",
        modes(&[quiet]),
        json!({ "mode": "argv" }),
        json!({ "mechanism": "none" }),
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

fn start(sandbox: &Sandbox, kind: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "start", kind, "--workspace", "main", "--read-only"];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

fn id_of(record: &Value) -> String {
    record["id"].as_str().expect("session id").to_owned()
}

fn terminal_of(record: &Value) -> String {
    record["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned()
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

/// Runs `--json <args>`, which must fail, and returns the error document.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

/// The agent's screen, trimmed lines.
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

/// Waits until the terminal's screen shows `line`; returns the screen then.
fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) -> Vec<String> {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let shown = screen(sandbox, terminal);
        shown.iter().any(|shown| shown == line).then_some(shown)
    })
}

fn stop(sandbox: &Sandbox, session: &str) {
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

#[test]
fn an_unknown_mode_is_refused_with_the_known_ids() {
    let sandbox = sandbox();
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--mode",
            "reckless",
        ],
    );
    assert_eq!(error["code"], "agent.mode_unknown", "{error}");
    assert_eq!(
        error["data"],
        json!({ "kind": "fake", "mode": "reckless", "known": ["careful", "wild"] })
    );
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"], json!([]), "nothing was started");
}

/// Whether the agent's screen shows a `mode <id>` start line, the sign that a mode flag reached it.
fn a_mode_flag_reached(shown: &[String]) -> bool {
    shown
        .iter()
        .any(|line| line.starts_with("fake-agent: mode ") && !line.contains("switched"))
}

#[test]
fn a_start_without_a_mode_passes_no_flag_and_records_the_vendor_default() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    assert!(
        started.get("permission_mode").is_none(),
        "no claim: {started}"
    );
    assert_eq!(started["mode_source"], "vendor_default");
    assert!(started.get("mode_confirmed").is_none(), "{started}");
    let session = id_of(&started);
    until(&sandbox, &session, "idle");
    let shown = wait_for_line(
        &sandbox,
        &terminal_of(&started),
        "fake-agent: credentials missing",
    );
    assert!(!a_mode_flag_reached(&shown), "no mode flag: {shown:?}");
    let text = harness::stdout_text(
        &sandbox
            .run("default", &["agent", "status", &session])
            .success(),
    );
    assert!(text.contains("mode: the CLI's default"), "{text}");
    stop(&sandbox, &session);
}

#[test]
fn a_named_mode_is_passed_to_the_agent_as_its_arguments() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--mode", "wild"]);
    assert_eq!(started["permission_mode"], "wild");
    assert_eq!(started["mode_source"], "chosen");
    assert_eq!(started["mode_confirmed"], true);
    wait_for_line(&sandbox, &terminal_of(&started), "fake-agent: mode wild");
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"][0]["permission_mode"], "wild");
    stop(&sandbox, &id_of(&started));
}

#[test]
fn caller_arguments_leave_a_named_mode_unconfirmed() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--mode", "careful", "--", "extra"]);
    assert_eq!(started["permission_mode"], "careful");
    assert!(
        started.get("mode_confirmed").is_none(),
        "the caller's arguments may override the mode: {started}"
    );
    wait_for_line(&sandbox, &terminal_of(&started), "fake-agent: mode careful");
    stop(&sandbox, &id_of(&started));
}

#[test]
fn a_bypass_mode_is_never_inherited_by_a_delegated_child() {
    let sandbox = sandbox();
    let parent = start(&sandbox, "fake", &["--mode", "wild"]);
    let parent_id = id_of(&parent);
    until(&sandbox, &parent_id, "idle");

    let child = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &parent_id,
            "--prompt",
            "look around",
            "--read-only",
        ],
    );
    assert!(child.get("permission_mode").is_none(), "{child}");
    assert_eq!(child["mode_source"], "vendor_default");
    let shown = wait_for_line(
        &sandbox,
        &terminal_of(&child),
        "fake-agent: credentials missing",
    );
    assert!(
        !a_mode_flag_reached(&shown),
        "the parent's wild never reached it: {shown:?}"
    );

    let named = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &parent_id,
            "--prompt",
            "go wild",
            "--read-only",
            "--mode",
            "wild",
        ],
    );
    assert_eq!(named["permission_mode"], "wild", "a child may name it");
    wait_for_line(&sandbox, &terminal_of(&named), "fake-agent: mode wild");

    let error = error_of(
        &sandbox,
        &[
            "agent",
            "delegate",
            "--from",
            &parent_id,
            "--prompt",
            "nope",
            "--read-only",
            "--mode",
            "reckless",
        ],
    );
    assert_eq!(error["code"], "agent.mode_unknown", "{error}");
    for session in [id_of(&named), id_of(&child), parent_id] {
        stop(&sandbox, &session);
    }
}

#[test]
fn a_mode_with_only_a_live_switch_is_typed_at_the_first_idle_before_the_prompt() {
    let sandbox = sandbox();
    let started = start(
        &sandbox,
        "fake-live",
        &["--mode", "quiet", "--prompt", "hello"],
    );
    assert_eq!(started["permission_mode"], "quiet");
    assert_eq!(started["mode_source"], "chosen");
    assert!(
        started.get("mode_confirmed").is_none(),
        "a typed switch is only requested: {started}"
    );
    let terminal = terminal_of(&started);
    let shown = wait_for_line(&sandbox, &terminal, "fake-agent: done hello");
    let switched = shown
        .iter()
        .position(|line| line == "fake-agent: mode switched quiet")
        .unwrap_or_else(|| panic!("no switch on the screen: {shown:?}"));
    let prompted = shown
        .iter()
        .position(|line| line == "fake-agent: working on hello")
        .expect("the prompt ran");
    assert!(switched < prompted, "the switch goes first: {shown:?}");
    let text = harness::stdout_text(
        &sandbox
            .run("default", &["agent", "status", &id_of(&started)])
            .success(),
    );
    assert!(text.contains("mode: quiet (requested)"), "{text}");
    stop(&sandbox, &id_of(&started));
}

#[test]
fn a_switch_only_mode_is_refused_at_start_for_a_kind_that_never_reports_idle() {
    let sandbox = sandbox();
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "start",
            "fake-silent",
            "--workspace",
            "main",
            "--mode",
            "quiet",
        ],
    );
    assert_eq!(error["code"], "agent.mode_unavailable", "{error}");
    let started = start(&sandbox, "fake-silent", &[]);
    assert!(started.get("permission_mode").is_none(), "{started}");
    assert_eq!(started["mode_source"], "vendor_default");
    let session = id_of(&started);
    stopped(&sandbox, &session);

    let stored = error_of(&sandbox, &["agent", "mode", &session, "quiet"]);
    assert_eq!(
        stored["code"], "agent.mode_unavailable",
        "a resume could never apply it either: {stored}"
    );
}

/// `agent mode <session> <mode>`, which must succeed; returns its result.
fn set_mode(sandbox: &Sandbox, session: &str, mode: &str) -> Value {
    sandbox.json_ok("default", &["agent", "mode", session, mode])
}

/// Stops the session's run and waits until it has exited.
fn stopped(sandbox: &Sandbox, session: &str) {
    stop(sandbox, session);
    until(sandbox, session, "exited");
}

fn resume(sandbox: &Sandbox, session: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "resume", session];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

#[test]
fn a_live_switch_is_typed_into_the_running_agent_and_reported_as_requested() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let set = set_mode(&sandbox, &session, "wild");
    assert_eq!(set["applied"], "live", "{set}");
    assert_eq!(set["session"]["permission_mode"], "wild");
    assert!(
        set["session"].get("mode_confirmed").is_none(),
        "no hook confirms a typed switch: {set}"
    );
    wait_for_line(
        &sandbox,
        &terminal_of(&started),
        "fake-agent: mode switched wild",
    );
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["permission_mode"], "wild", "the row was updated");

    sandbox.json_ok(
        "default",
        &["agent", "prompt", &session, "after the switch"],
    );
    wait_for_line(
        &sandbox,
        &terminal_of(&started),
        "fake-agent: done after the switch",
    );
    let text = harness::stdout_text(
        &sandbox
            .run("default", &["agent", "mode", &session, "wild"])
            .success(),
    );
    assert!(text.contains("switched to wild"), "{text}");
    stop(&sandbox, &session);
}

#[test]
fn a_mode_without_a_live_switch_waits_for_the_next_resume_without_touching_the_run() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--mode", "wild"]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let set = set_mode(&sandbox, &session, "careful");
    assert_eq!(set["applied"], "next_resume", "{set}");
    assert_eq!(
        set["session"]["permission_mode"], "wild",
        "the run is still wild"
    );
    assert_eq!(set["session"]["mode_confirmed"], true);
    assert_eq!(set["session"]["pending_permission_mode"], "careful");
    let screen_now = screen(&sandbox, &terminal_of(&started));
    assert!(
        !screen_now.iter().any(|line| line.contains("mode switched")),
        "nothing was typed: {screen_now:?}"
    );

    stopped(&sandbox, &session);
    let resumed = resume(&sandbox, &session, &[]);
    assert_eq!(resumed["permission_mode"], "careful");
    assert_eq!(resumed["mode_confirmed"], true);
    assert!(
        resumed.get("pending_permission_mode").is_none(),
        "{resumed}"
    );
    wait_for_line(&sandbox, &terminal_of(&resumed), "fake-agent: mode careful");
    stop(&sandbox, &session);
}

#[test]
fn a_bypass_mode_is_used_on_resume_only_when_the_resume_names_it() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--mode", "wild"]);
    let session = id_of(&started);
    stopped(&sandbox, &session);

    let plain = resume(&sandbox, &session, &[]);
    assert!(plain.get("permission_mode").is_none(), "{plain}");
    assert_eq!(plain["mode_source"], "vendor_default");
    let shown = wait_for_line(
        &sandbox,
        &terminal_of(&plain),
        "fake-agent: credentials missing",
    );
    assert!(!a_mode_flag_reached(&shown), "{shown:?}");
    stopped(&sandbox, &session);

    let set = set_mode(&sandbox, &session, "wild");
    assert_eq!(set["applied"], "next_resume", "{set}");
    assert_eq!(set["session"]["pending_permission_mode"], "wild");
    let unnamed = resume(&sandbox, &session, &[]);
    assert!(
        unnamed.get("permission_mode").is_none(),
        "a pending bypass is not used unless named: {unnamed}"
    );
    assert_eq!(
        unnamed["pending_permission_mode"], "wild",
        "it stays pending"
    );
    stopped(&sandbox, &session);

    let named = resume(&sandbox, &session, &["--mode", "wild"]);
    assert_eq!(named["permission_mode"], "wild");
    assert!(named.get("pending_permission_mode").is_none(), "{named}");
    wait_for_line(&sandbox, &terminal_of(&named), "fake-agent: mode wild");
    stopped(&sandbox, &session);

    let error = error_of(
        &sandbox,
        &["agent", "resume", &session, "--mode", "reckless"],
    );
    assert_eq!(error["code"], "agent.mode_unknown", "{error}");
}

#[test]
fn a_mode_for_an_exited_session_that_cannot_resume_is_refused() {
    let sandbox = sandbox();
    let kind = json!({
        "id": "fake-once",
        "name": "Fake agent once",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": status(),
        "permission_modes": modes(&[]),
        "default_permission_mode": "careful"
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::write(dir.join("fake-once.json"), kind.to_string()).expect("write a kind file");
    let started = start(&sandbox, "fake-once", &[]);
    let session = id_of(&started);
    stopped(&sandbox, &session);

    let error = error_of(&sandbox, &["agent", "mode", &session, "wild"]);
    assert_eq!(error["code"], "agent.resume_unavailable", "{error}");
    let unknown = error_of(&sandbox, &["agent", "mode", &session, "reckless"]);
    assert_eq!(unknown["code"], "agent.mode_unknown", "{unknown}");
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert!(
        shown.get("permission_mode").is_none(),
        "nothing changed: {shown}"
    );
    assert!(shown.get("pending_permission_mode").is_none(), "{shown}");
}

#[test]
fn a_live_switch_into_an_agent_that_reports_nothing_needs_force() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake-silent", &[]);
    let session = id_of(&started);
    let terminal = terminal_of(&started);
    wait_for_line(&sandbox, &terminal, "fake-agent: credentials missing");

    let error = error_of(&sandbox, &["agent", "mode", &session, "wild"]);
    assert_eq!(error["code"], "agent.mode_unavailable", "{error}");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|text| text.contains("--force")),
        "the refusal names the way through and the risk: {error}"
    );
    assert!(
        !screen(&sandbox, &terminal)
            .iter()
            .any(|line| line.contains("mode switched")),
        "nothing was typed"
    );

    let set = sandbox.json_ok("default", &["agent", "mode", &session, "wild", "--force"]);
    assert_eq!(set["applied"], "live", "{set}");
    wait_for_line(&sandbox, &terminal, "fake-agent: mode switched wild");
    stop(&sandbox, &session);
}
