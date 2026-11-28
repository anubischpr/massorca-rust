//! `massorca team message send|reply` and `massorca team inbox` end to end (decision 0129): outside a bot's agent run
//! each is refused with its own code before any service call.

mod harness;

use harness::{Sandbox, single_document, with_json};
use serde_json::Value;

fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure().code(1);
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

#[test]
fn team_messages_typed_outside_an_agent_run_are_refused_with_their_code() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");

    let send = error_of(
        &sandbox,
        &[
            "team",
            "message",
            "send",
            "--to",
            "parent",
            "--type",
            "worker_done",
            "--subject",
            "Lexer",
            "--body",
            "Done",
            "--outcome",
            "succeeded",
        ],
    );
    assert_eq!(send["code"], "team.message_not_a_bot", "{send}");
    let reply = error_of(
        &sandbox,
        &["team", "message", "reply", "m1", "--body", "Thanks"],
    );
    assert_eq!(reply["code"], "team.message_not_a_bot", "{reply}");
    let inbox = error_of(&sandbox, &["team", "inbox", "--mark-read"]);
    assert_eq!(inbox["code"], "team.message_not_a_bot", "{inbox}");
}
