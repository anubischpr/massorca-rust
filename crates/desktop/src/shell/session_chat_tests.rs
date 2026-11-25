//! A chat session's view against a scripted service (decision 0101): what follows its transcript, what pages its
//! history, and what its composer and approvals send. Each test answers the calls by hand.

use super::session_view_tests::{Rig, chat_s1, draw, kind, s1, session_rig};
use super::*;
use crate::service::{ChatCommand, Command, Script, StoreUpdate};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::chat::{
    ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatItem, ChatItemBody,
    ChatSnapshot, MessageState,
};

fn cursor(seq: u64) -> ChatCursor {
    ChatCursor {
        epoch: "e1".to_owned(),
        seq,
    }
}

fn message(id: &str, position: u64, text: &str) -> ChatItem {
    ChatItem {
        id: id.to_owned(),
        position,
        seq: position,
        turn: None,
        parent: None,
        at_ms: 0,
        body: ChatItemBody::AssistantMessage {
            text: text.to_owned(),
            state: MessageState::Completed,
        },
    }
}

fn snapshot(session: &str, items: Vec<ChatItem>, has_older: bool) -> StoreUpdate {
    let head = items.last().map_or(0, |item| item.seq);
    StoreUpdate::Chat {
        session: session.to_owned(),
        event: ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(head),
            items,
            has_older,
            latest_turn: None,
            activity: None,
        })),
    }
}

/// Every chat call the shell sent since the last look, in order.
fn chat_calls(script: &mut Script) -> Vec<ChatCommand> {
    let mut calls = Vec::new();
    while let Some(command) = script.next() {
        if let Command::Chat(call) = command {
            calls.push(call);
        }
    }
    calls
}

fn push(rig: &Rig, cx: &mut VisualTestContext, update: StoreUpdate) {
    rig.updates.send(update).expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn showing_a_chat_session_follows_its_transcript_and_closing_the_view_stops(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);

    let calls = chat_calls(&mut rig.script);
    assert!(
        matches!(calls.as_slice(), [ChatCommand::Watch { session }] if session == "s1"),
        "the open chat is followed once: {calls:?}"
    );
    draw(cx);
    assert!(chat_calls(&mut rig.script).is_empty(), "a redraw asks nothing");

    rig.shell.update(cx, |shell, cx| {
        shell.model.close_session();
        cx.notify();
    });
    cx.run_until_parked();
    draw(cx);

    let calls = chat_calls(&mut rig.script);
    assert!(
        matches!(calls.as_slice(), [ChatCommand::Unwatch { session }] if session == "s1"),
        "closing the view stops the stream: {calls:?}"
    );
}

#[gpui::test]
fn a_terminal_session_follows_no_transcript(cx: &mut TestAppContext) {
    let (mut rig, _cx) = session_rig(cx, vec![kind(false)], vec![s1(false, None)]);

    assert!(chat_calls(&mut rig.script).is_empty());
}

#[gpui::test]
fn the_streams_snapshot_and_batches_draw_in_the_chat_view(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);

    push(
        &rig,
        cx,
        snapshot("s1", vec![message("a", 1, "Reading it now.")], false),
    );

    assert!(cx.debug_bounds("transcript-row-a").is_some());
}

fn history_calls(script: &mut Script) -> Vec<ChatHistoryParams> {
    chat_calls(script)
        .into_iter()
        .filter_map(|call| match call {
            ChatCommand::History { params, .. } => Some(params),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn older_items_are_asked_for_once_from_the_oldest_held_position_and_join_the_view(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
    push(
        &rig,
        cx,
        snapshot("s1", vec![message("c", 8, "three"), message("d", 9, "four")], true),
    );
    chat_calls(&mut rig.script);

    rig.shell.update(cx, |shell, cx| shell.load_older_chat("s1", cx));
    rig.shell.update(cx, |shell, cx| shell.load_older_chat("s1", cx));
    cx.run_until_parked();

    let mut asked = Vec::new();
    let mut reply = None;
    while let Some(command) = rig.script.next() {
        if let Command::Chat(ChatCommand::History { params, reply: sender }) = command {
            asked.push(params);
            reply = Some(sender);
        }
    }
    assert_eq!(asked.len(), 1, "a second ask waits for the first: {asked:?}");
    assert_eq!(asked[0].before, Some(cursor(8)), "before the oldest held item");
    let page = ChatHistoryPage {
        epoch: "e1".to_owned(),
        items: vec![message("a", 2, "one")],
        has_older: false,
    };
    // The bridge adds the page to the store, then answers the caller.
    rig.updates
        .send(StoreUpdate::ChatHistory {
            session: "s1".to_owned(),
            page: page.clone(),
        })
        .expect("the store listens");
    reply
        .expect("the history call is waiting")
        .send(Ok(page))
        .expect("the shell waits for the page");
    cx.run_until_parked();
    draw(cx);

    assert!(cx.debug_bounds("transcript-row-a").is_some());
    rig.shell.update(cx, |shell, cx| shell.load_older_chat("s1", cx));
    assert!(
        history_calls(&mut rig.script).is_empty(),
        "nothing older is left to ask for"
    );
}
