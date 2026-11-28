//! `massorca team delegate` end to end (decision 0128): outside a bot's agent run it is refused with its own code, before any
//! service call, so a person who types it gets a clear answer and no service starts.

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
fn a_delegation_typed_outside_an_agent_run_is_refused_with_its_code() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");

    let error = error_of(
        &sandbox,
        &["team", "delegate", "--text", "Parse the headers"],
    );
    assert_eq!(error["code"], "team.delegate_not_a_bot", "{error}");
}
