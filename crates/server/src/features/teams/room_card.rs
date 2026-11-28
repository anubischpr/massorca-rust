//! Choice cards of the Conference room (decision 0121): a person's line that names no bot, in a team of several bots
//! with no lead and no receiver picked, gets a card asking where it goes. `team.room_answer` answers it once: one of the
//! team's bots takes it as a line naming that bot would, another team's Board gets it in Todo, or this team's Board
//! keeps it in Todo with no bot.

use std::cell::Cell;

use massorca_core_agents::{Agents, SecretGrant};
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
use super::room_mention::{self, Projects};
use super::task::create_task;
use super::task_start::{Changes, publish_changes};
use super::{blocking, invalid, selectors, store_error};

/// What a card says; an older client shows only this.
const CARD_TEXT: &str = "No bot was named and the team has no lead. Who should take this? \
Choose one of the team's bots, another team, or keep it in this team's Todo.";
/// The label of the option that keeps a request in this team's Todo.
const TODO_LABEL: &str = "Keep in this team's Todo";
const TODO_CHOICE: &str = "todo";
/// Why a card's bot runs without vault variables when the person who posted it was not a host admin.
const AUTHOR_NOT_ADMIN: &str =
    "vault secrets go only to runs a host-admin caller asked for; this request's author is not one";

/// A card as the store keeps it: the person's line it asks about, whether that line's task starts at once, whether its
/// author held host-admin scope, and the options offered when it was posted.
#[derive(Serialize, Deserialize)]
struct StoredCard {
    request: String,
    start: bool,
    #[serde(default)]
    admin: bool,
    options: Vec<RoomChoice>,
}

/// Stores a person's line and the card that asks where it goes. Nothing else is made until the card is answered.
pub(super) fn card_lines(
    store: &Store,
    team: &TeamRow,
    bots: &[BotRow],
    text: String,
    at_once: bool,
    admin: bool,
) -> Result<Vec<TeamRoomMessageRow>, ErrorBody> {
    let person = insert_line(store, team, None, text, None)?;
    let card = card_for(store, team, bots, &person.id, (at_once, admin), CARD_TEXT)?;
    Ok(vec![person, card])
}

/// Stores the card that asks where the stored line `request` goes, saying `text`; `flags` are whether its task starts
/// at once and whether its author held host-admin scope.
pub(super) fn card_for(
    store: &Store,
    team: &TeamRow,
    bots: &[BotRow],
    request: &str,
    flags: (bool, bool),
    text: &str,
) -> Result<TeamRoomMessageRow, ErrorBody> {
    let (at_once, admin) = flags;
    let stored = StoredCard {
        request: request.to_owned(),
        start: at_once,
        admin,
        options: options(store, team, bots)?,
    };
    let mut card = new_line(team, None, text.to_owned(), None);
    // A card is the room's own line: a bot line of no bot, which the schema allows.
    card.author = "bot".to_owned();
    card.card = Some(
        serde_json::to_string(&stored)
            .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?,
    );
    store.insert_team_room_message(&card).map_err(store_error)?;
    Ok(card)
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
    let workspaces = for_mutation(&ctx).await?;
    let grant = secret_grant(&ctx);
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
        let start = Start {
            workspaces,
            grant: answer_grant(stored.admin, grant),
            at_once: stored.start,
        };
        let made = under_claim(store, &row.id, &option.choice, |saved| {
            let answer = Answer {
                team: &team,
                option: &option,
                text: request.text,
                start,
            };
            make(store, agents, answer, &mut changes, saved)
        })?;
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

/// The vault grant of a card's bot: the answerer's, but never more than the author's (decision 0071), so an admin
/// answering a card does not lend a request its scope.
fn answer_grant(author_admin: bool, answerer: SecretGrant) -> SecretGrant {
    if author_admin {
        answerer
    } else {
        SecretGrant::Withhold(AUTHOR_NOT_ADMIN)
    }
}

/// Claims card `card` with `choice`, then runs `make`. When `make` fails or panics before it saved anything, the claim
/// is released and the card waits again; once something is saved, the card stays answered so it never delivers twice.
fn under_claim<T>(
    store: &Store,
    card: &str,
    choice: &str,
    make: impl FnOnce(&Cell<bool>) -> Result<T, ErrorBody>,
) -> Result<T, ErrorBody> {
    if !store
        .answer_team_room_card(card, choice)
        .map_err(store_error)?
    {
        return Err(answered_already());
    }
    let saved = Cell::new(false);
    let mut claim = Claim {
        store,
        card,
        saved: &saved,
        kept: false,
    };
    let made = make(&saved)?;
    claim.kept = true;
    Ok(made)
}

/// A card's claim while its answer is made; dropped unkept (an error or a panic), it releases the card unless
/// something was saved.
struct Claim<'a> {
    store: &'a Store,
    card: &'a str,
    saved: &'a Cell<bool>,
    kept: bool,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        if !self.kept && !self.saved.get() {
            // A failed release leaves the card answered, which delivers nothing twice.
            let _ = self.store.release_team_room_card(self.card);
        }
    }
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

/// Makes what the chosen option says: the bot's task or routine, another team's Todo task, or this team's. `saved`
/// turns true once the task or routine is stored.
fn make(
    store: &Store,
    agents: &Agents,
    answer: Answer<'_>,
    changes: &mut Changes,
    saved: &Cell<bool>,
) -> Result<Made, ErrorBody> {
    let Answer {
        team,
        option,
        text,
        start,
    } = answer;
    let bots = store.list_bots(&team.id).map_err(store_error)?;
    // The mentions are read again: a project or file removed since the card was posted is only left out.
    let mentions = Projects::of(store, &start.workspaces, team)
        .and_then(|projects| room_mention::resolve(store, team, &bots, &projects, &text))
        .unwrap_or_default();
    match option.choice.split_once(':') {
        Some(("bot", target)) if option.kind == RoomChoiceKind::Bot => {
            let bot = selectors::bot(store, team, target)?;
            let room = Room {
                store,
                agents,
                team,
                bots: &bots,
                saved,
            };
            let request = Request {
                text,
                posted: true,
                reply_to: None,
                context: None,
                mentions,
            };
            Ok(Made {
                lines: deliver(&room, start, &bot, request, changes)?,
                other_team: None,
            })
        }
        _ => file_task(store, team, option, text, mentions.notes(), saved),
    }
}

/// Stores a person's line that names another team, and files it in that team's Todo as a card's team option does
/// (AUTO-13).
pub(super) fn send_to_team(
    store: &Store,
    team: &TeamRow,
    other: &TeamRow,
    text: String,
    notes: Option<String>,
) -> Result<Vec<TeamRoomMessageRow>, ErrorBody> {
    let person = insert_line(store, team, None, text.clone(), None)?;
    let mut lines = vec![person];
    lines.extend(file_to_team(store, team, other, text, notes)?);
    Ok(lines)
}

/// Files a request already in `team`'s room in `other`'s Todo, and says so in the room.
pub(super) fn file_to_team(
    store: &Store,
    team: &TeamRow,
    other: &TeamRow,
    text: String,
    notes: Option<String>,
) -> Result<Vec<TeamRoomMessageRow>, ErrorBody> {
    let option = RoomChoice {
        choice: format!("team:{}", other.id),
        kind: RoomChoiceKind::Team,
        label: other.name.clone(),
    };
    Ok(file_task(store, team, &option, text, notes, &Cell::new(false))?.lines)
}

/// Files a card's request as a Todo task of another team or of the card's own team, and says so in the card's room.
/// The task's body is the request, then `notes`.
fn file_task(
    store: &Store,
    team: &TeamRow,
    option: &RoomChoice,
    text: String,
    notes: Option<String>,
    saved: &Cell<bool>,
) -> Result<Made, ErrorBody> {
    let bots = store.list_bots(&team.id).map_err(store_error)?;
    let title = task_title(&text, &bots);
    let text = match notes {
        Some(notes) => format!("{text}\n\n{notes}"),
        None => text,
    };
    let (kind, target) = option
        .choice
        .split_once(':')
        .unwrap_or((option.choice.as_str(), ""));
    match (option.kind, kind) {
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
            saved.set(true);
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
            saved.set(true);
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

        let lines = card_lines(
            &store,
            &alpha,
            &bots,
            "Add sign-in".to_owned(),
            false,
            false,
        )
        .unwrap();
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
        let lines =
            card_lines(&store, &alpha, &bots, "Add sign-in".to_owned(), true, true).unwrap();
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

    /// Team alpha with a posted card, team beta, and a copy of alpha that the store no longer holds.
    fn posted_card(store: &Store) -> (TeamRow, TeamRow, String) {
        let alpha = team(store, "alpha");
        let beta = team(store, "beta");
        let bots = [bot(store, &alpha, "Mara"), bot(store, &alpha, "Bruno")];
        let lines = card_lines(
            store,
            &alpha,
            &bots,
            "Review the copy".to_owned(),
            true,
            false,
        )
        .expect("a card");
        (alpha, beta, lines[1].id.clone())
    }

    fn answer_of(store: &Store, card: &str) -> Option<String> {
        store
            .get_team_room_message(card)
            .unwrap()
            .and_then(|row| row.card_answer)
    }

    fn option(kind: RoomChoiceKind, choice: &str) -> RoomChoice {
        RoomChoice {
            choice: choice.to_owned(),
            kind,
            label: String::new(),
        }
    }

    #[test]
    fn a_failure_after_the_task_is_saved_keeps_the_card_answered() {
        let store = Store::open_in_memory().expect("store");
        let (alpha, beta, card) = posted_card(&store);
        let sent = option(RoomChoiceKind::Team, &format!("team:{}", beta.id));
        // The card's room line cannot be stored once the card's team is gone, after beta's task is.
        let mut gone = alpha.clone();
        gone.id = new_id();
        let failed = under_claim(&store, &card, &sent.choice, |saved| {
            file_task(
                &store,
                &gone,
                &sent,
                "Review the copy".to_owned(),
                None,
                saved,
            )
        });
        assert!(failed.is_err());
        assert_eq!(store.list_tasks(Some(&beta.id)).unwrap().len(), 1);
        assert_eq!(
            answer_of(&store, &card),
            Some(sent.choice.clone()),
            "the saved task is not delivered again"
        );
        let again = under_claim(&store, &card, "todo", |_| Ok(()));
        assert_eq!(
            again.err().map(|error| error.code),
            Some(codes::TEAM_ROOM_CARD_ANSWERED.to_owned())
        );
    }

    #[test]
    fn a_failure_or_a_panic_before_anything_is_saved_lets_the_card_wait_again() {
        let store = Store::open_in_memory().expect("store");
        let (alpha, _, card) = posted_card(&store);
        let mut gone = alpha.clone();
        gone.id = new_id();
        let todo = option(RoomChoiceKind::Todo, "todo");
        let failed = under_claim(&store, &card, "todo", |saved| {
            file_task(
                &store,
                &gone,
                &todo,
                "Review the copy".to_owned(),
                None,
                saved,
            )
        });
        assert!(failed.is_err(), "a team that is gone takes no task");
        assert_eq!(answer_of(&store, &card), None);

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = under_claim(&store, &card, "todo", |_| -> Result<(), ErrorBody> {
                panic!("the answer broke")
            });
        }));
        assert!(panicked.is_err());
        assert_eq!(
            answer_of(&store, &card),
            None,
            "a panic leaves no stuck claim"
        );

        under_claim(&store, &card, "todo", |saved| {
            file_task(
                &store,
                &alpha,
                &todo,
                "Review the copy".to_owned(),
                None,
                saved,
            )
        })
        .expect("the card is answered");
        assert_eq!(answer_of(&store, &card).as_deref(), Some("todo"));
        assert_eq!(store.list_tasks(Some(&alpha.id)).unwrap().len(), 1);
    }

    #[test]
    fn a_card_bot_never_gets_more_vault_scope_than_the_request_author_had() {
        let store = Store::open_in_memory().expect("store");
        let (alpha, _, card) = posted_card(&store);
        let (_, stored) = waiting_card(&store, &alpha, &card).expect("a waiting card");
        assert!(!stored.admin, "posted by a caller below host admin");
        assert!(matches!(
            answer_grant(stored.admin, SecretGrant::Provide),
            SecretGrant::Withhold(_)
        ));
        assert!(matches!(
            answer_grant(true, SecretGrant::Provide),
            SecretGrant::Provide
        ));
        assert!(matches!(
            answer_grant(true, SecretGrant::Withhold("not admin")),
            SecretGrant::Withhold(_)
        ));
    }
}
