use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{ChatItemBody, NoticeLevel, TurnState};
use serde_json::json;

use super::super::CodexAdapter;
use super::{feed, lines, sent};
use crate::chat::adapter::{ChatAdapter, JournalOp};

#[test]
fn the_launch_plan_becomes_app_server_with_its_overrides_and_the_threads_settings() {
    let mut adapter = CodexAdapter::new();
    let args = adapter.launch_args(
        [
            "resume",
            "thr-9",
            "--sandbox",
            "workspace-write",
            "--ask-for-approval",
            "on-request",
            "-m",
            "gpt-5.5",
            "-c",
            "model_reasoning_effort=high",
            "-c",
            "notify=[\"x\"]",
            "-c",
            "approval_policy=never",
            "--search",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    assert_eq!(args, ["app-server", "-c", "model_reasoning_effort=high"]);
    let opening = lines(&adapter.opening());
    let ops = feed(
        &mut adapter,
        json!({ "id": opening[0]["id"], "result": {} }),
    );
    let out = sent(&ops);
    assert_eq!(out[0], json!({ "method": "initialized", "params": {} }));
    assert_eq!(out[1]["method"], "thread/resume");
    assert_eq!(
        out[1]["params"],
        json!({ "threadId": "thr-9", "model": "gpt-5.5", "sandbox": "workspace-write",
                "approvalPolicy": "on-request" })
    );
    assert!(out.iter().all(|line| line.get("jsonrpc").is_none()));
}

#[test]
fn the_bypass_and_auto_review_modes_are_the_threads_policy_and_no_mode_asks() {
    let mut adapter = CodexAdapter::new();
    adapter.launch_args(Vec::new());
    let (_, params) = super::super::launch::open_thread(&adapter.plan);
    assert_eq!(
        params,
        json!({ "sandbox": "workspace-write", "approvalPolicy": "on-request" })
    );
    adapter.launch_args(vec![
        "--dangerously-bypass-approvals-and-sandbox".to_owned(),
    ]);
    let (_, params) = super::super::launch::open_thread(&adapter.plan);
    assert_eq!(
        params,
        json!({ "sandbox": "danger-full-access", "approvalPolicy": "never" })
    );
    adapter.launch_args(vec!["--approve-for-me".to_owned()]);
    let (method, params) = super::super::launch::open_thread(&adapter.plan);
    assert_eq!(method, "thread/start");
    assert_eq!(params["approvalsReviewer"], "auto_review");
}

#[test]
fn a_prompt_waits_for_the_thread_and_the_thread_id_is_learned() {
    let mut adapter = CodexAdapter::new();
    adapter.launch_args(Vec::new());
    adapter.opening();
    assert!(
        adapter.encode_prompt("hello").is_empty(),
        "the thread is not open yet"
    );
    feed(&mut adapter, json!({ "id": 1, "result": {} }));
    let ops = feed(
        &mut adapter,
        json!({ "id": 2, "result": { "thread": { "id": "thr-1" } } }),
    );
    assert_eq!(
        ops[0],
        JournalOp::LearnRef {
            agent_ref: "thr-1".to_owned()
        }
    );
    let turn = &sent(&ops)[0];
    assert_eq!(turn["method"], "turn/start");
    assert_eq!(
        turn["params"],
        json!({ "threadId": "thr-1", "input": [{ "type": "text", "text": "hello" }] })
    );
    // Once open, a prompt goes out at once.
    assert_eq!(
        lines(&adapter.encode_prompt("again"))[0]["method"],
        "turn/start"
    );
}

#[test]
fn a_refused_thread_is_an_error_and_the_waiting_turn_ends_failed() {
    let mut adapter = CodexAdapter::new();
    adapter.launch_args(vec!["resume".to_owned(), "thr-gone".to_owned()]);
    adapter.opening();
    adapter.encode_prompt("hello");
    feed(&mut adapter, json!({ "id": 1, "result": {} }));
    let ops = feed(
        &mut adapter,
        json!({ "id": 2, "error": { "code": -32600, "message": "no rollout found" } }),
    );
    assert!(
        matches!(&ops[0], JournalOp::Add { body: ChatItemBody::Notice { level: NoticeLevel::Error, text }, .. }
        if text.contains("no rollout found"))
    );
    assert_eq!(
        ops[1],
        JournalOp::EndTurn {
            state: TurnState::Completed,
            outcome: Some(Outcome::Failure)
        }
    );
}
