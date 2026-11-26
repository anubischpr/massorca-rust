//! The Conference room of a team (capability `team-room/1`, decision 0102): the lines a person and the team's bots
//! post, and the task a person's line becomes. `team-room/3` adds conversation replies and the typing line (decision 0122).

use std::fmt;

use serde::{Deserialize, Serialize};

use super::tasks::Task;
use super::teams::Routine;

/// Params: [`TeamRoomListParams`]. Result: [`TeamRoomList`], oldest first.
pub const ROOM_LIST: &str = "team.room_list";
/// Params: [`TeamRoomPostParams`]. Result: [`TeamRoomPosted`].
pub const ROOM_POST: &str = "team.room_post";
/// Params: [`TeamRoomAnswerParams`]. Result: [`TeamRoomAnswered`]. Answers a choice card once (decision 0121).
pub const ROOM_ANSWER: &str = "team.room_answer";

/// Who wrote a room line. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomAuthor {
    Person,
    Bot,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// One line of a team's room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomMessage {
    pub id: String,
    pub team_id: String,
    pub author: RoomAuthor,
    /// The bot that posted a bot line; absent once that bot was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_name: Option<String>,
    /// At most 4000 characters.
    pub text: String,
    /// The task the line is about, as the Board shows it now; absent once the task was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<Task>,
    /// The one-time routine the line scheduled (decision 0121); absent once the routine was removed, and from older hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine: Option<Routine>,
    /// The choice card the line is (decision 0121); absent on any other line, and from older hosts, which read the
    /// line's text alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<RoomCard>,
    /// The line this one replies to (decision 0122); absent from older hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// The line it replies to, as the room shows it; absent once that line was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted: Option<RoomQuote>,
    pub created_at_ms: i64,
}

/// The line a reply quotes (decision 0122).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomQuote {
    pub id: String,
    pub author: RoomAuthor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_name: Option<String>,
    /// At most 200 characters of its text.
    pub text: String,
}

/// A bot of the team that works on a room reply or a room task now (decision 0122).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomTyping {
    pub bot_id: String,
    pub bot_name: String,
}

/// A choice card: the room could not tell who takes a person's request and asks where it goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomCard {
    /// The person's line the card asks about.
    pub request_id: String,
    pub options: Vec<RoomChoice>,
    /// The `choice` it was answered with; absent while it waits for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

/// One option of a choice card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomChoice {
    /// What `team.room_answer` sends: `bot:<id>`, `team:<id>` or `todo`.
    pub choice: String,
    pub kind: RoomChoiceKind,
    /// The bot's or the team's name, or "Keep in this team's Todo".
    pub label: String,
}

/// Where an option sends the request. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomChoiceKind {
    /// A bot of this team takes it, as a line naming that bot would.
    Bot,
    /// Another team's Board gets it, in Todo.
    Team,
    /// This team's Board gets it in Todo, with no bot.
    Todo,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `team.room_list`. `limit` keeps the newest that many lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomListParams {
    /// Team id or exact name.
    pub team: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Result of `team.room_list`, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomList {
    pub messages: Vec<TeamRoomMessage>,
    /// The bots working on a reply or a room task now, in team order (decision 0122); empty from older hosts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub typing: Vec<RoomTyping>,
}

/// Params of `team.room_post`. Without `bot`, a person posts and the line becomes a task for a bot of the team: the bot
/// named with `@name` at the start of a word, else `to`, else the team's lead, else its only bot. With several bots and
/// none of those, the room answers with a choice card instead (decision 0121). A line that names one
/// day and one time becomes a one-time routine of that bot instead (decision 0121). With `bot`, that bot
/// posts. An agent run sends its `session` and `agent_token`, and then posts only as the bot of its own task or reply.
/// With `ask`, or `reply_to` a bot's line, the line is a conversation: one bot answers it once and no task is made
/// (decision 0122).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomPostParams {
    /// Team id or exact name.
    pub team: String,
    pub text: String,
    /// The bot that posts: id, or exact name within the team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    /// The bot a person's line goes to when it names none: id, or exact name within the team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// The agent session that posts (`MASSORCA_AGENT_SESSION`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The run token of `session` (`MASSORCA_AGENT_TOKEN`). `Debug` never prints it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
    /// `false` leaves a person's task in Todo (the Board's Open column) instead of starting it; absent starts it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    /// The line this one replies to, kept as its context (decision 0122).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// `true` asks a bot without making a task; `false` makes a reply to a bot's line a task. Absent: a reply to a bot's
    /// line is a conversation and any other line is a task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<bool>,
}

impl fmt::Debug for TeamRoomPostParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TeamRoomPostParams")
            .field("team", &self.team)
            .field("text", &self.text)
            .field("bot", &self.bot)
            .field("to", &self.to)
            .field("start", &self.start)
            .field("reply_to", &self.reply_to)
            .field("ask", &self.ask)
            .field("session", &self.session)
            .field(
                "agent_token",
                &self.agent_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

/// Result of `team.room_post`: the lines it added, oldest first. A person's line is followed by the bot's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomPosted {
    pub messages: Vec<TeamRoomMessage>,
}

/// Params of `team.room_answer`: answers the choice card `message` of `team` with one of its options' `choice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomAnswerParams {
    /// Team id or exact name.
    pub team: String,
    /// The card's line id.
    pub message: String,
    /// One option's `choice`: `bot:<id>`, `team:<id>` or `todo`.
    pub choice: String,
}

/// Result of `team.room_answer`: the card with its answer, and the lines the answer added, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRoomAnswered {
    pub card: TeamRoomMessage,
    pub messages: Vec<TeamRoomMessage>,
}

/// Params: [`TeamMessageSendParams`]. Result: [`TeamMessageSent`]. Called by a bot's run (capability `team-messages/1`).
pub const MESSAGE_SEND: &str = "team.message_send";
/// Params: [`TeamMessageReplyParams`]. Result: [`TeamMessageSent`]. Answers a message in the caller's inbox.
pub const MESSAGE_REPLY: &str = "team.message_reply";
/// Params: [`TeamInboxParams`]. Result: [`TeamInbox`], the caller's unread messages, oldest first.
pub const INBOX: &str = "team.inbox";
/// The address of a run's parent task in [`TeamMessageSendParams::to`].
pub const TO_PARENT: &str = "parent";
/// The most bytes a message body holds, so a delivered message fits one agent prompt.
pub const MAX_MESSAGE_BODY_BYTES: usize = 6000;
/// The most characters a message subject holds.
pub const MAX_MESSAGE_SUBJECT_CHARS: usize = 200;

/// What a message between bots is (decision 0129). A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Status,
    Question,
    /// A child's report that its task ended; it carries an outcome and settles that task.
    WorkerDone,
    Handoff,
    Escalation,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// How soon a message wants its reader. `high` and `urgent` wake a live parent run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagePriority {
    #[default]
    Normal,
    High,
    Urgent,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The outcome a `worker_done` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutcome {
    Succeeded,
    Failed,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where a message went: the recipient's inbox only, or also its live run's prompt queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDelivery {
    #[default]
    Inbox,
    /// Handed to the recipient's live run as a prompt; it stays in the inbox too.
    Run,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `team.message_send`. The run names its own `session` and `agent_token`; the sender is that run's task and
/// bot. `body` is never logged.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMessageSendParams {
    /// [`TO_PARENT`], or the id of a child task the sender's task delegated.
    pub to: String,
    #[serde(rename = "type")]
    pub kind: MessageType,
    #[serde(default)]
    pub priority: MessagePriority,
    pub subject: String,
    pub body: String,
    /// Required on `worker_done`, refused on any other type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<WorkerOutcome>,
    /// The task a `worker_done` settles. Absent: the sender's own task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The thread the message continues. Absent: a new thread, named by the message's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    /// The message this one answers; its thread is kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The run token of `session`. `Debug` never prints it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
}

impl fmt::Debug for TeamMessageSendParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TeamMessageSendParams")
            .field("to", &self.to)
            .field("kind", &self.kind)
            .field("priority", &self.priority)
            .field("subject", &self.subject)
            .field("body", &"[redacted]")
            .field("outcome", &self.outcome)
            .field("task", &self.task)
            .field("thread", &self.thread)
            .field("reply_to", &self.reply_to)
            .field("session", &self.session)
            .field("agent_token", &redacted(self.agent_token.as_ref()))
            .finish()
    }
}

/// Params of `team.message_reply`: answers `message`, which must be in the caller's inbox, on its thread.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMessageReplyParams {
    pub message: String,
    pub body: String,
    /// Absent: `status`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MessageType>,
    #[serde(default)]
    pub priority: MessagePriority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
}

impl fmt::Debug for TeamMessageReplyParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TeamMessageReplyParams")
            .field("message", &self.message)
            .field("body", &"[redacted]")
            .field("kind", &self.kind)
            .field("priority", &self.priority)
            .field("session", &self.session)
            .field("agent_token", &redacted(self.agent_token.as_ref()))
            .finish()
    }
}

/// Params of `team.inbox`. `mark_read` marks the listed messages read after listing them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamInboxParams {
    #[serde(default, skip_serializing_if = "is_false")]
    pub mark_read: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
}

impl fmt::Debug for TeamInboxParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TeamInboxParams")
            .field("mark_read", &self.mark_read)
            .field("session", &self.session)
            .field("agent_token", &redacted(self.agent_token.as_ref()))
            .finish()
    }
}

fn redacted(token: Option<&String>) -> Option<&'static str> {
    token.map(|_| "[redacted]")
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// One message between two bots of a delegation (decision 0129).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: MessageType,
    pub priority: MessagePriority,
    pub subject: String,
    pub body: String,
    pub thread: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<WorkerOutcome>,
    /// The task a `worker_done` settled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settles: Option<String>,
    pub from_task: String,
    /// The sending bot's name; absent once it was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_bot: Option<String>,
    pub to_task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_bot: Option<String>,
    #[serde(default)]
    pub delivery: MessageDelivery,
    #[serde(default, skip_serializing_if = "is_false")]
    pub read: bool,
    pub created_at_ms: i64,
}

/// Result of `team.message_send` and `team.message_reply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMessageSent {
    pub message: TeamMessage,
    /// `true` when a `worker_done` repeated one already stored: `message` is the first one, and nothing changed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub duplicate: bool,
}

/// Result of `team.inbox`: the caller's unread messages, oldest first, as they were before `mark_read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamInbox {
    pub messages: Vec<TeamMessage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_author_from_a_newer_peer_decodes_as_unknown() {
        let author: RoomAuthor = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(author, RoomAuthor::Unknown);
    }

    #[test]
    fn a_line_from_an_older_host_has_no_card_and_an_unknown_choice_kind_decodes_as_unknown() {
        let line: TeamRoomMessage = serde_json::from_value(serde_json::json!({
            "id": "m1", "team_id": "t1", "author": "bot", "text": "Who should take this?",
            "created_at_ms": 1
        }))
        .unwrap();
        assert_eq!(line.card, None);
        assert_eq!((line.reply_to, line.quoted), (None, None));
        let list: TeamRoomList =
            serde_json::from_value(serde_json::json!({"messages": []})).unwrap();
        assert!(list.typing.is_empty(), "an older host sends no typing line");
        let choice: RoomChoice = serde_json::from_value(
            serde_json::json!({"choice": "new_bot", "kind": "new_bot", "label": "Create a bot"}),
        )
        .unwrap();
        assert_eq!(choice.kind, RoomChoiceKind::Unknown);
    }

    #[test]
    fn a_post_without_a_bot_omits_it_on_the_wire() {
        let mut params = TeamRoomPostParams {
            team: "Desarrollo".to_owned(),
            text: "Add magic link sign-in".to_owned(),
            bot: None,
            to: None,
            session: None,
            agent_token: None,
            start: None,
            reply_to: None,
            ask: None,
        };
        let wire = serde_json::to_value(&params).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({"team": "Desarrollo", "text": "Add magic link sign-in"})
        );
        params.agent_token = Some("run-secret".to_owned());
        assert!(!format!("{params:?}").contains("run-secret"));
    }
}
