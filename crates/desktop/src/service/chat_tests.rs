//! The store's chat transcripts (decision 0101): the stream's events and history, and what a closed or refused stream
//! leaves.

use std::time::Instant;

use massorca_protocol::codes;
use massorca_protocol::methods::chat::{
    ChatBatch, ChatCursor, ChatEvent, ChatHistoryPage, ChatItem, ChatItemBody, ChatSnapshot,
    ChatUpdate, MessageState,
};

use super::{Refusal, StoreState, StoreUpdate};

fn refused(code: &str, message: &str) -> Refusal {
    Refusal::Failed {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

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
            state: MessageState::Running,
        },
    }
}

fn snapshot(seq: u64, items: Vec<ChatItem>, has_older: bool) -> ChatEvent {
    ChatEvent::Snapshot(Box::new(ChatSnapshot {
        cursor: cursor(seq),
        items,
        has_older,
        latest_turn: None,
        activity: None,
    }))
}

fn chat(session: &str, event: ChatEvent) -> StoreUpdate {
    StoreUpdate::Chat {
        session: session.to_owned(),
        event,
    }
}

#[test]
fn the_streams_events_build_each_sessions_transcript_in_the_store() {
    let now = Instant::now();
    let mut state = StoreState::default();
    state.apply(
        chat("s1", snapshot(1, vec![message("a", 1, "Hel")], false)),
        now,
    );
    state.apply(
        chat(
            "s1",
            ChatEvent::Batch(ChatBatch {
                cursor: cursor(2),
                updates: vec![ChatUpdate::Append {
                    id: "a".to_owned(),
                    seq: 2,
                    offset: 3,
                    text: "lo".to_owned(),
                }],
                activity: None,
            }),
        ),
        now,
    );
    state.apply(
        chat("s2", snapshot(1, vec![message("b", 1, "Other")], false)),
        now,
    );

    let held = &state.chat["s1"];
    assert!(matches!(
        &held.items()[0].body,
        ChatItemBody::AssistantMessage { text, .. } if text == "Hello"
    ));
    assert_eq!(held.cursor(), Some(&cursor(2)));
    assert_eq!(
        state.chat["s2"].items().len(),
        1,
        "each session has its own"
    );
}

#[test]
fn a_page_of_history_goes_before_the_items_the_store_holds() {
    let now = Instant::now();
    let mut state = StoreState::default();
    state.apply(
        chat("s1", snapshot(9, vec![message("c", 8, "three")], true)),
        now,
    );

    state.apply(
        StoreUpdate::ChatHistory {
            session: "s1".to_owned(),
            page: ChatHistoryPage {
                epoch: "e1".to_owned(),
                items: vec![message("a", 2, "one")],
                has_older: false,
            },
        },
        now,
    );

    let ids: Vec<&str> = state.chat["s1"]
        .items()
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(ids, ["a", "c"]);
    assert!(!state.chat["s1"].has_older());
}

#[test]
fn closing_a_session_drops_its_transcript_and_a_refusal_is_kept_until_events_arrive() {
    let now = Instant::now();
    let mut state = StoreState::default();
    state.apply(
        chat("s1", snapshot(1, vec![message("a", 1, "x")], false)),
        now,
    );
    state.apply(StoreUpdate::ChatClosed("s1".to_owned()), now);
    assert!(!state.chat.contains_key("s1"));

    let refusal = refused(codes::AGENT_NOT_CHAT, "Not a chat");
    state.apply(
        StoreUpdate::ChatRefused {
            session: "s1".to_owned(),
            refusal: refusal.clone(),
        },
        now,
    );
    assert_eq!(state.chat_refused.get("s1"), Some(&refusal));

    state.apply(chat("s1", snapshot(1, Vec::new(), false)), now);
    assert!(state.chat_refused.is_empty());
}
