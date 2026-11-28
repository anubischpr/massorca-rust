//! `massorca automations run|runs` end to end (decision 0125): a run now starts the automation's agent in its project
//! folder and completes with the agent's result; an automation whose project was removed records why and starts nothing;
//! `--now` is required; and the text output names each run.

mod harness;

use harness::{Sandbox, eventually, single_document, stdout_text, with_json};
use serde_json::{Value, json};

/// Declares kind `fake`, which runs the fake agent with an argv prompt and reports its activity and result.
fn declare_fake(sandbox: &Sandbox) {
    sandbox.prepare_profile("default");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": {
            "mechanism": "env-command",
            "events": {
                "start": "idle", "prompt": "working", "ask": "waiting",
                "stop": "done", "fail": "done:failure"
            },
            "result_field": "result"
        }
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kind `fake`, folder project `app` and a daily automation `Morning PRs` for the fake agent in it.
fn automation_sandbox() -> (Sandbox, String) {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let folder = sandbox.home().join("app");
    std::fs::create_dir_all(&folder).expect("create the folder");
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            folder.to_str().expect("utf-8 path"),
            "--name",
            "app",
            "--folder",
        ],
    );
    let created = sandbox.json_ok(
        "default",
        &[
            "automations",
            "create",
            "Morning PRs",
            "--project",
            "app",
            "--agent",
            "fake",
            "--prompt",
            "Summarise the open pull requests",
            "--time",
            "09:00",
            "--zone",
            "UTC",
        ],
    );
    let id = created["id"].as_str().expect("an id").to_owned();
    (sandbox, id)
}

fn runs(sandbox: &Sandbox, id: &str) -> Value {
    sandbox.json_ok("default", &["automations", "runs", id])
}

#[test]
fn a_run_now_starts_the_agent_in_the_project_folder_and_completes_with_its_result() {
    let (sandbox, id) = automation_sandbox();

    let run = sandbox.json_ok("default", &["automations", "run", &id, "--now"]);
    assert_eq!(run["number"], 1, "{run}");
    assert_eq!(run["trigger"], "manual", "{run}");
    let session = run["session_id"]
        .as_str()
        .expect("the run started an agent")
        .to_owned();

    let completed = eventually("the run completes", || {
        let listed = runs(&sandbox, &id);
        let first = listed["runs"][0].clone();
        (first["state"] == "completed").then_some(first)
    });
    assert_eq!(completed["outcome"], "success", "{completed}");
    assert_eq!(
        completed["result"], "done: Summarise the open pull requests",
        "{completed}"
    );
    assert_eq!(completed["usage"]["status"], "unavailable", "{completed}");
    assert!(completed["ended_at_ms"].is_i64(), "{completed}");

    let agent = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(agent["kind"], "fake");
    assert_eq!(agent["title"], "Morning PRs");
    let shown = sandbox.json_ok("default", &["automations", "show", &id]);
    assert!(shown["last_run_at_ms"].is_i64(), "{shown}");

    let text = stdout_text(
        &sandbox
            .run("default", &["automations", "runs", &id])
            .success(),
    );
    assert!(text.starts_with("run 1 completed for "), "{text}");
}

#[test]
fn a_run_of_an_automation_whose_project_was_removed_records_why_and_starts_no_agent() {
    let (sandbox, id) = automation_sandbox();
    sandbox.json_ok("default", &["project", "rm", "app"]);

    let run = sandbox.json_ok("default", &["automations", "run", &id, "--now"]);

    assert_eq!(run["state"], "skipped_unavailable", "{run}");
    assert_eq!(run["reason_code"], "automation.no_project", "{run}");
    let listed = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(listed["sessions"], json!([]), "nothing was started");
    let text = stdout_text(
        &sandbox
            .run("default", &["automations", "runs", &id])
            .success(),
    );
    assert!(
        text.contains("not started") && text.contains("no project"),
        "{text}"
    );
}

#[test]
fn run_without_now_is_refused_and_starts_nothing() {
    let (sandbox, id) = automation_sandbox();

    let outcome = sandbox
        .run("default", &with_json(&["automations", "run", &id]))
        .failure()
        .code(1);

    let document = single_document(&outcome);
    assert_eq!(
        document["error"]["code"], "request.invalid_params",
        "{document}"
    );
    assert_eq!(runs(&sandbox, &id)["runs"], json!([]));
}
