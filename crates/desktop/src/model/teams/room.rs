//! The Conference room as the page draws it (bt-conference-room, screens 4.2, decision 0102): the lines of the team's
//! room with who wrote them and the task, routine or choice card each is about, and the bot that receives a line naming
//! no bot.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use massorca_protocol::methods::tasks::Column as WireColumn;
use massorca_protocol::methods::team_room::{
    RoomAuthor, RoomCard, RoomQuote, TeamRoomMessage, WorkerOutcome,
};
use massorca_protocol::methods::teams::{DelegationLink, Routine, RoutineCadence};

use super::types::{BotId, Team};
use crate::service::{TeamIds, TeamRoomRead};

/// What the room says while it has no line.
pub const EMPTY_ROOM: &str = "Ask a bot, or describe what the team should do.";
/// What the room of a team without bots says above its composer.
pub const NO_BOTS: &str = "Add a bot to this team first.";

/// What the room shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomView {
    /// The room was not read yet.
    Loading,
    /// The room cannot be read: the service's sentence.
    Unavailable(String),
    /// The lines, oldest first; empty draws `EMPTY_ROOM`.
    Lines(Vec<RoomLine>),
}

/// One line of the room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomLine {
    pub id: String,
    pub speaker: Speaker,
    pub text: String,
    /// The local clock time it was posted, "10:42".
    pub time: String,
    pub task: Option<RoomTask>,
    /// The one-time routine the line scheduled (decision 0121).
    pub routine: Option<RoomRoutine>,
    /// The choice card the line is (decision 0121).
    pub card: Option<RoomCard>,
    /// The line it replies to, "Mara: Parsed the headers" (decision 0122).
    pub quote: Option<String>,
}

/// The routine a line scheduled, as its "Routine created" card shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomRoutine {
    pub name: String,
    /// "Once on 2026-11-30 at 09:00, Europe/Madrid": the zone it runs on, its own or the host's.
    pub when: String,
}

/// Who wrote a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Speaker {
    Person,
    /// A bot of the team; `lead` draws the Lead chip.
    Bot {
        name: String,
        lead: bool,
    },
    /// The room itself, which asks on a choice card who takes a request.
    Room,
    /// A bot that was removed since, or an author this build does not know.
    Gone,
}

/// The task a line is about, as its card shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomTask {
    pub id: String,
    pub title: String,
    pub column: &'static str,
    /// The children the task delegated, oldest first (decision 0128).
    pub children: Vec<RoomChild>,
}

/// A child a task delegated, as its card lists it under the task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomChild {
    pub title: String,
    pub column: &'static str,
    /// The child only read in its parent's workspace.
    pub read_only: bool,
    /// The subject of the child's latest status message to its parent (decision 0129).
    pub status: Option<String>,
    /// The outcome its `worker_done` reported: `succeeded` or `failed`.
    pub reported: Option<&'static str>,
}

impl RoomChild {
    /// The row a task card lists the child as: title, state, workspace, report and latest status (X-DL-01).
    pub fn row_text(&self) -> String {
        let mut text = format!("{} · {}", self.title, self.column);
        if self.read_only {
            text.push_str(" · reads in the parent's workspace");
        }
        if let Some(reported) = self.reported {
            text.push_str(" · reported ");
            text.push_str(reported);
        }
        if let Some(status) = &self.status {
            text.push_str(" · ");
            text.push_str(status);
        }
        text
    }
}

/// The room of `team` from the store's read of it.
pub fn room_view(
    read: Option<&TeamRoomRead>,
    team: &Team,
    ids: &TeamIds,
    zone: &TimeZone,
) -> RoomView {
    let links = read.map_or(&[][..], |read| read.delegations.as_slice());
    match read.map(|read| &read.room) {
        None => RoomView::Loading,
        Some(Err(refusal)) => RoomView::Unavailable(refusal.sentence()),
        Some(Ok(messages)) => RoomView::Lines(
            messages
                .iter()
                .map(|message| room_line(message, team, ids, zone, links))
                .collect(),
        ),
    }
}

fn room_line(
    message: &TeamRoomMessage,
    team: &Team,
    ids: &TeamIds,
    zone: &TimeZone,
    links: &[DelegationLink],
) -> RoomLine {
    let speaker = match message.author {
        RoomAuthor::Person => Speaker::Person,
        RoomAuthor::Bot if message.card.is_some() => Speaker::Room,
        RoomAuthor::Bot => match (&message.bot_name, message.bot_id.as_deref()) {
            (Some(name), Some(id)) => Speaker::Bot {
                name: name.clone(),
                lead: team.lead.is_some() && ids.get(id).map(BotId) == team.lead,
            },
            _ => Speaker::Gone,
        },
        RoomAuthor::Unknown => Speaker::Gone,
    };
    RoomLine {
        id: message.id.clone(),
        speaker,
        text: message.text.clone(),
        time: clock_text(message.created_at_ms, zone),
        task: message.task.as_ref().map(|task| RoomTask {
            id: task.id.clone(),
            title: task.title.clone(),
            column: column_word(task.column),
            children: children_of(&task.id, links),
        }),
        routine: message.routine.as_ref().map(|routine| RoomRoutine {
            name: routine.name.clone(),
            when: routine_when(routine),
        }),
        card: message.card.clone(),
        quote: message.quoted.as_ref().map(quote_text),
    }
}

/// A quoted line as a reply shows it: who wrote it, then its text.
fn quote_text(quote: &RoomQuote) -> String {
    let who = match (quote.author, quote.bot_name.as_deref()) {
        (RoomAuthor::Person, _) => "You",
        (RoomAuthor::Bot, Some(name)) => name,
        _ => "A removed bot",
    };
    format!("{who}: {}", quote.text)
}

/// When a routine runs, in words, with its zone; the room schedules only one-time routines.
fn routine_when(routine: &Routine) -> String {
    let when = match (routine.cadence, routine.run_date.as_deref()) {
        (RoutineCadence::Once, Some(date)) => format!("Once on {date} at {}", routine.time),
        _ => format!("At {}", routine.time),
    };
    match routine.zone.as_deref().or(routine.zone_used.as_deref()) {
        Some(zone) => format!("{when}, {zone}"),
        None => when,
    }
}

/// The column a task card names.
/// The children `parent` delegated, oldest first, as the task card lists them.
fn children_of(parent: &str, links: &[DelegationLink]) -> Vec<RoomChild> {
    links
        .iter()
        .filter(|link| link.parent == parent)
        .map(|link| RoomChild {
            title: link.child.title.clone(),
            column: column_word(link.child.column),
            read_only: link.read_only,
            status: link.report.status.clone(),
            reported: link.report.outcome.and_then(outcome_word),
        })
        .collect()
}

/// The word a child row reports its `worker_done` with; an outcome from a newer peer shows none.
fn outcome_word(outcome: WorkerOutcome) -> Option<&'static str> {
    match outcome {
        WorkerOutcome::Succeeded => Some("succeeded"),
        WorkerOutcome::Failed => Some("failed"),
        WorkerOutcome::Unknown => None,
    }
}

fn column_word(column: WireColumn) -> &'static str {
    match column {
        WireColumn::Open => "Open",
        WireColumn::Running => "Running",
        WireColumn::Waiting => "Waiting for you",
        WireColumn::Done => "Done",
        WireColumn::Unknown => "On the Board",
    }
}

/// "10:42" in `zone`; empty for a time out of range.
pub fn clock_text(at_ms: i64, zone: &TimeZone) -> String {
    Timestamp::from_millisecond(at_ms)
        .map(|at| {
            at.to_zoned_datetime(zone.clone())
                .strftime("%H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

/// The bot that receives a line naming no bot: the one picked in the composer while it is still on the team, else
/// the lead, else the only bot. `None` with several bots: the room then asks on a choice card.
pub fn receiver(team: &Team, picked: Option<BotId>) -> Option<BotId> {
    picked
        .filter(|bot| team.bot(*bot).is_some())
        .or(team.lead)
        .or(match team.bots.as_slice() {
            [only] => Some(only.id),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::teams::{SampleTeamsSource, TeamsSource};
    use crate::service::Refusal;
    use massorca_protocol::methods::tasks::{CardState, Task, TaskOrigin};
    use massorca_protocol::methods::teams::ChildReport;

    fn team() -> Team {
        SampleTeamsSource.teams().teams()[0].clone()
    }

    fn message(author: RoomAuthor, bot: Option<(&str, &str)>, text: &str) -> TeamRoomMessage {
        TeamRoomMessage {
            id: text.to_owned(),
            author,
            bot_id: bot.map(|(id, _)| id.to_owned()),
            bot_name: bot.map(|(_, name)| name.to_owned()),
            text: text.to_owned(),
            ..crate::service::fixtures::base_team_room_message()
        }
    }

    #[test]
    fn a_reply_shows_who_wrote_the_line_it_quotes() {
        let mut reply = message(RoomAuthor::Person, None, "Why so slow?");
        reply.reply_to = Some("q".to_owned());
        reply.quoted = Some(RoomQuote {
            id: "q".to_owned(),
            author: RoomAuthor::Bot,
            bot_name: Some("Mara".to_owned()),
            text: "Parsed the headers".to_owned(),
        });
        let room = TeamRoomRead {
            team: "t".to_owned(),
            number: 1,
            answers_cards: true,
            answers_replies: true,
            typing: Vec::new(),
            delegations: Vec::new(),
            room: Ok(vec![reply, message(RoomAuthor::Person, None, "plain")]),
        };
        let RoomView::Lines(lines) =
            room_view(Some(&room), &team(), &TeamIds::default(), &TimeZone::UTC)
        else {
            panic!("the room has lines");
        };
        assert_eq!(lines[0].quote.as_deref(), Some("Mara: Parsed the headers"));
        assert_eq!(lines[1].quote, None);
    }

    #[test]
    fn a_line_that_scheduled_a_routine_carries_its_card() {
        let routine = Routine {
            id: "r1".to_owned(),
            team_id: "t".to_owned(),
            bot_id: None,
            name: "Back up the docs".to_owned(),
            prompt: "Back up the docs tomorrow at 09:00".to_owned(),
            cadence: RoutineCadence::Once,
            time: "09:00".to_owned(),
            weekday: None,
            month_day: None,
            run_date: Some("2099-01-02".to_owned()),
            zone: None,
            zone_used: Some("Europe/Madrid".to_owned()),
            paused: false,
            starts_as_admin: false,
            next_run_at_ms: Some(1),
            created_at_ms: 1,
        };
        let mut line = message(RoomAuthor::Bot, Some(("b", "Mara")), "Routine created");
        line.routine = Some(routine);
        let room = TeamRoomRead {
            team: "t".to_owned(),
            number: 1,
            answers_cards: true,
            answers_replies: true,
            typing: Vec::new(),
            delegations: Vec::new(),
            room: Ok(vec![line]),
        };
        let RoomView::Lines(lines) =
            room_view(Some(&room), &team(), &TeamIds::default(), &TimeZone::UTC)
        else {
            panic!("the room has lines");
        };
        assert_eq!(
            lines[0].routine,
            Some(RoomRoutine {
                name: "Back up the docs".to_owned(),
                when: "Once on 2099-01-02 at 09:00, Europe/Madrid".to_owned(),
            })
        );
        assert_eq!(lines[0].task, None);
    }

    #[test]
    fn an_unread_room_loads_and_a_refused_one_says_why() {
        let team = team();
        let ids = TeamIds::default();
        assert_eq!(
            room_view(None, &team, &ids, &TimeZone::UTC),
            RoomView::Loading
        );
        let refused = TeamRoomRead {
            team: "t".to_owned(),
            number: 1,
            answers_cards: true,
            answers_replies: true,
            typing: Vec::new(),
            delegations: Vec::new(),
            room: Err(Refusal::unavailable("The service is not connected.")),
        };
        assert!(matches!(
            room_view(Some(&refused), &team, &ids, &TimeZone::UTC),
            RoomView::Unavailable(sentence) if sentence.contains("not connected")
        ));
    }

    #[test]
    fn lines_name_their_speaker_their_time_and_their_task() {
        let team = team();
        let ids = TeamIds::default();
        let mut answer = message(
            RoomAuthor::Bot,
            Some(("b1", "Mara")),
            "Added to the Board: Sign-in",
        );
        answer.task = Some(Task {
            id: "task-1".to_owned(),
            team_id: "t".to_owned(),
            project_id: None,
            bot_id: Some("b1".to_owned()),
            title: "Sign-in".to_owned(),
            body: String::new(),
            origin: TaskOrigin::Room,
            column: WireColumn::Running,
            state: CardState::Working,
            session_id: None,
            outcome: None,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            last_known: false,
            created_at_ms: 1,
            updated_at_ms: 1,
        });
        let read = TeamRoomRead {
            team: "t".to_owned(),
            number: 1,
            answers_cards: true,
            answers_replies: true,
            typing: Vec::new(),
            delegations: Vec::new(),
            room: Ok(vec![
                message(RoomAuthor::Person, None, "We need sign-in"),
                answer,
                message(RoomAuthor::Bot, None, "From a removed bot"),
            ]),
        };
        let RoomView::Lines(lines) = room_view(Some(&read), &team, &ids, &TimeZone::UTC) else {
            panic!("the room has lines");
        };
        assert_eq!(lines[0].speaker, Speaker::Person);
        assert_eq!(lines[0].time, "22:13");
        assert_eq!(
            lines[1].speaker,
            Speaker::Bot {
                name: "Mara".to_owned(),
                lead: false
            }
        );
        assert_eq!(
            lines[1].task,
            Some(RoomTask {
                id: "task-1".to_owned(),
                title: "Sign-in".to_owned(),
                column: "Running",
                children: Vec::new(),
            })
        );
        assert_eq!(lines[2].speaker, Speaker::Gone);
    }

    #[test]
    fn a_task_card_lists_the_children_it_delegated_and_no_others() {
        let team = team();
        let ids = TeamIds::default();
        let mut answer = message(RoomAuthor::Bot, Some(("b1", "Mara")), "Added to the Board");
        answer.task = Some(wire_task("task-1", "Sign-in", WireColumn::Running));
        let links = vec![
            DelegationLink {
                parent: "task-1".to_owned(),
                read_only: true,
                child: wire_task("task-2", "Review the headers", WireColumn::Open),
                report: ChildReport::default(),
            },
            DelegationLink {
                parent: "task-9".to_owned(),
                read_only: false,
                child: wire_task("task-10", "Elsewhere", WireColumn::Open),
                report: ChildReport::default(),
            },
        ];
        let read = TeamRoomRead {
            team: "t".to_owned(),
            number: 1,
            answers_cards: true,
            answers_replies: true,
            typing: Vec::new(),
            delegations: links,
            room: Ok(vec![answer]),
        };
        let RoomView::Lines(lines) = room_view(Some(&read), &team, &ids, &TimeZone::UTC) else {
            panic!("the room has lines");
        };
        let task = lines[0].task.as_ref().expect("the line is about a task");
        assert_eq!(
            task.children,
            vec![RoomChild {
                title: "Review the headers".to_owned(),
                column: "Open",
                read_only: true,
                status: None,
                reported: None,
            }]
        );
    }

    #[test]
    fn a_child_row_shows_its_latest_status_and_its_reported_outcome() {
        let links = vec![DelegationLink {
            parent: "task-1".to_owned(),
            read_only: false,
            child: wire_task("task-2", "Write the lexer", WireColumn::Done),
            report: ChildReport {
                status: Some("Numbers parse".to_owned()),
                outcome: Some(WorkerOutcome::Succeeded),
            },
        }];
        let children = children_of("task-1", &links);
        assert_eq!(children[0].status.as_deref(), Some("Numbers parse"));
        assert_eq!(children[0].reported, Some("succeeded"));
        assert_eq!(
            children[0].row_text(),
            "Write the lexer · Done · reported succeeded · Numbers parse"
        );
        let unknown = DelegationLink {
            report: ChildReport {
                status: None,
                outcome: Some(WorkerOutcome::Unknown),
            },
            ..links[0].clone()
        };
        let quiet = children_of("task-1", &[unknown]);
        assert_eq!(quiet[0].row_text(), "Write the lexer · Done");
    }

    fn wire_task(id: &str, title: &str, column: WireColumn) -> Task {
        Task {
            id: id.to_owned(),
            team_id: "t".to_owned(),
            project_id: None,
            bot_id: Some("b1".to_owned()),
            title: title.to_owned(),
            body: String::new(),
            origin: TaskOrigin::Room,
            column,
            state: CardState::Working,
            session_id: None,
            outcome: None,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            last_known: false,
            created_at_ms: 1,
            updated_at_ms: 1,
        }
    }

    #[test]
    fn a_card_line_is_the_room_speaking_and_carries_its_options() {
        use massorca_protocol::methods::team_room::{RoomChoice, RoomChoiceKind};
        let card = RoomCard {
            request_id: "m1".to_owned(),
            options: vec![RoomChoice {
                choice: "todo".to_owned(),
                kind: RoomChoiceKind::Todo,
                label: "Keep in this team's Todo".to_owned(),
            }],
            answer: None,
        };
        let mut line = message(RoomAuthor::Bot, None, "Who should take this?");
        line.card = Some(card.clone());
        let read = TeamRoomRead {
            team: "t".to_owned(),
            number: 1,
            answers_cards: true,
            answers_replies: true,
            typing: Vec::new(),
            delegations: Vec::new(),
            room: Ok(vec![line]),
        };
        let RoomView::Lines(lines) =
            room_view(Some(&read), &team(), &TeamIds::default(), &TimeZone::UTC)
        else {
            panic!("the room has lines");
        };
        assert_eq!(lines[0].speaker, Speaker::Room);
        assert_eq!(lines[0].card, Some(card));
    }

    #[test]
    fn a_line_naming_no_bot_goes_to_the_pick_else_the_lead_else_the_only_bot() {
        let mut team = team();
        let first = team.bots[0].id;
        let second = team.bots[1].id;
        team.lead = Some(second);
        assert_eq!(receiver(&team, None), Some(second), "the lead");
        assert_eq!(receiver(&team, Some(first)), Some(first), "the pick");
        assert_eq!(
            receiver(&team, Some(BotId(999))),
            Some(second),
            "a removed pick"
        );
        team.lead = None;
        assert_eq!(receiver(&team, None), None, "several bots: a choice card");
        assert_eq!(receiver(&team, Some(first)), Some(first), "the pick");
        team.bots.truncate(1);
        assert_eq!(receiver(&team, None), Some(first), "the only bot");
        team.bots.clear();
        assert_eq!(receiver(&team, None), None);
    }
}
