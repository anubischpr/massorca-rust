//! Choice cards of the Conference room (decision 0121): a person's line that names no bot, in a team of several bots
//! with no lead and no receiver picked, gets a card asking where it goes. `team.room_answer` answers it once: one of the
//! team's bots takes it as a line naming that bot would, another team's Board gets it in Todo, or this team's Board
//! keeps it in Todo with no bot.

use massorca_core_store::{BotRow, Store, TeamRoomMessageRow, TeamRow};
use massorca_protocol::methods::events::{
    TOPIC_TASK_CHANGED, TOPIC_TEAM_CHANGED, TOPIC_TEAM_ROOM_CHANGED,
};
use massorca_protocol::methods::tasks::{TaskCreateParams, TaskOrigin};
use massorca_protocol::methods::team_room::{
    RoomCard, RoomChoice, RoomChoiceKind, TeamRoomAnswerParams, TeamRoomAnswered,
};
use massorca_protocol::{ErrorBody, codes};
use serde::{Deserialize, Serialize};

use crate::context::RequestCtx;
use crate::features::agents::secret_grant;
use crate::features::workspace_domain::{for_mutation, publish};

use super::room::{Request, Room, Start, deliver, insert_line, new_line, task_title, views};
use super::task::create_task;
use super::task_start::{Changes, publish_changes};
use super::{blocking, invalid, selectors, store_error};

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

/// `team.room_answer`.
pub(super) async fn answer(
    ctx: RequestCtx,
    params: TeamRoomAnswerParams,
) -> Result<TeamRoomAnswered, ErrorBody> {
    let start = Start {
        workspaces: for_mutation(&ctx).await?,
        grant: secret_grant(&ctx),
        at_once: true,
    };
    let (answered, changes, teams) = blocking(&ctx, move |store, agents| {
        let mut changes = Changes::default();
        let team = selectors::team(store, &params.team)?;
        let (row, stored) = waiting_card(store, &team, &params.message)?;
        let option = stored
            .options
            .iter()
            .find(|option| option.choice == params.choice)
            .cloned()
            .ok_or_else(|| invalid(format!("the card offers no choice {:?}", params.choice)))?;
        let request = store
            .get_team_room_message(&stored.request)
            .map_err(store_error)?
            .ok_or_else(not_a_card)?;
        if !store
            .answer_team_room_card(&row.id, &option.choice)
            .map_err(store_error)?
        {
            return Err(answered_already());
        }
        let answer = Answer {
            team: &team,
            option: &option,
            text: request.text,
            start: Start {
                at_once: stored.start,
                ..start
            },
        };
        let made = match make(store, agents, answer, &mut changes) {
            Ok(made) => made,
            Err(error) => {
                // What the answer was to make could not be made, so the card waits again.
                store.release_team_room_card(&row.id).map_err(store_error)?;
                return Err(error);
            }
        };
        let card = store
            .get_team_room_message(&row.id)
            .map_err(store_error)?
            .ok_or_else(not_a_card)?;
        let mut teams = vec![team.id.clone()];
        teams.extend(made.other_team);
        let card = views(store, agents, &team, &[card])?.remove(0);
        let messages = views(store, agents, &team, &made.lines)?;
        Ok((TeamRoomAnswered { card, messages }, changes, teams))
    })
    .await?;
    publish_changes(&ctx.state, &changes);
    for task in answered
        .messages
        .iter()
        .filter_map(|line| line.task.as_ref())
    {
        publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    }
    if answered.messages.iter().any(|line| line.routine.is_some()) {
        publish(&ctx, TOPIC_TEAM_CHANGED, &teams[0]);
    }
    for team in &teams {
        publish(&ctx, TOPIC_TEAM_ROOM_CHANGED, team);
    }
    Ok(answered)
}

/// The card `message` of `team`, refused when it is not a card of that team or was answered already.
fn waiting_card(
    store: &Store,
    team: &TeamRow,
    message: &str,
) -> Result<(TeamRoomMessageRow, StoredCard), ErrorBody> {
    let row = store
        .get_team_room_message(message)
        .map_err(store_error)?
        .filter(|row| row.team_id == team.id)
        .ok_or_else(not_a_card)?;
    let stored: StoredCard = row
        .card
        .as_deref()
        .and_then(|card| serde_json::from_str(card).ok())
        .ok_or_else(not_a_card)?;
    if row.card_answer.is_some() {
        return Err(answered_already());
    }
    Ok((row, stored))
}

fn not_a_card() -> ErrorBody {
    ErrorBody::new(
        codes::TEAM_ROOM_CARD_NOT_FOUND,
        "the room has no choice card with this id",
    )
}

fn answered_already() -> ErrorBody {
    ErrorBody::new(
        codes::TEAM_ROOM_CARD_ANSWERED,
        "this choice card was answered already",
    )
}

/// One answer to make: the card's team, the option chosen, the request's text, and how a bot's task starts.
struct Answer<'a> {
    team: &'a TeamRow,
    option: &'a RoomChoice,
    text: String,
    start: Start,
}

/// The lines an answer added to the card's room, and the other team whose Board got the task.
struct Made {
    lines: Vec<TeamRoomMessageRow>,
    other_team: Option<String>,
}

/// Makes what the chosen option says: the bot's task or routine, another team's Todo task, or this team's.
fn make(
    store: &Store,
    agents: &massorca_core_agents::Agents,
    answer: Answer<'_>,
    changes: &mut Changes,
) -> Result<Made, ErrorBody> {
    let Answer {
        team,
        option,
        text,
        start,
    } = answer;
    let bots = store.list_bots(&team.id).map_err(store_error)?;
    let title = task_title(&text, &bots);
    let (kind, target) = option
        .choice
        .split_once(':')
        .unwrap_or((option.choice.as_str(), ""));
    match (option.kind, kind) {
        (RoomChoiceKind::Bot, "bot") => {
            let bot = selectors::bot(store, team, target)?;
            let room = Room {
                store,
                agents,
                team,
                bots: &bots,
            };
            let request = Request { text, posted: true };
            Ok(Made {
                lines: deliver(&room, start, &bot, request, changes)?,
                other_team: None,
            })
        }
        (RoomChoiceKind::Team, "team") => {
            let other = selectors::team(store, target)?;
            let other_bots = store.list_bots(&other.id).map_err(store_error)?;
            // The other team's lead, else its only bot, takes it; it waits there for that team to start it.
            let owner = other_bots
                .iter()
                .find(|bot| bot.lead)
                .or(match other_bots.as_slice() {
                    [only] => Some(only),
                    _ => None,
                });
            let task = create_task(
                store,
                TaskCreateParams {
                    team: other.id.clone(),
                    title: title.clone(),
                    body: Some(text),
                    bot: owner.map(|bot| bot.id.clone()),
                    project: None,
                    origin: Some(TaskOrigin::Room),
                },
            )?;
            let line = room_line(
                store,
                team,
                format!("Sent to team {}, in its Todo: {title}", other.name),
                task.id,
            )?;
            Ok(Made {
                lines: vec![line],
                other_team: Some(other.id),
            })
        }
        (RoomChoiceKind::Todo, TODO_CHOICE) => {
            let task = create_task(
                store,
                TaskCreateParams {
                    team: team.id.clone(),
                    title: title.clone(),
                    body: Some(text),
                    bot: None,
                    project: None,
                    origin: Some(TaskOrigin::Room),
                },
            )?;
            let line = room_line(
                store,
                team,
                format!("Kept in this team's Todo: {title}"),
                task.id,
            )?;
            Ok(Made {
                lines: vec![line],
                other_team: None,
            })
        }
        _ => Err(invalid(format!(
            "this build cannot answer the choice {:?}",
            option.choice
        ))),
    }
}

/// A room line of no bot about `task`, as a card's own answer.
fn room_line(
    store: &Store,
    team: &TeamRow,
    text: String,
    task: String,
) -> Result<TeamRoomMessageRow, ErrorBody> {
    let mut row = new_line(team, None, text, Some(task));
    row.author = "bot".to_owned();
    store.insert_team_room_message(&row).map_err(store_error)?;
    Ok(row)
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

    #[test]
    fn a_card_answered_or_of_another_team_is_refused_with_its_code() {
        let store = Store::open_in_memory().expect("store");
        let alpha = team(&store, "alpha");
        let beta = team(&store, "beta");
        let bots = [bot(&store, &alpha, "Mara"), bot(&store, &alpha, "Bruno")];
        let lines = card_lines(&store, &alpha, &bots, "Add sign-in".to_owned(), true).unwrap();
        let card = &lines[1];

        let refused = waiting_card(&store, &beta, &card.id)
            .err()
            .expect("another team's card");
        assert_eq!(refused.code, codes::TEAM_ROOM_CARD_NOT_FOUND);
        let refused = waiting_card(&store, &alpha, &lines[0].id)
            .err()
            .expect("not a card");
        assert_eq!(refused.code, codes::TEAM_ROOM_CARD_NOT_FOUND);
        assert!(waiting_card(&store, &alpha, &card.id).is_ok());

        assert!(store.answer_team_room_card(&card.id, "todo").unwrap());
        let refused = waiting_card(&store, &alpha, &card.id)
            .err()
            .expect("answered");
        assert_eq!(refused.code, codes::TEAM_ROOM_CARD_ANSWERED);
        let answered = store.get_team_room_message(&card.id).unwrap().unwrap();
        assert_eq!(
            card_view(&answered).unwrap().answer.as_deref(),
            Some("todo")
        );
    }
}
