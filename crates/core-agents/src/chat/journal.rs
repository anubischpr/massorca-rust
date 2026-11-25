//! The transcript journal of chat sessions (decision 0101, D3-D5), over the store. A writer applies one line's
//! [`JournalOp`]s as one batch; the readers build the snapshot, the replay after a cursor and history pages.
//!
//! Every change takes the journal's next `seq`. An item keeps the `seq` it was added at as its `position`. The text of an
//! `assistant_message` or `thinking` is stored apart from its body, so an append writes only the text and the stream gets
//! an `append` update with the byte offset it starts at; a replay sends the item's whole state instead.

use std::collections::{BTreeSet, HashMap};

use massorca_core_store::{ChatItemRow, NewChatItem, Store, StoreError, now_ms};
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    Approval, ApprovalScope, ApprovalState, ChatBatch, ChatCursor, ChatItem, ChatItemBody,
    ChatUpdate, MessageState, ResetReason, SubagentStatus, ToolStatus, Turn, TurnState,
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
    /// The turns of prompts handed to the CLI while a turn ran (a forced prompt, decision 0101, D7), oldest first. They
    /// end with the open turn: a `result` closes them all (the CLI may fold them into the running turn), an interrupt
    /// interrupts them (it cancels the CLI's queued messages), and the run's end ends them.
    held_turns: Vec<String>,
    keys: HashMap<String, String>,
    /// The approval items still pending (decision 0101, D6).
    pending_approvals: BTreeSet<String>,
    /// How the newest turn ended, while none is open.
    last_end: Option<(TurnState, Option<Outcome>)>,
    /// The activity last recorded for the session, so only a change is written.
    recorded: Option<ChatActivity>,
}

/// What a chat session is doing, as its journal shows it: the activity the service records for it (decision 0101).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatActivity {
    /// No turn runs: before the first prompt, or after an interrupted or unverifiable turn.
    Idle,
    /// A turn runs.
    Working,
    /// A turn runs and waits for an answer to a permission request.
    Waiting,
    /// The newest turn completed, with the CLI's verdict.
    Done(Option<Outcome>),
}

impl JournalState {
    /// The item id the adapter's `key` names, when the run used it.
    pub(crate) fn item_id(&self, key: &str) -> Option<&str> {
        self.keys.get(key).map(String::as_str)
    }

    /// Whether a turn runs or a permission request waits: a prompt now would land inside a turn (decision 0101, D7).
    pub(crate) fn busy(&self) -> bool {
        self.turn_running() || !self.pending_approvals.is_empty()
    }

    /// Whether a turn is open, the running one or one opened for a held prompt.
    fn turn_running(&self) -> bool {
        self.open_turn.is_some() || !self.held_turns.is_empty()
    }

    /// The session's activity as the journal shows it.
    pub(crate) fn activity(&self) -> ChatActivity {
        match (self.turn_running(), &self.last_end) {
            (true, _) if !self.pending_approvals.is_empty() => ChatActivity::Waiting,
            (true, _) => ChatActivity::Working,
            (false, Some((TurnState::Completed, outcome))) => ChatActivity::Done(*outcome),
            (false, _) => ChatActivity::Idle,
        }
    }
}

impl JournalState {
    /// Opens the session's journal (a new epoch when it has none) and reads back its open turn: the newest turn item,
    /// when it is still running.
    pub(crate) fn open(store: &Store, session: &str) -> Result<Self, StoreError> {
        let head = store.open_chat_journal(session, &uuid::Uuid::now_v7().to_string())?;
        // Every turn still running is read back, so the next end closes them all.
        let mut held_turns = store.running_chat_turns(session)?;
        let open_turn = if held_turns.is_empty() {
            None
        } else {
            Some(held_turns.remove(0))
        };
        let pending_approvals = store.pending_chat_approvals(session)?.into_iter().collect();
        Ok(Self {
            epoch: head.epoch,
            open_turn,
            held_turns,
            keys: HashMap::new(),
            pending_approvals,
            last_end: None,
            recorded: None,
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

    /// Whether a turn runs or a permission request waits.
    pub(crate) fn busy(&self) -> bool {
        self.state.busy()
    }

    /// The open turn's id, or else the first turn opened for a held prompt.
    pub(crate) fn open_turn(&self) -> Option<String> {
        self.state
            .open_turn
            .clone()
            .or_else(|| self.state.held_turns.first().cloned())
    }

    /// The item id the adapter's `key` names.
    pub(crate) fn item_id(&self, key: &str) -> Option<String> {
        self.state.item_id(key).map(str::to_owned)
    }

    /// The item `id`, as it is now.
    pub(crate) fn item(&self, id: &str) -> Result<Option<ChatItem>, StoreError> {
        Ok(self.store.chat_item(self.session, id)?.map(item_of))
    }

    /// The session's activity when it differs from the one last recorded, which it then becomes.
    pub(crate) fn activity_change(&mut self) -> Option<ChatActivity> {
        let now = self.state.activity();
        (self.state.recorded != Some(now)).then(|| {
            self.state.recorded = Some(now);
            now
        })
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
        if let ChatItemBody::Approval(approval) = &item.body {
            if approval.state == ApprovalState::Pending {
                self.state.pending_approvals.insert(item.id.clone());
            } else {
                self.state.pending_approvals.remove(&item.id);
            }
        }
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
        if self.state.turn_running() {
            self.end_turn(TurnState::Unverifiable, None)?;
        }
        self.open_new_turn(text, false)
    }

    /// Opens a turn for a prompt handed to the CLI while a turn runs (decision 0101, D7): its turn item and its
    /// `user_message` show at once, and the turn ends with the running one. Without a running turn it is an ordinary
    /// turn.
    pub(crate) fn hold_turn(&mut self, text: &str) -> Result<String, StoreError> {
        let held = self.state.open_turn.is_some();
        self.open_new_turn(text, held)
    }

    /// Adds a running turn item and the `user_message` that opened it; `held` keeps it beside the open turn.
    fn open_new_turn(&mut self, text: &str, held: bool) -> Result<String, StoreError> {
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
        if held {
            self.state.held_turns.push(turn_id.clone());
        } else {
            self.state.open_turn = Some(turn_id.clone());
        }
        Ok(turn_id)
    }

    /// Ends the open turn: every running message of it completes, then the turn takes `state` and `outcome`.
    pub(crate) fn end_turn(
        &mut self,
        state: TurnState,
        outcome: Option<Outcome>,
    ) -> Result<(), StoreError> {
        // A request still pending when its turn ends is cancelled, never answered (decision 0101, D6).
        self.cancel_pending_approvals()?;
        let held = std::mem::take(&mut self.state.held_turns);
        if !held.is_empty() || self.state.open_turn.is_some() {
            self.state.last_end = Some((state, outcome));
        }
        for held in held {
            self.close_turn(&held, state, outcome)?;
        }
        let Some(turn_id) = self.state.open_turn.take() else {
            return Ok(());
        };
        if state != TurnState::Completed {
            self.interrupt_open_calls(&turn_id)?;
        }
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
        self.close_turn(&turn_id, state, outcome)
    }

    /// The turn item `turn_id` takes `state` and `outcome`, when it is still running.
    fn close_turn(
        &mut self,
        turn_id: &str,
        state: TurnState,
        outcome: Option<Outcome>,
    ) -> Result<(), StoreError> {
        if let Some(row) = self.store.chat_item(self.session, turn_id)?
            && let ChatItemBody::Turn(turn) = item_of(row).body
            && turn.state == TurnState::Running
        {
            let ended = ChatItemBody::Turn(Turn {
                state,
                outcome,
                completed_at_ms: Some(now_ms()),
                ..turn
            });
            self.replace(turn_id, &ended)?;
        }
        Ok(())
    }

    /// Every pending approval becomes `cancelled`.
    fn cancel_pending_approvals(&mut self) -> Result<(), StoreError> {
        let pending: Vec<String> = self.state.pending_approvals.iter().cloned().collect();
        for id in pending {
            match self.store.chat_item(self.session, &id)?.map(item_of) {
                Some(ChatItem {
                    body: ChatItemBody::Approval(approval),
                    ..
                }) if approval.state == ApprovalState::Pending => {
                    let cancelled = Approval {
                        state: ApprovalState::Cancelled,
                        settled_at_ms: Some(now_ms()),
                        ..approval
                    };
                    self.replace(&id, &ChatItemBody::Approval(cancelled))?;
                }
                _ => {
                    self.state.pending_approvals.remove(&id);
                }
            }
        }
        Ok(())
    }

    /// The tool calls and sub-agents of `turn` still running end `interrupted` (decision 0101, D8).
    fn interrupt_open_calls(&mut self, turn: &str) -> Result<(), StoreError> {
        for item_type in ["tool_use", "subagent"] {
            for row in self.store.chat_turn_items(self.session, turn, item_type)? {
                let item = item_of(row);
                let interrupted = match item.body {
                    ChatItemBody::ToolUse(mut tool)
                        if matches!(tool.status, ToolStatus::Started | ToolStatus::Updated) =>
                    {
                        tool.status = ToolStatus::Interrupted;
                        ChatItemBody::ToolUse(tool)
                    }
                    ChatItemBody::Subagent {
                        tool_use_id,
                        description,
                        agent_type,
                        status: SubagentStatus::Running,
                    } => ChatItemBody::Subagent {
                        tool_use_id,
                        description,
                        agent_type,
                        status: SubagentStatus::Interrupted,
                    },
                    _ => continue,
                };
                self.replace(&item.id, &interrupted)?;
            }
        }
        Ok(())
    }

    /// Settles the pending approval `id` with the person's answer: `allowed` with `scope`, or `denied`. Returns the item
    /// after the answer, or `None` when it is not a pending approval: the first answer wins (decision 0101, D6).
    pub(crate) fn settle_approval(
        &mut self,
        id: &str,
        state: ApprovalState,
        scope: Option<ApprovalScope>,
    ) -> Result<Option<ChatItem>, StoreError> {
        let Some(item) = self.store.chat_item(self.session, id)?.map(item_of) else {
            return Ok(None);
        };
        let ChatItemBody::Approval(approval) = item.body else {
            return Ok(None);
        };
        if approval.state != ApprovalState::Pending {
            return Ok(None);
        }
        let settled = Approval {
            state,
            scope,
            settled_at_ms: Some(now_ms()),
            ..approval
        };
        self.replace(id, &ChatItemBody::Approval(settled))?;
        Ok(self.store.chat_item(self.session, id)?.map(item_of))
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
            // Bytes for the run, which the service writes; nothing for the journal.
            JournalOp::Send { .. } => Ok(()),
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

#[cfg(test)]
mod tests {
    use massorca_core_store::{AgentSessionRow, RunMode, SessionOrigin};
    use massorca_protocol::methods::chat::{NoticeLevel, ResetReason};

    use super::*;

    /// A store with one chat session; returns the store and the session id.
    fn chat_session() -> (Store, String) {
        let store = Store::open_in_memory().unwrap();
        let dir = std::env::temp_dir();
        let project = massorca_core_store::ProjectRow {
            id: massorca_core_store::new_id(),
            kind: massorca_core_store::ProjectKind::Folder,
            path: dir.display().to_string(),
            path_key: "chat-journal-test".to_owned(),
            name: "notes".to_owned(),
            workspaces_root: dir.display().to_string(),
            added_at: 1,
            base_ref: None,
            icon: None,
        };
        store.insert_project(&project).unwrap();
        let workspace = massorca_core_store::WorkspaceRow {
            id: massorca_core_store::new_id(),
            project_id: project.id,
            kind: massorca_core_store::WorkspaceKind::Folder,
            name: "main".to_owned(),
            path: dir.display().to_string(),
            path_key: "chat-journal-test-main".to_owned(),
            branch: None,
            base_ref: None,
            created_branch: false,
            state: massorca_core_store::WorkspaceState::Ready,
            request_id: None,
            created_at: 1,
            comment: None,
        };
        store.insert_workspace(&workspace).unwrap();
        let row = AgentSessionRow {
            id: massorca_core_store::new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace.id,
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: None,
            request_id: None,
            created_at: 1,
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        };
        store
            .insert_agent_session_with_origin(&row, &SessionOrigin::default(), Some("run"))
            .unwrap();
        (store, row.id)
    }

    fn text(key: &str, text: &str) -> JournalOp {
        JournalOp::Add {
            key: Some(key.to_owned()),
            parent: None,
            body: ChatItemBody::AssistantMessage {
                text: text.to_owned(),
                state: MessageState::Running,
            },
        }
    }

    fn pending(request_id: &str) -> JournalOp {
        JournalOp::Add {
            key: Some(crate::chat::approval_key(request_id)),
            parent: None,
            body: ChatItemBody::Approval(Approval {
                request_id: request_id.to_owned(),
                tool: "Bash".to_owned(),
                input_summary: "ls".to_owned(),
                truncated: false,
                tool_use_id: Some("toolu_1".to_owned()),
                can_always: false,
                suggestions: Vec::new(),
                state: ApprovalState::Pending,
                scope: None,
                settled_at_ms: None,
            }),
        }
    }

    fn tool(id: &str) -> JournalOp {
        JournalOp::Add {
            key: Some(format!("tool:{id}")),
            parent: None,
            body: ChatItemBody::ToolUse(massorca_protocol::methods::chat::ToolUse {
                tool_use_id: id.to_owned(),
                name: "Bash".to_owned(),
                input_summary: "ls".to_owned(),
                status: ToolStatus::Started,
                output_summary: None,
                truncated: false,
            }),
        }
    }

    fn body_of(store: &Store, session: &str, state: &JournalState, key: &str) -> ChatItemBody {
        let id = state.item_id(key).expect("a key the run used");
        item_of(store.chat_item(session, id).unwrap().unwrap()).body
    }

    #[test]
    fn the_activity_follows_the_turn_and_its_pending_requests_and_the_first_answer_wins() {
        let (store, session) = chat_session();
        let mut state = JournalState::open(&store, &session).unwrap();
        assert_eq!(state.activity(), ChatActivity::Idle);
        let mut writer = JournalWriter::new(&store, &session, &mut state);
        writer.start_turn("go").unwrap();
        assert_eq!(writer.activity_change(), Some(ChatActivity::Working));
        writer.apply(pending("r-1")).unwrap();
        assert!(writer.busy());
        assert_eq!(writer.activity_change(), Some(ChatActivity::Waiting));
        assert_eq!(writer.activity_change(), None, "only a change is reported");
        let id = writer.item_id(&crate::chat::approval_key("r-1")).unwrap();
        let allowed = writer
            .settle_approval(&id, ApprovalState::Allowed, Some(ApprovalScope::Once))
            .unwrap()
            .expect("the first answer settles it");
        assert!(matches!(&allowed.body, ChatItemBody::Approval(approval)
            if approval.state == ApprovalState::Allowed && approval.settled_at_ms.is_some()));
        assert_eq!(
            writer
                .settle_approval(&id, ApprovalState::Denied, None)
                .unwrap(),
            None,
            "a second answer loses"
        );
        assert_eq!(writer.activity_change(), Some(ChatActivity::Working));
        writer
            .apply(JournalOp::EndTurn {
                state: TurnState::Completed,
                outcome: Some(Outcome::Success),
            })
            .unwrap();
        assert_eq!(
            writer.activity_change(),
            Some(ChatActivity::Done(Some(Outcome::Success)))
        );
        assert!(!writer.busy());
    }

    #[test]
    fn an_interrupted_turn_interrupts_its_calls_and_cancels_its_requests_even_after_a_restart() {
        let (store, session) = chat_session();
        let mut state = JournalState::open(&store, &session).unwrap();
        let mut writer = JournalWriter::new(&store, &session, &mut state);
        writer.start_turn("go").unwrap();
        writer.apply(tool("toolu_1")).unwrap();
        writer.apply(pending("r-1")).unwrap();
        writer.finish();
        let tool_id = state.item_id("tool:toolu_1").unwrap().to_owned();
        let approval_id = state
            .item_id(&crate::chat::approval_key("r-1"))
            .unwrap()
            .to_owned();

        // A service that restarts reads the open turn and the pending request back from the store.
        let mut reopened = JournalState::open(&store, &session).unwrap();
        assert!(reopened.busy());
        assert_eq!(reopened.activity(), ChatActivity::Waiting);
        let mut writer = JournalWriter::new(&store, &session, &mut reopened);
        writer.end_turn(TurnState::Interrupted, None).unwrap();
        assert_eq!(writer.activity_change(), Some(ChatActivity::Idle));
        writer.finish();
        let body = |id: &str| item_of(store.chat_item(&session, id).unwrap().unwrap()).body;
        assert!(
            matches!(body(&tool_id), ChatItemBody::ToolUse(tool) if tool.status == ToolStatus::Interrupted)
        );
        assert!(
            matches!(body(&approval_id), ChatItemBody::Approval(approval)
            if approval.state == ApprovalState::Cancelled)
        );
        assert!(!reopened.busy());
        // A completed turn leaves its calls as they are.
        let mut writer = JournalWriter::new(&store, &session, &mut reopened);
        writer.start_turn("again").unwrap();
        writer.apply(tool("toolu_2")).unwrap();
        writer
            .apply(JournalOp::EndTurn {
                state: TurnState::Completed,
                outcome: None,
            })
            .unwrap();
        writer.finish();
        assert!(
            matches!(body_of(&store, &session, &reopened, "tool:toolu_2"),
            ChatItemBody::ToolUse(tool) if tool.status == ToolStatus::Started)
        );
    }

    #[test]
    fn a_turn_opens_with_its_prompt_and_an_append_carries_its_byte_offset() {
        let (store, session) = chat_session();
        let mut state = JournalState::open(&store, &session).unwrap();
        let mut writer = JournalWriter::new(&store, &session, &mut state);
        let turn = writer.start_turn("hello").unwrap();
        let batch = writer.finish().unwrap();
        assert_eq!(batch.cursor.seq, 2);
        let [
            ChatUpdate::Item { item: turn_item },
            ChatUpdate::Item { item: user },
        ] = batch.updates.as_slice()
        else {
            panic!("{batch:?}");
        };
        assert_eq!(turn_item.id, turn);
        assert_eq!((turn_item.position, user.position), (1, 2));
        assert_eq!(user.turn.as_deref(), Some(turn.as_str()));
        assert!(
            matches!(&turn_item.body, ChatItemBody::Turn(open) if open.state == TurnState::Running && open.user_item.as_ref() == Some(&user.id))
        );

        let mut writer = JournalWriter::new(&store, &session, &mut state);
        writer.apply(text("m", "Hé")).unwrap();
        writer
            .apply(JournalOp::Append {
                key: "m".to_owned(),
                text: "llo".to_owned(),
            })
            .unwrap();
        let batch = writer.finish().unwrap();
        let [ChatUpdate::Item { item: message }, append] = batch.updates.as_slice() else {
            panic!("{batch:?}");
        };
        assert_eq!(message.turn.as_deref(), Some(turn.as_str()));
        assert_eq!(
            append,
            &ChatUpdate::Append {
                id: message.id.clone(),
                seq: 4,
                offset: 3,
                text: "llo".to_owned(),
            }
        );
        assert_eq!(batch.cursor.seq, 4);

        let mut writer = JournalWriter::new(&store, &session, &mut state);
        writer
            .apply(JournalOp::EndTurn {
                state: TurnState::Completed,
                outcome: Some(Outcome::Success),
            })
            .unwrap();
        let batch = writer.finish().unwrap();
        let [
            ChatUpdate::Item { item: completed },
            ChatUpdate::Item { item: ended },
        ] = batch.updates.as_slice()
        else {
            panic!("{batch:?}");
        };
        assert_eq!(
            completed.body,
            ChatItemBody::AssistantMessage {
                text: "Héllo".to_owned(),
                state: MessageState::Completed,
            },
            "the stored text and body read back as one item"
        );
        assert_eq!(completed.position, 3, "a change keeps the position");
        assert!(
            matches!(&ended.body, ChatItemBody::Turn(done) if done.state == TurnState::Completed && done.outcome == Some(Outcome::Success) && done.completed_at_ms.is_some())
        );
        assert_eq!(state.open_turn, None);
    }

    #[test]
    fn ops_on_unknown_keys_change_nothing_and_an_empty_batch_is_none() {
        let (store, session) = chat_session();
        let mut state = JournalState::open(&store, &session).unwrap();
        let mut writer = JournalWriter::new(&store, &session, &mut state);
        writer
            .apply(JournalOp::Append {
                key: "x".to_owned(),
                text: "y".to_owned(),
            })
            .unwrap();
        writer
            .apply(JournalOp::EndTurn {
                state: TurnState::Completed,
                outcome: None,
            })
            .unwrap();
        assert_eq!(writer.finish(), None);
    }

    #[test]
    fn a_replay_sends_whole_items_after_the_cursor_and_resets_a_stale_one() {
        let (store, session) = chat_session();
        let mut state = JournalState::open(&store, &session).unwrap();
        let epoch = state.epoch.clone();
        let mut writer = JournalWriter::new(&store, &session, &mut state);
        writer.apply(text("m", "a")).unwrap();
        writer
            .apply(JournalOp::Add {
                key: None,
                parent: None,
                body: ChatItemBody::Notice {
                    level: NoticeLevel::Info,
                    text: "n".to_owned(),
                },
            })
            .unwrap();
        writer
            .apply(JournalOp::Append {
                key: "m".to_owned(),
                text: "b".to_owned(),
            })
            .unwrap();
        writer.finish();

        let batch = replay(
            &store,
            &session,
            &ChatCursor {
                epoch: epoch.clone(),
                seq: 2,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(batch.cursor.seq, 3);
        let [ChatUpdate::Item { item }] = batch.updates.as_slice() else {
            panic!("{batch:?}");
        };
        assert_eq!(
            item.body,
            ChatItemBody::AssistantMessage {
                text: "ab".to_owned(),
                state: MessageState::Running,
            }
        );
        assert_eq!(
            replay(
                &store,
                &session,
                &ChatCursor {
                    epoch: "another".to_owned(),
                    seq: 1,
                },
            )
            .unwrap(),
            Err(ResetReason::EpochChanged)
        );
        assert_eq!(
            replay(&store, &session, &ChatCursor { epoch, seq: 9 }).unwrap(),
            Err(ResetReason::EpochChanged)
        );
    }

    #[test]
    fn a_cursor_older_than_a_dropped_change_is_too_old() {
        let (store, session) = chat_session();
        let state = JournalState::open(&store, &session).unwrap();
        for id in ["1", "2", "3"] {
            store
                .add_chat_item(
                    &session,
                    &NewChatItem {
                        id,
                        turn: None,
                        parent: None,
                        at_ms: 1,
                        item_type: "notice",
                        body: r#"{"type":"notice","level":"info","text":"n"}"#,
                        text: None,
                    },
                    2,
                )
                .unwrap();
        }
        let at = |seq| ChatCursor {
            epoch: state.epoch.clone(),
            seq,
        };
        assert_eq!(
            replay(&store, &session, &at(0)).unwrap(),
            Err(ResetReason::TooOld),
            "item 1 was dropped after the cursor's change"
        );
        let batch = replay(&store, &session, &at(1)).unwrap().unwrap();
        assert_eq!(batch.updates.len(), 2);
    }

    #[test]
    fn a_reopened_journal_keeps_its_epoch_and_its_open_turn() {
        let (store, session) = chat_session();
        let mut state = JournalState::open(&store, &session).unwrap();
        let turn = JournalWriter::new(&store, &session, &mut state)
            .start_turn("hi")
            .map(Some)
            .unwrap();
        let reopened = JournalState::open(&store, &session).unwrap();
        assert_eq!(reopened.epoch, state.epoch);
        assert_eq!(reopened.open_turn, turn);
        let page = newest_page(&store, &session, &state.epoch, 1).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.has_older);
        assert_eq!(page.latest_turn.map(|item| item.id), turn);
        assert_eq!(page.cursor.seq, 2);
    }
}
