//! Agent sessions through the CLI, with the fake agent declared as kind `fake` in the test profile (ADR 0057).
//!
//! Every CLI call is its own process, so a session that is still listed as live has outlived the call that started it.

mod harness;

use harness::{Sandbox, eventually, single_document, stdout_text, with_json};
use std::path::{Path, PathBuf};

use predicates::str::contains;
use serde_json::{Value, json};

/// Declares kind `fake`: the fake agent binary, reporting through `massorca agent hook` (`env-command`).
fn declare_fake(sandbox: &Sandbox) {
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": {
            "mechanism": "env-command",
            "events": { "start": "idle", "prompt": "working", "stop": "done", "fail": "done:failure" },
            "result_field": "result"
        },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
    });
    write_kind(sandbox, "fake.json", &kind);
}

fn write_kind(sandbox: &Sandbox, file: &str, kind: &Value) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(file), kind.to_string()).expect("write a kind file");
}

/// Registers `<home>/notes` as a folder project and creates its workspace `main`; returns the workspace id.
fn folder_workspace(sandbox: &Sandbox) -> String {
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    let created = sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    created["id"].as_str().expect("workspace id").to_owned()
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

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {value}"))
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
fn a_started_agent_runs_in_its_workspace_and_stops_and_is_forgotten() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let workspace_id = folder_workspace(&sandbox);

    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--prompt",
            "hello agent",
            "--title",
            "first",
        ],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();
    assert_eq!(started["kind"], "fake");
    assert_eq!(started["workspace_id"], workspace_id);
    assert_eq!(started["title"], "first");
    assert_eq!(started["read_only"], false);
    assert!(
        started["agent_ref"].is_string(),
        "the ref is assigned at launch: {started}"
    );
    assert_eq!(started["execution"]["state"], "live");

    wait_for_line(&sandbox, &terminal, "fake-agent: done hello agent");

    // The start command has exited; the session is still live in later, separate calls.
    let listed = sandbox.json_ok("default", &["agent", "list", "--workspace", "main"]);
    assert_eq!(listed["sessions"][0]["id"], session.as_str());
    assert_eq!(listed["sessions"][0]["execution"]["state"], "live");
    let shown = sandbox.json_ok("default", &["agent", "status", &session[..18]]);
    assert_eq!(
        shown["id"],
        session.as_str(),
        "a long enough prefix selects the session"
    );
    assert_eq!(shown["execution"]["state"], "live");
    sandbox
        .run("default", &["agent", "status", &session])
        .success()
        .stdout(contains(format!("id: {session}")))
        .stdout(contains("execution: live"))
        .stdout(contains("activity: done (success) from hook"));

    // A folder takes a second writing session beside the first (decision 0169); it lists the first.
    let second = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let second_id = text(&second, "id").to_owned();
    assert_eq!(second["shared_folder_with"], json!([session]));
    sandbox.json_ok("default", &["agent", "stop", &second_id, "--force"]);
    sandbox.json_ok("default", &["agent", "forget", &second_id]);
    let reader = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    assert_eq!(reader["read_only"], true);
    assert_eq!(reader["execution"]["state"], "live");

    assert_eq!(
        error_code(&sandbox, &["agent", "forget", &session]),
        "agent.busy"
    );
    let stopped = sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    assert_eq!(stopped["execution"]["state"], "exited");
    assert_eq!(stopped["execution"]["exit"]["reason"], "killed");
    let again = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(again["execution"]["exit"]["reason"], "killed");

    let forgotten = sandbox.json_ok("default", &["agent", "forget", &session]);
    assert_eq!(forgotten["id"], session.as_str());
    assert_eq!(
        error_code(&sandbox, &["agent", "status", &session]),
        "agent.not_found"
    );
    let remaining = sandbox.json_ok("default", &["agent", "list"]);
    let ids: Vec<&str> = remaining["sessions"]
        .as_array()
        .expect("sessions")
        .iter()
        .filter_map(|listed| listed["id"].as_str())
        .collect();
    assert_eq!(ids, [text(&reader, "id")]);

    // With the writer gone, the workspace takes a new writing session.
    let next = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    assert_eq!(next["execution"]["state"], "live");
}

#[test]
fn kinds_that_cannot_start_are_refused() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);
    let missing = sandbox.home().join("no-such-agent-program");
    write_kind(
        &sandbox,
        "ghost.json",
        &json!({ "id": "ghost", "name": "Ghost", "program": [missing.to_string_lossy()] }),
    );

    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "start", "ghost", "--workspace", "main"]
        ),
        "agent.not_installed"
    );
    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "start", "nobody", "--workspace", "main"]
        ),
        "agent.kind_not_found"
    );
    let empty = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(empty["sessions"], json!([]));
    let listing = stdout_text(&sandbox.run("default", &["agent", "list"]).success());
    assert!(listing.contains("no agent sessions"), "{listing}");
}

#[test]
fn a_service_restart_leaves_the_session_exited_by_the_service_stopping() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let session = text(&started, "id").to_owned();
    wait_for_line(&sandbox, text(&started, "terminal_id"), "fake-agent: ready");

    sandbox.run("default", &["service", "restart"]).success();

    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["execution"]["state"], "exited");
    assert_eq!(shown["execution"]["exit"]["reason"], "service_stopped");
    // Either service may record the end: the stopping one with the program's exit code when it saw the program end
    // with one, the next one with no code. Both read "service stopped".
    let listed = sandbox.run("default", &["agent", "list"]).success();
    let line = stdout_text(&listed)
        .lines()
        .find(|line| line.contains(&session))
        .map(str::to_owned)
        .expect("the session is listed");
    let code = shown["execution"]["exit"]["code"].as_i64();
    let expected = match code {
        Some(code) => format!("exited (service stopped, code {code})"),
        None => "exited (service stopped)".to_owned(),
    };
    assert!(line.contains(&expected), "{line}");
}

#[test]
fn a_plain_stop_reads_as_a_signal_with_no_exit_code_on_every_platform() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let session = text(&started, "id").to_owned();
    wait_for_line(&sandbox, text(&started, "terminal_id"), "fake-agent: ready");

    sandbox.json_ok("default", &["agent", "stop", &session]);
    let exit = eventually("the stopped run to exit", || {
        let shown = sandbox.json_ok("default", &["agent", "status", &session]);
        (shown["execution"]["state"] == "exited").then(|| shown["execution"]["exit"].clone())
    });
    assert_eq!(exit["reason"], "killed", "{exit}");
    // Unix records the hangup's signal; Windows the console event's, rather than the code 3221225786.
    assert!(exit["code"].is_null(), "a stop has no exit code: {exit}");
    assert!(
        exit["signal"].is_string(),
        "a stop names its signal: {exit}"
    );
}

/// Starts a fake agent in `main`, waits for its answer, and stops its run; returns the session id and its terminal.
fn started_and_stopped(sandbox: &Sandbox) -> (String, String) {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--prompt",
            "hello agent",
        ],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();
    wait_for_line(sandbox, &terminal, "fake-agent: done hello agent");
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    (session, terminal)
}

#[test]
fn a_released_run_frees_its_terminal_and_keeps_its_output_readable() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--prompt",
            "hello agent",
        ],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();
    wait_for_line(&sandbox, &terminal, "fake-agent: done hello agent");

    assert_eq!(
        error_code(&sandbox, &["agent", "release", &session]),
        "agent.busy",
        "a live run is refused"
    );
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);

    let released = sandbox.json_ok("default", &["agent", "release", &session]);
    assert_eq!(released["id"], session.as_str());
    assert_eq!(released["released"], true);
    assert_eq!(released["execution"]["state"], "exited");
    assert_eq!(
        error_code(&sandbox, &["terminal", "screen", &terminal]),
        "terminal.not_found",
        "the terminal is freed"
    );
    let again = sandbox.json_ok("default", &["agent", "release", &session]);
    assert_eq!(again["released"], true, "a second release changes nothing");

    let archive = sandbox.json_ok("default", &["agent", "archive", &session]);
    assert_eq!(archive["session"], session.as_str());
    assert_eq!(archive["released"], true);
    assert_eq!(archive["truncated"], false);
    assert!(
        text(&archive, "text").contains("fake-agent: done hello agent"),
        "{archive}"
    );
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"][0]["released"], true);
}

#[test]
fn an_archive_of_a_session_not_released_says_so() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let (session, _) = started_and_stopped(&sandbox);

    let archive = sandbox.json_ok("default", &["agent", "archive", &session]);
    assert_eq!(archive["released"], false);
    assert!(archive.get("text").is_none_or(Value::is_null), "{archive}");
}

#[test]
fn a_retained_session_is_listed_as_retained_survives_a_restart_and_clears_with_off() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let (session, _) = started_and_stopped(&sandbox);

    let retained = sandbox.json_ok("default", &["agent", "retain", &session]);
    assert_eq!(retained["retained"], true);
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"][0]["retained"], true);
    sandbox
        .run("default", &["agent", "list"])
        .success()
        .stdout(contains("retained"));

    sandbox.run("default", &["service", "restart"]).success();
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["retained"], true, "retain is durable");

    let off = sandbox.json_ok("default", &["agent", "retain", &session, "--off"]);
    assert_ne!(off["retained"], true, "{off}");
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_ne!(listed["sessions"][0]["retained"], true, "{listed}");
}

#[test]
fn an_explicit_release_of_a_retained_session_clears_the_retain_and_releases_it() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let (session, terminal) = started_and_stopped(&sandbox);
    sandbox.json_ok("default", &["agent", "retain", &session]);

    let released = sandbox.json_ok("default", &["agent", "release", &session]);
    assert_ne!(released["retained"], true, "{released}");
    assert_eq!(released["released"], true);
    assert_eq!(
        error_code(&sandbox, &["terminal", "screen", &terminal]),
        "terminal.not_found"
    );
}

/// Declares kind `tuned`: the fake agent, declaring the flags a model and an effort pass to (decision 0094).
fn declare_tuned(sandbox: &Sandbox) {
    let kind = json!({
        "id": "tuned",
        "name": "Tuned agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": {
            "mechanism": "env-command",
            "events": { "start": "idle", "prompt": "working", "stop": "done", "fail": "done:failure" },
            "result_field": "result"
        },
        "model": { "flag": "--model" },
        "effort": { "flag": "--effort" }
    });
    write_kind(sandbox, "tuned.json", &kind);
}

/// Sets `conversations.folder` through `config set`, which refuses a relative path.
fn set_conversations_folder(sandbox: &Sandbox, folder: &Path) {
    let text = folder.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["config", "set", "conversations.folder", text.as_str()],
    );
}

/// Starts a session with no workspace and no prompt; returns its id, its terminal and its workspace id.
fn start_without_project(sandbox: &Sandbox, title: &str) -> (String, String, String) {
    let started = sandbox.json_ok("default", &["agent", "start", "fake", "--title", title]);
    (
        text(&started, "id").to_owned(),
        text(&started, "terminal_id").to_owned(),
        text(&started, "workspace_id").to_owned(),
    )
}

/// The canonical folder a workspace is in, so tests compare paths as the operating system stores them.
fn canonical_folder_of(sandbox: &Sandbox, workspace_id: &str) -> PathBuf {
    let shown = sandbox.json_ok("default", &["workspace", "show", workspace_id]);
    std::fs::canonicalize(text(&shown, "path")).expect("the workspace folder exists")
}

#[test]
fn a_session_without_a_project_runs_in_its_own_folder_of_the_conversations_folder() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let conversations = sandbox.home().join("Conversations");
    set_conversations_folder(&sandbox, &conversations);
    assert!(!conversations.exists(), "the folder is made on first use");

    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--prompt",
            "hello loose",
            "--title",
            "loose",
        ],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();
    let workspace_id = text(&started, "workspace_id").to_owned();
    assert_eq!(started["title"], "loose");
    assert_eq!(started["execution"]["state"], "live");
    wait_for_line(&sandbox, &terminal, "fake-agent: done hello loose");

    // A folder with no branch: no worktree, and the conversations folder holds the session's own subfolder.
    let shown = sandbox.json_ok("default", &["workspace", "show", &workspace_id]);
    assert_eq!(shown["kind"], "folder", "{shown}");
    assert!(shown["branch"].is_null(), "no branch: {shown}");
    let folder = canonical_folder_of(&sandbox, &workspace_id);
    let root = std::fs::canonicalize(&conversations).expect("the conversations folder was made");
    assert_eq!(folder.parent(), Some(root.as_path()));

    // The session is not a project: the project list stays empty.
    sandbox
        .run("default", &["project", "list"])
        .success()
        .stdout(contains("no projects"));

    // Listed without a workspace filter, stopped, and resumed in the same folder.
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"][0]["id"], session.as_str());
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    // Resumed at the size of the view that will show it.
    let resumed = sandbox.json_ok(
        "default",
        &["agent", "resume", &session, "--cols", "150", "--rows", "60"],
    );
    assert_eq!(resumed["id"], session.as_str());
    assert_eq!(resumed["workspace_id"], workspace_id.as_str());
    assert_eq!(resumed["execution"]["state"], "live");
    let shown = sandbox.json_ok(
        "default",
        &["terminal", "show", text(&resumed, "terminal_id")],
    );
    assert_eq!(shown["cols"], 150, "{shown}");
    assert_eq!(shown["rows"], 60, "{shown}");
    assert_eq!(canonical_folder_of(&sandbox, &workspace_id), folder);
}

#[test]
fn each_session_without_a_project_gets_a_folder_of_its_own() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    set_conversations_folder(&sandbox, &sandbox.home().join("Conversations"));

    let (_, _, first) = start_without_project(&sandbox, "first");
    let (_, _, second) = start_without_project(&sandbox, "second");
    assert_ne!(first, second);
    assert_ne!(
        canonical_folder_of(&sandbox, &first),
        canonical_folder_of(&sandbox, &second)
    );
}

#[test]
fn the_conversations_folder_setting_moves_new_sessions_and_a_relative_path_is_refused() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let first_root = sandbox.home().join("First");
    let second_root = sandbox.home().join("Second");

    set_conversations_folder(&sandbox, &first_root);
    let (_, _, first) = start_without_project(&sandbox, "first");
    set_conversations_folder(&sandbox, &second_root);
    let (_, _, second) = start_without_project(&sandbox, "second");

    let first_root = std::fs::canonicalize(&first_root).expect("the first folder was made");
    let second_root = std::fs::canonicalize(&second_root).expect("the second folder was made");
    assert_eq!(
        canonical_folder_of(&sandbox, &first).parent(),
        Some(first_root.as_path())
    );
    assert_eq!(
        canonical_folder_of(&sandbox, &second).parent(),
        Some(second_root.as_path())
    );

    sandbox
        .run(
            "default",
            &["config", "set", "conversations.folder", "relative/path"],
        )
        .code(1)
        .stderr(contains("config.invalid_value"));
    let got = sandbox.json_ok("default", &["config", "get", "conversations.folder"]);
    assert_eq!(
        got["is_default"], false,
        "the refused value changed nothing"
    );
}

#[test]
fn the_hidden_project_of_the_conversations_folder_cannot_be_named_or_added() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let conversations = sandbox.home().join("Conversations");
    set_conversations_folder(&sandbox, &conversations);
    let (_, _, workspace_id) = start_without_project(&sandbox, "loose");
    let shown = sandbox.json_ok("default", &["workspace", "show", &workspace_id]);
    let hidden = text(&shown, "project_id").to_owned();

    // No command acts on the hidden project, by id or by path.
    let root = conversations.to_string_lossy().into_owned();
    for selector in [hidden.as_str(), root.as_str()] {
        assert_eq!(
            error_code(&sandbox, &["project", "show", selector]),
            "project.not_found"
        );
        assert_eq!(
            error_code(
                &sandbox,
                &["project", "rm", selector, "--forget-workspaces"]
            ),
            "project.not_found"
        );
        assert_eq!(
            error_code(&sandbox, &["workspace", "create", selector, "shared"]),
            "project.not_found"
        );
    }
    let inspected = sandbox.json_ok("default", &["project", "inspect", root.as_str()]);
    assert!(inspected["registered"].is_null(), "{inspected}");

    // Adding the conversations folder as a project is refused by name, and the session is still there.
    assert_eq!(
        error_code(&sandbox, &["project", "add", root.as_str(), "--folder"]),
        "project.exists"
    );
    sandbox.json_ok("default", &["workspace", "show", &workspace_id]);
}

#[test]
fn a_project_at_the_conversations_folder_refuses_a_start_without_a_project() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let conversations = sandbox.home().join("Conversations");
    std::fs::create_dir_all(&conversations).expect("create the folder");
    let root = conversations.to_string_lossy().into_owned();
    sandbox.json_ok("default", &["project", "add", root.as_str(), "--folder"]);
    set_conversations_folder(&sandbox, &conversations);

    let outcome = sandbox
        .run("default", &["--json", "agent", "start", "fake"])
        .failure();
    let document = single_document(&outcome);
    assert_eq!(
        document["error"]["code"], "config.invalid_value",
        "{document}"
    );
    let message = document["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("Conversations"),
        "names the project: {message}"
    );
}

#[test]
fn a_model_and_an_effort_reach_the_flags_the_kind_declares() {
    let sandbox = Sandbox::new();
    declare_tuned(&sandbox);
    folder_workspace(&sandbox);

    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "tuned",
            "--workspace",
            "main",
            "--model",
            "fast-model",
            "--effort",
            "high",
            "--prompt",
            "hi",
        ],
    );
    let terminal = text(&started, "terminal_id").to_owned();
    wait_for_line(&sandbox, &terminal, "fake-agent: model fast-model");
    wait_for_line(&sandbox, &terminal, "fake-agent: effort high");
}

#[test]
fn a_model_or_an_effort_the_kind_does_not_declare_is_refused_and_names_the_kind() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);

    for args in [vec!["--model", "fast-model"], vec!["--effort", "high"]] {
        let mut command = vec!["--json", "agent", "start", "fake", "--workspace", "main"];
        command.extend(args);
        let outcome = sandbox.run("default", &command).failure();
        let document = single_document(&outcome);
        assert_eq!(
            document["error"]["code"], "agent.option_unsupported",
            "{document}"
        );
        let message = document["error"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("fake"),
            "the message names the kind: {message}"
        );
    }
}

#[test]
fn forget_deletes_a_released_sessions_archive_too() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let (session, _) = started_and_stopped(&sandbox);
    sandbox.json_ok("default", &["agent", "release", &session]);

    sandbox.json_ok("default", &["agent", "forget", &session]);
    assert_eq!(
        error_code(&sandbox, &["agent", "archive", &session]),
        "agent.not_found"
    );
}
