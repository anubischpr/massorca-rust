//! The transcript journal of chat sessions (decision 0101, D3-D5), over the store. A writer applies one line's
//! [`JournalOp`]s as one batch; the readers build the snapshot, the replay after a cursor and history pages.
//!
//! Every change takes the journal's next `seq`. An item keeps the `seq` it was added at as its `position`. The text of an
//! `assistant_message` or `thinking` is stored apart from its body, so an append writes only the text and the stream gets
//! an `append` update with the byte offset it starts at; a replay sends the item's whole state instead.

use std::collections::HashMap;

use massorca_core_store::{ChatItemRow, NewChatItem, Store, StoreError, now_ms};
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ChatBatch, ChatCursor, ChatItem, ChatItemBody, ChatUpdate, MessageState, ResetReason, Turn,
    TurnState,
};
use serde_json::Value;

use super::MAX_JOURNAL_ITEMS;
use super::adapter::JournalOp;

/// What a session's journal writer remembers while its run lives: the epoch, the open turn, and the item id of each key
/// the adapter named.
#[derive(Debug, Clone, Default)]
pub(crate) struct JournalState {
    pub(crate) epoch: String,
    pub(crate) open_turn: Option<String>,
    keys: HashMap<String, String>,
}

impl JournalState {
    /// Opens the session's journal (a new epoch when it has none) and reads back its open turn: the newest turn item,
    /// when it is still running.
    pub(crate) fn open(store: &Store, session: &str) -> Result<Self, StoreError> {
        let head = store.open_chat_journal(session, &uuid::Uuid::now_v7().to_string())?;
        let open_turn = store
            .latest_chat_item_of_type(session, "turn")?
            .map(item_of)
            .filter(|item| {
                matches!(&item.body, ChatItemBody::Turn(turn) if turn.state == TurnState::Running)
            })
            .map(|item| item.id);
        Ok(Self {
            epoch: head.epoch,
            open_turn,
            keys: HashMap::new(),
        })
    }
}

/// Splits a body into its type, its JSON without the growing text, and that text.
fn split(body: &ChatItemBody) -> Result<(String, String, Option<String>), serde_json::Error> {
    let mut value = serde_json::to_value(body)?;
    let item_type = value["type"].as_str().unwrap_or_default().to_owned();
    let text = match body {
        ChatItemBody::AssistantMessage { .. } | ChatItemBody::Thinking { .. } => value
            .as_object_mut()
            .and_then(|object| object.remove("text"))
            .and_then(|text| text.as_str().map(str::to_owned)),
        _ => None,
    };
    Ok((item_type, value.to_string(), text))
}

/// The wire item of a stored row. A body this build cannot read is `unknown`, which readers skip.
pub(crate) fn item_of(row: ChatItemRow) -> ChatItem {
    let mut value: Value = serde_json::from_str(&row.body).unwrap_or(Value::Null);
    if let (Some(object), Some(text)) = (value.as_object_mut(), row.text) {
        object.insert("text".to_owned(), Value::String(text));
    }
    ChatItem {
        id: row.id,
        position: row.position,
        seq: row.seq,
        turn: row.turn,
        parent: row.parent,
        at_ms: row.at_ms,
        body: serde_json::from_value(value).unwrap_or(ChatItemBody::Unknown),
    }
}

fn store_error(error: serde_json::Error) -> StoreError {
    StoreError::Io(std::io::Error::other(error))
}

/// Applies changes to one session's journal and collects them as one batch.
pub(crate) struct JournalWriter<'a> {
    store: &'a Store,
    session: &'a str,
    state: &'a mut JournalState,
    updates: Vec<ChatUpdate>,
    seq: Option<u64>,
}

impl<'a> JournalWriter<'a> {
    pub(crate) fn new(store: &'a Store, session: &'a str, state: &'a mut JournalState) -> Self {
        Self {
            store,
            session,
            state,
            updates: Vec::new(),
            seq: None,
        }
    }

    /// The batch of every change applied, or `None` when nothing changed.
    pub(crate) fn finish(self) -> Option<ChatBatch> {
        let seq = self.seq?;
        Some(ChatBatch {
            cursor: ChatCursor {
                epoch: self.state.epoch.clone(),
                seq,
            },
            updates: self.updates,
            activity: None,
        })
    }

    fn changed(&mut self, item: ChatItem) {
        self.seq = Some(item.seq);
        self.updates.push(ChatUpdate::Item { item });
    }

    /// Adds an item with a new id under `turn`.
    fn add(
        &mut self,
        id: &str,
        turn: Option<&str>,
        parent: Option<&str>,
        body: &ChatItemBody,
    ) -> Result<Option<ChatItem>, StoreError> {
        let (item_type, body, text) = split(body).map_err(store_error)?;
        let added = self.store.add_chat_item(
            self.session,
            &NewChatItem {
                id,
                turn,
                parent,
                at_ms: now_ms(),
                item_type: &item_type,
                body: &body,
                text: text.as_deref(),
            },
            MAX_JOURNAL_ITEMS,
        )?;
        let item = added.map(item_of);
        if let Some(item) = &item {
            self.changed(item.clone());
        }
        Ok(item)
    }

    fn replace(&mut self, id: &str, body: &ChatItemBody) -> Result<(), StoreError> {
        let (_, body, text) = split(body).map_err(store_error)?;
        if let Some(row) = self
            .store
            .replace_chat_item(self.session, id, &body, text.as_deref())?
        {
            self.changed(item_of(row));
        }
        Ok(())
    }

    /// Opens a turn for the person's `text`: a running turn item, then the `user_message` that opened it. Returns the
    /// turn id. A turn still open is ended `unverifiable` first: nothing says how it ended.
    pub(crate) fn start_turn(&mut self, text: &str) -> Result<String, StoreError> {
        if self.state.open_turn.is_some() {
            self.end_turn(TurnState::Unverifiable, None)?;
        }
        let turn_id = uuid::Uuid::now_v7().to_string();
        let user_id = uuid::Uuid::now_v7().to_string();
        let turn = ChatItemBody::Turn(Turn {
            state: TurnState::Running,
            outcome: None,
            user_item: Some(user_id.clone()),
            completed_at_ms: None,
        });
        self.add(&turn_id, None, None, &turn)?;
        self.add(
            &user_id,
            Some(&turn_id),
            None,
            &ChatItemBody::UserMessage {
                text: text.to_owned(),
            },
        )?;
        self.state.open_turn = Some(turn_id.clone());
        Ok(turn_id)
    }

    /// Ends the open turn: every running message of it completes, then the turn takes `state` and `outcome`.
    pub(crate) fn end_turn(
        &mut self,
        state: TurnState,
        outcome: Option<Outcome>,
    ) -> Result<(), StoreError> {
        let Some(turn_id) = self.state.open_turn.take() else {
            return Ok(());
        };
        for item_type in ["assistant_message", "thinking"] {
            for row in self
                .store
                .chat_turn_items(self.session, &turn_id, item_type)?
            {
                let item = item_of(row);
                let completed = match item.body {
                    ChatItemBody::AssistantMessage {
                        text,
                        state: MessageState::Running,
                    } => ChatItemBody::AssistantMessage {
                        text,
                        state: MessageState::Completed,
                    },
                    ChatItemBody::Thinking {
                        text,
                        state: MessageState::Running,
                    } => ChatItemBody::Thinking {
                        text,
                        state: MessageState::Completed,
                    },
                    _ => continue,
                };
                self.replace(&item.id, &completed)?;
            }
        }
        if let Some(row) = self.store.chat_item(self.session, &turn_id)?
            && let ChatItemBody::Turn(turn) = item_of(row).body
        {
            let ended = ChatItemBody::Turn(Turn {
                state,
                outcome,
                completed_at_ms: Some(now_ms()),
                ..turn
            });
            self.replace(&turn_id, &ended)?;
        }
        Ok(())
    }

    /// Applies one adapter op.
    pub(crate) fn apply(&mut self, op: JournalOp) -> Result<(), StoreError> {
        match op {
            JournalOp::Add { key, parent, body } => {
                if let Some(id) = key.as_ref().and_then(|key| self.state.keys.get(key)) {
                    let id = id.clone();
                    return self.replace(&id, &body);
                }
                let parent = parent.and_then(|parent| self.state.keys.get(&parent).cloned());
                let id = uuid::Uuid::now_v7().to_string();
                let turn = self.state.open_turn.clone();
                if self
                    .add(&id, turn.as_deref(), parent.as_deref(), &body)?
                    .is_some()
                    && let Some(key) = key
                {
                    self.state.keys.insert(key, id);
                }
                Ok(())
            }
            JournalOp::Replace { key, body } => match self.state.keys.get(&key).cloned() {
                Some(id) => self.replace(&id, &body),
                None => Ok(()),
            },
            JournalOp::Append { key, text } => {
                let Some(id) = self.state.keys.get(&key).cloned() else {
                    return Ok(());
                };
                if let Some(appended) =
                    self.store.append_chat_item_text(self.session, &id, &text)?
                {
                    self.seq = Some(appended.seq);
                    self.updates.push(ChatUpdate::Append {
                        id,
                        seq: appended.seq,
                        offset: appended.offset,
                        text,
                    });
                }
                Ok(())
            }
            JournalOp::EndTurn { state, outcome } => self.end_turn(state, outcome),
        }
    }
}

/// The newest page of a session's journal, the cursor of its newest change, and whether older items exist.
pub(crate) struct Page {
    pub(crate) cursor: ChatCursor,
    pub(crate) items: Vec<ChatItem>,
    pub(crate) has_older: bool,
    pub(crate) latest_turn: Option<ChatItem>,
}

/// The newest `limit` items, with the newest turn item.
pub(crate) fn newest_page(
    store: &Store,
    session: &str,
    epoch: &str,
    limit: u32,
) -> Result<Page, StoreError> {
    let head = store.chat_journal_head(session)?;
    let (items, has_older) = store.chat_items_before(session, None, limit)?;
    let latest_turn = store
        .latest_chat_item_of_type(session, "turn")?
        .map(item_of);
    Ok(Page {
        cursor: ChatCursor {
            epoch: head
                .as_ref()
                .map_or_else(|| epoch.to_owned(), |head| head.epoch.clone()),
            seq: head.map_or(0, |head| head.seq),
        },
        items: items.into_iter().map(item_of).collect(),
        has_older,
        latest_turn,
    })
}

/// Everything changed after `after` as one batch of whole items, or why it cannot be replayed.
pub(crate) fn replay(
    store: &Store,
    session: &str,
    after: &ChatCursor,
) -> Result<Result<ChatBatch, ResetReason>, StoreError> {
    let Some(head) = store.chat_journal_head(session)? else {
        return Ok(Err(ResetReason::EpochChanged));
    };
    // A cursor this journal never reached was not issued by it, as one from another epoch.
    if after.epoch != head.epoch || after.seq > head.seq {
        return Ok(Err(ResetReason::EpochChanged));
    }
    if after.seq < head.pruned_seq {
        return Ok(Err(ResetReason::TooOld));
    }
    let changed =
        store.chat_items_changed_after(session, after.seq, super::MAX_REPLAY_ITEMS + 1)?;
    if changed.len() > super::MAX_REPLAY_ITEMS as usize {
        return Ok(Err(ResetReason::TooOld));
    }
    Ok(Ok(ChatBatch {
        cursor: ChatCursor {
            epoch: head.epoch,
            seq: head.seq,
        },
        updates: changed
            .into_iter()
            .map(|row| ChatUpdate::Item { item: item_of(row) })
            .collect(),
        activity: None,
    }))
}

