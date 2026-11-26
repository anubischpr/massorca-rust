use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ChatItemBody, DiffStatus, MAX_SUMMARY_BYTES, MessageState, NoticeLevel, TokenUsage, ToolStatus,
    TurnState,
};
use serde_json::{Value, json};

use super::{feed, notify, opened};
use crate::chat::adapter::JournalOp;

#[test]
fn text_streams_into_one_message_and_the_turn_completes_with_its_usage() {
    let (mut adapter, _) = opened(&[]);
    let delta = |text: &str| {
        notify(
            "item/agentMessage/delta",
            json!({ "threadId": "thr-1", "turnId": "t1", "itemId": "m1", "delta": text }),
        )
    };
    let first = feed(&mut adapter, delta("Hel"));
    assert!(
        matches!(&first[0], JournalOp::Add { key: Some(key), body: ChatItemBody::AssistantMessage { text, state: MessageState::Running }, .. }
        if key == "item:m1" && text == "Hel")
    );
    assert_eq!(
        feed(&mut adapter, delta("lo")),
        [JournalOp::Append {
            key: "item:m1".to_owned(),
            text: "lo".to_owned()
        }]
    );
    let done = feed(
        &mut adapter,
        notify(
            "item/completed",
            json!({ "threadId": "thr-1", "turnId": "t1",
            "item": { "type": "agentMessage", "id": "m1", "text": "Hello" } }),
        ),
    );
    assert!(
        matches!(&done[0], JournalOp::Replace { body: ChatItemBody::AssistantMessage { text, state: MessageState::Completed }, .. } if text == "Hello")
    );
    let usage = feed(
        &mut adapter,
        notify(
            "thread/tokenUsage/updated",
            json!({ "threadId": "thr-1", "turnId": "t1",
            "tokenUsage": { "total": { "inputTokens": 10, "cachedInputTokens": 4, "outputTokens": 3,
                                       "reasoningOutputTokens": 1, "totalTokens": 13 } } }),
        ),
    );
    assert_eq!(
        usage,
        [JournalOp::Usage {
            usage: TokenUsage {
                input_tokens: 10,
                cached_input_tokens: 4,
                output_tokens: 3,
                reasoning_output_tokens: 1,
                total_tokens: 13
            }
        }]
    );
    let end = feed(
        &mut adapter,
        notify(
            "turn/completed",
            json!({ "threadId": "thr-1", "turn": { "id": "t1", "status": "completed" } }),
        ),
    );
    assert_eq!(
        end,
        [JournalOp::EndTurn {
            state: TurnState::Completed,
            outcome: Some(Outcome::Success)
        }]
    );
}

#[test]
fn a_command_runs_as_a_tool_with_its_output_and_a_file_change_adds_its_diffs() {
    let (mut adapter, _) = opened(&[]);
    let item = |status: &str, extra: Value| {
        let mut item = json!({ "type": "commandExecution", "id": "c1", "command": "ls -la", "status": status });
        for (key, value) in extra.as_object().unwrap() {
            item[key] = value.clone();
        }
        item
    };
    feed(
        &mut adapter,
        notify(
            "item/started",
            json!({ "item": item("inProgress", json!({})) }),
        ),
    );
    let streamed = feed(
        &mut adapter,
        notify(
            "item/commandExecution/outputDelta",
            json!({ "itemId": "c1", "delta": "a.txt\n" }),
        ),
    );
    assert!(
        matches!(&streamed[0], JournalOp::Replace { body: ChatItemBody::ToolUse(tool), .. }
        if tool.status == ToolStatus::Updated && tool.output_summary.as_deref() == Some("a.txt\n"))
    );
    let done = feed(
        &mut adapter,
        notify(
            "item/completed",
            json!({ "item": item("completed", json!({ "exitCode": 2, "aggregatedOutput": "boom" })) }),
        ),
    );
    assert!(
        matches!(&done[0], JournalOp::Add { body: ChatItemBody::ToolUse(tool), .. }
        if tool.name == "shell" && tool.input_summary == "ls -la" && tool.status == ToolStatus::Failed
            && tool.output_summary.as_deref() == Some("boom"))
    );

    let change = feed(
        &mut adapter,
        notify(
            "item/completed",
            json!({ "item": { "type": "fileChange", "id": "f1", "status": "completed",
            "changes": [{ "path": "a.rs", "kind": { "type": "add" }, "diff": "+x" },
                        { "path": "b.rs", "kind": { "type": "update", "move_path": "c.rs" }, "diff": "-y" }] } }),
        ),
    );
    assert_eq!(change.len(), 3, "{change:?}");
    assert!(
        matches!(&change[1], JournalOp::Add { body: ChatItemBody::Diff { path, status: DiffStatus::Added, .. }, .. } if path == "a.rs")
    );
    assert!(
        matches!(&change[2], JournalOp::Add { body: ChatItemBody::Diff { status: DiffStatus::Renamed, old_path: Some(old), .. }, .. } if old == "c.rs")
    );
}

#[test]
fn an_unknown_message_or_item_is_skipped_with_one_notice_naming_it() {
    let (mut adapter, _) = opened(&[]);
    let first = feed(
        &mut adapter,
        notify("thread/newThing", json!({ "secret": "value" })),
    );
    let JournalOp::Add {
        body: ChatItemBody::Notice { level, text },
        ..
    } = &first[0]
    else {
        panic!("{first:?}");
    };
    assert_eq!(*level, NoticeLevel::Warning);
    assert!(
        text.contains("thread/newThing") && !text.contains("value"),
        "{text}"
    );
    assert!(feed(&mut adapter, notify("thread/newThing", json!({}))).is_empty());
    let item = feed(
        &mut adapter,
        notify(
            "item/started",
            json!({ "item": { "type": "hologram", "id": "h1" } }),
        ),
    );
    assert_eq!(item.len(), 1);
    assert!(feed(&mut adapter, notify("thread/status/changed", json!({}))).is_empty());
    assert!(feed(&mut adapter, notify("codex/event/task_started", json!({}))).is_empty());
}

#[test]
fn a_command_output_is_kept_to_the_summary_bound_and_output_past_it_replaces_nothing() {
    let (mut adapter, _) = opened(&[]);
    feed(
        &mut adapter,
        notify(
            "item/started",
            json!({ "item": { "type": "commandExecution", "id": "c2", "command": "yes", "status": "inProgress" } }),
        ),
    );
    let output = |delta: &str| {
        notify(
            "item/commandExecution/outputDelta",
            json!({ "itemId": "c2", "delta": delta }),
        )
    };
    let flood = "y".repeat(MAX_SUMMARY_BYTES + 100);
    let first = feed(&mut adapter, output(&flood));
    assert!(
        matches!(&first[0], JournalOp::Replace { body: ChatItemBody::ToolUse(tool), .. }
        if tool.output_summary.as_ref().is_some_and(|text| text.len() == MAX_SUMMARY_BYTES) && tool.truncated)
    );
    // Past the bound the item shows the same text, so no replacement is written and no more output is kept.
    assert!(feed(&mut adapter, output("more")).is_empty());
    assert!(feed(&mut adapter, output("")).is_empty());
    assert!(adapter.tools["c2"].output.len() <= MAX_SUMMARY_BYTES);
}
