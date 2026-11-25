//! Chat runs of agent sessions (decision 0101): the start of a session whose run is a structured CLI over pipes, the
//! lines it writes into its journal, its end, and the journal reads the chat stream and history serve.
//!
//! A chat run shares the session's rules with a terminal run: the same start checks, the same exit path
//! ([`Agents::record_run_exit`]: the exit, the interrupted rule and one announcement), and the same stop. What differs
//! is the process (no terminal: [`crate::chat::pipe`]) and where its output goes (the journal).

use std::sync::{Arc, Mutex};

use massorca_core_store::{RunExit, RunExitReason};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentSession, AgentStartParams, Execution};
use massorca_protocol::methods::chat::{
    ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatItemBody, ChatSnapshot,
    DEFAULT_PAGE_ITEMS, MAX_PAGE_ITEMS, NoticeLevel, RunType, TurnState,
};
use massorca_protocol::methods::terminals::TerminalState;

use super::{AgentError, Agents, Change, LiveRuns, RunRef, gone, lock, running, write_files};
use crate::chat::journal::{self, JournalState, JournalWriter};
use crate::chat::pipe::{self, Line, PipeSpec};
use crate::chat::{ChatAdapter, ChatChanged, JournalOp, KILL_GRACE};
use crate::launch::LaunchPlan;
use crate::provided_env::SecretGrant;

/// How a start runs its program.
pub(crate) enum RunStart {
    Terminal,
    Chat(Box<dyn ChatAdapter>),
}

/// What a start started: a terminal's id, or a chat run.
pub(super) enum Started {
    Terminal(String),
    Chat(Arc<ChatRun>),
}

/// A live chat run of a session.
pub(crate) struct ChatRun {
    /// Names the run in the session row, as a terminal id names a terminal run (decision 0058).
    pub(crate) run_id: String,
    pipe: Arc<pipe::PipeRun>,
    adapter: Mutex<Box<dyn ChatAdapter>>,
}

/// The first events of a chat stream, and the journal position they bring the client to.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatOpened {
    /// The session's full id.
    pub session: String,
    pub events: Vec<ChatEvent>,
    /// The cursor after the events: batches at or before it are already covered.
    pub cursor: ChatCursor,
}

fn not_chat(session: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_NOT_CHAT,
        format!("session {session} does not run as a chat"),
    )
}

fn page_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(DEFAULT_PAGE_ITEMS).clamp(1, MAX_PAGE_ITEMS)
}

impl Agents {
    /// Starts a session whose run is a chat with `adapter` (decision 0101): the start checks of [`Agents::start`], then
    /// the program with piped standard streams, its output read into the session's journal, and `params.prompt`, if
    /// any, sent as its first message. `cols`, `rows` and `args` are the caller's to refuse; they are ignored here.
    pub fn start_chat(
        &self,
        params: AgentStartParams,
        grant: SecretGrant,
        adapter: Box<dyn ChatAdapter>,
    ) -> Result<AgentSession, AgentError> {
        self.start_session_as(params, None, grant, RunStart::Chat(adapter))
    }

    /// Writes a run's files and starts its program with piped standard streams in the workspace root. The run is
    /// registered at once, and reads no output until [`Self::begin_chat`] or [`Self::abandon_chat_run`].
    pub(super) fn spawn_chat_run(
        &self,
        session: &str,
        plan: LaunchPlan,
        workspace_id: &str,
        cwd: &str,
        adapter: Box<dyn ChatAdapter>,
    ) -> Result<Arc<ChatRun>, AgentError> {
        if !plan.files.is_empty() {
            crate::credentials::create_agents_dir(self.profile_dir())?;
        }
        write_files(&plan.files)?;
        let mut env: Vec<_> = plan.env.into_iter().collect();
        env.push((
            "MASSORCA_WORKSPACE_ID".to_owned(),
            massorca_core_terminal::EnvValue::new(workspace_id.to_owned()),
        ));
        let spec = PipeSpec {
            program: plan.program,
            args: adapter.launch_args(plan.args),
            cwd: cwd.into(),
            env,
            remove_env: plan.remove_env,
        };
        let run_id = uuid::Uuid::now_v7().to_string();
        let (on_line, on_exit) = {
            let (lines, ends) = (Arc::downgrade(&self.inner), Arc::downgrade(&self.inner));
            let (line_session, end_session) = (session.to_owned(), session.to_owned());
            let (line_run, end_run) = (run_id.clone(), run_id.clone());
            (
                move |line: Line| {
                    if let Some(inner) = lines.upgrade() {
                        Self { inner }.on_chat_line(&line_session, &line_run, line);
                    }
                },
                move |exit: RunExit| {
                    if let Some(inner) = ends.upgrade() {
                        Self { inner }.on_chat_exit(&end_session, &end_run, exit);
                    }
                },
            )
        };
        let pipe = pipe::spawn(&spec, on_line, on_exit)?;
        let run = Arc::new(ChatRun {
            run_id,
            pipe,
            adapter: Mutex::new(adapter),
        });
        lock(&self.inner.chat_runs).insert(session.to_owned(), Arc::clone(&run));
        Ok(run)
    }

    /// Opens the journal of a started chat session, lets its output in, and sends its first message.
    pub(super) fn begin_chat(
        &self,
        session: &str,
        run: &ChatRun,
        prompt: Option<&str>,
    ) -> Result<(), AgentError> {
        let opened = JournalState::open(self.store(), session).map(|state| {
            // A change applied meanwhile opened the same journal; its writer state stays.
            lock(&self.inner.journals)
                .entry(session.to_owned())
                .or_insert(state);
        });
        // The output is let in either way; on an error the caller ends the run.
        run.pipe.release();
        opened?;
        if crate::chat::injected_failure("begin") {
            return Err(AgentError::new(
                codes::INTERNAL,
                format!("the chat start of session {session} failed on purpose (test)"),
            ));
        }
        if let Some(text) = prompt {
            self.send_chat_text(session, run, text)?;
        }
        Ok(())
    }

    /// Ends a chat run whose session could not be recorded: its lines are dropped and its tree is killed.
    pub(super) fn abandon_chat_run(&self, session: &str, run: &ChatRun) {
        run.pipe.discard();
        run.pipe.stop(true, RunExitReason::Killed, KILL_GRACE);
        let mut runs = lock(&self.inner.chat_runs);
        if runs
            .get(session)
            .is_some_and(|live| live.run_id == run.run_id)
        {
            runs.remove(session);
        }
    }

    /// Opens a turn with the person's `text` in the journal, then writes it to the run.
    fn send_chat_text(&self, session: &str, run: &ChatRun, text: &str) -> Result<(), AgentError> {
        let bytes = lock(&run.adapter).encode_prompt(text);
        self.change_journal(session, |writer| writer.start_turn(text).map(drop))?;
        run.pipe.write(&bytes).map_err(|error| {
            AgentError::new(
                codes::AGENT_NOT_LIVE,
                format!("the chat run of session {session} did not take the message: {error}"),
            )
        })
    }

    /// Applies `change` to the session's journal and announces the batch it made, under the journal lock.
    fn change_journal(
        &self,
        session: &str,
        change: impl FnOnce(&mut JournalWriter<'_>) -> Result<(), massorca_core_store::StoreError>,
    ) -> Result<(), AgentError> {
        let mut journals = lock(&self.inner.journals);
        if !journals.contains_key(session) {
            let state = JournalState::open(self.store(), session)?;
            journals.insert(session.to_owned(), state);
        }
        let Some(state) = journals.get_mut(session) else {
            return Ok(());
        };
        let mut writer = JournalWriter::new(self.store(), session, state);
        let applied = change(&mut writer);
        // What was applied before a failure is announced, so the stream matches the store.
        if let Some(batch) = writer.finish() {
            self.notify(Change::Chat(Box::new(ChatChanged {
                session: session.to_owned(),
                batch,
            })));
        }
        Ok(applied?)
    }

    /// One line of a chat run's standard output, into the journal through the run's adapter.
    fn on_chat_line(&self, session: &str, run_id: &str, line: Line) {
        let Some(run) = self.chat_run(session).filter(|run| run.run_id == run_id) else {
            return;
        };
        let ops = match line {
            Line::Text(text) => lock(&run.adapter).translate(&text),
            Line::TooLong => vec![JournalOp::Add {
                key: None,
                parent: None,
                body: ChatItemBody::Notice {
                    level: NoticeLevel::Warning,
                    text: "a line from the agent was too long to read and was skipped".to_owned(),
                },
            }],
        };
        let applied = self.change_journal(session, |writer| {
            ops.into_iter().try_for_each(|op| writer.apply(op))
        });
        if let Err(error) = applied {
            tracing::warn!(session = %session, code = error.code, "a chat line could not be journaled");
        }
    }

    /// A chat run's end: recorded through the session's exit path, then its open turn ends (`interrupted` when the run
    /// was stopped, `unverifiable` otherwise: nothing says how it ended).
    fn on_chat_exit(&self, session: &str, run_id: &str, exit: RunExit) {
        let stopped = exit.reason != RunExitReason::Exited;
        {
            let mut live = lock(&self.inner.status);
            match self.store().get_agent_session(session) {
                Ok(Some(mut row)) if row.exit.is_none() => {
                    if let Err(error) =
                        self.record_run_exit(&mut live, &mut row, RunRef::Chat(run_id), exit)
                    {
                        tracing::warn!(session = %session, code = error.code, "a chat run's end could not be recorded");
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(session = %session, error = %error, "a chat run's end could not be read");
                }
            }
            let mut runs = lock(&self.inner.chat_runs);
            if runs.get(session).is_some_and(|run| run.run_id == run_id) {
                runs.remove(session);
            }
        }
        if self
            .store()
            .chat_journal_head(session)
            .ok()
            .flatten()
            .is_none()
        {
            return;
        }
        let state = if stopped {
            TurnState::Interrupted
        } else {
            TurnState::Unverifiable
        };
        if let Err(error) = self.change_journal(session, |writer| writer.end_turn(state, None)) {
            tracing::warn!(session = %session, code = error.code, "a chat run's open turn could not be ended");
        }
    }

    pub(super) fn chat_run(&self, session: &str) -> Option<Arc<ChatRun>> {
        lock(&self.inner.chat_runs).get(session).cloned()
    }

    /// The newest standard error of a session's chat run, for the exit's evidence (decision 0160).
    pub(super) fn chat_output(&self, session: &str) -> Option<String> {
        self.chat_run(session).map(|run| run.pipe.stderr_tail())
    }

    /// A session's run type, from the store.
    pub(super) fn run_type_of(&self, session: &str) -> Result<RunType, AgentError> {
        Ok(
            match self
                .store()
                .agent_session_run(session)?
                .map(|run| run.run_type)
            {
                Some(massorca_core_store::SessionRunType::Chat) => RunType::Chat,
                _ => RunType::Terminal,
            },
        )
    }

    /// The execution of a chat session with no recorded end: live while its run is, else ended now as a run gone
    /// without an end the service saw (after a service restart, say).
    pub(super) fn chat_execution(
        &self,
        live: &mut LiveRuns,
        row: &mut massorca_core_store::AgentSessionRow,
    ) -> Result<Execution, AgentError> {
        if self
            .chat_run(&row.id)
            .is_some_and(|run| !run.pipe.has_exited())
        {
            return Ok(running(TerminalState::Live));
        }
        match self
            .store()
            .agent_session_run(&row.id)?
            .and_then(|run| run.run_id)
        {
            Some(run_id) => self.record_run_exit(live, row, RunRef::Chat(&run_id), gone()),
            None => Ok(Execution {
                state: TerminalState::Exited,
                exit: None,
            }),
        }
    }

    /// Ends a session's chat run as [`Agents::stop`] does a terminal's: its input closes and it has the grace to end,
    /// then its tree is killed (at once with `force`). Called without the status lock: the end it waits for takes it.
    pub(super) fn stop_chat_run(&self, session: &str, force: bool) {
        if let Some(run) = self.chat_run(session) {
            run.pipe.stop(force, RunExitReason::Killed, KILL_GRACE);
        }
    }

    /// Ends every chat run as the service stops; each end is recorded as `service_stopped`.
    pub fn stop_chat_runs(&self) {
        let runs: Vec<_> = lock(&self.inner.chat_runs).values().cloned().collect();
        for run in runs {
            run.pipe
                .stop(true, RunExitReason::ServiceStopped, KILL_GRACE);
        }
    }

    /// Drops what the service holds for a forgotten session's chat.
    pub(super) fn forget_chat(&self, session: &str) {
        lock(&self.inner.journals).remove(session);
        lock(&self.inner.chat_runs).remove(session);
    }

    /// The id of a chat session; [`codes::AGENT_NOT_CHAT`] for a session that is not one.
    fn chat_session(&self, selector: &str) -> Result<String, AgentError> {
        let row = self.resolve(selector)?;
        if self.run_type_of(&row.id)? != RunType::Chat {
            return Err(not_chat(&row.id));
        }
        Ok(row.id)
    }

    /// The opening events of `agent.chat.subscribe` (decision 0101, D4): without `after`, a snapshot of the newest page;
    /// with a current `after`, one batch of every item changed after it; with one that cannot be served, a `reset` and a
    /// snapshot. Read under the journal lock, so the returned cursor says which later batches the stream still sends.
    pub fn chat_open(
        &self,
        selector: &str,
        after: Option<&ChatCursor>,
        limit: Option<u32>,
    ) -> Result<ChatOpened, AgentError> {
        let session = self.chat_session(selector)?;
        let activity = self.show(&session)?.activity;
        let mut journals = lock(&self.inner.journals);
        if !journals.contains_key(&session) {
            let state = JournalState::open(self.store(), &session)?;
            journals.insert(session.clone(), state);
        }
        let epoch = journals
            .get(&session)
            .map(|state| state.epoch.clone())
            .unwrap_or_default();
        let mut events = Vec::new();
        if let Some(after) = after {
            match journal::replay(self.store(), &session, after)? {
                Ok(batch) => {
                    let cursor = batch.cursor.clone();
                    events.push(ChatEvent::Batch(batch));
                    return Ok(ChatOpened {
                        session,
                        events,
                        cursor,
                    });
                }
                Err(reason) => events.push(ChatEvent::Reset { reason }),
            }
        }
        let page = journal::newest_page(self.store(), &session, &epoch, page_limit(limit))?;
        let cursor = page.cursor.clone();
        events.push(ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: page.cursor,
            items: page.items,
            has_older: page.has_older,
            latest_turn: page.latest_turn,
            activity,
        })));
        Ok(ChatOpened {
            session,
            events,
            cursor,
        })
    }

    /// `agent.chat.history` (decision 0101, D5): the items before the position in `before`, oldest first, or the newest
    /// page without it. A `before` from another epoch is [`codes::AGENT_CURSOR_STALE`].
    pub fn chat_history(&self, params: &ChatHistoryParams) -> Result<ChatHistoryPage, AgentError> {
        let session = self.chat_session(&params.session)?;
        let head = match self.store().chat_journal_head(&session)? {
            Some(head) => head,
            None => {
                let mut journals = lock(&self.inner.journals);
                let state = JournalState::open(self.store(), &session)?;
                journals.insert(session.clone(), state);
                self.store()
                    .chat_journal_head(&session)?
                    .ok_or_else(|| not_chat(&session))?
            }
        };
        if let Some(before) = &params.before
            && before.epoch != head.epoch
        {
            return Err(AgentError::new(
                codes::AGENT_CURSOR_STALE,
                format!("the cursor names another journal of session {session}; subscribe again"),
            ));
        }
        let (items, has_older) = self.store().chat_items_before(
            &session,
            params.before.as_ref().map(|before| before.seq),
            page_limit(params.limit),
        )?;
        Ok(ChatHistoryPage {
            epoch: head.epoch,
            items: items.into_iter().map(journal::item_of).collect(),
            has_older,
        })
    }
}
