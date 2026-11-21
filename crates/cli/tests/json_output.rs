//! `--json` writes exactly one document on stdout: an `ok` envelope with the result, or with the error.

mod harness;

use harness::{Sandbox, single_document};
use predicates::str::{is_empty, starts_with};

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
fn error_document_holds_ok_false_and_the_code_message_and_data() {
    let sandbox = Sandbox::new();

    let outcome = sandbox
        .run(
            "default",
            &["--json", "config", "set", "workspaces.default_root", "42"],
        )
        .code(1);

    let doc = single_document(&outcome);
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["code"], "config.invalid_value");
    assert!(
        doc["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );
    assert!(doc["error"].get("data").is_some());
    assert!(doc.get("result").is_none());
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
fn text_errors_go_to_stderr_and_leave_stdout_empty() {
    let sandbox = Sandbox::new();

    sandbox
        .run(
            "default",
            &["config", "set", "workspaces.default_root", "relative/path"],
        )
        .code(1)
        .stdout(is_empty())
        .stderr(starts_with("error: "));
}

#[test]
fn json_usage_error_is_a_single_document_with_exit_2() {
    let sandbox = Sandbox::new();

    let outcome = sandbox.run("default", &["--json", "frobnicate"]).code(2);

    let doc = single_document(&outcome);
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["code"], "usage.invalid_arguments");
}
