//! How a Conference room conversation gets its one answer (decision 0122): the bot posts it from its reply run, or the
//! run's turn ends and the host posts the turn's result. Each move of the reply's state is one statement, so two answers
//! never post.

use massorca_core_agents::Agents;
use massorca_core_store::{Store, TaskOutcome, TeamRoomMessageRow, TeamRow, new_id, now_ms};
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::{ErrorBody, codes};

use super::room::insert_row;
use super::room_quote::cut;
use super::task_state::observed_outcome;
use super::{selectors, store_error};

/// The longest answer, in Unicode scalar values (pv96's `REPLY_MAX_CHARS`).
pub(super) const MAX_REPLY_CHARS: usize = 1200;
/// What a bot posts when its run gave no answer.
pub(super) const NO_ANSWER: &str = "I could not answer this time.";

/// A conversation line whose bot has not answered.
pub(super) const OPEN: &str = "open";
/// The bot posted its answer from its run, which has not ended yet.
const ANSWERED: &str = "answered";
/// The answer is posted and the run ended or was stopped.
const CLOSED: &str = "closed";

/// How a reply run's turn ended: with its result text when it succeeded, or forgotten by the agents domain.
pub(crate) enum ReplyEnd {
    Finished(Option<String>),
    Forgotten,
}

/// How a session's turn ended, once it ended: a Done the host saw, or a run that exited without one.
pub(crate) fn reply_end(session: &AgentSession) -> Option<ReplyEnd> {
    match observed_outcome(session) {
        Some(TaskOutcome::Success) => Some(ReplyEnd::Finished(session.result.clone())),
        Some(_) => Some(ReplyEnd::Finished(None)),
        None => {
            (session.execution.state == TerminalState::Exited).then_some(ReplyEnd::Finished(None))
        }
    }
}

/// Settles the reply whose run is `session`, once its turn ended: posts its answer unless its bot posted one, and stops
/// the run. Returns the team whose room changed.
pub(crate) fn settle_reply(
    store: &Store,
    agents: &Agents,
    session: &str,
    end: &ReplyEnd,
) -> Result<Option<String>, ErrorBody> {
    let Some(question) = store.find_team_room_reply(session).map_err(store_error)? else {
        return Ok(None);
    };
    let settled = settle_line(store, &question, end)?;
    stop_run(agents, settled.stop.as_deref());
    Ok(settled.changed.then_some(question.team_id))
}

/// What settling a reply did: the answer it posted, whether the reply changed, and the run to stop.
pub(super) struct Settled {
    pub(super) posted: Option<TeamRoomMessageRow>,
    pub(super) changed: bool,
    pub(super) stop: Option<String>,
}

/// Posts the answer of an open reply, or closes one its bot answered. A run whose turn ended is stopped once, by the
/// call that closed its reply.
pub(super) fn settle_line(
    store: &Store,
    question: &TeamRoomMessageRow,
    end: &ReplyEnd,
) -> Result<Settled, ErrorBody> {
    let answer = match end {
        ReplyEnd::Finished(Some(result)) if !result.trim().is_empty() => {
            cut(result.trim(), MAX_REPLY_CHARS)
        }
        _ => NO_ANSWER.to_owned(),
    };
    let run = match end {
        ReplyEnd::Finished(_) => question.reply_session.clone(),
        ReplyEnd::Forgotten => None,
    };
    if let Some(posted) = close(store, question, OPEN, &answer)? {
        return Ok(Settled {
            posted: Some(posted),
            changed: true,
            stop: run,
        });
    }
    let changed = store
        .move_team_room_reply(&question.id, ANSWERED, CLOSED)
        .map_err(store_error)?;
    Ok(Settled {
        posted: None,
        changed,
        stop: run.filter(|_| changed),
    })
}

/// Moves the reply from `from` to closed and posts `answer` as its bot; nothing when the reply was not in `from`.
pub(super) fn close(
    store: &Store,
    question: &TeamRoomMessageRow,
    from: &str,
    answer: &str,
) -> Result<Option<TeamRoomMessageRow>, ErrorBody> {
    if !store
        .move_team_room_reply(&question.id, from, CLOSED)
        .map_err(store_error)?
    {
        return Ok(None);
    }
    Ok(Some(answer_line(store, question, answer.to_owned())?))
}

/// Ends a reply run whose turn is over, so no idle run is left per question. A failure is only logged.
pub(super) fn stop_run(agents: &Agents, session: Option<&str>) {
    if let Some(session) = session
        && let Err(error) = agents.stop(session, true)
    {
        tracing::warn!(
            code = %error.code,
            "a room reply run could not be stopped: {}",
            error.message
        );
    }
}

/// Stores the answer to `question` as its bot.
fn answer_line(
    store: &Store,
    question: &TeamRoomMessageRow,
    text: String,
) -> Result<TeamRoomMessageRow, ErrorBody> {
    let line = TeamRoomMessageRow {
        id: new_id(),
        team_id: question.team_id.clone(),
        author: "bot".to_owned(),
        bot_id: question.reply_bot_id.clone(),
        text,
        task_id: None,
        routine_id: None,
        card: None,
        card_answer: None,
        reply_to: Some(question.id.clone()),
        reply_bot_id: None,
        reply_session: None,
        reply_state: None,
        created_at: now_ms(),
    };
    insert_row(store, &line)?;
    Ok(line)
}

/// A line the reply run `session` posts itself: its answer, as its own bot, once. `None` when the run answers no
/// conversation of `team`.
pub(super) fn bot_answer(
    store: &Store,
    team: &TeamRow,
    session: &str,
    named: Option<&str>,
    text: &str,
) -> Result<Option<TeamRoomMessageRow>, ErrorBody> {
    let Some(question) = store
        .find_team_room_reply(session)
        .map_err(store_error)?
        .filter(|line| line.team_id == team.id)
    else {
        return Ok(None);
    };
    let Some(own) = question
        .reply_bot_id
        .as_deref()
        .map(|bot| selectors::bot(store, team, bot))
        .transpose()?
    else {
        return Ok(None);
    };
    if let Some(named) = named
        && selectors::bot(store, team, named)?.id != own.id
    {
        return Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            format!("an agent run posts only as its own bot, {}", own.name),
        ));
    }
    if !store
        .move_team_room_reply(&question.id, OPEN, ANSWERED)
        .map_err(store_error)?
    {
        return Err(ErrorBody::new(
            codes::PERMISSION_DENIED,
            "this conversation already has its answer",
        ));
    }
    Ok(Some(answer_line(
        store,
        &question,
        cut(text, MAX_REPLY_CHARS),
    )?))
}
