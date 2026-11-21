//! `--json` writes exactly one document on stdout: the error document for a usage mistake.

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use serde_json::Value;

#[test]
fn json_usage_error_is_a_single_document_with_exit_2() {
    let outcome = Command::new(cargo_bin("massorca"))
        .args(["--json", "frobnicate"])
        .assert()
        .code(2);

    let stdout = String::from_utf8_lossy(&outcome.get_output().stdout).into_owned();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout must hold exactly one JSON document: {stdout:?}"
    );
    let doc: Value = serde_json::from_str(lines[0]).expect("stdout is JSON");
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["code"], "usage.invalid_arguments");
}
