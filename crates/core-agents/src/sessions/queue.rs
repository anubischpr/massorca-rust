//! The per-session prompt queue (decision 0084): prompts a person queues while an agent is busy, kept by the service so
//! they survive the client and reach the agent in order. The calls here queue, list, edit and remove; delivery follows
//! the agent's activity, and every change is announced as `Change::Queue`.

use massorca_core_store::{NewQueuedPrompt, QueuedPromptRow, QueuedState, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    ActivityState, AgentQueueList, MAX_QUEUED_PROMPTS, QUEUE_REASON_ASKING,
    QUEUE_REASON_BEHIND_FAILED, QUEUE_REASON_BUSY, QueueState, QueuedPrompt,
};

use super::{AgentError, Agents, Change, LiveRuns, lock};
use crate::catalog::Catalog;
use crate::kind::StatusMechanism;
use crate::launch;

impl Agents {
    /// Queues `text` at the end of the session's queue (decision 0084). A kind with no status mechanism has no queue and is
    /// refused with `agent.queue_unavailable`; a queue of [`MAX_QUEUED_PROMPTS`] prompts with `agent.queue_full`; a text
    /// `launch::check_prompt` refuses with `agent.prompt_invalid`. A `request_id` the session already queued returns the
    /// prompt it queued then, and queues nothing.
    pub fn queue_add(
        &self,
        selector: &str,
        text: &str,
        request_id: Option<&str>,
    ) -> Result<QueuedPrompt, AgentError> {
        launch::check_prompt(text)?;
        let row = self.resolve(selector)?;
        self.require_queue_kind(&row.kind)?;
        let mut live = lock(&self.inner.status);
        let row = self.reload(&row.id)?;
        if let Some(request_id) = request_id
            && let Some(existing) = self
                .store()
                .find_queued_prompt_by_request_id(&row.id, request_id)?
        {
            return self.queued_item(&mut live, &existing.session_id, &existing.id);
        }
        if self.store().list_queued_prompts(&row.id)?.len() >= MAX_QUEUED_PROMPTS {
            return Err(AgentError::new(
                codes::AGENT_QUEUE_FULL,
                format!(
                    "session {} already holds {MAX_QUEUED_PROMPTS} queued prompts; remove one first",
                    row.id
                ),
            ));
        }
        let stored = self.store().insert_queued_prompt(&NewQueuedPrompt {
            id: new_id(),
            session_id: row.id.clone(),
            text: text.to_owned(),
            request_id: request_id.map(str::to_owned),
            at: now_ms(),
        })?;
        self.notify(Change::Queue(row.id.clone()));
        self.queued_item(&mut live, &row.id, &stored.id)
    }

    /// The session's queue, head first (decision 0084).
    pub fn queue_list(&self, selector: &str) -> Result<AgentQueueList, AgentError> {
        let row = self.resolve(selector)?;
        self.require_queue_kind(&row.kind)?;
        let mut live = lock(&self.inner.status);
        let items = self.queue_items(&mut live, &row.id)?;
        Ok(AgentQueueList {
            session: row.id,
            items,
        })
    }

    /// Replaces the text of a waiting or failed prompt; a failed prompt stays failed (decision 0084). A prompt being sent
    /// is refused with `agent.busy`.
    pub fn queue_edit(&self, id: &str, text: &str) -> Result<QueuedPrompt, AgentError> {
        launch::check_prompt(text)?;
        let session = self.queue_session(id)?;
        self.require_queue_kind_of(&session)?;
        let mut live = lock(&self.inner.status);
        let row = self.queue_row(id)?;
        if row.state == QueuedState::Sending {
            return Err(sending_refusal(id));
        }
        self.store()
            .update_queued_prompt_text(&row.id, text, now_ms())?;
        self.notify(Change::Queue(row.session_id.clone()));
        self.queued_item(&mut live, &row.session_id, &row.id)
    }

    /// Removes a waiting or failed prompt from its queue and returns it as it was. A prompt being sent is refused with
    /// `agent.busy`.
    pub fn queue_remove(&self, id: &str) -> Result<QueuedPrompt, AgentError> {
        let session = self.queue_session(id)?;
        self.require_queue_kind_of(&session)?;
        let mut live = lock(&self.inner.status);
        let row = self.queue_row(id)?;
        if row.state == QueuedState::Sending {
            return Err(sending_refusal(id));
        }
        let removed = self.queued_item(&mut live, &row.session_id, &row.id)?;
        self.store().delete_queued_prompt(&row.id)?;
        self.notify(Change::Queue(row.session_id.clone()));
        Ok(removed)
    }

    /// The session a queued prompt belongs to, so a caller can check its access before acting on the prompt.
    pub fn queue_session_of(&self, id: &str) -> Result<String, AgentError> {
        self.queue_session(id)
    }

    /// Refuses `kind` with `agent.queue_unavailable` unless it is declared, valid and reports its activity: only then is
    /// there a busy agent to wait for (decision 0084).
    pub(crate) fn require_queue_kind(&self, kind: &str) -> Result<(), AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        let reports = catalog
            .find(kind)
            .and_then(|entry| entry.valid())
            .is_some_and(|decl| decl.status.mechanism != StatusMechanism::None);
        if reports {
            return Ok(());
        }
        Err(AgentError::new(
            codes::AGENT_QUEUE_UNAVAILABLE,
            format!("agent kind {kind} reports no activity, so it has no prompt queue"),
        ))
    }

    /// Refuses a session whose kind has no queue (see [`Agents::require_queue_kind`]), by the session's id.
    fn require_queue_kind_of(&self, session: &str) -> Result<(), AgentError> {
        let kind = self.reload(session)?.kind;
        self.require_queue_kind(&kind)
    }

    /// The queued prompt `id`.
    fn queue_row(&self, id: &str) -> Result<QueuedPromptRow, AgentError> {
        self.store().get_queued_prompt(id)?.ok_or_else(|| {
            AgentError::new(
                codes::AGENT_QUEUE_NOT_FOUND,
                format!("no queued prompt {id}"),
            )
        })
    }

    /// The session of a queued prompt.
    fn queue_session(&self, id: &str) -> Result<String, AgentError> {
        Ok(self.queue_row(id)?.session_id)
    }

    /// The wire view of every prompt of a session, head first, under the status lock.
    fn queue_items(
        &self,
        live: &mut LiveRuns,
        session: &str,
    ) -> Result<Vec<QueuedPrompt>, AgentError> {
        let row = self.reload(session)?;
        let view = self.view_with(live, row)?;
        let asking = view
            .activity
            .as_ref()
            .is_some_and(|activity| activity.state == ActivityState::Waiting);
        let rows = self.store().list_queued_prompts(session)?;
        Ok(rows
            .iter()
            .enumerate()
            .map(|(index, row)| queued_item(row, &rows[..index], asking))
            .collect())
    }

    /// One prompt of a session, as the queue shows it now.
    fn queued_item(
        &self,
        live: &mut LiveRuns,
        session: &str,
        id: &str,
    ) -> Result<QueuedPrompt, AgentError> {
        self.queue_items(live, session)?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| {
                AgentError::new(
                    codes::AGENT_QUEUE_NOT_FOUND,
                    format!("no queued prompt {id}"),
                )
            })
    }
}

/// The refusal for a change to a prompt that is being typed into its run.
fn sending_refusal(id: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_BUSY,
        format!("queued prompt {id} is being sent; try again once it is typed"),
    )
}

/// The wire view of one prompt. `ahead` holds the prompts before it in the queue; `asking` says its agent is asking the
/// person something. A waiting prompt's reason is its own pause, else the failure or pause of a prompt ahead, else
/// whether the agent asks or is busy (decision 0084).
fn queued_item(row: &QueuedPromptRow, ahead: &[QueuedPromptRow], asking: bool) -> QueuedPrompt {
    let (state, reason) = match row.state {
        QueuedState::Failed => (QueueState::Failed, row.reason.clone()),
        QueuedState::Sending => (QueueState::Sending, None),
        QueuedState::Waiting => {
            let reason = if let Some(pause) = &row.reason {
                pause.clone()
            } else if ahead.iter().any(|item| item.state == QueuedState::Failed) {
                QUEUE_REASON_BEHIND_FAILED.to_owned()
            } else if let Some(pause) = ahead.iter().find_map(|item| item.reason.clone()) {
                pause
            } else if asking {
                QUEUE_REASON_ASKING.to_owned()
            } else {
                QUEUE_REASON_BUSY.to_owned()
            };
            (QueueState::Waiting, Some(reason))
        }
    };
    QueuedPrompt {
        id: row.id.clone(),
        session: row.session_id.clone(),
        position: u64::try_from(row.position).unwrap_or(0),
        text: row.text.clone(),
        state,
        reason,
        created_at_ms: row.created_at_ms,
        updated_at_ms: row.updated_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use massorca_core_store::{AgentSessionRow, QueuedState, RunMode, new_id, now_ms};
    use massorca_protocol::codes;
    use massorca_protocol::methods::agents::{
        AgentQueueList, MAX_QUEUED_PROMPTS, QUEUE_REASON_ASKING, QUEUE_REASON_BEHIND_FAILED,
        QUEUE_REASON_BUSY, QueueState,
    };

    use crate::sessions::tests::service;
    use crate::sessions::{Agents, Change};

    /// A session of the built-in `claude` kind, which reports its activity, with no terminal: it has no live run.
    fn claude_session(agents: &Agents, workspace_id: &str) -> String {
        let row = AgentSessionRow {
            id: new_id(),
            kind: "claude".to_owned(),
            workspace_id: workspace_id.to_owned(),
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: None,
            request_id: None,
            created_at: now_ms(),
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
        };
        agents.store().insert_agent_session(&row).unwrap();
        row.id
    }

    fn texts(list: &AgentQueueList) -> Vec<&str> {
        list.items.iter().map(|item| item.text.as_str()).collect()
    }

    #[test]
    fn queued_prompts_wait_in_order_with_the_reason_busy() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);

        let first = agents.queue_add(&session, "one", None).unwrap();
        let second = agents.queue_add(&session, "two", None).unwrap();

        assert_eq!((first.position, second.position), (1, 2));
        assert_eq!(first.state, QueueState::Waiting);
        assert_eq!(first.reason.as_deref(), Some(QUEUE_REASON_BUSY));
        let listed = agents.queue_list(&session).unwrap();
        assert_eq!(listed.session, session);
        assert_eq!(texts(&listed), ["one", "two"]);
    }

    #[test]
    fn a_kind_with_no_declared_queue_refuses_it() {
        let (agents, workspace, _profile) = service();
        let row = AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace,
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: None,
            request_id: None,
            created_at: now_ms(),
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
        };
        agents.store().insert_agent_session(&row).unwrap();

        let error = agents.queue_add(&row.id, "later", None).unwrap_err();
        assert_eq!(error.code, codes::AGENT_QUEUE_UNAVAILABLE, "{error}");
    }

    #[test]
    fn a_request_id_returns_the_prompt_it_queued_first() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);

        let first = agents.queue_add(&session, "once", Some("req-1")).unwrap();
        let again = agents.queue_add(&session, "twice", Some("req-1")).unwrap();

        assert_eq!(again.id, first.id);
        assert_eq!(texts(&agents.queue_list(&session).unwrap()), ["once"]);
    }

    #[test]
    fn the_queue_holds_at_most_one_hundred_prompts() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        for index in 0..MAX_QUEUED_PROMPTS {
            agents
                .store()
                .insert_queued_prompt(&massorca_core_store::NewQueuedPrompt {
                    id: new_id(),
                    session_id: session.clone(),
                    text: format!("prompt {index}"),
                    request_id: None,
                    at: now_ms(),
                })
                .unwrap();
        }

        let error = agents
            .queue_add(&session, "one too many", None)
            .unwrap_err();

        assert_eq!(error.code, codes::AGENT_QUEUE_FULL, "{error}");
        assert_eq!(
            agents.queue_list(&session).unwrap().items.len(),
            MAX_QUEUED_PROMPTS
        );
    }

    #[test]
    fn a_prompt_is_checked_before_it_is_queued() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);

        let error = agents.queue_add(&session, "a\0b", None).unwrap_err();

        assert_eq!(error.code, codes::AGENT_PROMPT_INVALID);
        assert!(agents.queue_list(&session).unwrap().items.is_empty());
    }

    #[test]
    fn a_prompt_is_edited_or_removed_only_while_waiting_or_failed() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let waiting = agents.queue_add(&session, "one", None).unwrap();
        let sending = agents.queue_add(&session, "two", None).unwrap();
        agents
            .store()
            .set_queued_prompt_state(&sending.id, QueuedState::Sending, None, now_ms())
            .unwrap();

        let edited = agents.queue_edit(&waiting.id, "edited").unwrap();
        assert_eq!(edited.text, "edited");
        assert_eq!(
            agents.queue_edit(&sending.id, "x").unwrap_err().code,
            codes::AGENT_BUSY
        );
        assert_eq!(
            agents.queue_remove(&sending.id).unwrap_err().code,
            codes::AGENT_BUSY
        );
        let removed = agents.queue_remove(&waiting.id).unwrap();
        assert_eq!(removed.id, waiting.id);
        assert_eq!(texts(&agents.queue_list(&session).unwrap()), ["two"]);
    }

    #[test]
    fn an_unknown_queued_prompt_is_not_found() {
        let (agents, _workspace, _profile) = service();

        for error in [
            agents.queue_edit("no-such-prompt", "x").unwrap_err(),
            agents.queue_remove("no-such-prompt").unwrap_err(),
            agents.queue_session_of("no-such-prompt").unwrap_err(),
        ] {
            assert_eq!(error.code, codes::AGENT_QUEUE_NOT_FOUND, "{error}");
        }
    }

    #[test]
    fn a_prompt_says_asking_while_its_agent_asks_and_holds_behind_a_failed_one() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let head = agents.queue_add(&session, "head", None).unwrap();
        agents.queue_add(&session, "next", None).unwrap();
        agents
            .store()
            .set_agent_session_status(
                &session,
                Some(&massorca_core_store::StoredActivity {
                    state: "waiting".to_owned(),
                    outcome: None,
                    source: "hook".to_owned(),
                    at: now_ms(),
                }),
                None,
                None,
            )
            .unwrap();
        let asking = agents.queue_list(&session).unwrap();
        assert!(
            asking
                .items
                .iter()
                .all(|item| item.reason.as_deref() == Some(QUEUE_REASON_ASKING))
        );

        agents
            .store()
            .set_queued_prompt_state(
                &head.id,
                QueuedState::Failed,
                Some("the run has exited"),
                now_ms(),
            )
            .unwrap();
        let held = agents.queue_list(&session).unwrap();
        assert_eq!(held.items[0].state, QueueState::Failed);
        assert_eq!(held.items[0].reason.as_deref(), Some("the run has exited"));
        assert_eq!(held.items[1].state, QueueState::Waiting);
        assert_eq!(
            held.items[1].reason.as_deref(),
            Some(QUEUE_REASON_BEHIND_FAILED)
        );
    }

    #[test]
    fn a_queue_change_is_announced_with_its_session() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        agents.set_listener(Box::new(move |change| sink.lock().unwrap().push(change)));

        let added = agents.queue_add(&session, "one", None).unwrap();
        agents.queue_edit(&added.id, "edited").unwrap();
        agents.queue_remove(&added.id).unwrap();

        let announced: Vec<Change> = seen.lock().unwrap().clone();
        assert_eq!(
            announced,
            vec![
                Change::Queue(session.clone()),
                Change::Queue(session.clone()),
                Change::Queue(session),
            ]
        );
    }
}
