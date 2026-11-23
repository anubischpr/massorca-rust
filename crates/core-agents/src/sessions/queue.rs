//! The per-session prompt queue (decision 0084): prompts a person queues while an agent is busy, kept by the service so
//! they survive the client and reach the agent in order. The calls here queue, list, edit and remove; delivery follows
//! the agent's activity, and every change is announced as `Change::Queue`.

use std::thread;

use massorca_core_store::{
    NewQueuedPrompt, QueuedPromptRow, QueuedState, StoreError, new_id, now_ms,
};
use massorca_core_terminal::TerminalError;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    ActivityState, AgentQueueList, MAX_DELIVERED_REQUEST_IDS, MAX_QUEUED_PROMPTS,
    QUEUE_REASON_ASKING, QUEUE_REASON_BEHIND_FAILED, QUEUE_REASON_BUSY,
    QUEUE_REASON_PAUSED_STOPPED, QueueState, QueuedPrompt,
};

use super::{AgentError, Agents, Change, LiveRun, LiveRuns, input_refusal, lock, not_live};
use crate::catalog::Catalog;
use crate::kind::StatusMechanism;
use crate::launch;
use crate::prompt;

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
        host_admin: bool,
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
        if let Some(request_id) = request_id
            && self
                .store()
                .find_delivered_request(&row.id, request_id)?
                .is_some()
        {
            return Err(AgentError::new(
                codes::AGENT_QUEUE_ALREADY_DELIVERED,
                format!(
                    "the prompt with request id {request_id} was already delivered to session {}",
                    row.id
                ),
            ));
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
            host_admin,
        })?;
        self.notify(Change::Queue(row.id.clone()));
        let shown = self.queued_item(&mut live, &row.id, &stored.id)?;
        drop(live);
        // An idle agent takes the new prompt at once; a busy one takes it at its next idle (decision 0084, D3).
        self.drain_queue(&row.id);
        Ok(shown)
    }

    /// The session's queue, head first (decision 0084).
    pub fn queue_list(&self, selector: &str) -> Result<AgentQueueList, AgentError> {
        let row = self.resolve(selector)?;
        let mut live = lock(&self.inner.status);
        let items = self.queue_items(&mut live, &row.id)?;
        Ok(AgentQueueList {
            session: row.id,
            items,
        })
    }

    /// Replaces the text of a waiting or failed prompt; a failed prompt stays failed (decision 0084). A prompt being sent
    /// is refused with `agent.busy`.
    pub fn queue_edit(
        &self,
        id: &str,
        text: &str,
        host_admin: bool,
    ) -> Result<QueuedPrompt, AgentError> {
        launch::check_prompt(text)?;
        let mut live = lock(&self.inner.status);
        let row = self.queue_row(id)?;
        if row.state == QueuedState::Sending {
            return Err(sending_refusal(id));
        }
        self.store()
            .update_queued_prompt_text(&row.id, text, host_admin, now_ms())?;
        self.notify(Change::Queue(row.session_id.clone()));
        self.queued_item(&mut live, &row.session_id, &row.id)
    }

    /// Removes a waiting or failed prompt from its queue and returns it as it was. A prompt being sent is refused with
    /// `agent.busy`.
    pub fn queue_remove(&self, id: &str) -> Result<QueuedPrompt, AgentError> {
        let mut live = lock(&self.inner.status);
        let row = self.queue_row(id)?;
        if row.state == QueuedState::Sending {
            return Err(sending_refusal(id));
        }
        let removed = self.queued_item(&mut live, &row.session_id, &row.id)?;
        self.store().delete_queued_prompt(&row.id)?;
        self.notify(Change::Queue(row.session_id.clone()));
        drop(live);
        // The prompt that waited behind it may be next now (decision 0084, D4).
        self.drain_queue(&removed.session);
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

/// What a delivery types into a run: the queued prompt it sends, the run's terminal and the bytes of its text. It holds
/// the prompt text, so it has no `Debug`.
struct Delivery {
    session: String,
    item: String,
    terminal: String,
    bytes: Vec<u8>,
}

impl Agents {
    /// Delivers the head of a session's queue if its agent is ready (decision 0084, D3). Called on every hook report that
    /// turns idle or done, when a mode switch ends, after a bring-back, and when a queue call may have changed what waits.
    pub(crate) fn drain_queue(&self, session: &str) {
        if let Some(delivery) = self.prepare_drain(session) {
            self.send_on_thread(delivery);
        }
    }

    /// Types a delivery on a thread of its own. The write can wait for the agent to read, and the agent may be waiting for
    /// the hook report that triggered this drain to return, so the report must not wait for the write (decision 0054).
    /// The delivery is already marked sending, so nothing else types meanwhile.
    ///
    /// No test stalls this write: `TerminalManager` is concrete and a PTY write cannot be blocked deterministically in
    /// process. The property rests on this spawn, which `deliver_first_input` uses for the same reason.
    fn send_on_thread(&self, delivery: Delivery) {
        let agents = self.clone();
        let session = delivery.session.clone();
        let item = delivery.item.clone();
        let spawned = thread::Builder::new()
            .name("agent-queue-delivery".to_owned())
            .spawn(move || {
                let _ = agents.send_delivery(delivery);
            });
        if let Err(error) = spawned {
            tracing::warn!(session = %session, %error, "a queued prompt has no thread to be typed on");
            self.finish_delivery(
                &session,
                &item,
                Err(AgentError::new(
                    codes::INTERNAL,
                    "the queued prompt has no thread to be typed on",
                )),
            );
        }
    }

    /// The head of a session's queue, when it may be typed now: waiting and not paused, its run live and ready, the agent
    /// not asking, no merge holding the workspace, and no other input being typed. Marks it sending under the status
    /// lock; the caller types the bytes. A failed store write holds the prompt, so nothing is typed that is not recorded.
    fn prepare_drain(&self, session: &str) -> Option<Delivery> {
        let mut live = lock(&self.inner.status);
        let rows = self.store().list_queued_prompts(session).ok()?;
        let head = rows.first()?;
        if head.state != QueuedState::Waiting || head.reason.is_some() {
            return None;
        }
        let row = self.reload(session).ok()?;
        let view = self.view_with(&mut live, row).ok()?;
        // A run that holds vault secrets (decision 0071) types only prompts a host-admin caller queued. Others fail here,
        // and a host-admin send-now may still retry them (decision 0084, D7).
        if view.holds_secrets && !head.host_admin {
            if self
                .store()
                .set_queued_prompt_state(
                    &head.id,
                    QueuedState::Failed,
                    Some(SECRETS_HELD_REFUSAL),
                    now_ms(),
                )
                .is_ok()
            {
                self.notify(Change::Queue(session.to_owned()));
            }
            return None;
        }
        let state = view.activity.as_ref().map(|activity| activity.state);
        if state == Some(ActivityState::Waiting)
            || self.refuse_while_merging(&view.workspace_id).is_err()
        {
            return None;
        }
        let terminal = view.terminal_id.clone()?;
        let run = live.get_mut(session)?;
        if run.delivering || input_refusal(run, session, state, false).is_some() {
            return None;
        }
        self.start_sending(run, head, terminal).ok()
    }

    /// Records that `item` is being typed into `run` (decision 0084): the store marks it sending first, then the run is
    /// marked as typing, so no other input interleaves. The text counts as typed, so the next prompt waits for a report
    /// (decision 0058).
    fn start_sending(
        &self,
        run: &mut LiveRun,
        item: &QueuedPromptRow,
        terminal: String,
    ) -> Result<Delivery, StoreError> {
        self.store()
            .set_queued_prompt_state(&item.id, QueuedState::Sending, None, now_ms())?;
        self.notify(Change::Queue(item.session_id.clone()));
        run.typed_since_report = run.prompt.has_status;
        run.delivering = true;
        Ok(Delivery {
            session: item.session_id.clone(),
            item: item.id.clone(),
            terminal,
            bytes: prompt::input_bytes(&item.text, run.prompt.paste, &run.prompt.submit),
        })
    }

    /// Types a delivery's bytes outside the status lock, as a prompt is typed, then records how it ended.
    fn send_delivery(&self, delivery: Delivery) -> Result<(), AgentError> {
        let written = self
            .inner
            .config
            .terminals
            .input(&delivery.terminal, &delivery.bytes)
            .map_err(|error| match error {
                TerminalError::Exited | TerminalError::NotFound => not_live(&delivery.session),
                other => AgentError::from(other),
            });
        self.finish_delivery(&delivery.session, &delivery.item, written.clone());
        written
    }

    /// Records how a delivery ended (decision 0084, D4): a typed prompt leaves the queue; one that could not be typed is
    /// failed with its reason, and holds everything behind it until it is removed or sent. Announced either way.
    fn finish_delivery(&self, session: &str, item: &str, written: Result<(), AgentError>) {
        let mut live = lock(&self.inner.status);
        if let Some(run) = live.get_mut(session) {
            run.delivering = false;
        }
        let stored = match written {
            Ok(()) => {
                if let Some(request_id) = self
                    .store()
                    .get_queued_prompt(item)
                    .ok()
                    .flatten()
                    .and_then(|row| row.request_id)
                    && let Err(error) = self.store().record_delivered_request(
                        session,
                        &request_id,
                        now_ms(),
                        keep_delivered_request_ids(),
                    )
                {
                    tracing::warn!(session = %session, %error, "a delivered request id could not be remembered");
                }
                self.store().delete_queued_prompt(item).map(drop)
            }
            Err(error) => self
                .store()
                .set_queued_prompt_state(item, QueuedState::Failed, Some(&error.message), now_ms())
                .map(drop),
        };
        if let Err(error) = stored {
            tracing::warn!(session = %session, %error, "a queued prompt's delivery could not be recorded");
        }
        self.notify(Change::Queue(session.to_owned()));
    }

    /// Sends a queued prompt at once, with force, skipping the busy hold and FIFO (decision 0084, D6). It is refused while
    /// the agent asks the person something (`agent.busy`), while another input is being typed, while a merge holds the
    /// workspace, and when the run is not live. On success the prompt leaves the queue and the session's pauses lift.
    pub fn queue_send_now(&self, id: &str) -> Result<QueuedPrompt, AgentError> {
        let session = self.queue_session(id)?;
        let (delivery, shown) = {
            let mut live = lock(&self.inner.status);
            let item = self.queue_row(id)?;
            if item.state == QueuedState::Sending {
                return Err(sending_refusal(id));
            }
            let view = self.view_with(&mut live, self.reload(&session)?)?;
            self.refuse_while_merging(&view.workspace_id)?;
            let state = view.activity.as_ref().map(|activity| activity.state);
            if state == Some(ActivityState::Waiting) {
                return Err(AgentError::new(
                    codes::AGENT_BUSY,
                    format!(
                        "session {session} is asking the person something; the queued prompt waits for the answer"
                    ),
                ));
            }
            let shown = self.queued_item(&mut live, &session, id)?;
            let terminal = view.terminal_id.clone().ok_or_else(|| not_live(&session))?;
            let run = live.get_mut(&session).ok_or_else(|| not_live(&session))?;
            if let Some(refusal) = input_refusal(run, &session, state, true) {
                return Err(refusal);
            }
            let delivery = self.start_sending(run, &item, terminal)?;
            (delivery, shown)
        };
        let outcome = self.send_delivery(delivery);
        if outcome.is_ok() {
            self.lift_pauses(&session);
        }
        outcome.map(|()| shown)
    }

    /// Lifts the pauses on a session's queue and delivers its head if the agent is ready (decision 0084, D5). Returns the
    /// queue as it is then.
    pub fn queue_resume(&self, selector: &str) -> Result<AgentQueueList, AgentError> {
        let row = self.resolve(selector)?;
        self.lift_pauses(&row.id);
        self.drain_queue(&row.id);
        let mut live = lock(&self.inner.status);
        let items = self.queue_items(&mut live, &row.id)?;
        Ok(AgentQueueList {
            session: row.id,
            items,
        })
    }

    /// The person's stop pauses the session's waiting prompts that no pause holds yet (decision 0084, D5).
    pub(crate) fn pause_queue_stopped(&self, session: &str) {
        self.pause_queue(session, QUEUE_REASON_PAUSED_STOPPED);
    }

    /// Pauses a session's waiting prompts with `reason`, announcing the change when one paused.
    fn pause_queue(&self, session: &str, reason: &str) {
        let _live = lock(&self.inner.status);
        match self.store().pause_queued_prompts(session, reason, now_ms()) {
            Ok(0) => {}
            Ok(_) => self.notify(Change::Queue(session.to_owned())),
            Err(error) => {
                tracing::warn!(session = %session, %error, "the queue could not be paused");
            }
        }
    }

    /// Lifts the pauses on a session's queue, announcing the change when one lifted (decision 0084, D5). An accepted
    /// prompt, a `send_now` or a resume lifts them.
    pub(crate) fn lift_pauses(&self, session: &str) {
        let _live = lock(&self.inner.status);
        match self.store().unpause_queued_prompts(session, now_ms()) {
            Ok(0) => {}
            Ok(_) => self.notify(Change::Queue(session.to_owned())),
            Err(error) => {
                tracing::warn!(session = %session, %error, "the queue's pauses could not be lifted");
            }
        }
    }
}

/// How many delivered request ids a session remembers (decision 0084).
fn keep_delivered_request_ids() -> u32 {
    u32::try_from(MAX_DELIVERED_REQUEST_IDS).unwrap_or(u32::MAX)
}

/// Why a queued prompt is failed instead of typed: its caller held no host-admin scope, and the run now holds vault secrets.
/// Names no secret.
const SECRETS_HELD_REFUSAL: &str =
    "queued by a caller without host-admin; the run now holds secrets";

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

    use massorca_core_store::{
        AgentSessionRow, QueuedState, RunExit, RunExitReason, RunMode, new_id, now_ms,
    };
    use massorca_protocol::codes;
    use massorca_protocol::methods::agents::{
        AgentQueueList, MAX_QUEUED_PROMPTS, QUEUE_REASON_ASKING, QUEUE_REASON_BEHIND_FAILED,
        QUEUE_REASON_BUSY, QueueState,
    };

    use crate::sessions::tests::service;
    use crate::sessions::{AgentError, Agents, Change};

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

        let first = agents.queue_add(&session, "one", None, false).unwrap();
        let second = agents.queue_add(&session, "two", None, false).unwrap();

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

        let error = agents.queue_add(&row.id, "later", None, false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_QUEUE_UNAVAILABLE, "{error}");
    }

    #[test]
    fn a_request_id_returns_the_prompt_it_queued_first() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);

        let first = agents
            .queue_add(&session, "once", Some("req-1"), false)
            .unwrap();
        let again = agents
            .queue_add(&session, "twice", Some("req-1"), false)
            .unwrap();

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
                    host_admin: false,
                })
                .unwrap();
        }

        let error = agents
            .queue_add(&session, "one too many", None, false)
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

        let error = agents.queue_add(&session, "a\0b", None, false).unwrap_err();

        assert_eq!(error.code, codes::AGENT_PROMPT_INVALID);
        assert!(agents.queue_list(&session).unwrap().items.is_empty());
    }

    #[test]
    fn a_prompt_is_edited_or_removed_only_while_waiting_or_failed() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let waiting = agents.queue_add(&session, "one", None, false).unwrap();
        let sending = agents.queue_add(&session, "two", None, false).unwrap();
        agents
            .store()
            .set_queued_prompt_state(&sending.id, QueuedState::Sending, None, now_ms())
            .unwrap();

        let edited = agents.queue_edit(&waiting.id, "edited", false).unwrap();
        assert_eq!(edited.text, "edited");
        assert_eq!(
            agents.queue_edit(&sending.id, "x", false).unwrap_err().code,
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
            agents.queue_edit("no-such-prompt", "x", false).unwrap_err(),
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
        let head = agents.queue_add(&session, "head", None, false).unwrap();
        agents.queue_add(&session, "next", None, false).unwrap();
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

        let added = agents.queue_add(&session, "one", None, false).unwrap();
        agents.queue_edit(&added.id, "edited", false).unwrap();
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

    #[test]
    fn a_failed_delivery_marks_its_prompt_and_holds_the_rest() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let head = agents.queue_add(&session, "one", None, false).unwrap();
        let next = agents.queue_add(&session, "two", None, false).unwrap();
        agents
            .store()
            .set_queued_prompt_state(&head.id, QueuedState::Sending, None, now_ms())
            .unwrap();

        agents.finish_delivery(
            &session,
            &head.id,
            Err(AgentError::new(
                codes::AGENT_NOT_LIVE,
                "the run of session has exited",
            )),
        );

        let held = agents.queue_list(&session).unwrap();
        assert_eq!(held.items[0].state, QueueState::Failed);
        assert_eq!(
            held.items[0].reason.as_deref(),
            Some("the run of session has exited")
        );
        assert_eq!(held.items[1].id, next.id);
        assert_eq!(
            held.items[1].reason.as_deref(),
            Some(QUEUE_REASON_BEHIND_FAILED)
        );
        assert!(
            agents.prepare_drain(&session).is_none(),
            "nothing is delivered behind a failed prompt, with or without a run"
        );
    }

    #[test]
    fn a_delivered_prompt_leaves_the_queue_and_is_announced() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let head = agents.queue_add(&session, "one", None, false).unwrap();
        agents.queue_add(&session, "two", None, false).unwrap();
        agents
            .store()
            .set_queued_prompt_state(&head.id, QueuedState::Sending, None, now_ms())
            .unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        agents.set_listener(Box::new(move |change| sink.lock().unwrap().push(change)));

        agents.finish_delivery(&session, &head.id, Ok(()));

        assert_eq!(texts(&agents.queue_list(&session).unwrap()), ["two"]);
        assert_eq!(*seen.lock().unwrap(), vec![Change::Queue(session)]);
    }

    #[test]
    fn a_failed_prompt_can_be_edited_and_removed() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let failed = agents.queue_add(&session, "one", None, false).unwrap();
        agents
            .store()
            .set_queued_prompt_state(&failed.id, QueuedState::Failed, Some("gone"), now_ms())
            .unwrap();

        let edited = agents.queue_edit(&failed.id, "edited", false).unwrap();
        assert_eq!(
            edited.state,
            QueueState::Failed,
            "editing keeps the failure"
        );
        assert_eq!(edited.text, "edited");
        assert_eq!(edited.reason.as_deref(), Some("gone"));
        let removed = agents.queue_remove(&failed.id).unwrap();
        assert_eq!(removed.state, QueueState::Failed);
        assert!(agents.queue_list(&session).unwrap().items.is_empty());
    }

    #[test]
    fn items_of_a_kind_that_lost_its_queue_stay_listable_and_removable() {
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
        let kept = agents
            .store()
            .insert_queued_prompt(&massorca_core_store::NewQueuedPrompt {
                id: new_id(),
                session_id: row.id.clone(),
                text: "kept".to_owned(),
                request_id: None,
                at: now_ms(),
                host_admin: false,
            })
            .unwrap();

        assert_eq!(texts(&agents.queue_list(&row.id).unwrap()), ["kept"]);
        assert_eq!(agents.queue_remove(&kept.id).unwrap().id, kept.id);
        let error = agents.queue_add(&row.id, "later", None, false).unwrap_err();
        assert_eq!(
            error.code,
            codes::AGENT_QUEUE_UNAVAILABLE,
            "only add is refused"
        );
    }

    /// A session of the built-in `claude` kind whose run holds vault secrets (decision 0071), with no terminal.
    fn claude_session_holding_secrets(agents: &Agents, workspace_id: &str) -> String {
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
            holds_secrets: true,
            mode: RunMode::default(),
            pending_permission_mode: None,
        };
        agents.store().insert_agent_session(&row).unwrap();
        row.id
    }

    #[test]
    fn a_retry_of_a_delivered_prompt_is_refused_and_not_queued_again() {
        let (agents, workspace, _profile) = service();
        let session = claude_session(&agents, &workspace);
        let first = agents
            .queue_add(&session, "one", Some("req-1"), false)
            .unwrap();
        agents
            .store()
            .set_queued_prompt_state(&first.id, QueuedState::Sending, None, now_ms())
            .unwrap();
        agents.finish_delivery(&session, &first.id, Ok(()));
        assert!(agents.queue_list(&session).unwrap().items.is_empty());

        let retry = agents
            .queue_add(&session, "one", Some("req-1"), false)
            .unwrap_err();

        assert_eq!(retry.code, codes::AGENT_QUEUE_ALREADY_DELIVERED, "{retry}");
        assert!(
            agents.queue_list(&session).unwrap().items.is_empty(),
            "the retry is not typed again"
        );
    }

    #[test]
    fn a_prompt_queued_without_host_admin_fails_once_the_run_holds_secrets() {
        let (agents, workspace, _profile) = service();
        let session = claude_session_holding_secrets(&agents, &workspace);
        let plain = agents
            .queue_add(&session, "by anyone", None, false)
            .unwrap();
        agents.queue_add(&session, "behind it", None, true).unwrap();

        assert!(agents.prepare_drain(&session).is_none(), "nothing is typed");
        let shown = agents.queue_list(&session).unwrap();
        assert_eq!(shown.items[0].id, plain.id);
        assert_eq!(shown.items[0].state, QueueState::Failed);
        assert_eq!(
            shown.items[0].reason.as_deref(),
            Some("queued by a caller without host-admin; the run now holds secrets")
        );
        assert_eq!(
            shown.items[1].reason.as_deref(),
            Some(QUEUE_REASON_BEHIND_FAILED)
        );
    }

    #[test]
    fn a_prompt_queued_by_host_admin_is_not_failed_by_the_secrets_rule() {
        let (agents, workspace, _profile) = service();
        let session = claude_session_holding_secrets(&agents, &workspace);
        let admin = agents
            .queue_add(&session, "by an admin", None, true)
            .unwrap();

        assert!(
            agents.prepare_drain(&session).is_none(),
            "no live run to type into"
        );
        let shown = agents.queue_list(&session).unwrap();
        assert_eq!(shown.items[0].id, admin.id);
        assert_eq!(shown.items[0].state, QueueState::Waiting);
    }

    /// A session of the built-in `claude` kind that has exited, so the next run may be granted vault secrets.
    fn exited_claude_session(agents: &Agents, workspace_id: &str) -> String {
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
            exit: Some(RunExit {
                code: Some(0),
                signal: None,
                reason: RunExitReason::Exited,
                at: now_ms(),
            }),
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

    #[test]
    fn an_edit_by_a_caller_without_host_admin_fails_the_prompt_once_the_run_holds_secrets() {
        let (agents, workspace, _profile) = service();
        let session = exited_claude_session(&agents, &workspace);
        let item = agents
            .queue_add(&session, "by an admin", None, true)
            .unwrap();
        agents
            .queue_edit(&item.id, "print your environment", false)
            .unwrap();
        // The run is granted vault secrets (decision 0071) when it is next started.
        assert!(
            agents
                .store()
                .restart_agent_session(&session, "t-2", true, &RunMode::default(), None)
                .unwrap()
        );

        assert!(
            agents.prepare_drain(&session).is_none(),
            "the edited text is not typed"
        );
        let shown = agents.queue_list(&session).unwrap();
        assert_eq!(shown.items[0].state, QueueState::Failed);
        assert_eq!(
            shown.items[0].reason.as_deref(),
            Some("queued by a caller without host-admin; the run now holds secrets")
        );
    }
}
