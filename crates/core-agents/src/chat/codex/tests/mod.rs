//! Unit tests of the Codex adapter, one file per part of the dialect, over the helpers here.

mod items;
mod launch;
mod requests;
mod turns;

use massorca_protocol::methods::chat::{Approval, ChatItemBody};
use serde_json::{Value, json};

use super::CodexAdapter;
use crate::chat::adapter::{ChatAdapter, JournalOp};

/// The approval item `ops` add, with its journal key.
pub(super) fn approval_added(ops: &[JournalOp]) -> Option<(Option<String>, Approval)> {
    ops.iter().find_map(|op| match op {
        JournalOp::Add {
            key,
            body: ChatItemBody::Approval(approval),
            ..
        } => Some((key.clone(), approval.clone())),
        _ => None,
    })
}

pub(super) fn lines(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

pub(super) fn sent(ops: &[JournalOp]) -> Vec<Value> {
    ops.iter()
        .flat_map(|op| match op {
            JournalOp::Send { bytes } => lines(bytes),
            _ => Vec::new(),
        })
        .collect()
}

pub(super) fn feed(adapter: &mut CodexAdapter, message: Value) -> Vec<JournalOp> {
    adapter.translate(&message.to_string())
}

/// An adapter whose thread `thr-1` is open, with the opening's ops.
pub(super) fn opened(planned: &[&str]) -> (CodexAdapter, Vec<JournalOp>) {
    let mut adapter = CodexAdapter::new();
    adapter.launch_args(planned.iter().map(|&arg| arg.to_owned()).collect());
    let opening = lines(&adapter.opening());
    assert_eq!(opening[0]["method"], "initialize");
    feed(
        &mut adapter,
        json!({ "id": opening[0]["id"], "result": {} }),
    );
    let ops = feed(
        &mut adapter,
        json!({ "id": 2, "result": { "thread": { "id": "thr-1" } } }),
    );
    (adapter, ops)
}

pub(super) fn notify(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params })
}
