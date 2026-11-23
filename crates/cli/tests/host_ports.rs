//! `massorca host ports` (decision 0081): the listening ports of the terminals' processes, or `unverifiable` when the
//! service cannot be reached or this host's socket table cannot be read, never "no ports".

mod harness;

use harness::{Sandbox, single_document, stdout_text, with_json};

/// A sandbox has no terminals, so a host that reads its sockets answers an empty, verified list.
#[cfg(not(target_os = "macos"))]
#[test]
fn json_answers_the_ports_with_their_terminals_and_the_unverifiable_flag() {
    let sandbox = Sandbox::new();
    let result = sandbox.json_ok("p", &["host", "ports"]);
    assert!(result["ports"].is_array(), "{result}");
    assert!(result["terminals"].is_array(), "{result}");
    assert_eq!(result["unverifiable"], false, "{result}");
    assert!(
        result["sampled_at_ms"].as_i64().is_some_and(|at| at > 0),
        "{result}"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn text_says_no_port_when_the_answer_is_verified_and_empty() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run("p", &["host", "ports"]).success();
    let text = stdout_text(&outcome);
    assert!(
        text.lines().any(|line| line == "no listening ports"),
        "{text}"
    );
    assert!(!text.contains("unverifiable"), "{text}");
}

/// macOS has no socket reader yet (decision 0081): its socket table reads as unreadable, so the answer is unverifiable.
#[cfg(target_os = "macos")]
#[test]
fn macos_says_its_socket_table_cannot_be_read_and_never_no_ports() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run("p", &["host", "ports"]).failure();
    assert_eq!(outcome.get_output().status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&outcome.get_output().stderr).into_owned();
    assert!(stderr.contains("socket table"), "{stderr}");
}

#[test]
fn an_unreachable_service_is_unverifiable_and_never_no_ports() {
    let sandbox = Sandbox::new();

    let outcome = sandbox
        .run("p", &["--no-autostart", "host", "ports"])
        .failure();
    let output = outcome.get_output();
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unverifiable"), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("no listening ports"), "{stdout}");
    assert!(!stdout.contains("PORT"), "{stdout}");

    let outcome = sandbox
        .run("p", &with_json(&["--no-autostart", "host", "ports"]))
        .failure();
    assert_eq!(outcome.get_output().status.code(), Some(4));
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(
        document["error"]["code"], "ports.unverifiable",
        "{document}"
    );
    assert_eq!(
        document["error"]["data"]["state"], "unverifiable",
        "{document}"
    );
    assert!(document.get("result").is_none(), "{document}");
}
