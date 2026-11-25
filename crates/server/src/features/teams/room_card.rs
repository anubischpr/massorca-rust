//! Choice cards of the Conference room (decision 0121): a person's line that names no bot, in a team of several bots
//! with no lead and no receiver picked, gets a card asking where it goes: one of the team's bots, another team, or this
//! team's Todo.

use massorca_core_store::{BotRow, Store, TeamRoomMessageRow, TeamRow};
use massorca_protocol::methods::team_room::{RoomCard, RoomChoice, RoomChoiceKind};
use massorca_protocol::{ErrorBody, codes};
use serde::{Deserialize, Serialize};

use super::room::{insert_line, new_line};
use super::store_error;

/// What a card says; an older client shows only this.
const CARD_TEXT: &str = "No bot was named and the team has no lead. Who should take this? \
Choose one of the team's bots, another team, or keep it in this team's Todo.";
/// The label of the option that keeps a request in this team's Todo.
const TODO_LABEL: &str = "Keep in this team's Todo";
const TODO_CHOICE: &str = "todo";

/// A card as the store keeps it: the person's line it asks about, whether that line's task starts at once, and the
/// options offered when it was posted.
#[derive(Serialize, Deserialize)]
struct StoredCard {
    request: String,
    start: bool,
    options: Vec<RoomChoice>,
}

/// Stores a person's line and the card that asks where it goes. Nothing else is made until the card is answered.
pub(super) fn card_lines(
    store: &Store,
    team: &TeamRow,
    bots: &[BotRow],
    text: String,
    at_once: bool,
) -> Result<Vec<TeamRoomMessageRow>, ErrorBody> {
    let person = insert_line(store, team, None, text, None)?;
    let stored = StoredCard {
        request: person.id.clone(),
        start: at_once,
        options: options(store, team, bots)?,
    };
    let mut card = new_line(team, None, CARD_TEXT.to_owned(), None);
    // A card is the room's own line: a bot line of no bot, which the schema allows.
    card.author = "bot".to_owned();
    card.card = Some(
        serde_json::to_string(&stored)
            .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?,
    );
    store.insert_team_room_message(&card).map_err(store_error)?;
    Ok(vec![person, card])
}

/// Each bot of the team, then each other team, then this team's Todo.
fn options(store: &Store, team: &TeamRow, bots: &[BotRow]) -> Result<Vec<RoomChoice>, ErrorBody> {
    let teams = store.list_teams().map_err(store_error)?;
    let bots = bots.iter().map(|bot| RoomChoice {
        choice: format!("bot:{}", bot.id),
        kind: RoomChoiceKind::Bot,
        label: bot.name.clone(),
    });
    let others = teams
        .into_iter()
        .filter(|other| other.id != team.id)
        .map(|other| RoomChoice {
            choice: format!("team:{}", other.id),
            kind: RoomChoiceKind::Team,
            label: other.name,
        });
    Ok(bots
        .chain(others)
        .chain(std::iter::once(RoomChoice {
            choice: TODO_CHOICE.to_owned(),
            kind: RoomChoiceKind::Todo,
            label: TODO_LABEL.to_owned(),
        }))
        .collect())
}

/// The card a stored line is, with its answer; a card this build cannot read is shown as its text alone.
pub(super) fn card_view(row: &TeamRoomMessageRow) -> Option<RoomCard> {
    let stored: StoredCard = serde_json::from_str(row.card.as_deref()?).ok()?;
    Some(RoomCard {
        request_id: stored.request,
        options: stored.options,
        answer: row.card_answer.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_core_store::new_id;

    fn team(store: &Store, name: &str) -> TeamRow {
        let row = TeamRow {
            id: new_id(),
            name: name.to_owned(),
            purpose: String::new(),
            character: "triangle".to_owned(),
            access: "default".to_owned(),
            workspaces: false,
            created_at: 1,
        };
        store.insert_team(&row).expect("team");
        row
    }

    fn bot(store: &Store, team: &TeamRow, name: &str) -> BotRow {
        let row = BotRow {
            id: new_id(),
            team_id: team.id.clone(),
            name: name.to_owned(),
            job: String::new(),
            kind: "claude".to_owned(),
            model: None,
            effort: None,
            lead: false,
            created_at: 1,
        };
        store.insert_bot(&row).expect("bot");
        row
    }

    #[test]
    fn a_card_offers_each_bot_then_each_other_team_then_this_team_todo() {
        let store = Store::open_in_memory().expect("store");
        let alpha = team(&store, "alpha");
        let beta = team(&store, "beta");
        let bots = [bot(&store, &alpha, "Mara"), bot(&store, &alpha, "Bruno")];

        let lines = card_lines(&store, &alpha, &bots, "Add sign-in".to_owned(), false).unwrap();
        let [person, card] = lines.as_slice() else {
            panic!("the line and its card: {lines:?}");
        };
        assert_eq!(person.author, "person");
        assert_eq!(
            (card.author.as_str(), card.bot_id.as_deref()),
            ("bot", None)
        );
        assert_eq!(card.text, CARD_TEXT);
        let view = card_view(card).expect("a card");
        assert_eq!(view.request_id, person.id);
        assert_eq!(view.answer, None);
        let offered: Vec<(RoomChoiceKind, String, String)> = view
            .options
            .into_iter()
            .map(|option| (option.kind, option.choice, option.label))
            .collect();
        assert_eq!(
            offered,
            [
                (
                    RoomChoiceKind::Bot,
                    format!("bot:{}", bots[0].id),
                    "Mara".to_owned()
                ),
                (
                    RoomChoiceKind::Bot,
                    format!("bot:{}", bots[1].id),
                    "Bruno".to_owned()
                ),
                (
                    RoomChoiceKind::Team,
                    format!("team:{}", beta.id),
                    "beta".to_owned()
                ),
                (
                    RoomChoiceKind::Todo,
                    "todo".to_owned(),
                    TODO_LABEL.to_owned()
                ),
            ]
        );
        assert!(
            store.list_tasks(Some(&alpha.id)).unwrap().is_empty(),
            "nothing made yet"
        );
        assert!(card_view(person).is_none(), "a plain line is no card");
    }
}
