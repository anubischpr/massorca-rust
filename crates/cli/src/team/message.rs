//! `massorca team message send|reply` and `massorca team inbox` (decision 0129): a bot's run messages its parent's or a
//! child's bot and reads its own inbox. The run's session and token come from its environment, so these are refused
//! before connecting when they are not typed inside an agent run.

use clap::{Args, Subcommand, ValueEnum};
use massorca_protocol::codes;
use massorca_protocol::methods::team_room::{
    MessageDelivery, MessagePriority, MessageType, TeamInbox, TeamInboxParams, TeamMessage,
    TeamMessageReplyParams, TeamMessageSendParams, TeamMessageSent, WorkerOutcome,
};

use crate::agent_hook::{ENV_SESSION, ENV_TOKEN};
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;

#[derive(Debug, Subcommand)]
pub enum MessageCommand {
    /// Sends a message to this run's parent task's bot (--to parent) or to a child task's bot (--to <task id>).
    Send(SendArgs),
    /// Answers a message in this run's inbox, on its thread.
    Reply(ReplyArgs),
}

#[derive(Debug, Args)]
pub struct SendArgs {
    /// `parent`, or the id of a child task this run's task delegated.
    #[arg(long, value_name = "TASK")]
    pub to: String,
    /// What the message is. A worker_done reports this run's task ended and needs --outcome.
    #[arg(long = "type", value_enum, value_name = "TYPE")]
    pub kind: TypeArg,
    /// How soon it wants its reader; high and urgent wake a live parent run.
    #[arg(long, value_enum, value_name = "PRIORITY", default_value = "normal")]
    pub priority: PriorityArg,
    #[arg(long, value_name = "TEXT")]
    pub subject: String,
    #[arg(long, value_name = "TEXT")]
    pub body: String,
    /// How this run's task ended; only with --type worker_done.
    #[arg(long, value_enum, value_name = "OUTCOME")]
    pub outcome: Option<OutcomeArg>,
    /// The id of the message this one answers; its thread is kept.
    #[arg(long, value_name = "MESSAGE")]
    pub reply_to: Option<String>,
}

#[derive(Debug, Args)]
pub struct ReplyArgs {
    /// The id of a message in this run's inbox.
    pub message: String,
    #[arg(long, value_name = "TEXT")]
    pub body: String,
    /// What the answer is. Defaults to status.
    #[arg(long = "type", value_enum, value_name = "TYPE")]
    pub kind: Option<TypeArg>,
    #[arg(long, value_enum, value_name = "PRIORITY", default_value = "normal")]
    pub priority: PriorityArg,
}

#[derive(Debug, Args)]
pub struct InboxArgs {
    /// Marks the listed messages read, so the next inbox leaves them out.
    #[arg(long)]
    pub mark_read: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeArg {
    Status,
    Question,
    #[value(name = "worker_done", alias = "worker-done")]
    WorkerDone,
    Handoff,
    Escalation,
}

impl From<TypeArg> for MessageType {
    fn from(value: TypeArg) -> Self {
        match value {
            TypeArg::Status => Self::Status,
            TypeArg::Question => Self::Question,
            TypeArg::WorkerDone => Self::WorkerDone,
            TypeArg::Handoff => Self::Handoff,
            TypeArg::Escalation => Self::Escalation,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PriorityArg {
    Normal,
    High,
    Urgent,
}

impl From<PriorityArg> for MessagePriority {
    fn from(value: PriorityArg) -> Self {
        match value {
            PriorityArg::Normal => Self::Normal,
            PriorityArg::High => Self::High,
            PriorityArg::Urgent => Self::Urgent,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutcomeArg {
    Succeeded,
    Failed,
}

impl From<OutcomeArg> for WorkerOutcome {
    fn from(value: OutcomeArg) -> Self {
        match value {
            OutcomeArg::Succeeded => Self::Succeeded,
            OutcomeArg::Failed => Self::Failed,
        }
    }
}

/// The run's session and token, or the refusal a person typing the command gets.
fn run_env() -> Result<(String, String), CliError> {
    let value = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    match (value(ENV_SESSION), value(ENV_TOKEN)) {
        (Some(session), Some(token)) => Ok((session, token)),
        _ => Err(CliError::new(
            codes::TEAM_MESSAGE_NOT_A_BOT,
            "team messages are sent and read only inside a bot's agent run; a person talks to bots in the Conference room",
            exit::ERROR,
        )),
    }
}

/// Runs `team message send|reply`. Outside an agent run it refuses and connects to nothing.
pub async fn run(
    target: &Target,
    autostart: bool,
    command: MessageCommand,
    output: Output,
) -> Result<(), CliError> {
    let (session, token) = run_env()?;
    let client = connect(target, autostart).await?;
    let sent = match command {
        MessageCommand::Send(args) => {
            client
                .team_message_send(&TeamMessageSendParams {
                    to: args.to,
                    kind: args.kind.into(),
                    priority: args.priority.into(),
                    subject: args.subject,
                    body: args.body,
                    outcome: args.outcome.map(WorkerOutcome::from),
                    task: None,
                    thread: None,
                    reply_to: args.reply_to,
                    session: Some(session),
                    agent_token: Some(token),
                })
                .await?
        }
        MessageCommand::Reply(args) => {
            client
                .team_message_reply(&TeamMessageReplyParams {
                    message: args.message,
                    body: args.body,
                    kind: args.kind.map(MessageType::from),
                    priority: args.priority.into(),
                    session: Some(session),
                    agent_token: Some(token),
                })
                .await?
        }
    };
    output.success(&sent, &[sent_line(&sent)]);
    Ok(())
}

/// Runs `team inbox`. Outside an agent run it refuses and connects to nothing.
pub async fn inbox(
    target: &Target,
    autostart: bool,
    args: InboxArgs,
    output: Output,
) -> Result<(), CliError> {
    let (session, token) = run_env()?;
    let client = connect(target, autostart).await?;
    let inbox = client
        .team_inbox(&TeamInboxParams {
            mark_read: args.mark_read,
            session: Some(session),
            agent_token: Some(token),
        })
        .await?;
    output.success(&inbox, &inbox_lines(&inbox));
    Ok(())
}

fn sent_line(sent: &TeamMessageSent) -> String {
    let message = &sent.message;
    let to = if message.to_bot.is_some() {
        format!(
            "{} (task {})",
            message.to_bot.as_deref().unwrap_or_default(),
            message.to_task
        )
    } else {
        format!("task {}", message.to_task)
    };
    let place = match message.delivery {
        MessageDelivery::Run => "delivered to its live run",
        _ => "waiting in its inbox",
    };
    if sent.duplicate {
        return format!(
            "already reported as message {} to {to}; nothing changed",
            message.id
        );
    }
    format!("sent message {} to {to}, {place}", message.id)
}

fn inbox_lines(inbox: &TeamInbox) -> Vec<String> {
    if inbox.messages.is_empty() {
        return vec!["no unread messages".to_owned()];
    }
    inbox.messages.iter().flat_map(message_lines).collect()
}

/// A message's header and its body, indented, so a run reads everything it needs to answer.
fn message_lines(message: &TeamMessage) -> Vec<String> {
    let from = message.from_bot.as_deref().unwrap_or("a removed bot");
    let kind = serde_name(&message.kind);
    let outcome = message
        .outcome
        .map(|outcome| format!(" {}", serde_name(&outcome)))
        .unwrap_or_default();
    let mut lines = vec![format!(
        "{} from {from} (task {}): {kind}{outcome}, {}: {}",
        message.id,
        message.from_task,
        serde_name(&message.priority),
        message.subject
    )];
    lines.extend(message.body.lines().map(|line| format!("  {line}")));
    lines
}

fn serde_name<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => "unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(delivery: MessageDelivery) -> TeamMessage {
        TeamMessage {
            id: "m1".to_owned(),
            kind: MessageType::WorkerDone,
            priority: MessagePriority::High,
            subject: "Lexer done".to_owned(),
            body: "All tests pass.\nNumbers too.".to_owned(),
            thread: "m1".to_owned(),
            reply_to: None,
            outcome: Some(WorkerOutcome::Succeeded),
            settles: Some("c1".to_owned()),
            from_task: "c1".to_owned(),
            from_bot: Some("Jun".to_owned()),
            to_task: "p1".to_owned(),
            to_bot: Some("Mara".to_owned()),
            delivery,
            read: false,
            created_at_ms: 1,
        }
    }

    #[test]
    fn an_inbox_line_shows_the_header_then_the_body_indented() {
        let lines = message_lines(&message(MessageDelivery::Inbox));
        assert_eq!(
            lines,
            vec![
                "m1 from Jun (task c1): worker_done succeeded, high: Lexer done".to_owned(),
                "  All tests pass.".to_owned(),
                "  Numbers too.".to_owned(),
            ]
        );
    }

    #[test]
    fn a_sent_line_says_where_the_message_went_and_a_duplicate_changed_nothing() {
        let run = TeamMessageSent {
            message: message(MessageDelivery::Run),
            duplicate: false,
        };
        assert_eq!(
            sent_line(&run),
            "sent message m1 to Mara (task p1), delivered to its live run"
        );
        let repeat = TeamMessageSent {
            message: message(MessageDelivery::Inbox),
            duplicate: true,
        };
        assert!(sent_line(&repeat).contains("nothing changed"));
    }
}
