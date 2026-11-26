//! `massorca automations ...` end to end (decision 0123): a definition is created, shown, listed, edited, enabled,
//! disabled and removed through the service; an edit changes only the options given; a refused definition fails with
//! its code; and the text output names the schedule and the next run.

mod harness;

use harness::{Sandbox, single_document, stdout_text, with_json};
use serde_json::Value;

/// A folder project named `app`, which the automations run in.
fn project(sandbox: &Sandbox) {
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
}

/// Creates a daily 09:00 automation named `name` in `app`, in UTC, with `extra` options.
fn create(sandbox: &Sandbox, name: &str, extra: &[&str]) -> Value {
    let mut args = vec![
        "automations",
        "create",
        name,
        "--project",
        "app",
        "--prompt",
        "Summarise the open pull requests",
        "--time",
        "09:00",
        "--zone",
        "UTC",
    ];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

/// Runs `--json <args>`, which must fail with exit code 1; returns the error body.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure().code(1);
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

#[test]
fn an_automation_is_created_shown_listed_and_removed() {
    let sandbox = Sandbox::new();
    project(&sandbox);

    let created = create(&sandbox, "Morning PRs", &[]);
    assert_eq!(created["name"], "Morning PRs");
    assert_eq!(created["cadence"], "daily");
    assert_eq!(created["enabled"], true);
    assert_eq!(created["project_name"], "app");
    let id = created["id"]
        .as_str()
        .expect("the automation has an id")
        .to_owned();

    let shown = sandbox.json_ok("default", &["automations", "show", "Morning PRs"]);
    assert_eq!(shown["id"], id);

    let listed = sandbox.json_ok("default", &["automations", "list"]);
    assert_eq!(listed["automations"].as_array().map(Vec::len), Some(1));

    let removed = sandbox.json_ok("default", &["automations", "remove", &id]);
    assert_eq!(removed["id"], id);
    let listed = sandbox.json_ok("default", &["automations", "list"]);
    assert_eq!(listed["automations"].as_array().map(Vec::len), Some(0));
}

#[test]
fn a_disabled_automation_has_no_next_run_and_enabling_schedules_one() {
    let sandbox = Sandbox::new();
    project(&sandbox);
    let created = create(&sandbox, "Morning PRs", &[]);
    let id = created["id"].as_str().expect("id").to_owned();

    let disabled = sandbox.json_ok("default", &["automations", "disable", &id]);
    assert_eq!(disabled["enabled"], false);
    assert!(disabled.get("next_run_at_ms").is_none(), "{disabled}");

    let enabled = sandbox.json_ok("default", &["automations", "enable", &id]);
    assert_eq!(enabled["enabled"], true);
    assert!(enabled["next_run_at_ms"].is_i64(), "{enabled}");
}

#[test]
fn an_edit_changes_only_the_options_given() {
    let sandbox = Sandbox::new();
    project(&sandbox);
    let created = create(&sandbox, "Morning PRs", &[]);
    let id = created["id"].as_str().expect("id").to_owned();

    let renamed = sandbox.json_ok(
        "default",
        &["automations", "edit", &id, "--name", "Renamed"],
    );
    assert_eq!(renamed["name"], "Renamed");
    assert_eq!(renamed["time"], "09:00");

    let moved = sandbox.json_ok("default", &["automations", "edit", &id, "--time", "10:30"]);
    assert_eq!(moved["name"], "Renamed");
    assert_eq!(moved["time"], "10:30");
}

#[test]
fn a_refused_definition_fails_with_its_code() {
    let sandbox = Sandbox::new();
    project(&sandbox);

    let error = error_of(
        &sandbox,
        &[
            "automations",
            "create",
            "Unknown agent",
            "--project",
            "app",
            "--prompt",
            "Summarise",
            "--agent",
            "no-such-agent",
        ],
    );
    assert_eq!(error["code"], "agent.kind_not_found", "{error}");

    let error = error_of(
        &sandbox,
        &[
            "automations",
            "create",
            "Never",
            "--project",
            "app",
            "--prompt",
            "Summarise",
            "--cadence",
            "cron",
            "--cron",
            "0 0 30 2 *",
        ],
    );
    assert_eq!(error["code"], "automation.schedule_invalid", "{error}");
}

#[test]
fn the_text_output_names_the_schedule_and_the_next_run() {
    let sandbox = Sandbox::new();
    project(&sandbox);

    let created = sandbox
        .run(
            "default",
            &[
                "automations",
                "create",
                "Weekday check",
                "--project",
                "app",
                "--prompt",
                "Check the build",
                "--cadence",
                "weekdays",
                "--time",
                "08:00",
                "--zone",
                "Europe/Madrid",
            ],
        )
        .success();
    let line = stdout_text(&created);
    assert!(line.contains("created automation Weekday check"), "{line}");
    assert!(line.contains("in app"), "{line}");

    let listed = sandbox.run("default", &["automations", "list"]).success();
    let text = stdout_text(&listed);
    assert!(text.contains("weekdays at 08:00 Europe/Madrid"), "{text}");
    assert!(text.contains("next run"), "{text}");
}
