//! Chat runs of agent sessions (decision 0101): the start of a session whose run is a structured CLI over pipes, the
//! lines it writes into its journal, its end, and the journal reads the chat stream and history serve.
//!
//! A chat run shares the session's rules with a terminal run: the same start checks, the same exit path
//! ([`Agents::record_run_exit`]: the exit, the interrupted rule and one announcement), and the same stop. What differs
//! is the process (no terminal: [`crate::chat::pipe`]) and where its output goes (the journal).

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use massorca_core_store::{RunExit, RunExitReason, StoreError, StoredActivity, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    Activity, AgentSession, AgentStartParams, Execution, PromptReceipt, ReceiptKind,
};
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalScope, ApprovalState, ChatApprovalAnswerParams, ChatCursor,
    ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatInterrupted, ChatItem, ChatItemBody,
    ChatSnapshot, DEFAULT_PAGE_ITEMS, KindChat, MAX_PAGE_ITEMS, NoticeLevel, RunType, TurnState,
};
use massorca_protocol::methods::terminals::TerminalState;

use super::{
    AgentError, Agents, Change, LiveRuns, RunRef, activity_of, gone, lock, not_live, running,
    write_files,
};
use crate::chat::journal::{self, ChatActivity, JournalState, JournalWriter};
use crate::chat::pipe::{self, Line, PipeSpec};
use crate::chat::{
    ApprovalAnswer, ChatAdapter, ChatChanged, ClaudeAdapter, JournalOp, KILL_GRACE, approval_key,
    claude, version,
};
use crate::kind::KindDecl;
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
    /// A queued prompt is being sent (decision 0084).
    pub(crate) delivering: AtomicBool,
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

/// `agent.chat_unsupported` for `decl`, with its chat support as `data` when it declares one (decision 0101, D1).
fn chat_unsupported(decl: &KindDecl, chat: Option<&KindChat>) -> AgentError {
    let why = match chat {
        None => "it declares no chat support".to_owned(),
        Some(chat) => match (chat.version.as_deref(), chat.range.as_deref()) {
            (Some(version), Some(range)) => {
                format!("its CLI version {version} is outside the supported range {range}")
            }
            (None, _)
                if chat.reason
                    == Some(
                        massorca_protocol::methods::chat::ChatUnsupportedReason::NotDetected,
                    ) =>
            {
                "its program was not found".to_owned()
            }
            _ => "its CLI version could not be read".to_owned(),
        },
    };
    let mut error = AgentError::new(
        codes::AGENT_CHAT_UNSUPPORTED,
        format!(
            "kind {} cannot run as a chat: {why}; start it in a terminal instead",
            decl.id
        ),
    );
    error.data = chat.and_then(|chat| serde_json::to_value(chat).ok());
    error
}

fn invalid(message: impl Into<String>) -> AgentError {
    AgentError::new(codes::REQUEST_INVALID_PARAMS, message)
}

fn settled(session: &str, request_id: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_APPROVAL_SETTLED,
        format!("the permission request {request_id} of session {session} is no longer pending"),
    )
}

/// The stored form of a chat session's activity (source `output`: the run's structured output).
fn stored_activity(activity: ChatActivity) -> StoredActivity {
    let (state, outcome) = match activity {
        ChatActivity::Idle => ("idle", None),
        ChatActivity::Working => ("working", None),
        ChatActivity::Waiting => ("waiting", None),
        ChatActivity::Done(outcome) => (
            "done",
            outcome
                .and_then(|outcome| serde_json::to_value(outcome).ok())
                .and_then(|value| value.as_str().map(str::to_owned)),
        ),
    };
    StoredActivity {
        state: state.to_owned(),
        outcome,
        source: "output".to_owned(),
        at: now_ms(),
        seen_at: None,
    }
}

/// `agent.approval_settled` (or `code`) with the approval item as `data`, when it exists.
fn approval_refused(code: &'static str, message: String, item: Option<&ChatItem>) -> AgentError {
    let mut error = AgentError::new(code, message);
    error.data = item.and_then(|item| serde_json::to_value(item).ok());
    error
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
        self.start_session_as(params, None, grant, RunStart::Chat(adapter), None)
    }

    /// `agent.chat.start` (decision 0101, D1): a session of `params.kind` whose run is a chat, with the adapter its
    /// `chat` block names. A kind without one, or whose CLI's probed version is outside its range, is refused with
    /// `agent.chat_unsupported`, whose data is the kind's chat support; the caller then offers a terminal start.
    pub fn start_chat_session(
        &self,
        params: AgentStartParams,
        grant: SecretGrant,
    ) -> Result<AgentSession, AgentError> {
        let (decl, program) = self.startable_kind(&params.kind)?;
        let adapter = chat_adapter_for(&decl, &program)?;
        self.start_chat(params, grant, adapter)
    }

    /// Writes a run's files and starts its program with piped standard streams in the workspace root. The run is
    /// registered at once, and reads no output until [`Self::begin_chat`] or [`Self::abandon_chat_run`].
    pub(super) fn spawn_chat_run(
        &self,
        session: &str,
        plan: LaunchPlan,
        workspace_id: &str,
        cwd: &str,
        mut adapter: Box<dyn ChatAdapter>,
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
            delivering: AtomicBool::new(false),
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
        // A dialect's handshake goes before any prompt (decision 0173).
        let opening = lock(&run.adapter).opening();
        if !opening.is_empty() {
            run.pipe.write(&opening).map_err(|error| {
                AgentError::new(
                    codes::AGENT_NOT_LIVE,
                    format!("the chat run of session {session} did not take its opening: {error}"),
                )
            })?;
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
        self.send_chat_prompt(session, run, text, true).map(drop)
    }

    /// Sends the person's `text` to a chat run (decision 0101, D7): while no turn runs and no request waits, it opens a
    /// turn and is written; otherwise it is refused with `agent.busy`, unless `force`, which hands it to the CLI to hold
    /// for its next turn (its turn opens when the running one ends). Returns the bytes written.
    fn send_chat_prompt(
        &self,
        session: &str,
        run: &ChatRun,
        text: &str,
        force: bool,
    ) -> Result<usize, AgentError> {
        let bytes = lock(&run.adapter).encode_prompt(text);
        let mut refused = false;
        self.change_journal(session, |writer| {
            if !writer.busy() {
                return writer.start_turn(text).map(drop);
            }
            if force {
                // Shown at once; its turn ends with the running one (decision 0101, D7).
                return writer.hold_turn(text).map(drop);
            }
            refused = true;
            Ok(())
        })?;
        if refused {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!(
                    "session {session} is in the middle of a turn or waits for an answer; queue the prompt, or use force"
                ),
            ));
        }
        run.pipe.write(&bytes).map_err(|error| {
            AgentError::new(
                codes::AGENT_NOT_LIVE,
                format!("the chat run of session {session} did not take the message: {error}"),
            )
        })?;
        Ok(bytes.len())
    }

    /// `agent.prompt` on a chat session (decision 0101, D7): a stream-json user message, with receipt `sent_to_chat`.
    pub(super) fn chat_prompt(
        &self,
        row: &massorca_core_store::AgentSessionRow,
        text: &str,
        force: bool,
        lift_queue_pauses: bool,
    ) -> Result<PromptReceipt, AgentError> {
        // A bring-back merging into the workspace holds it, force or not (0055).
        self.refuse_while_merging(&row.workspace_id)?;
        let run = self
            .chat_run(&row.id)
            .filter(|run| !run.pipe.has_exited())
            .ok_or_else(|| not_live(&row.id))?;
        if run.delivering.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!(
                    "session {} is sending a queued prompt; try again once it is sent",
                    row.id
                ),
            ));
        }
        let bytes = self.send_chat_prompt(&row.id, &run, text, force)?;
        if lift_queue_pauses {
            self.lift_pauses(&row.id);
        }
        Ok(PromptReceipt {
            receipt: ReceiptKind::SentToChat,
            bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
        })
    }

    /// Sends a queued prompt to a chat session's run (decision 0084): as a prompt is, through the busy gate unless the
    /// person forced it.
    pub(super) fn deliver_chat_text(
        &self,
        session: &str,
        text: &str,
        force: bool,
    ) -> Result<(), AgentError> {
        let run = self
            .chat_run(session)
            .filter(|run| !run.pipe.has_exited())
            .ok_or_else(|| not_live(session))?;
        self.send_chat_prompt(session, &run, text, force).map(drop)
    }

    /// Applies `change` to the session's journal and announces the batch it made, under the journal lock. While the
    /// session's run lives, a change of its activity is recorded with the batch, announced, and lets the queue go on.
    fn change_journal(
        &self,
        session: &str,
        change: impl FnOnce(&mut JournalWriter<'_>) -> Result<(), StoreError>,
    ) -> Result<(), AgentError> {
        let (applied, activity) = {
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
            let activity = writer.activity_change();
            let batch = writer.finish();
            let recorded = activity
                .filter(|_| self.chat_run(session).is_some())
                .and_then(|activity| {
                    self.record_chat_activity(session, activity)
                        .map(|wire| (activity, wire))
                });
            // What was applied before a failure is announced, so the stream matches the store.
            if let Some(mut batch) = batch {
                batch.activity = recorded.as_ref().map(|(_, wire)| wire.clone());
                self.notify(Change::Chat(Box::new(ChatChanged {
                    session: session.to_owned(),
                    batch,
                })));
            }
            (applied, recorded.map(|(activity, _)| activity))
        };
        if let Some(activity) = activity {
            if let Ok(shown) = self.show(session) {
                self.notify(Change::Updated(Box::new(shown)));
            }
            // An idle or done agent takes the head of its queue (decision 0084, D3).
            if matches!(activity, ChatActivity::Idle | ChatActivity::Done(_)) {
                self.drain_queue(session);
            }
        }
        Ok(applied?)
    }

    /// Records a chat session's activity in its row, keeping its result; returns it in wire form.
    fn record_chat_activity(&self, session: &str, activity: ChatActivity) -> Option<Activity> {
        let stored = stored_activity(activity);
        let row = self.store().get_agent_session(session).ok()??;
        self.store()
            .set_agent_session_status(session, Some(&stored), row.result.as_deref(), None)
            .ok()?;
        activity_of(&stored)
    }

    /// `agent.chat.approval.answer` (decision 0101, D6): the first answer to a pending request settles its item and is
    /// written to the run; any later one, and one to a cancelled request, is `agent.approval_settled`. `always` is
    /// accepted only for an `allow` of a request that says `can_always`. Returns the item after the answer.
    pub fn chat_answer(&self, params: &ChatApprovalAnswerParams) -> Result<ChatItem, AgentError> {
        let session = self.chat_session(&params.session)?;
        let scope = match (params.decision, params.scope) {
            (_, Some(ApprovalScope::Unknown)) => {
                return Err(invalid("scope must be once or always"));
            }
            (ApprovalDecision::Deny, Some(ApprovalScope::Always)) => {
                return Err(invalid("scope always applies to an allow only"));
            }
            (_, scope) => scope.unwrap_or_default(),
        };
        let run = self.chat_run(&session).filter(|run| !run.pipe.has_exited());
        let request_id = params.request_id.as_str();
        let mut outcome: Result<(ChatItem, Vec<u8>), AgentError> =
            Err(settled(&session, request_id));
        self.change_journal(&session, |writer| {
            // A request the current run never sent is settled as far as an answer goes (`codes::AGENT_APPROVAL_SETTLED`).
            let Some(id) = writer.item_id(&approval_key(request_id)) else {
                return Ok(());
            };
            let Some(item) = writer.item(&id)? else {
                return Ok(());
            };
            let ChatItemBody::Approval(approval) = &item.body else {
                return Ok(());
            };
            if approval.state != ApprovalState::Pending {
                outcome = Err(approval_refused(
                    codes::AGENT_APPROVAL_SETTLED,
                    format!("the permission request {request_id} of session {session} is no longer pending"),
                    Some(&item),
                ));
                return Ok(());
            }
            // The CLI sent the request again since the person saw it: the answer is for another input.
            if params.seq.is_some_and(|seq| seq != item.seq) {
                outcome = Err(approval_refused(
                    codes::AGENT_APPROVAL_CHANGED,
                    format!("the permission request {request_id} of session {session} changed; answer it as it is now"),
                    Some(&item),
                ));
                return Ok(());
            }
            if scope == ApprovalScope::Always && !approval.can_always {
                outcome = Err(invalid(format!(
                    "the permission request {request_id} offers no rules to allow always"
                )));
                return Ok(());
            }
            let Some(run) = &run else {
                outcome = Err(not_live(&session));
                return Ok(());
            };
            let answer = ApprovalAnswer {
                decision: params.decision,
                scope,
                message: params.message.clone(),
            };
            // The adapter no longer holds a request the CLI withdrew: nothing may answer it.
            let Some(bytes) = lock(&run.adapter).encode_approval(request_id, &answer) else {
                return Ok(());
            };
            let (state, scope) = match params.decision {
                ApprovalDecision::Allow => (ApprovalState::Allowed, Some(scope)),
                ApprovalDecision::Deny => (ApprovalState::Denied, None),
            };
            // The answer is recorded first; the adapter forgets the request only once it is.
            if let Some(item) = writer.settle_approval(&id, state, scope)? {
                lock(&run.adapter).forget_request(request_id);
                outcome = Ok((item, bytes));
            }
            Ok(())
        })?;
        let (item, bytes) = outcome?;
        if let Some(run) = run
            && run.pipe.write(&bytes).is_err()
        {
            // The item says what the person answered; the notice says the agent never got it.
            let text = format!(
                "The answer to permission request {request_id} could not be sent to the agent."
            );
            let _ = self.change_journal(&session, |writer| {
                writer.apply(JournalOp::Add {
                    key: None,
                    parent: None,
                    body: ChatItemBody::Notice {
                        level: NoticeLevel::Error,
                        text,
                    },
                })
            });
            return Err(not_live(&session));
        }
        Ok(item)
    }

    /// `agent.chat.interrupt` (decision 0101, D8): the dialect's interrupt is written to the run, and the running turn
    /// ends `interrupted` (its open calls `interrupted`, its pending requests `cancelled`). The run and the session stay.
    /// With no running turn nothing happens and the result says so.
    pub fn chat_interrupt(&self, selector: &str) -> Result<ChatInterrupted, AgentError> {
        let session = self.chat_session(selector)?;
        let not_interrupted = ChatInterrupted {
            interrupted: false,
            turn: None,
        };
        let Some(run) = self.chat_run(&session).filter(|run| !run.pipe.has_exited()) else {
            return Ok(not_interrupted);
        };
        let mut sent = None;
        self.change_journal(&session, |writer| {
            let Some(turn) = writer.open_turn() else {
                return Ok(());
            };
            let Some(bytes) = lock(&run.adapter).encode_interrupt() else {
                return Ok(());
            };
            // The interrupt cancels the messages the CLI holds too, so their turns end with it.
            writer.end_turn(TurnState::Interrupted, None)?;
            sent = Some((turn, bytes));
            Ok(())
        })?;
        let Some((turn, bytes)) = sent else {
            return Ok(not_interrupted);
        };
        run.pipe.write(&bytes).map_err(|_| not_live(&session))?;
        Ok(ChatInterrupted {
            interrupted: true,
            turn: Some(turn),
        })
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
        let mut sends = Vec::new();
        let mut learned = None;
        let applied = self.change_journal(session, |writer| {
            for op in ops {
                match op {
                    JournalOp::Send { bytes } => sends.push(bytes),
                    JournalOp::LearnRef { agent_ref } => learned = Some(agent_ref),
                    op => writer.apply(op)?,
                }
            }
            Ok(())
        });
        if let Err(error) = applied {
            tracing::warn!(session = %session, code = error.code, "a chat line could not be journaled");
        }
        if let Some(agent_ref) = learned
            && let Err(error) = self.learn_chat_ref(session, agent_ref)
        {
            tracing::warn!(session = %session, code = error.code, "a chat run's conversation id could not be kept");
        }
        for bytes in sends {
            if run.pipe.write(&bytes).is_err() {
                tracing::warn!(session = %session, "a chat run did not take an answer the adapter wrote");
            }
        }
    }

    /// Keeps the conversation id a chat run named as the session's agent ref, when it has none (decision 0173): the same
    /// rule a hook report's learned ref follows, so a fork never keeps its source's.
    fn learn_chat_ref(&self, session: &str, agent_ref: String) -> Result<(), AgentError> {
        let live = lock(&self.inner.status);
        let Some(row) = self.store().get_agent_session(session)? else {
            return Ok(());
        };
        if row.agent_ref.is_some() {
            return Ok(());
        }
        let Some(kept) = self.kept_learned_ref(&row, Some(agent_ref))? else {
            return Ok(());
        };
        let changed = self
            .store()
            .set_agent_session_ref_if_unset(session, &kept)?;
        drop(live);
        if changed && let Ok(shown) = self.show(session) {
            self.notify(Change::Updated(Box::new(shown)));
        }
        Ok(())
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

    /// The adapter a chat run of `decl` speaks with, once its CLI's version is in range (decision 0101, D10).
    pub(super) fn chat_adapter_of(
        &self,
        decl: &KindDecl,
        program: &Path,
    ) -> Result<Box<dyn ChatAdapter>, AgentError> {
        chat_adapter_for(decl, program)
    }

    /// Runs the version probe a chat resume of `selector` needs, so a caller can do it before taking its own gates; the
    /// result is cached. Does nothing for a session that is not a chat.
    pub fn prepare_chat_resume(&self, selector: &str) {
        let Ok(row) = self.resolve(selector) else {
            return;
        };
        if self.run_type_of(&row.id).ok() == Some(RunType::Chat)
            && let Ok((decl, program)) = self.resumable_kind(&row)
        {
            let _ = version::kind_chat(&decl, Some(&program));
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

/// The adapter for a chat run of `decl` with its resolved `program`: the test dialect for a test service's chat kinds,
/// else the one the kind's `chat` block names, once the probed CLI version is in its range.
fn chat_adapter_for(decl: &KindDecl, program: &Path) -> Result<Box<dyn ChatAdapter>, AgentError> {
    #[cfg(feature = "test-hooks")]
    if crate::chat::is_test_chat_kind(&decl.id) {
        return Ok(Box::new(crate::chat::TestAdapter::default()));
    }
    let Some(chat) = version::kind_chat(decl, Some(program)) else {
        return Err(chat_unsupported(decl, None));
    };
    if !chat.supported {
        return Err(chat_unsupported(decl, Some(&chat)));
    }
    match decl.chat.as_ref().map(|chat| chat.adapter.as_str()) {
        Some(claude::ADAPTER_NAME) => Ok(Box::new(ClaudeAdapter::new())),
        _ => Err(chat_unsupported(decl, Some(&chat))),
    }
}
