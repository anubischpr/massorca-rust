//! A kind's standing arguments reach a chat run too (decision 0174): the Codex adapter keeps `--enable` from them, as it
//! keeps it from the kind's own launch arguments. See `chat_codex_support` for the fake app-server.

mod chat_claude_support;
mod chat_codex_support;
mod harness;

use chat_codex_support::*;
use harness::eventually;
use serde_json::json;

#[test]
fn a_chat_run_starts_with_the_kinds_standing_arguments() {
    let (sandbox, logs) = sandbox(
        &[
            await_method("turn/start"),
            turn_started(),
            turn_completed("completed"),
        ],
        &[],
    );
    let set = sandbox.json_ok(
        "default",
        &[
            "agent",
            "settings",
            "set",
            "fakecodex",
            "--arg",
            "--enable",
            "--arg",
            "standing-feature",
        ],
    );
    assert_eq!(set["args"], json!(["--enable", "standing-feature"]));

    start_chat(&sandbox, "hello");
    let run = eventually("the chat run's arguments", || {
        logs.runs().into_iter().next()
    });
    let at = run
        .iter()
        .position(|arg| arg == "--enable")
        .unwrap_or_else(|| panic!("no --enable in {run:?}"));
    assert_eq!(
        run.get(at + 1).map(String::as_str),
        Some("standing-feature"),
        "{run:?}"
    );
}
