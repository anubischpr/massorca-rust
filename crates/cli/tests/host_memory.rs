//! `massorca host memory` (decision 0081): the service's memory and its terminals', or `unverifiable` when the service
//! cannot be reached, never zero.

mod harness;

use harness::{Sandbox, single_document, stdout_text, with_json};

#[test]
fn json_answers_the_service_memory_with_its_metric_and_terminals() {
    let sandbox = Sandbox::new();
    let result = sandbox.json_ok("p", &["host", "memory"]);

    let metric = if cfg!(windows) {
        "working_set"
    } else {
        "resident"
    };
    assert_eq!(result["metric"], metric, "{result}");
    let service = result["service"]["bytes"].as_u64().expect("service bytes");
    let total = result["total_bytes"].as_u64().expect("total bytes");
    assert!(service > 0, "{result}");
    assert!(total >= service, "{result}");
    assert!(result["service"]["pid"].as_u64().is_some(), "{result}");
    assert!(result["terminals"].is_array(), "{result}");
    assert!(result["unreadable"].as_u64().is_some(), "{result}");
    assert!(result["sampled_at_ms"].as_i64().is_some_and(|at| at > 0));
}

#[test]
fn text_shows_the_total_in_binary_units() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run("p", &["host", "memory"]).success();
    let text = stdout_text(&outcome);
    let first = text.lines().next().unwrap_or_default();
    let metric = if cfg!(windows) {
        "working set"
    } else {
        "resident"
    };
    assert!(first.starts_with("memory: "), "{text}");
    assert!(first.ends_with(&format!("({metric})")), "{text}");
    assert!(
        [" KB ", " MB ", " GB "]
            .iter()
            .any(|unit| first.contains(unit)),
        "{text}"
    );
    assert!(text.contains("service"), "{text}");
}

#[test]
fn an_unreachable_service_is_unverifiable_and_never_zero() {
    let sandbox = Sandbox::new();

    let outcome = sandbox
        .run("p", &["--no-autostart", "host", "memory"])
        .failure();
    let output = outcome.get_output();
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unverifiable"), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("0 KB"), "{stdout}");
    assert!(!stdout.contains("memory:"), "{stdout}");

    let outcome = sandbox
        .run("p", &with_json(&["--no-autostart", "host", "memory"]))
        .failure();
    assert_eq!(outcome.get_output().status.code(), Some(4));
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(
        document["error"]["code"], "memory.unverifiable",
        "{document}"
    );
    assert_eq!(
        document["error"]["data"]["state"], "unverifiable",
        "{document}"
    );
    assert!(document.get("result").is_none(), "{document}");
}
