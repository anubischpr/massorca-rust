//! Messages between the bots of a delegation (capability `team-messages/1`, decision 0129). A bot's run sends to its
//! parent task's bot or to a child task's bot, only as itself. A child's `worker_done` settles its own task once. Every
//! message waits in its recipient's inbox. Bodies are never logged.

use massorca_core_agents::Agents;
use massorca_core_store::{
    BotRow, Store, TaskOutcome, TaskRow, TeamMessageRow, WorkerDoneStored, new_id, now_ms,
};
use massorca_protocol::methods::events::{TOPIC_TASK_CHANGED, TOPIC_TEAM_ROOM_CHANGED};
use massorca_protocol::methods::team_room::{
    self, MAX_MESSAGE_BODY_BYTES, MAX_MESSAGE_SUBJECT_CHARS, MessageDelivery, MessagePriority,
    MessageType, TO_PARENT, TeamMessage, TeamMessageSendParams, TeamMessageSent, WorkerOutcome,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::team_delegate::bot_run;
use super::{blocking, invalid, non_blank, optional_text, store_error};

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::TEAM_MESSAGES, Owner::Core)?,
        methods.register(
            Owner::Core,
            team_room::MESSAGE_SEND,
            capabilities::TEAM_MESSAGES,
            Permission::Workspace,
            send,
        )?,
    ])
}

/// The tasks and rooms a call changed, published after it returns.
#[derive(Debug, Default)]
pub(super) struct Touched {
    tasks: Vec<String>,
    rooms: Vec<String>,
}

fn publish_touched(ctx: &RequestCtx, touched: &Touched) {
    for task in &touched.tasks {
        publish(ctx, TOPIC_TASK_CHANGED, task);
    }
    for team in &touched.rooms {
        publish(ctx, TOPIC_TEAM_ROOM_CHANGED, team);
    }
}

async fn send(
    ctx: RequestCtx,
    params: TeamMessageSendParams,
) -> Result<TeamMessageSent, ErrorBody> {
    let (touched, sent) = blocking(&ctx, move |store, agents| {
        let mut touched = Touched::default();
        let sent = send_as_run(store, agents, params, &mut touched);
        Ok((touched, sent))
    })
    .await?;
    publish_touched(&ctx, &touched);
    sent
}

/// The calling run's task, bot and session: only a live bot run sends, and only as itself.
fn sender(
    store: &Store,
    agents: &Agents,
    session: Option<String>,
    token: Option<String>,
) -> Result<(TaskRow, BotRow, String), ErrorBody> {
    let (task, bot) = bot_run(store, agents, session.as_deref(), token.as_deref(), || {
        ErrorBody::new(
            codes::TEAM_MESSAGE_NOT_A_BOT,
            "only a bot's agent run on a task can send or read team messages, with its own session and run token",
        )
    })?;
    Ok((task, bot, session.unwrap_or_default()))
}

/// What a bot asks to send.
#[derive(Debug, Clone)]
pub(super) struct Draft {
    pub(super) to: String,
    pub(super) kind: MessageType,
    pub(super) priority: MessagePriority,
    pub(super) subject: String,
    pub(super) body: String,
    pub(super) outcome: Option<WorkerOutcome>,
    pub(super) task: Option<String>,
    pub(super) thread: Option<String>,
    pub(super) reply_to: Option<String>,
}

/// The sending run: its task, its bot and its session.
pub(super) struct SenderRun<'a> {
    pub(super) task: &'a TaskRow,
    pub(super) bot: &'a BotRow,
    pub(super) session: &'a str,
}

fn send_as_run(
    store: &Store,
    agents: &Agents,
    params: TeamMessageSendParams,
    touched: &mut Touched,
) -> Result<TeamMessageSent, ErrorBody> {
    let (task, bot, session) = sender(store, agents, params.session, params.agent_token)?;
    let draft = Draft {
        to: params.to,
        kind: params.kind,
        priority: params.priority,
        subject: params.subject,
        body: params.body,
        outcome: params.outcome,
        task: params.task,
        thread: params.thread,
        reply_to: params.reply_to,
    };
    let from = SenderRun {
        task: &task,
        bot: &bot,
        session: &session,
    };
    deliver(store, &from, draft, touched)
}

/// Stores a message along one delegation edge and settles the task a `worker_done` names.
pub(super) fn deliver(
    store: &Store,
    from: &SenderRun<'_>,
    draft: Draft,
    touched: &mut Touched,
) -> Result<TeamMessageSent, ErrorBody> {
    let kind = checked_kind(draft.kind)?;
    if draft.priority == MessagePriority::Unknown {
        return Err(invalid("priority must be normal, high or urgent"));
    }
    let subject = checked_subject(&draft.subject)?;
    let body = checked_body(&draft.body)?;
    let (recipient, upward) = recipient(store, from.task, &draft.to)?;
    let outcome = checked_outcome(kind, draft.outcome)?;
    let id = new_id();
    let thread = match optional_text(draft.reply_to.clone()) {
        Some(answered) => {
            let row = store
                .get_team_message(&answered)
                .map_err(store_error)?
                .filter(|row| row.to_task_id == from.task.id || row.from_task_id == from.task.id)
                .ok_or_else(|| unknown_message(&answered))?;
            row.thread_id
        }
        None => optional_text(draft.thread.clone()).unwrap_or_else(|| id.clone()),
    };
    let mut row = TeamMessageRow {
        id,
        from_task_id: from.task.id.clone(),
        to_task_id: recipient.id.clone(),
        from_bot_id: Some(from.bot.id.clone()),
        to_bot_id: recipient.bot_id.clone(),
        kind: wire_name(&kind),
        priority: wire_name(&draft.priority),
        subject,
        body,
        thread_id: thread,
        reply_to: optional_text(draft.reply_to),
        outcome: outcome.as_ref().map(wire_name),
        settles_task_id: None,
        delivery: wire_name(&MessageDelivery::Inbox),
        read_at: None,
        created_at: now_ms(),
    };
    if let Some(outcome) = outcome {
        let settles = settled_task(store, from.task, &recipient, draft.task)?;
        row.settles_task_id = Some(settles.id.clone());
        match store
            .insert_worker_done(&row, from.session, task_outcome(outcome))
            .map_err(store_error)?
        {
            WorkerDoneStored::Duplicate(first) => {
                return Ok(TeamMessageSent {
                    message: view(store, &first)?,
                    duplicate: true,
                });
            }
            WorkerDoneStored::Conflict(current) => {
                return Err(already_settled(&settles.id, current));
            }
            WorkerDoneStored::Settled | WorkerDoneStored::Matched => {}
        }
    } else {
        store.insert_team_message(&row).map_err(store_error)?;
    }
    touched
        .tasks
        .extend([from.task.id.clone(), recipient.id.clone()]);
    // The room re-reads its task cards' delegation trees, which show each child's report.
    let parent_team = if upward {
        &recipient.team_id
    } else {
        &from.task.team_id
    };
    touched.rooms.push(parent_team.clone());
    Ok(TeamMessageSent {
        message: view(store, &row)?,
        duplicate: false,
    })
}

/// The task a message goes to, and whether it goes up to the sender's parent. Only the sender's parent and the children
/// its task delegated are reachable.
fn recipient(store: &Store, sender: &TaskRow, to: &str) -> Result<(TaskRow, bool), ErrorBody> {
    let to = non_blank(to, "the recipient")?;
    let parent = store
        .task_delegation_parent(&sender.id)
        .map_err(store_error)?
        .map(|(parent, _)| parent);
    if to == TO_PARENT || parent.as_deref() == Some(to.as_str()) {
        let parent = parent.ok_or_else(|| {
            no_edge(&format!(
                "task {} was not delegated, so it has no parent to message",
                sender.id
            ))
        })?;
        let row = store
            .get_task(&parent)
            .map_err(store_error)?
            .ok_or_else(|| no_edge("the parent task was removed"))?;
        return Ok((row, true));
    }
    let child = store.get_task(&to).map_err(store_error)?;
    let delegated_here = store
        .task_delegation_parent(&to)
        .map_err(store_error)?
        .is_some_and(|(parent, _)| parent == sender.id);
    match child {
        Some(child) if delegated_here => Ok((child, false)),
        _ => Err(no_edge(&format!(
            "{to} is neither the parent of task {} nor a child it delegated",
            sender.id
        ))),
    }
}

/// The task a `worker_done` settles: the sender's own task, delegated by the recipient.
fn settled_task(
    store: &Store,
    sender: &TaskRow,
    recipient: &TaskRow,
    named: Option<String>,
) -> Result<TaskRow, ErrorBody> {
    let task = match optional_text(named) {
        Some(id) => store.get_task(&id).map_err(store_error)?.ok_or_else(|| {
            ErrorBody::new(
                codes::TEAM_MESSAGE_UNKNOWN_TASK,
                format!("no task {id} to settle"),
            )
        })?,
        None => sender.clone(),
    };
    let parent = store
        .task_delegation_parent(&task.id)
        .map_err(store_error)?
        .map(|(parent, _)| parent);
    if task.id != sender.id || parent.as_deref() != Some(recipient.id.as_str()) {
        return Err(ErrorBody::new(
            codes::TEAM_MESSAGE_NOT_YOUR_CHILD,
            format!(
                "task {} is not the sender's own task delegated by task {}; a worker_done settles only that",
                task.id, recipient.id
            ),
        ));
    }
    Ok(task)
}

fn checked_kind(kind: MessageType) -> Result<MessageType, ErrorBody> {
    if kind == MessageType::Unknown {
        return Err(invalid(
            "type must be status, question, worker_done, handoff or escalation",
        ));
    }
    Ok(kind)
}

fn checked_outcome(
    kind: MessageType,
    outcome: Option<WorkerOutcome>,
) -> Result<Option<WorkerOutcome>, ErrorBody> {
    match (kind, outcome) {
        (MessageType::WorkerDone, Some(WorkerOutcome::Succeeded | WorkerOutcome::Failed)) => {
            Ok(outcome)
        }
        (MessageType::WorkerDone, _) => Err(invalid(
            "a worker_done needs an outcome: succeeded or failed",
        )),
        (_, None) => Ok(None),
        (_, Some(_)) => Err(invalid("only a worker_done carries an outcome")),
    }
}

fn checked_subject(subject: &str) -> Result<String, ErrorBody> {
    let subject = non_blank(subject, "the subject")?;
    if subject.chars().count() > MAX_MESSAGE_SUBJECT_CHARS {
        return Err(invalid(format!(
            "a subject holds at most {MAX_MESSAGE_SUBJECT_CHARS} characters"
        )));
    }
    if subject.chars().any(char::is_control) {
        return Err(invalid("a subject is one line without control characters"));
    }
    Ok(subject)
}

/// A body fits one agent prompt and holds no control character a terminal could act on (decision 0058).
fn checked_body(body: &str) -> Result<String, ErrorBody> {
    let body = non_blank(body, "the body")?;
    if body.len() > MAX_MESSAGE_BODY_BYTES {
        return Err(invalid(format!(
            "a body holds at most {MAX_MESSAGE_BODY_BYTES} bytes"
        )));
    }
    if body
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\n' | '\r'))
    {
        return Err(invalid(
            "a body holds no control characters other than tab and line breaks",
        ));
    }
    Ok(body)
}

fn task_outcome(outcome: WorkerOutcome) -> TaskOutcome {
    match outcome {
        WorkerOutcome::Succeeded => TaskOutcome::Success,
        WorkerOutcome::Failed | WorkerOutcome::Unknown => TaskOutcome::Failure,
    }
}

fn view(store: &Store, row: &TeamMessageRow) -> Result<TeamMessage, ErrorBody> {
    Ok(TeamMessage {
        id: row.id.clone(),
        kind: from_wire(&row.kind),
        priority: from_wire(&row.priority),
        subject: row.subject.clone(),
        body: row.body.clone(),
        thread: row.thread_id.clone(),
        reply_to: row.reply_to.clone(),
        outcome: row.outcome.as_deref().map(from_wire),
        settles: row.settles_task_id.clone(),
        from_task: row.from_task_id.clone(),
        from_bot: bot_name(store, row.from_bot_id.as_deref())?,
        to_task: row.to_task_id.clone(),
        to_bot: bot_name(store, row.to_bot_id.as_deref())?,
        delivery: from_wire(&row.delivery),
        read: row.read_at.is_some(),
        created_at_ms: row.created_at,
    })
}

fn bot_name(store: &Store, id: Option<&str>) -> Result<Option<String>, ErrorBody> {
    let Some(id) = id else {
        return Ok(None);
    };
    Ok(store.get_bot(id).map_err(store_error)?.map(|bot| bot.name))
}

/// The protocol's name of a unit enum value, as the store keeps it.
fn wire_name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// A stored name read back; a name this build does not know is the enum's `Unknown`.
fn from_wire<T: DeserializeOwned + UnknownValue>(name: &str) -> T {
    serde_json::from_value(serde_json::Value::String(name.to_owned()))
        .unwrap_or_else(|_| T::unknown())
}

/// The value a stored name this build cannot read becomes.
trait UnknownValue {
    fn unknown() -> Self;
}

impl UnknownValue for MessageType {
    fn unknown() -> Self {
        Self::Unknown
    }
}

impl UnknownValue for MessagePriority {
    fn unknown() -> Self {
        Self::Unknown
    }
}

impl UnknownValue for WorkerOutcome {
    fn unknown() -> Self {
        Self::Unknown
    }
}

impl UnknownValue for MessageDelivery {
    fn unknown() -> Self {
        Self::Unknown
    }
}

fn no_edge(message: &str) -> ErrorBody {
    ErrorBody::new(codes::TEAM_MESSAGE_NO_EDGE, message)
}

fn unknown_message(id: &str) -> ErrorBody {
    ErrorBody::new(
        codes::TEAM_MESSAGE_UNKNOWN,
        format!("no message {id} in this run's conversations"),
    )
}

fn already_settled(task: &str, current: Option<TaskOutcome>) -> ErrorBody {
    let shown = current.map(|outcome| outcome.as_str());
    let mut error = ErrorBody::new(
        codes::TEAM_MESSAGE_ALREADY_SETTLED,
        format!(
            "task {task} already ended ({}); a worker_done cannot change it",
            shown.unwrap_or("unknown")
        ),
    );
    error.data = Some(serde_json::json!({ "task": task, "outcome": shown }));
    error
}

#[cfg(test)]
#[path = "team_message_tests.rs"]
mod tests;
