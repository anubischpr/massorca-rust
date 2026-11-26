//! Tool calls, their results, diffs, sub-agents, plans, and bounded names.

use massorca_protocol::methods::chat::{
    ChatItemBody, DiffStatus, MAX_SUMMARY_BYTES, PlanEntry, PlanStatus, SubagentStatus, ToolStatus,
    ToolUse,
};
use serde_json::json;

use super::*;
use crate::chat::adapter::{ChatAdapter, JournalOp};

#[test]
fn a_tool_call_starts_updates_and_completes_with_bounded_output() {
    let mut adapter = adapter();
    let started = ToolUse {
        tool_use_id: "toolu_1".to_owned(),
        name: "Bash".to_owned(),
        input_summary: "cargo test".to_owned(),
        status: ToolStatus::Started,
        output_summary: None,
        truncated: false,
    };
    assert_eq!(
        adapter.translate(&assistant(
            "m4",
            None,
            json!([{ "type": "tool_use", "id": "toolu_1", "name": "Bash",
                     "input": { "command": "cargo test", "description": "Run tests" } }])
        )),
        [add(
            "tool:toolu_1",
            None,
            ChatItemBody::ToolUse(started.clone())
        )]
    );
    assert_eq!(
        adapter.translate(&frame(json!({
            "type": "tool_progress", "tool_use_id": "toolu_1", "tool_name": "Bash",
            "elapsed_time_seconds": 3, "parent_tool_use_id": null, "session_id": "s-1"
        }))),
        [JournalOp::Replace {
            key: "tool:toolu_1".to_owned(),
            body: ChatItemBody::ToolUse(ToolUse {
                status: ToolStatus::Updated,
                ..started.clone()
            })
        }]
    );
    let long = "x".repeat(MAX_SUMMARY_BYTES + 10);
    let ops = adapter.translate(&tool_result("toolu_1", json!(long), false, None));
    let [
        JournalOp::Replace {
            key,
            body: ChatItemBody::ToolUse(done),
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!(key, "tool:toolu_1");
    assert_eq!(done.status, ToolStatus::Completed);
    assert!(done.truncated);
    assert_eq!(
        done.output_summary.as_ref().map(String::len),
        Some(MAX_SUMMARY_BYTES)
    );
    // A second result for the same call changes nothing.
    assert_eq!(
        adapter.translate(&tool_result("toolu_1", json!("again"), false, None)),
        []
    );
}

#[test]
fn a_failed_tool_call_fails_with_the_text_of_its_result() {
    let mut adapter = adapter();
    adapter.translate(&assistant(
        "m5",
        None,
        json!([{ "type": "tool_use", "id": "toolu_2", "name": "Read", "input": { "file_path": "src/a.rs" } }]),
    ));
    let ops = adapter.translate(&tool_result(
        "toolu_2",
        json!([{ "type": "text", "text": "No such file" }]),
        true,
        None,
    ));
    let [
        JournalOp::Replace {
            body: ChatItemBody::ToolUse(failed),
            ..
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}");
    };
    assert_eq!(failed.status, ToolStatus::Failed);
    assert_eq!(failed.input_summary, "src/a.rs");
    assert_eq!(failed.output_summary.as_deref(), Some("No such file"));
}

#[test]
fn an_edit_adds_a_modified_diff_from_its_structured_patch() {
    let mut adapter = adapter();
    adapter.translate(&assistant(
        "m6",
        None,
        json!([{ "type": "tool_use", "id": "toolu_3", "name": "Edit",
                 "input": { "file_path": "src/a.rs", "old_string": "one", "new_string": "two" } }]),
    ));
    let ops = adapter.translate(&tool_result(
        "toolu_3",
        json!("The file src/a.rs has been updated."),
        false,
        Some(json!({
            "type": "update", "filePath": "src/a.rs",
            "structuredPatch": [{ "oldStart": 4, "oldLines": 1, "newStart": 4, "newLines": 1,
                                  "lines": ["-one", "+two"] }]
        })),
    ));
    assert_eq!(ops.len(), 2, "{ops:?}");
    assert_eq!(
        ops[1],
        add(
            "diff:toolu_3",
            None,
            ChatItemBody::Diff {
                path: "src/a.rs".to_owned(),
                status: DiffStatus::Modified,
                old_path: None,
                patch: "@@ -4,1 +4,1 @@\n-one\n+two\n".to_owned(),
                truncated: false,
            }
        )
    );
}

#[test]
fn a_write_of_a_new_file_adds_an_added_diff_and_an_edit_without_a_patch_is_built_from_its_input() {
    let mut adapter = adapter();
    adapter.translate(&assistant(
        "m7",
        None,
        json!([
            { "type": "tool_use", "id": "toolu_4", "name": "Write",
              "input": { "file_path": "notes.md", "content": "a\nb" } },
            { "type": "tool_use", "id": "toolu_5", "name": "MultiEdit",
              "input": { "file_path": "x.rs", "edits": [{ "old_string": "p", "new_string": "q" }] } }
        ]),
    ));
    let written = adapter.translate(&tool_result(
        "toolu_4",
        json!("File created successfully at: notes.md"),
        false,
        Some(json!({ "type": "create", "filePath": "notes.md", "content": "a\nb" })),
    ));
    assert_eq!(
        written[1],
        add(
            "diff:toolu_4",
            None,
            ChatItemBody::Diff {
                path: "notes.md".to_owned(),
                status: DiffStatus::Added,
                old_path: None,
                patch: "@@ -0,0 +1,2 @@\n+a\n+b\n".to_owned(),
                truncated: false,
            }
        )
    );
    let edited = adapter.translate(&tool_result("toolu_5", json!("ok"), false, None));
    assert!(
        matches!(&edited[1], JournalOp::Add { body: ChatItemBody::Diff { status: DiffStatus::Modified, patch, .. }, .. }
            if patch == "@@ -1,1 +1,1 @@\n-p\n+q\n"),
        "{edited:?}"
    );
    // A failed edit changed nothing: no diff.
    adapter.translate(&assistant(
        "m8",
        None,
        json!([{ "type": "tool_use", "id": "toolu_6", "name": "Edit",
                 "input": { "file_path": "y.rs", "old_string": "a", "new_string": "b" } }]),
    ));
    assert_eq!(
        adapter
            .translate(&tool_result(
                "toolu_6",
                json!("String not found"),
                true,
                None
            ))
            .len(),
        1
    );
}

#[test]
fn a_task_call_is_a_subagent_whose_items_name_it_as_parent() {
    let mut adapter = adapter();
    assert_eq!(
        adapter.translate(&assistant(
            "m9",
            None,
            json!([{ "type": "tool_use", "id": "toolu_7", "name": "Task",
                     "input": { "description": "Search the code", "subagent_type": "Explore",
                                "prompt": "find it" } }])
        )),
        [add(
            "tool:toolu_7",
            None,
            ChatItemBody::Subagent {
                tool_use_id: "toolu_7".to_owned(),
                description: "Search the code".to_owned(),
                agent_type: Some("Explore".to_owned()),
                status: SubagentStatus::Running,
            }
        )]
    );
    // The sub-agent's own tool call and text name its item.
    let ops = adapter.translate(&assistant(
        "m10",
        Some("toolu_7"),
        json!([{ "type": "text", "text": "Looking." },
               { "type": "tool_use", "id": "toolu_8", "name": "Grep", "input": { "pattern": "fn main" } }]),
    ));
    assert!(
        ops.iter().all(|op| matches!(op, JournalOp::Add { parent: Some(parent), .. } if parent == "tool:toolu_7")),
        "{ops:?}"
    );
    // Its permission request is the sub-agent's too.
    let request = adapter.translate(&frame(json!({
        "type": "control_request", "request_id": "r-9",
        "request": { "subtype": "can_use_tool", "tool_name": "Grep", "input": { "pattern": "fn main" },
                     "tool_use_id": "toolu_8" }
    })));
    assert!(
        matches!(request.as_slice(), [JournalOp::Add { parent: Some(parent), .. }] if parent == "tool:toolu_7"),
        "{request:?}"
    );
    // The Agent tool is a sub-agent too, and its result completes it.
    let agent = adapter.translate(&assistant(
        "m11",
        None,
        json!([{ "type": "tool_use", "id": "toolu_9", "name": "Agent", "input": { "description": "Review" } }]),
    ));
    assert!(matches!(
        agent.as_slice(),
        [JournalOp::Add {
            body: ChatItemBody::Subagent { .. },
            ..
        }]
    ));
    assert_eq!(
        adapter.translate(&tool_result("toolu_7", json!("found"), false, None)),
        [JournalOp::Replace {
            key: "tool:toolu_7".to_owned(),
            body: ChatItemBody::Subagent {
                tool_use_id: "toolu_7".to_owned(),
                description: "Search the code".to_owned(),
                agent_type: Some("Explore".to_owned()),
                status: SubagentStatus::Completed,
            }
        }]
    );
    // A task notification ends a sub-agent whose result never came.
    let ended = adapter.translate(&frame(json!({
        "type": "system", "subtype": "task_notification", "task_id": "t-1",
        "tool_use_id": "toolu_9", "status": "failed", "session_id": "s-1"
    })));
    assert!(
        matches!(
            ended.as_slice(),
            [JournalOp::Replace {
                body: ChatItemBody::Subagent {
                    status: SubagentStatus::Failed,
                    ..
                },
                ..
            }]
        ),
        "{ended:?}"
    );
}

#[test]
fn a_todo_list_is_a_plan_and_a_plan_file_names_its_document() {
    let mut adapter = adapter();
    assert_eq!(
        adapter.translate(&assistant(
            "m12",
            None,
            json!([{ "type": "tool_use", "id": "toolu_10", "name": "TodoWrite", "input": { "todos": [
                { "content": "Read the code", "status": "completed", "activeForm": "Reading" },
                { "content": "Fix the bug", "status": "in_progress", "activeForm": "Fixing" },
                { "content": "Test", "status": "pending", "activeForm": "Testing" },
                { "content": "Ship", "status": "blocked", "activeForm": "Shipping" }
            ]}}])
        )),
        [add(
            "tool:toolu_10",
            None,
            ChatItemBody::Plan {
                entries: [
                    ("Read the code", PlanStatus::Completed),
                    ("Fix the bug", PlanStatus::InProgress),
                    ("Test", PlanStatus::Pending),
                    ("Ship", PlanStatus::Unknown),
                ]
                .into_iter()
                .map(|(text, status)| PlanEntry {
                    text: text.to_owned(),
                    status
                })
                .collect(),
                document: None,
            }
        )]
    );
    assert_eq!(
        adapter.translate(&tool_result("toolu_10", json!("ok"), false, None)),
        []
    );
    let presented = adapter.translate(&assistant(
        "m13",
        None,
        json!([{ "type": "tool_use", "id": "toolu_11", "name": "ExitPlanMode",
                 "input": { "plan": "1. Fix", "planFilePath": "plans/fix.md" } }]),
    ));
    assert_eq!(
        presented[1],
        add(
            "plan:toolu_11",
            None,
            ChatItemBody::Plan {
                entries: Vec::new(),
                document: Some("plans/fix.md".to_owned())
            }
        )
    );
}

#[test]
fn names_paths_and_plan_entries_from_the_cli_are_bounded() {
    let mut adapter = adapter();
    let long_name = "x".repeat(5_000);
    for value in [
        json!({ "type": "system", "subtype": long_name.clone() }),
        json!({ "type": "system", "subtype": "permission_denied", "tool_name": long_name.clone() }),
        json!({ "type": long_name.clone() }),
    ] {
        let ops = adapter.translate(&frame(value));
        let [
            JournalOp::Add {
                body: ChatItemBody::Notice { text, .. },
                ..
            },
        ] = ops.as_slice()
        else {
            panic!("{ops:?}");
        };
        assert!(text.len() < 300, "{}", text.len());
    }
    let long_text = "y".repeat(MAX_SUMMARY_BYTES + 100);
    let todos: Vec<_> = (0..500)
        .map(|_| json!({ "content": long_text.clone(), "status": "pending" }))
        .collect();
    let ops = adapter.translate(&assistant(
        "m1",
        None,
        json!([
            { "type": "tool_use", "id": "toolu_1", "name": "TodoWrite", "input": { "todos": todos } },
            { "type": "tool_use", "id": "toolu_2", "name": "ExitPlanMode",
              "input": { "plan": "p", "planFilePath": long_text.clone() } },
            { "type": "tool_use", "id": "toolu_3", "name": long_name.clone(), "input": {} }
        ]),
    ));
    let JournalOp::Add {
        body: ChatItemBody::Plan { entries, .. },
        ..
    } = &ops[0]
    else {
        panic!("{ops:?}");
    };
    assert_eq!(entries.len(), 200);
    assert!(
        entries
            .iter()
            .all(|entry| entry.text.len() == MAX_SUMMARY_BYTES)
    );
    let JournalOp::Add {
        body: ChatItemBody::Plan {
            document: Some(document),
            ..
        },
        ..
    } = &ops[2]
    else {
        panic!("{ops:?}");
    };
    assert_eq!(document.len(), MAX_SUMMARY_BYTES);
    let JournalOp::Add {
        body: ChatItemBody::ToolUse(tool),
        ..
    } = &ops[3]
    else {
        panic!("{ops:?}");
    };
    assert_eq!(tool.name.chars().count(), 128);
}
