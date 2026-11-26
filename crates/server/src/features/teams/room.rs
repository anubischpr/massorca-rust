//! `team.room_list` and `team.room_post` (capability `team-room/1`, decision 0102): a team's Conference room. A
//! person's line becomes a task for one bot, which starts at once; the bot answers in the room, and posts again when the
//! task ends (see `task_outcome`). A line that names a day and a time becomes a one-time routine instead, and a line
//! no bot can be picked for gets a choice card (decision 0121, `room_card`). A line that replies to another keeps it as
//! its context (decision 0122, `room_quote`).

use std::cell::Cell;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{BotRow, Store, TeamRoomMessageRow, TeamRow, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::{
    TOPIC_TASK_CHANGED, TOPIC_TEAM_CHANGED, TOPIC_TEAM_ROOM_CHANGED,
};
use massorca_protocol::methods::tasks::{TaskCreateParams, TaskOrigin};
use massorca_protocol::methods::team_room::{
    self, RoomAuthor, TeamRoomList, TeamRoomListParams, TeamRoomMessage, TeamRoomPostParams,
    TeamRoomPosted,
};
use massorca_protocol::methods::teams::{RoutineAddParams, RoutineCadence};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::agents::secret_grant;
use crate::features::workspace_domain::{for_mutation, publish};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::room_card::{self, card_lines, card_view};
use super::room_schedule::{Moment, scheduled_moment};
use super::routines::{HostZone, add_routine, routine_view};
use super::task::{create_task, view_of};
use super::task_start::{Changes, publish_changes, start_task};
use super::{blocking, invalid, non_blank, selectors, store_error, task_state};
use super::room_quote;

/// The longest room line, in Unicode scalar values.
const MAX_TEXT_CHARS: usize = 4000;
/// The longest task title a line makes, in Unicode scalar values.
const MAX_TITLE_CHARS: usize = 80;
/// Lines `team.room_list` returns without a limit: the newest that many.
const DEFAULT_LIMIT: usize = 200;
/// What a person's line answers when the team has no bot to take it.
pub(super) const NO_BOTS: &str = "Add a bot to this team first.";

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::TEAM_ROOM, Owner::Core)?,
        registries
            .capabilities
            .register(capabilities::TEAM_ROOM_CARDS, Owner::Core)?,
        methods.register(
            Owner::Core,
            team_room::ROOM_LIST,
            capabilities::TEAM_ROOM,
            Permission::Read,
            list,
        )?,
        // A person's line starts an agent, as `task.start` does.
        methods.register(
            Owner::Core,
            team_room::ROOM_POST,
            capabilities::TEAM_ROOM,
            Permission::Workspace,
            post,
        )?,
        // An answer makes the task a person's line would, so it needs the same right.
        methods.register(
            Owner::Core,
            team_room::ROOM_ANSWER,
            capabilities::TEAM_ROOM_CARDS,
            Permission::Workspace,
            room_card::answer,
        )?,
    ])
}

async fn list(ctx: RequestCtx, params: TeamRoomListParams) -> Result<TeamRoomList, ErrorBody> {
    blocking(&ctx, move |store, agents| {
        let team = selectors::team(store, &params.team)?;
        let limit = params.limit.map_or(DEFAULT_LIMIT, |limit| {
            usize::try_from(limit).unwrap_or(usize::MAX)
        });
        let rows = store
            .list_team_room(&team.id, Some(limit))
            .map_err(store_error)?;
        Ok(TeamRoomList {
            messages: views(store, agents, &team, &rows)?,
            typing: Vec::new(),
        })
    })
    .await
}

/// What starting a person's task needs: the workspaces root, the caller's vault grant, and whether it starts at once.
pub(super) struct Start {
    pub(super) workspaces: Workspaces,
    pub(super) grant: SecretGrant,
    pub(super) at_once: bool,
}

impl Start {
    /// Whether the caller holds host-admin scope, which a routine it schedules keeps for its runs (decision 0071).
    fn admin(&self) -> bool {
        matches!(self.grant, SecretGrant::Provide)
    }
}

/// Who posts a line: a person, a bot a person names, or the agent run of a bot's own task.
enum Poster {
    Person {
        asked: Box<PersonAsk>,
        start: Start,
    },
    Bot(String),
    Agent {
        session: String,
        bot: Option<String>,
    },
}

async fn post(ctx: RequestCtx, params: TeamRoomPostParams) -> Result<TeamRoomPosted, ErrorBody> {
    let poster = match (params.agent_token.as_deref(), params.session.clone()) {
        // An agent run proves its session with its run token, and then posts only as its own task's bot.
        (Some(token), Some(session)) if ctx.state.agents.run_token_matches(&session, token) => {
            Poster::Agent {
                session,
                bot: params.bot.clone(),
            }
        }
        (Some(_), _) => {
            return Err(ErrorBody::new(
                codes::PERMISSION_DENIED,
                "the agent token does not match a live run of that session",
            ));
        }
        (None, _) => match params.bot.clone() {
            Some(bot) => Poster::Bot(bot),
            // Only a person's line starts a task, so only it needs the workspaces root and the vault grant.
            None => Poster::Person {
                asked: Box::new(PersonAsk {
                    to: params.to.clone(),
                    reply_to: params.reply_to.clone(),
                }),
                start: Start {
                    workspaces: for_mutation(&ctx).await?,
                    grant: secret_grant(&ctx),
                    at_once: params.start.unwrap_or(true),
                },
            },
        },
    };
    let (team_id, changes, messages) = blocking(&ctx, move |store, agents| {
        let mut changes = Changes::default();
        let team = selectors::team(store, &params.team)?;
        let text = room_text(&params.text)?;
        let rows = match poster {
            Poster::Bot(bot) => {
                let bot = selectors::bot(store, &team, &bot)?;
                vec![insert_line(store, &team, Some(&bot), text, None)?]
            }
            Poster::Agent { session, bot } => {
                vec![agent_line(store, &team, &session, bot.as_deref(), text)?]
            }
            Poster::Person { asked, start } => {
                person_line(store, agents, start, &team, &asked, text, &mut changes)?
            }
        };
        let messages = views(store, agents, &team, &rows)?;
        Ok((team.id, changes, messages))
    })
    .await?;
    publish_changes(&ctx.state, &changes);
    for task in messages.iter().filter_map(|line| line.task.as_ref()) {
        publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    }
    if messages.iter().any(|line| line.routine.is_some()) {
        publish(&ctx, TOPIC_TEAM_CHANGED, &team_id);
    }
    publish(&ctx, TOPIC_TEAM_ROOM_CHANGED, &team_id);
    Ok(TeamRoomPosted { messages })
}

/// A line from the agent run of `session`: it posts as the bot of the team's task that run works on, and naming any
/// other bot is refused.
fn agent_line(
    store: &Store,
    team: &TeamRow,
    session: &str,
    named: Option<&str>,
    text: String,
) -> Result<TeamRoomMessageRow, ErrorBody> {
    let not_a_bot = || {
        ErrorBody::new(
            codes::PERMISSION_DENIED,
            format!("this agent run works on no task of team {}", team.name),
        )
    };
    let task = store
        .find_task_by_session(session)
        .map_err(store_error)?
        .filter(|task| task.team_id == team.id)
        .ok_or_else(not_a_bot)?;
    let own = task
        .bot_id
        .as_deref()
        .map(|bot| selectors::bot(store, team, bot))
        .transpose()?
        .ok_or_else(not_a_bot)?;
    if let Some(named) = named
        && selectors::bot(store, team, named)?.id != own.id
    {
        return Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            format!("an agent run posts only as its own bot, {}", own.name),
        ));
    }
    insert_line(store, team, Some(&own), text, Some(task.id))
}

/// What a person's line asks for besides its text: the bot it goes to when it names none, and the line it replies to.
struct PersonAsk {
    to: Option<String>,
    reply_to: Option<String>,
}

/// Stores a person's line, makes it a task for the bot it names (else `to`, else the lead, else the only bot), starts
/// the task and stores the bot's answer. A line that asks for a day and a time becomes that bot's one-time routine
/// instead, unless the routine is refused. A line no bot can be picked for gets a choice card. A line that replies to a
/// bot's line goes to that bot unless it names another, and its task keeps the quoted line.
/// A team without bots refuses the line and stores nothing.
fn person_line(
    store: &Store,
    agents: &Agents,
    start: Start,
    team: &TeamRow,
    asked: &PersonAsk,
    text: String,
    changes: &mut Changes,
) -> Result<Vec<TeamRoomMessageRow>, ErrorBody> {
    let bots = store.list_bots(&team.id).map_err(store_error)?;
    if bots.is_empty() {
        return Err(ErrorBody::new(codes::TASK_NO_BOT, NO_BOTS));
    }
    let to = asked
        .to
        .as_deref()
        .map(|to| selectors::bot(store, team, to))
        .transpose()?;
    let quoted = asked
        .reply_to
        .as_deref()
        .map(|id| room_quote::quoted_line(store, team, id))
        .transpose()?;
    // A task asked in reply to a bot's line goes to that bot unless the line names another.
    let receiver = room_quote::replied_bot(&bots, quoted.as_ref()).or(to.as_ref());
    match addressed_bot(&bots, &text, receiver) {
        Some(bot) => deliver(
            &Room {
                store,
                agents,
                team,
                bots: &bots,
                saved: &Cell::new(false),
            },
            start,
            bot,
            Request {
                context: quoted.as_ref().map(|line| room_quote::context(line, &bots)),
                reply_to: quoted.map(|line| line.id),
                text,
                posted: false,
            },
            changes,
        ),
        None => card_lines(store, team, &bots, text, start.at_once, start.admin()),
    }
}

/// The room a request is delivered in: the store, the agents, the team and its bots. `saved` turns true once the
/// request's task or routine is stored, so a failure after it is not undone as if nothing was made.
pub(super) struct Room<'a> {
    pub(super) store: &'a Store,
    pub(super) agents: &'a Agents,
    pub(super) team: &'a TeamRow,
    pub(super) bots: &'a [BotRow],
    pub(super) saved: &'a Cell<bool>,
}

/// A person's request: its text, whether its line is in the room already (a choice card's answer), the line it replies
/// to, and what its task keeps of that line.
pub(super) struct Request {
    pub(super) text: String,
    pub(super) posted: bool,
    pub(super) reply_to: Option<String>,
    pub(super) context: Option<String>,
}

impl Request {
    /// The request's line as the person posted it.
    fn line(&self, team: &TeamRow) -> TeamRoomMessageRow {
        let mut line = new_line(team, None, self.text.clone(), None);
        line.reply_to = self.reply_to.clone();
        line
    }

    /// A task's body: the line, then the quoted line it replies to.
    fn body(&self) -> String {
        match &self.context {
            Some(context) => format!("{}\n\n{context}", self.text),
            None => self.text.clone(),
        }
    }
}

/// Makes `request` `bot`'s one-time routine when it asks for a day and a time, else `bot`'s task, started unless
/// `start` says otherwise, and stores the request's line (unless posted) and the bot's answer.
pub(super) fn deliver(
    room: &Room<'_>,
    start: Start,
    bot: &BotRow,
    request: Request,
    changes: &mut Changes,
) -> Result<Vec<TeamRoomMessageRow>, ErrorBody> {
    let Room {
        store,
        agents,
        team,
        bots,
        saved,
    } = *room;
    let now = now_ms();
    if let Some(moment) = host_today(now).and_then(|today| scheduled_moment(&request.text, today))
        && let Some(lines) = schedule_line(
            store,
            team,
            bots,
            bot,
            &request,
            Asked {
                moment,
                admin: start.admin(),
                now,
                saved,
            },
        )?
    {
        return Ok(lines);
    }
    let title = task_title(&request.text, bots);
    let task = create_task(
        store,
        TaskCreateParams {
            team: team.id.clone(),
            title: title.clone(),
            body: Some(request.body()),
            bot: Some(bot.id.clone()),
            project: None,
            origin: Some(TaskOrigin::Room),
        },
    )?;
    saved.set(true);
    let mut lines = Vec::new();
    // Stored once the task is, so a refused task leaves no line without an answer.
    if !request.posted {
        let line = request.line(team);
        insert_row(store, &line)?;
        lines.push(line);
    }
    if !start.at_once {
        let answer = format!("Added to the Board, not started: {title}");
        lines.push(insert_line(store, team, Some(bot), answer, Some(task.id))?);
        return Ok(lines);
    }
    let answer = match start_task(
        store,
        agents,
        &start.workspaces,
        &task.id,
        start.grant,
        changes,
    ) {
        Ok(_) => format!("Added to the Board: {title}"),
        Err(error) => format!("Added to the Board. It did not start: {}", error.message),
    };
    lines.push(insert_line(store, team, Some(bot), answer, Some(task.id))?);
    Ok(lines)
}

/// The moment a line asks for, with the poster's host-admin scope, the time it was posted, and the flag set once the
/// routine is stored.
struct Asked<'a> {
    moment: Moment,
    admin: bool,
    now: i64,
    saved: &'a Cell<bool>,
}

/// Schedules a person's line that names a day and a time as `bot`'s one-time routine through the path of
/// `team.routine_add`, then stores the line (unless posted) and the bot's answer about the routine. A routine the path
/// refuses (a moment already past, an unreadable host zone) stores nothing and is `None`, so the line becomes a task
/// instead.
fn schedule_line(
    store: &Store,
    team: &TeamRow,
    bots: &[BotRow],
    bot: &BotRow,
    request: &Request,
    asked: Asked<'_>,
) -> Result<Option<Vec<TeamRoomMessageRow>>, ErrorBody> {
    let text = &request.text;
    let Asked {
        moment,
        admin,
        now,
        saved,
    } = asked;
    let title = task_title(text, bots);
    let (date, time) = (moment.run_date(), moment.clock());
    let added = add_routine(
        store,
        RoutineAddParams {
            team: team.id.clone(),
            bot: bot.id.clone(),
            name: title.clone(),
            prompt: text.clone(),
            cadence: RoutineCadence::Once,
            time: time.clone(),
            weekday: None,
            month_day: None,
            run_date: Some(date.clone()),
            zone: None,
        },
        admin,
        now,
        &HostZone::read(),
    );
    let Ok(routine) = added else {
        return Ok(None);
    };
    saved.set(true);
    let mut lines = Vec::new();
    if !request.posted {
        let line = request.line(team);
        insert_row(store, &line)?;
        lines.push(line);
    }
    let mut answer = new_line(
        team,
        Some(bot),
        format!("Routine created: {title}. It runs once on {date} at {time}."),
        None,
    );
    answer.routine_id = Some(routine.id);
    store
        .insert_team_room_message(&answer)
        .map_err(store_error)?;
    lines.push(answer);
    Ok(Some(lines))
}

/// Today on the clock a routine without a zone runs on: the host's zone, else UTC, as core-schedule reads it.
fn host_today(now: i64) -> Option<Date> {
    let zone = TimeZone::try_system().unwrap_or(TimeZone::UTC);
    Some(
        zone.to_datetime(Timestamp::from_millisecond(now).ok()?)
            .date(),
    )
}

/// The bot a line names with `@name` at the start of a word (the longest name wins), else `to`, else the lead, else the
/// only bot. `None` when the team has several bots and none of those picks one.
pub(super) fn addressed_bot<'a>(
    bots: &'a [BotRow],
    text: &str,
    to: Option<&'a BotRow>,
) -> Option<&'a BotRow> {
    mentioned_bot(bots, text)
        .or(to)
        .or_else(|| bots.iter().find(|bot| bot.lead))
        .or(match bots {
            [only] => Some(only),
            _ => None,
        })
}

/// The bot a line names with `@name` at the start of a word; the longest name wins.
fn mentioned_bot<'a>(bots: &'a [BotRow], text: &str) -> Option<&'a BotRow> {
    let lower = text.to_lowercase();
    bots.iter()
        .filter(|bot| {
            let mention = format!("@{}", bot.name.to_lowercase());
            lower.match_indices(&mention).any(|(at, _)| {
                let before = lower[..at].chars().next_back();
                let after = lower[at + mention.len()..].chars().next();
                before.is_none_or(|previous| !previous.is_alphanumeric())
                    && after.is_none_or(|next| !next.is_alphanumeric())
            })
        })
        .max_by_key(|bot| bot.name.chars().count())
}

/// The task title a line makes: its first line that is not blank without the `@name` of the team's bots, cut to 80
/// characters.
pub(super) fn task_title(text: &str, bots: &[BotRow]) -> String {
    let first = text
        .lines()
        .map(|line| without_mentions(line, bots))
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if first.chars().count() <= MAX_TITLE_CHARS {
        return first;
    }
    let cut: String = first.chars().take(MAX_TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// `line` without each `@name` of `bots` at the start of a word (the longest name wins), with its spaces collapsed and
/// the punctuation a removed mention leaves at its start ("@mara, add" is "add").
fn without_mentions(line: &str, bots: &[BotRow]) -> String {
    let mut names: Vec<String> = bots.iter().map(|bot| bot.name.to_lowercase()).collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.chars().count()));
    let mut kept = String::new();
    let mut rest = line;
    let mut previous: Option<char> = None;
    while let Some(next) = rest.chars().next() {
        if next == '@' && previous.is_none_or(|before| !before.is_alphanumeric()) {
            let after = &rest[1..];
            let named = names.iter().find_map(|name| {
                let count = name.chars().count();
                let candidate: String = after.chars().take(count).collect();
                let tail = &after[candidate.len()..];
                (candidate.to_lowercase() == *name
                    && tail.chars().next().is_none_or(|c| !c.is_alphanumeric()))
                .then_some(tail)
            });
            if let Some(tail) = named {
                rest = tail;
                previous = Some(' ');
                kept.push(' ');
                continue;
            }
        }
        kept.push(next);
        previous = Some(next);
        rest = &rest[next.len_utf8()..];
    }
    kept.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches([',', ':', ';', '-', ' '])
        .to_owned()
}

fn room_text(text: &str) -> Result<String, ErrorBody> {
    let text = non_blank(text, "the room line")?;
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(format!(
            "a room line holds at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(text)
}

/// Stores one line of `team`'s room, by a person when `bot` is `None`.
pub(super) fn insert_line(
    store: &Store,
    team: &TeamRow,
    bot: Option<&BotRow>,
    text: String,
    task_id: Option<String>,
) -> Result<TeamRoomMessageRow, ErrorBody> {
    let row = new_line(team, bot, text, task_id);
    insert_row(store, &row)?;
    Ok(row)
}

/// Stores a line made with `new_line`.
pub(super) fn insert_row(store: &Store, row: &TeamRoomMessageRow) -> Result<(), ErrorBody> {
    store.insert_team_room_message(row).map_err(store_error)
}

/// One line of `team`'s room, not yet stored.
pub(super) fn new_line(
    team: &TeamRow,
    bot: Option<&BotRow>,
    text: String,
    task_id: Option<String>,
) -> TeamRoomMessageRow {
    TeamRoomMessageRow {
        id: new_id(),
        team_id: team.id.clone(),
        author: if bot.is_some() { "bot" } else { "person" }.to_owned(),
        bot_id: bot.map(|bot| bot.id.clone()),
        text,
        task_id,
        routine_id: None,
        card: None,
        card_answer: None,
        reply_to: None,
        reply_bot_id: None,
        reply_session: None,
        reply_state: None,
        created_at: now_ms(),
    }
}

/// The lines as the room shows them: each bot's name, each task as the Board shows it now, and each card.
pub(super) fn views(
    store: &Store,
    agents: &Agents,
    team: &TeamRow,
    rows: &[TeamRoomMessageRow],
) -> Result<Vec<TeamRoomMessage>, ErrorBody> {
    let bots = store.list_bots(&team.id).map_err(store_error)?;
    let host = HostZone::read();
    rows.iter()
        .map(|row| {
            let routine = match row.routine_id.as_deref() {
                Some(id) => store
                    .get_routine(id)
                    .map_err(store_error)?
                    .map(|routine| routine_view(&routine, &host)),
                None => None,
            };
            let task = match row.task_id.as_deref() {
                Some(id) => match store.get_task(id).map_err(store_error)? {
                    Some(task) => Some(view_of(&task, &task_state::load(agents, &task)?)),
                    None => None,
                },
                None => None,
            };
            let quoted = match row.reply_to.as_deref() {
                Some(id) => store
                    .get_team_room_message(id)
                    .map_err(store_error)?
                    .map(|line| room_quote::quote(&line, &bots)),
                None => None,
            };
            Ok(TeamRoomMessage {
                id: row.id.clone(),
                team_id: row.team_id.clone(),
                author: match row.author.as_str() {
                    "person" => RoomAuthor::Person,
                    "bot" => RoomAuthor::Bot,
                    _ => RoomAuthor::Unknown,
                },
                bot_id: row.bot_id.clone(),
                bot_name: row.bot_id.as_deref().and_then(|id| {
                    bots.iter()
                        .find(|bot| bot.id == id)
                        .map(|bot| bot.name.clone())
                }),
                text: row.text.clone(),
                task,
                routine,
                card: card_view(row),
                reply_to: row.reply_to.clone(),
                quoted,
                created_at_ms: row.created_at,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bot(name: &str, lead: bool) -> BotRow {
        BotRow {
            id: name.to_lowercase(),
            team_id: "t".to_owned(),
            name: name.to_owned(),
            job: String::new(),
            kind: "claude".to_owned(),
            model: None,
            effort: None,
            lead,
            created_at: 1,
        }
    }

    #[test]
    fn a_mention_picks_its_bot_else_the_lead_else_the_first() {
        let bots = [
            bot("Mara", false),
            bot("Bruno", true),
            bot("Bruno Jr", false),
        ];
        let pick = |text: &str| addressed_bot(&bots, text, None).map(|bot| bot.name.clone());
        assert_eq!(pick("@mara add sign-in").as_deref(), Some("Mara"));
        assert_eq!(pick("(@mara) add sign-in").as_deref(), Some("Mara"));
        assert_eq!(pick("ask @Bruno Jr, please").as_deref(), Some("Bruno Jr"));
        assert_eq!(
            pick("mail me@mara.dev").as_deref(),
            Some("Bruno"),
            "not a mention in the middle of a word"
        );
        assert_eq!(pick("add sign-in").as_deref(), Some("Bruno"), "the lead");
        assert_eq!(
            addressed_bot(&bots[..1], "add sign-in", None).map(|bot| bot.name.as_str()),
            Some("Mara")
        );
        assert_eq!(
            addressed_bot(&bots, "add sign-in", Some(&bots[0])).map(|bot| bot.name.as_str()),
            Some("Mara"),
            "the receiver the composer picked, ahead of the lead"
        );
        assert_eq!(
            addressed_bot(&bots, "@bruno add sign-in", Some(&bots[0])).map(|bot| bot.name.as_str()),
            Some("Bruno"),
            "a mention wins over the receiver"
        );
        assert!(addressed_bot(&[], "add sign-in", None).is_none());
    }

    #[test]
    fn without_a_mention_a_receiver_or_a_lead_only_a_team_of_one_bot_picks_one() {
        let bots = [bot("Mara", false), bot("Bruno", false)];
        assert!(
            addressed_bot(&bots, "add sign-in", None).is_none(),
            "two bots and no lead: the room asks"
        );
        assert_eq!(
            addressed_bot(&bots, "@bruno add sign-in", None).map(|bot| bot.name.as_str()),
            Some("Bruno")
        );
        assert_eq!(
            addressed_bot(&bots[..1], "add sign-in", None).map(|bot| bot.name.as_str()),
            Some("Mara"),
            "the only bot"
        );
    }

    #[test]
    fn a_title_is_the_first_line_cut_to_eighty_characters() {
        assert_eq!(
            task_title("\n  Add magic link sign-in\nwith email", &[]),
            "Add magic link sign-in"
        );
        let title = task_title(&"x".repeat(100), &[]);
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn a_title_leaves_out_the_mentions_of_the_team_bots() {
        let bots = [bot("Mara", false), bot("Bruno Jr", false)];
        assert_eq!(task_title("@mara, add sign-in", &bots), "add sign-in");
        assert_eq!(
            task_title("Ask @Bruno Jr to parse   the headers", &bots),
            "Ask to parse the headers"
        );
        assert_eq!(
            task_title("@Mara\nParse the headers", &bots),
            "Parse the headers",
            "a first line that is only a mention is skipped"
        );
        assert_eq!(
            task_title("mail me@mara.dev and @iris", &bots),
            "mail me@mara.dev and @iris",
            "not a mention of a team bot"
        );
        let long = format!("@mara {}", "x".repeat(90));
        assert_eq!(task_title(&long, &bots).chars().count(), MAX_TITLE_CHARS);
    }

    /// A store with team `alpha`, bots Mara and Bruno, and a room task of Bruno's run by `session-1`.
    fn store_with_room_task() -> (Store, TeamRow, String) {
        use massorca_core_store::{TaskOrigin as StoredOrigin, TaskRow};
        let store = Store::open_in_memory().expect("in-memory store opens");
        let team = TeamRow {
            id: new_id(),
            name: "alpha".to_owned(),
            purpose: String::new(),
            character: "triangle".to_owned(),
            access: "default".to_owned(),
            workspaces: false,
            created_at: 1,
        };
        store.insert_team(&team).expect("team");
        for (name, created) in [("Mara", 1), ("Bruno", 2)] {
            let mut row = bot(name, false);
            row.team_id = team.id.clone();
            row.created_at = created;
            store.insert_bot(&row).expect("bot");
        }
        let task = TaskRow {
            id: new_id(),
            team_id: team.id.clone(),
            project_id: None,
            bot_id: Some("bruno".to_owned()),
            title: "parse".to_owned(),
            body: String::new(),
            origin: StoredOrigin::Room,
            session_id: Some("session-1".to_owned()),
            outcome: None,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            created_at: 1,
            updated_at: 1,
        };
        store.insert_task(&task).expect("task");
        (store, team, task.id)
    }

    #[test]
    fn an_agent_run_posts_only_as_the_bot_of_its_own_task() {
        let (store, team, task) = store_with_room_task();
        let line = agent_line(&store, &team, "session-1", None, "On it".to_owned()).unwrap();
        assert_eq!(line.bot_id.as_deref(), Some("bruno"));
        assert_eq!(line.task_id.as_deref(), Some(task.as_str()));
        assert!(
            agent_line(
                &store,
                &team,
                "session-1",
                Some("Bruno"),
                "Still on it".to_owned()
            )
            .is_ok()
        );

        let refused = agent_line(
            &store,
            &team,
            "session-1",
            Some("Mara"),
            "I am Mara".to_owned(),
        )
        .expect_err("another bot is refused");
        assert_eq!(refused.code, codes::PERMISSION_DENIED);
        let unknown = agent_line(&store, &team, "session-9", None, "hi".to_owned())
            .expect_err("a run with no task of the team is refused");
        assert_eq!(unknown.code, codes::PERMISSION_DENIED);
        let texts: Vec<String> = store
            .list_team_room(&team.id, None)
            .unwrap()
            .into_iter()
            .map(|line| line.text)
            .collect();
        assert_eq!(texts, ["On it", "Still on it"]);
    }

    #[test]
    fn a_line_with_a_day_and_a_time_becomes_a_one_time_routine_of_its_bot() {
        let (store, team, _) = store_with_room_task();
        let bots = store.list_bots(&team.id).unwrap();
        let mara = bots.iter().find(|bot| bot.name == "Mara").unwrap();
        let text = "Back up the docs on 2099-01-02 at 09:00".to_owned();
        let moment = scheduled_moment(&text, Date::constant(2030, 1, 2)).unwrap();
        let asked = Asked {
            moment,
            admin: true,
            now: now_ms(),
            saved: &Cell::new(false),
        };
        let request = Request {
            text: text.clone(),
            posted: false,
            reply_to: None,
            context: None,
        };
        let lines = schedule_line(&store, &team, &bots, mara, &request, asked)
            .unwrap()
            .expect("the routine is made");

        assert_eq!(lines[0].author, "person");
        assert_eq!(lines[0].text, text);
        assert_eq!(lines[1].bot_id.as_deref(), Some("mara"));
        assert_eq!(
            lines[1].text,
            "Routine created: Back up the docs on 2099-01-02 at 09:00. It runs once on 2099-01-02 at 09:00."
        );
        let routines = store.list_routines(&team.id).unwrap();
        assert_eq!(routines.len(), 1);
        let routine = &routines[0];
        assert_eq!(lines[1].routine_id.as_deref(), Some(routine.id.as_str()));
        assert_eq!(
            (
                routine.cadence.as_str(),
                routine.run_date.as_deref(),
                routine.time.as_str()
            ),
            ("once", Some("2099-01-02"), "09:00")
        );
        assert_eq!(routine.bot_id.as_deref(), Some("mara"));
        assert_eq!(routine.prompt, text);
        assert!(
            routine.starts_as_admin,
            "it keeps the poster's host-admin scope"
        );
        assert!(routine.next_run_at.is_some());
        assert_eq!(
            store.list_tasks(Some(&team.id)).unwrap().len(),
            1,
            "no task beyond the fixture's"
        );
    }

    #[test]
    fn a_refused_routine_stores_nothing_so_the_line_becomes_a_task() {
        let (store, team, _) = store_with_room_task();
        let bots = store.list_bots(&team.id).unwrap();
        let text = "2000-01-01 at 9am rotate logs".to_owned();
        let moment = scheduled_moment(&text, Date::constant(2030, 1, 2)).unwrap();
        let asked = Asked {
            moment,
            admin: false,
            now: now_ms(),
            saved: &Cell::new(false),
        };
        let request = Request {
            text,
            posted: false,
            reply_to: None,
            context: None,
        };
        let refused = schedule_line(&store, &team, &bots, &bots[0], &request, asked).unwrap();
        assert_eq!(refused, None, "a moment already past");
        assert!(store.list_routines(&team.id).unwrap().is_empty());
        assert!(store.list_team_room(&team.id, None).unwrap().is_empty());
    }
}
