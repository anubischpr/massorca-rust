//! A chat session's transcript as the desktop holds it: the items of its journal by id in timeline order, changed the way
//! the stream's events change them (decision 0101, D3-D4).

use std::cmp::Ordering;

use massorca_protocol::methods::agents::Activity;
use massorca_protocol::methods::chat::{
    ChatBatch, ChatCursor, ChatEvent, ChatHistoryPage, ChatItem, ChatItemBody, ChatSnapshot,
    ChatUpdate, EndReason,
};

/// What the reader does after an event: keep the stream, or resubscribe after the last applied cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Keep,
    Resubscribe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Appended {
    Applied,
    Repeated,
    Gap,
}

/// The transcript of one chat session as far as its stream has delivered it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatTranscript {
    items: Vec<ChatItem>,
    has_older: bool,
    cursor: Option<ChatCursor>,
    latest_turn: Option<ChatItem>,
    activity: Option<Activity>,
    awaiting_snapshot: bool,
    ended: Option<EndReason>,
}

impl ChatTranscript {
    /// The held items, oldest first by `position`.
    pub fn items(&self) -> &[ChatItem] {
        &self.items
    }

    /// Items older than the oldest one held exist; `agent.chat.history` pages them.
    pub fn has_older(&self) -> bool {
        self.has_older
    }

    /// Where the next page of older items starts: the oldest held item's position in the journal's epoch.
    pub fn history_cursor(&self) -> Option<ChatCursor> {
        let epoch = self.cursor.as_ref()?.epoch.clone();
        let oldest = self.items.first()?;
        Some(ChatCursor {
            epoch,
            seq: oldest.position,
        })
    }

    pub fn item(&self, id: &str) -> Option<&ChatItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// The position the stream last confirmed; a resubscribe starts after it.
    pub fn cursor(&self) -> Option<&ChatCursor> {
        self.cursor.as_ref()
    }

    /// The session's newest turn, which a snapshot may name even when the page does not hold it.
    pub fn latest_turn(&self) -> Option<&ChatItem> {
        self.latest_turn.as_ref()
    }

    pub fn activity(&self) -> Option<&Activity> {
        self.activity.as_ref()
    }

    /// True after a reset until the snapshot that follows it.
    pub fn awaiting_snapshot(&self) -> bool {
        self.awaiting_snapshot
    }

    pub fn ended(&self) -> Option<EndReason> {
        self.ended
    }

    /// Applies one stream event. A gap in an append asks for a resubscribe; the cursor keeps the last applied batch.
    pub fn apply(&mut self, event: ChatEvent) -> Next {
        if self.ended.is_some() {
            return Next::Keep;
        }
        match event {
            ChatEvent::Snapshot(snapshot) => {
                self.replace(*snapshot);
                Next::Keep
            }
            ChatEvent::Batch(batch) => self.apply_batch(batch),
            ChatEvent::Reset { .. } => {
                *self = Self {
                    awaiting_snapshot: true,
                    ..Self::default()
                };
                Next::Keep
            }
            ChatEvent::End { reason } => {
                self.ended = Some(reason);
                Next::Keep
            }
            ChatEvent::Unknown => Next::Keep,
        }
    }

    /// Adds a page of older items. A page from another epoch than the stream's is dropped: the stream resets, and its
    /// snapshot replaces everything.
    pub fn prepend(&mut self, page: ChatHistoryPage) {
        if self.cursor.as_ref().map(|cursor| &cursor.epoch) != Some(&page.epoch) {
            return;
        }
        for item in page.items {
            self.put(item);
        }
        self.has_older = page.has_older;
    }

    fn replace(&mut self, snapshot: ChatSnapshot) {
        self.items.clear();
        self.has_older = snapshot.has_older;
        self.latest_turn = None;
        for item in snapshot.items {
            self.put(item);
        }
        self.latest_turn = snapshot
            .latest_turn
            .filter(|turn| matches!(turn.body, ChatItemBody::Turn(_)));
        self.cursor = Some(snapshot.cursor);
        self.activity = snapshot.activity;
        self.awaiting_snapshot = false;
    }

    fn apply_batch(&mut self, batch: ChatBatch) -> Next {
        // Between a reset and its snapshot a batch is not relative to anything held, so it is dropped.
        if self.awaiting_snapshot {
            return Next::Keep;
        }
        for update in batch.updates {
            match update {
                ChatUpdate::Item { item } => self.put(item),
                ChatUpdate::Append {
                    id,
                    seq,
                    offset,
                    text,
                } => match self.append(&id, seq, offset, &text) {
                    Appended::Applied | Appended::Repeated => {}
                    Appended::Gap => return Next::Resubscribe,
                },
                // An update from a newer peer names no item, so only a resubscribe can bring its state back.
                ChatUpdate::Unknown => return Next::Resubscribe,
            }
        }
        self.cursor = Some(batch.cursor);
        if let Some(activity) = batch.activity {
            self.activity = Some(activity);
        }
        Next::Keep
    }

    /// Replaces the held item with the same id, or adds the item at its position. Unknown types are skipped.
    fn put(&mut self, item: ChatItem) {
        if matches!(item.body, ChatItemBody::Unknown) {
            return;
        }
        if let Some(turn) = self.latest_turn.as_mut().filter(|turn| turn.id == item.id) {
            *turn = item.clone();
        }
        match self.items.iter().position(|held| held.id == item.id) {
            Some(index) if self.items[index].position == item.position => self.items[index] = item,
            Some(index) => {
                self.items.remove(index);
                self.insert(item);
            }
            None => self.insert(item),
        }
    }

    fn insert(&mut self, item: ChatItem) {
        let index = self
            .items
            .partition_point(|held| held.position <= item.position);
        self.items.insert(index, item);
    }

    /// Appends `text` when the held text is exactly `offset` bytes long. A longer held text is a repeat; a shorter one,
    /// or an item that holds no text, is a gap.
    fn append(&mut self, id: &str, seq: u64, offset: u64, text: &str) -> Appended {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return Appended::Gap;
        };
        let held = match &mut item.body {
            ChatItemBody::AssistantMessage { text: held, .. }
            | ChatItemBody::Thinking { text: held, .. } => held,
            _ => return Appended::Gap,
        };
        match (held.len() as u64).cmp(&offset) {
            Ordering::Equal => {
                held.push_str(text);
                item.seq = seq;
                Appended::Applied
            }
            Ordering::Greater => Appended::Repeated,
            Ordering::Less => Appended::Gap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_protocol::methods::agents::ActivityState;
    use massorca_protocol::methods::chat::{MessageState, ResetReason, Turn, TurnState};

    fn cursor(seq: u64) -> ChatCursor {
        ChatCursor {
            seq,
            ..crate::service::fixtures::base_chat_cursor()
        }
    }

    fn item(id: &str, position: u64, body: ChatItemBody) -> ChatItem {
        ChatItem {
            id: id.to_owned(),
            position,
            seq: position,
            body,
            ..crate::service::fixtures::base_chat_item()
        }
    }

    fn prose(id: &str, position: u64, text: &str, state: MessageState) -> ChatItem {
        item(
            id,
            position,
            ChatItemBody::AssistantMessage {
                text: text.to_owned(),
                state,
            },
        )
    }

    fn turn_item(id: &str, position: u64, state: TurnState) -> ChatItem {
        item(
            id,
            position,
            ChatItemBody::Turn(Turn {
                state,
                user_item: None,
                completed_at_ms: None,
                outcome: None,
            }),
        )
    }

    fn user(id: &str, position: u64, text: &str) -> ChatItem {
        item(
            id,
            position,
            ChatItemBody::UserMessage {
                text: text.to_owned(),
            },
        )
    }

    fn snapshot(seq: u64, items: Vec<ChatItem>) -> ChatEvent {
        ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(seq),
            items,
            ..crate::service::fixtures::base_chat_snapshot()
        }))
    }

    fn batch(seq: u64, updates: Vec<ChatUpdate>) -> ChatEvent {
        ChatEvent::Batch(ChatBatch {
            cursor: cursor(seq),
            updates,
            ..crate::service::fixtures::base_chat_batch()
        })
    }

    fn append(id: &str, seq: u64, offset: u64, text: &str) -> ChatUpdate {
        ChatUpdate::Append {
            id: id.to_owned(),
            seq,
            offset,
            text: text.to_owned(),
        }
    }

    fn prose_text(transcript: &ChatTranscript, id: &str) -> String {
        match transcript.item(id).map(|item| &item.body) {
            Some(ChatItemBody::AssistantMessage { text, .. }) => text.clone(),
            other => panic!("expected held prose for {id}, found {other:?}"),
        }
    }

    fn ids(transcript: &ChatTranscript) -> Vec<&str> {
        transcript
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect()
    }

    #[test]
    fn a_snapshot_replaces_what_was_held_and_orders_items_by_position() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("old", 1, "stale")]));
        let next = transcript.apply(snapshot(
            5,
            vec![
                prose("answer", 4, "second", MessageState::Completed),
                user("ask", 2, "first"),
            ],
        ));

        assert_eq!(next, Next::Keep);
        assert_eq!(ids(&transcript), ["ask", "answer"]);
        assert_eq!(transcript.cursor(), Some(&cursor(5)));
        assert!(transcript.item("old").is_none());
    }

    #[test]
    fn a_snapshot_names_its_latest_turn_even_when_the_page_does_not_hold_it() {
        let turn = turn_item("turn-1", 2, TurnState::Running);
        let mut transcript = ChatTranscript::default();
        transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(9),
            items: vec![user("ask", 8, "later")],
            has_older: true,
            latest_turn: Some(turn.clone()),
            ..crate::service::fixtures::base_chat_snapshot()
        })));

        assert_eq!(transcript.latest_turn(), Some(&turn));
        assert!(transcript.item("turn-1").is_none());
    }

    #[test]
    fn a_batch_adds_an_item_and_replaces_one_with_the_same_id_in_place() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![
                user("ask", 1, "hello"),
                prose("answer", 3, "draft", MessageState::Running),
            ],
        ));
        transcript.apply(batch(
            2,
            vec![
                ChatUpdate::Item {
                    item: prose("answer", 3, "final", MessageState::Completed),
                },
                ChatUpdate::Item {
                    item: user("next", 4, "again"),
                },
            ],
        ));

        assert_eq!(ids(&transcript), ["ask", "answer", "next"]);
        assert_eq!(prose_text(&transcript, "answer"), "final");
        assert_eq!(transcript.cursor(), Some(&cursor(2)));
    }

    #[test]
    fn a_replaced_item_that_moves_is_placed_by_its_position() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![user("a", 1, "a"), user("b", 2, "b"), user("c", 3, "c")],
        ));
        transcript.apply(batch(
            2,
            vec![ChatUpdate::Item {
                item: user("a", 5, "a"),
            }],
        ));

        assert_eq!(ids(&transcript), ["b", "c", "a"]);
    }

    #[test]
    fn an_item_of_an_unknown_type_is_skipped_and_never_held() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));
        transcript.apply(batch(
            2,
            vec![ChatUpdate::Item {
                item: item("future", 2, ChatItemBody::Unknown),
            }],
        ));
        transcript.apply(batch(
            3,
            vec![ChatUpdate::Item {
                item: item("ask", 1, ChatItemBody::Unknown),
            }],
        ));

        assert_eq!(ids(&transcript), ["ask"]);
    }

    #[test]
    fn an_append_at_the_held_length_extends_the_text_and_sets_its_seq() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![prose("answer", 1, "Hel", MessageState::Running)],
        ));
        let next = transcript.apply(batch(2, vec![append("answer", 7, 3, "lo")]));

        assert_eq!(next, Next::Keep);
        assert_eq!(prose_text(&transcript, "answer"), "Hello");
        assert_eq!(transcript.item("answer").map(|item| item.seq), Some(7));
    }

    #[test]
    fn an_append_counts_its_offset_in_bytes_not_characters() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![prose("answer", 1, "é", MessageState::Running)],
        ));
        let next = transcript.apply(batch(2, vec![append("answer", 2, 2, "!")]));

        assert_eq!(next, Next::Keep);
        assert_eq!(prose_text(&transcript, "answer"), "é!");
    }

    #[test]
    fn an_append_the_held_text_already_has_is_a_repeat_and_changes_nothing() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![prose("answer", 1, "Hello", MessageState::Running)],
        ));
        let next = transcript.apply(batch(2, vec![append("answer", 3, 3, "lo")]));

        assert_eq!(next, Next::Keep);
        assert_eq!(prose_text(&transcript, "answer"), "Hello");
        assert_eq!(transcript.item("answer").map(|item| item.seq), Some(1));
    }

    #[test]
    fn applying_the_same_append_twice_converges_on_one_copy_of_the_text() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![prose("answer", 1, "Hi", MessageState::Running)],
        ));
        transcript.apply(batch(2, vec![append("answer", 2, 2, " there")]));
        transcript.apply(batch(2, vec![append("answer", 2, 2, " there")]));

        assert_eq!(prose_text(&transcript, "answer"), "Hi there");
    }

    #[test]
    fn an_append_past_the_held_text_asks_for_a_resubscribe_and_keeps_the_cursor() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            3,
            vec![prose("answer", 1, "He", MessageState::Running)],
        ));
        let next = transcript.apply(batch(4, vec![append("answer", 4, 5, "llo")]));

        assert_eq!(next, Next::Resubscribe);
        assert_eq!(prose_text(&transcript, "answer"), "He");
        assert_eq!(transcript.cursor(), Some(&cursor(3)));
    }

    #[test]
    fn an_append_to_a_missing_or_non_text_item_asks_for_a_resubscribe() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));

        assert_eq!(
            transcript.apply(batch(2, vec![append("missing", 2, 0, "x")])),
            Next::Resubscribe
        );
        assert_eq!(
            transcript.apply(batch(2, vec![append("ask", 2, 2, "x")])),
            Next::Resubscribe
        );
    }

    #[test]
    fn a_gap_stops_its_batch_so_later_updates_wait_for_the_resubscribe() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(
            1,
            vec![prose("answer", 1, "He", MessageState::Running)],
        ));
        let next = transcript.apply(batch(
            2,
            vec![
                append("answer", 2, 9, "x"),
                ChatUpdate::Item {
                    item: user("after", 2, "later"),
                },
            ],
        ));

        assert_eq!(next, Next::Resubscribe);
        assert!(transcript.item("after").is_none());
        assert_eq!(transcript.cursor(), Some(&cursor(1)));
    }

    #[test]
    fn a_batch_applied_twice_converges() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));
        let update = ChatUpdate::Item {
            item: user("next", 2, "again"),
        };
        transcript.apply(batch(2, vec![update.clone()]));
        let once = transcript.clone();
        transcript.apply(batch(2, vec![update]));

        assert_eq!(transcript.items(), once.items());
        assert_eq!(ids(&transcript), ["ask", "next"]);
    }

    #[test]
    fn a_reset_clears_the_transcript_and_holds_batches_until_the_snapshot() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));
        let next = transcript.apply(ChatEvent::Reset {
            reason: ResetReason::TooOld,
        });

        assert_eq!(next, Next::Keep);
        assert!(transcript.items().is_empty());
        assert!(transcript.cursor().is_none());
        assert!(transcript.awaiting_snapshot());

        transcript.apply(batch(
            5,
            vec![ChatUpdate::Item {
                item: user("stray", 5, "x"),
            }],
        ));
        assert!(transcript.items().is_empty());

        transcript.apply(snapshot(6, vec![user("fresh", 6, "new")]));
        assert!(!transcript.awaiting_snapshot());
        assert_eq!(ids(&transcript), ["fresh"]);
    }

    #[test]
    fn an_end_closes_the_transcript_and_later_events_change_nothing() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));
        transcript.apply(ChatEvent::End {
            reason: EndReason::Forgotten,
        });
        transcript.apply(batch(
            2,
            vec![ChatUpdate::Item {
                item: user("late", 2, "x"),
            }],
        ));

        assert_eq!(transcript.ended(), Some(EndReason::Forgotten));
        assert_eq!(ids(&transcript), ["ask"]);
    }

    #[test]
    fn an_unknown_event_changes_nothing() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));
        let before = transcript.clone();

        assert_eq!(transcript.apply(ChatEvent::Unknown), Next::Keep);
        assert_eq!(transcript, before);
    }

    #[test]
    fn an_unknown_update_asks_for_a_resubscribe_because_it_names_no_item() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(1, vec![user("ask", 1, "hi")]));

        let next = transcript.apply(batch(2, vec![ChatUpdate::Unknown]));

        assert_eq!(next, Next::Resubscribe);
        assert_eq!(transcript.cursor(), Some(&cursor(1)));
    }

    #[test]
    fn a_batch_keeps_the_activity_unless_it_carries_a_new_one() {
        let working = activity(ActivityState::Working);
        let mut transcript = ChatTranscript::default();
        transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(1),
            activity: Some(working.clone()),
            ..crate::service::fixtures::base_chat_snapshot()
        })));
        transcript.apply(batch(2, Vec::new()));
        assert_eq!(transcript.activity(), Some(&working));

        let waiting = activity(ActivityState::Waiting);
        transcript.apply(ChatEvent::Batch(ChatBatch {
            cursor: cursor(3),
            activity: Some(waiting.clone()),
            ..crate::service::fixtures::base_chat_batch()
        }));
        assert_eq!(transcript.activity(), Some(&waiting));
    }

    #[test]
    fn a_later_turn_update_keeps_the_latest_turn_current() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(2),
            items: vec![turn_item("turn-1", 2, TurnState::Running)],
            latest_turn: Some(turn_item("turn-1", 2, TurnState::Running)),
            ..crate::service::fixtures::base_chat_snapshot()
        })));
        transcript.apply(batch(
            3,
            vec![ChatUpdate::Item {
                item: turn_item("turn-1", 2, TurnState::Completed),
            }],
        ));

        assert_eq!(
            transcript.latest_turn(),
            Some(&turn_item("turn-1", 2, TurnState::Completed))
        );
    }

    fn activity(state: ActivityState) -> Activity {
        Activity {
            state,
            ..crate::service::fixtures::base_activity()
        }
    }

    fn older_page(epoch: &str, items: Vec<ChatItem>, has_older: bool) -> ChatHistoryPage {
        ChatHistoryPage {
            epoch: epoch.to_owned(),
            items,
            has_older,
        }
    }

    #[test]
    fn an_older_page_goes_before_the_held_items_and_moves_the_paging_cursor() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(9),
            items: vec![user("c", 8, "three"), user("d", 9, "four")],
            has_older: true,
            ..crate::service::fixtures::base_chat_snapshot()
        })));
        assert!(transcript.has_older());
        assert_eq!(transcript.history_cursor(), Some(cursor(8)));

        transcript.prepend(older_page(
            "epoch-1",
            vec![user("a", 2, "one"), user("b", 5, "two")],
            false,
        ));

        let ids: Vec<&str> = transcript
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b", "c", "d"]);
        assert!(!transcript.has_older());
        assert_eq!(transcript.history_cursor(), Some(cursor(2)));
    }

    #[test]
    fn an_older_page_of_another_epoch_changes_nothing() {
        let mut transcript = ChatTranscript::default();
        transcript.apply(snapshot(9, vec![user("c", 8, "three")]));

        transcript.prepend(older_page("epoch-2", vec![user("a", 2, "one")], false));

        assert_eq!(transcript.items().len(), 1);
    }
}
