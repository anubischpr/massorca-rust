//! The Conference room of a team (capability `team-room/1`, decision 0102): the lines a person and the team's bots
//! post, and the task a person's line becomes.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::tasks::Task;
use super::teams::Routine;

/// Params: [`TeamRoomListParams`]. Result: [`TeamRoomList`], oldest first.
pub const ROOM_LIST: &str = "team.room_list";
/// Params: [`TeamRoomPostParams`]. Result: [`TeamRoomPosted`].
pub const ROOM_POST: &str = "team.room_post";

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
    pub created_at_ms: i64,
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
}

/// Params of `team.room_post`. Without `bot`, a person posts and the line becomes a task for a bot of the team: the bot
/// named with `@name` at the start of a word, else `to`, else the team's lead, else its first bot. A line that names one
/// day and one time becomes a one-time routine of that bot instead (decision 0121). With `bot`, that bot
/// posts. An agent run sends its `session` and `agent_token`, and then posts only as the bot of its own task.
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
}

impl fmt::Debug for TeamRoomPostParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TeamRoomPostParams")
            .field("team", &self.team)
            .field("text", &self.text)
            .field("bot", &self.bot)
            .field("to", &self.to)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_author_from_a_newer_peer_decodes_as_unknown() {
        let author: RoomAuthor = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(author, RoomAuthor::Unknown);
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
