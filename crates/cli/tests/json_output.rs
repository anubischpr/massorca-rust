//! `--json` writes exactly one document on stdout: an `ok` envelope with the result, or with the error.

mod harness;

use harness::{Sandbox, single_document};
use predicates::str::is_empty;

#[test]
fn success_document_holds_ok_true_and_the_result() {
    let sandbox = Sandbox::new();

    let outcome = sandbox
        .run("default", &["--json", "service", "status"])
        .success();

    let doc = single_document(&outcome);
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["result"]["running"], false);
    assert!(doc["result"]["info"].is_null());
    assert!(doc.get("error").is_none());
}

#[test]
fn text_results_go_to_stdout_and_leave_stderr_empty() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["service", "status"])
        .success()
        .stderr(is_empty());
}

#[test]
fn json_usage_error_is_a_single_document_with_exit_2() {
    let sandbox = Sandbox::new();

    let outcome = sandbox.run("default", &["--json", "frobnicate"]).code(2);

    let doc = single_document(&outcome);
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["code"], "usage.invalid_arguments");
}
