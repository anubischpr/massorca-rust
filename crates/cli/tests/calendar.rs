//! `massorca calendar` end to end (decision 0124): a routine's runs in a window of days, each placed on the routine's zone;
//! the kind filter; the refusals of a bad window; and the text listing.

mod harness;

use harness::{Sandbox, stdout_text};
use serde_json::json;

/// 2030-01-01 00:00 UTC, far enough ahead that every run is upcoming.
const JAN_1_2030: i64 = 1_893_456_000_000;
const HOUR_MS: i64 = 3_600_000;

/// Declares kind `fake`, so the routine's bot never starts a real agent in a test.
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

/// A team with one bot and a daily 09:00 routine named `morning` in Europe/Madrid.
fn sandbox_with_a_routine() -> Sandbox {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    sandbox.json_ok(
        "default",
        &["team", "create", "alpha", "--purpose", "Ships the parser"],
    );
    sandbox.json_ok(
        "default",
        &[
            "team",
            "bot",
            "add",
            "alpha",
            "mara",
            "--agent",
            "fake",
            "--job",
            "Writes the parser",
        ],
    );
    sandbox.json_ok(
        "default",
        &[
            "team",
            "routine",
            "add",
            "alpha",
            "morning",
            "--bot",
            "mara",
            "--prompt",
            "Read the notes.",
            "--time",
            "09:00",
            "--cadence",
            "daily",
            "--zone",
            "Europe/Madrid",
        ],
    );
    sandbox
}

#[test]
fn a_window_lists_each_routine_run_placed_on_the_routines_zone() {
    let sandbox = sandbox_with_a_routine();
    let result = sandbox.json_ok(
        "default",
        &["calendar", "--from", "2030-01-01", "--days", "2"],
    );

    let runs = result["occurrences"].as_array().expect("a list of runs");
    assert_eq!(runs.len(), 2, "one run per day: {result}");
    assert_eq!(runs[0]["name"], "morning");
    assert_eq!(runs[0]["kind"], "routine");
    assert_eq!(
        runs[0]["at_ms"],
        json!(JAN_1_2030 + 8 * HOUR_MS),
        "09:00 Madrid is 08:00 UTC"
    );
    assert_eq!(runs[0]["local_date"], "2030-01-01");
    assert_eq!(runs[0]["local_time"], "09:00");
    assert_eq!(runs[0]["zone"], "Europe/Madrid");
    assert_eq!(runs[0]["done"], false);
    assert_eq!(runs[1]["local_date"], "2030-01-02");
    assert_eq!(result["truncated"], false);
}

#[test]
fn the_kind_filter_leaves_out_the_routines_when_automations_are_asked_for() {
    let sandbox = sandbox_with_a_routine();
    let result = sandbox.json_ok(
        "default",
        &[
            "calendar",
            "--from",
            "2030-01-01",
            "--days",
            "2",
            "--kind",
            "automations",
        ],
    );
    assert_eq!(result["occurrences"], json!([]));
}

#[test]
fn the_text_listing_names_the_run_its_time_and_its_zone() {
    let sandbox = sandbox_with_a_routine();
    let outcome = sandbox
        .run(
            "default",
            &["calendar", "--from", "2030-01-01", "--days", "1"],
        )
        .success();
    let text = stdout_text(&outcome);
    assert!(
        text.contains("2030-01-01 09:00 Europe/Madrid  routine"),
        "text listing: {text}"
    );
    assert!(text.contains("morning"), "text listing: {text}");
}

#[test]
fn a_window_longer_than_62_days_and_a_date_that_is_not_real_are_refused() {
    let sandbox = sandbox_with_a_routine();
    let too_long = sandbox.run(
        "default",
        &["calendar", "--from", "2030-01-01", "--days", "63"],
    );
    let stderr =
        String::from_utf8_lossy(&too_long.failure().code(1).get_output().stderr).into_owned();
    assert!(
        stderr.contains("--days must be from 1 to 62"),
        "stderr: {stderr}"
    );

    let not_real = sandbox.run(
        "default",
        &["calendar", "--from", "2030-02-30", "--days", "2"],
    );
    let stderr =
        String::from_utf8_lossy(&not_real.failure().code(1).get_output().stderr).into_owned();
    assert!(
        stderr.contains("--from must be a UTC date"),
        "stderr: {stderr}"
    );
}
