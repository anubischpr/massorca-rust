//! `massorca secret ...`: values come in on stdin only, and no output, log or profile file ever shows one.

mod harness;

use std::fs;
use std::path::Path;

use assert_cmd::assert::Assert;
use harness::{Sandbox, single_document, stdout_text};
use predicates::str::contains;

const VALUE: &str = "cli-vault-value-5d81e2";

fn assert_value_absent(outcome: &Assert) {
    let output = outcome.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(VALUE), "stdout shows the value: {stdout}");
    assert!(!stderr.contains(VALUE), "stderr shows the value: {stderr}");
}

fn set_from_stdin(sandbox: &Sandbox, scope: &str, name: &str) -> Assert {
    let input = format!("{VALUE}\n");
    let outcome = sandbox
        .run_with_input("default", &["secret", "set", scope, name], input.as_bytes())
        .success();
    assert_value_absent(&outcome);
    outcome
}

/// Every file under `dir`, recursively.
fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn set_from_stdin_then_list_status_and_remove_never_print_the_value() {
    let sandbox = Sandbox::new();

    set_from_stdin(&sandbox, "global", "API_TOKEN");

    let listed = sandbox.run("default", &["secret", "list"]).success();
    assert_value_absent(&listed);
    assert_eq!(stdout_text(&listed), "global API_TOKEN\n");

    let status = sandbox.run("default", &["secret", "status"]).success();
    assert_value_absent(&status);
    let text = stdout_text(&status);
    assert!(text.contains("protection: file"), "{text}");
    assert!(text.contains("state: ready"), "{text}");
    assert!(text.contains("secrets: 1"), "{text}");
    assert!(text.contains("stored on disk"), "{text}");

    let json = sandbox.json_ok("default", &["secret", "list", "--scope", "global"]);
    assert_eq!(json["secrets"][0]["name"], "API_TOKEN");
    assert!(!json.to_string().contains(VALUE));

    let removed = sandbox
        .run("default", &["secret", "remove", "global", "API_TOKEN"])
        .success();
    assert_value_absent(&removed);
    sandbox
        .run("default", &["secret", "remove", "global", "API_TOKEN"])
        .code(1)
        .stderr(contains("secret.not_found"));
}

#[test]
fn an_existing_name_needs_replace() {
    let sandbox = Sandbox::new();
    set_from_stdin(&sandbox, "global", "API_TOKEN");

    let refused = sandbox
        .run_with_input(
            "default",
            &["secret", "set", "global", "API_TOKEN"],
            VALUE.as_bytes(),
        )
        .code(1)
        .stderr(contains("secret.name_in_use"));
    assert_value_absent(&refused);
    let rotated = sandbox
        .run_with_input(
            "default",
            &["secret", "set", "global", "API_TOKEN", "--replace"],
            VALUE.as_bytes(),
        )
        .success();
    assert_value_absent(&rotated);
}

#[test]
fn an_empty_value_is_refused_before_the_service_is_asked() {
    let sandbox = Sandbox::new();

    let empty = sandbox
        .run_with_input("default", &["secret", "set", "global", "TOKEN"], b"")
        .code(1);
    let doc = sandbox
        .run_with_input(
            "default",
            &["--json", "secret", "set", "global", "TOKEN"],
            b"\n",
        )
        .code(1);
    assert!(stdout_text(&empty).is_empty());
    assert_eq!(single_document(&doc)["error"]["code"], "secret.empty_value");
    let status = sandbox.json_ok("default", &["secret", "status"]);
    assert_eq!(status["count"], 0);
    assert_eq!(status["protection"], "none");
}

#[test]
fn links_show_their_owner_and_the_owner_shows_its_links() {
    let sandbox = Sandbox::new();
    set_from_stdin(&sandbox, "global", "API_TOKEN");

    sandbox
        .run(
            "default",
            &[
                "secret",
                "link",
                "project:p1",
                "API_TOKEN",
                "--from",
                "global",
            ],
        )
        .success();
    let listed = sandbox.run("default", &["secret", "list"]).success();
    assert_eq!(
        stdout_text(&listed),
        "global API_TOKEN (linked by project:p1)\nproject:p1 API_TOKEN (linked from global)\n"
    );
    sandbox
        .run("default", &["secret", "remove", "global", "API_TOKEN"])
        .code(1)
        .stderr(contains("secret.has_links"));
    sandbox
        .run(
            "default",
            &["secret", "remove", "global", "API_TOKEN", "--include-links"],
        )
        .success();
    let empty = sandbox.run("default", &["secret", "list"]).success();
    assert!(stdout_text(&empty).is_empty());
}

#[test]
fn requests_can_be_listed_and_declined_once() {
    let sandbox = Sandbox::new();
    let request = sandbox.json_ok(
        "default",
        &[
            "secret",
            "request",
            "global",
            "TOKEN",
            "--reason",
            "deploy needs it",
        ],
    );
    let id = request["id"].as_str().expect("a request id").to_owned();

    let open = sandbox.run("default", &["secret", "requests"]).success();
    assert!(stdout_text(&open).contains(&id), "{}", stdout_text(&open));
    sandbox
        .run("default", &["secret", "decline", &id])
        .success();
    sandbox
        .run("default", &["secret", "decline", &id])
        .code(1)
        .stderr(contains("secret.request_resolved"));
    let all = sandbox.json_ok("default", &["secret", "requests", "--all"]);
    assert_eq!(all["requests"][0]["state"], "declined");
}

#[test]
fn a_value_sent_through_secret_set_appears_in_no_file_of_the_profile() {
    let sandbox = Sandbox::new();
    let input = format!("{VALUE}\n");
    let mut command = sandbox.command("default");
    command
        .env("MASSORCA_LOG", "trace")
        .args(["secret", "set", "global", "API_TOKEN"]);
    let mut stdin = tempfile::tempfile().expect("a temporary stdin");
    std::io::Write::write_all(&mut stdin, input.as_bytes()).expect("stdin is written");
    std::io::Seek::seek(&mut stdin, std::io::SeekFrom::Start(0)).expect("stdin rewinds");
    let output = harness::capture_with_stdin(
        &mut command,
        harness::COMMAND_TIMEOUT,
        std::process::Stdio::from(stdin),
    )
    .expect("secret set finishes");
    let outcome = Assert::new(output).success();
    assert_value_absent(&outcome);
    sandbox.run("default", &["service", "stop"]).success();

    let mut files = Vec::new();
    files_under(&sandbox.profile_dir("default"), &mut files);
    let logs: Vec<_> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    assert!(
        logs.iter()
            .any(|path| fs::metadata(path).is_ok_and(|meta| meta.len() > 0)),
        "the trace-level service wrote a log: {files:?}"
    );
    for path in &files {
        let bytes = fs::read(path).unwrap_or_default();
        let found = bytes
            .windows(VALUE.len())
            .any(|window| window == VALUE.as_bytes());
        assert!(!found, "{} holds the value", path.display());
    }
}
