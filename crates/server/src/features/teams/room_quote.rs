//! A Conference room line that replies to another (decision 0122): the quoted line it keeps, as the room shows it and
//! as a task made from the reply keeps it for context.

use massorca_core_store::{BotRow, Store, TeamRoomMessageRow, TeamRow};
use massorca_protocol::methods::team_room::{RoomAuthor, RoomQuote};
use massorca_protocol::{ErrorBody, codes};

use super::store_error;

/// The longest room line a reply's context or prompt reads.
pub(super) const LINE_CHARS: usize = 300;
/// The longest quote a line shows.
const QUOTE_CHARS: usize = 200;

/// Whether a line is a bot's own line: it names its bot and is not a choice card.
pub(super) fn is_bot_line(line: &TeamRoomMessageRow) -> bool {
    line.author == "bot" && line.bot_id.is_some() && line.card.is_none()
}

/// The line `id` of `team`'s room, which a person's line replies to.
pub(super) fn quoted_line(
    store: &Store,
    team: &TeamRow,
    id: &str,
) -> Result<TeamRoomMessageRow, ErrorBody> {
    store
        .get_team_room_message(id)
        .map_err(store_error)?
        .filter(|line| line.team_id == team.id)
        .ok_or_else(|| {
            ErrorBody::new(
                codes::TEAM_ROOM_LINE_NOT_FOUND,
                format!("no line {id} in the room of team {}", team.name),
            )
        })
}

/// The bot of the team whose line `quoted` is, when it is a bot's line.
pub(super) fn replied_bot<'a>(
    bots: &'a [BotRow],
    quoted: Option<&TeamRoomMessageRow>,
) -> Option<&'a BotRow> {
    let id = quoted.filter(|line| is_bot_line(line))?.bot_id.as_deref()?;
    bots.iter().find(|bot| bot.id == id)
}

/// What a task made from a reply keeps of the line it quotes.
pub(super) fn context(quoted: &TeamRoomMessageRow, bots: &[BotRow]) -> String {
    format!(
        "In reply to {}: {}",
        speaker(quoted, bots),
        excerpt(&quoted.text, LINE_CHARS)
    )
}

/// The quote a line shows of the line it replies to.
pub(super) fn quote(quoted: &TeamRoomMessageRow, bots: &[BotRow]) -> RoomQuote {
    let bot_name = quoted
        .bot_id
        .as_deref()
        .and_then(|id| bots.iter().find(|bot| bot.id == id))
        .map(|bot| bot.name.clone());
    RoomQuote {
        id: quoted.id.clone(),
        author: match quoted.author.as_str() {
            "person" => RoomAuthor::Person,
            "bot" => RoomAuthor::Bot,
            _ => RoomAuthor::Unknown,
        },
        bot_name,
        text: excerpt(&quoted.text, QUOTE_CHARS),
    }
}

/// Who wrote a line, as a reply's prompt and context name them.
pub(super) fn speaker(line: &TeamRoomMessageRow, bots: &[BotRow]) -> String {
    if line.author == "person" {
        return "Person".to_owned();
    }
    if line.card.is_some() {
        return "Conference room".to_owned();
    }
    line.bot_id
        .as_deref()
        .and_then(|id| bots.iter().find(|bot| bot.id == id))
        .map_or_else(|| "A removed bot".to_owned(), |bot| bot.name.clone())
}

/// `text` on one line, cut to `max` characters.
pub(super) fn excerpt(text: &str, max: usize) -> String {
    cut(&text.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

/// `text` cut to `max` characters, the last of them "…".
pub(super) fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max - 1).collect();
    format!("{}…", kept.trim_end())
}
