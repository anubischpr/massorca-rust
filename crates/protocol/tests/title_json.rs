//! `agent.generate_title` on the wire (capability `agent-title/1`, decision 0167): the params a caller sends and the slug
//! it gets back. The fields a caller does not set are left out of the JSON, as the other agent params leave them out.

use massorca_protocol::methods::agents::{self, GenerateTitleParams, GenerateTitleResult};
use massorca_protocol::{capabilities, codes};
use serde_json::json;

#[test]
fn the_method_capability_and_refusal_codes_are_the_contract() {
    assert_eq!(agents::GENERATE_TITLE, "agent.generate_title");
    assert_eq!(capabilities::AGENT_TITLE, "agent-title/1");
    assert_eq!(
        codes::AGENT_ONESHOT_UNSUPPORTED,
        "agent.oneshot_unsupported"
    );
    assert_eq!(codes::AGENT_ONESHOT_TIMEOUT, "agent.oneshot_timeout");
    assert_eq!(codes::AGENT_ONESHOT_FAILED, "agent.oneshot_failed");
}

#[test]
fn params_with_only_a_kind_and_a_prompt_leave_the_optional_fields_out() {
    let params = GenerateTitleParams {
        kind: "claude".to_owned(),
        account: None,
        model: None,
        prompt: "Explora el proyecto".to_owned(),
        reply: None,
    };
    assert_eq!(
        serde_json::to_value(&params).expect("the params serialize"),
        json!({ "kind": "claude", "prompt": "Explora el proyecto" })
    );
    let back: GenerateTitleParams = serde_json::from_value(json!({
        "kind": "claude",
        "prompt": "Explora el proyecto"
    }))
    .expect("the params parse");
    assert_eq!(back, params);
}

#[test]
fn params_carry_the_account_model_and_reply_when_set() {
    let wire = json!({
        "kind": "claude",
        "account": "0195c0de-0000-7000-8000-000000000001",
        "model": "sonnet",
        "prompt": "Fix the login bug",
        "reply": "I will look at the auth module"
    });
    let params: GenerateTitleParams =
        serde_json::from_value(wire.clone()).expect("the params parse");
    assert_eq!(
        params.account.as_deref(),
        Some("0195c0de-0000-7000-8000-000000000001")
    );
    assert_eq!(params.model.as_deref(), Some("sonnet"));
    assert_eq!(
        params.reply.as_deref(),
        Some("I will look at the auth module")
    );
    assert_eq!(serde_json::to_value(&params).expect("serialize"), wire);
}

#[test]
fn the_result_is_a_slug_object() {
    let result = GenerateTitleResult {
        slug: "explora-el-proyecto".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&result).expect("the result serializes"),
        json!({ "slug": "explora-el-proyecto" })
    );
}
