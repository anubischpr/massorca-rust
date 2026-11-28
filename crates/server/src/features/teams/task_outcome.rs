//! Latches a task's outcome from its agent session, and tells the Board when a linked session changes (decision 0072).
//! Only a Done the host can see latches; loss of contact never does. A Conference room reply run's end posts its answer
//! (decision 0122).

use std::sync::Arc;

use massorca_core_agents::Change;
use massorca_core_store::{
    Store, StoreError, TaskOrigin, TaskOutcome, TaskRow, TeamRoomMessageRow, new_id, now_ms,
};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_TASK_CHANGED, TOPIC_TEAM_ROOM_CHANGED,
};
use tokio::sync::broadcast::{Receiver, error::RecvError};

use crate::context::AppState;
use crate::shutdown::ShutdownSignal;

use super::room_reply_end::{ReplyEnd, reply_end, settle_reply};
use super::task_state::{Session, observed_outcome};
use super::{blocking_state, store_error};

/// Follows the agents domain until shutdown. It reads the sessions once at start, because a Done recorded while no
/// listener was attached never arrived, and again after a lag, because the events it missed are gone.
pub(crate) async fn follow_task_outcomes(
    state: Arc<AppState>,
    mut changes: Receiver<Change>,
    mut signal: ShutdownSignal,
) {
    resync(&state).await;
    loop {
        let received = tokio::select! {
            () = signal.wait() => return,
            received = changes.recv() => received,
        };
        match received {
            Ok(change) => react(&state, change).await,
            Err(RecvError::Lagged(_)) => resync(&state).await,
            Err(RecvError::Closed) => return,
        }
    }
}

/// Latches what one change shows, then tells the Board about the task its session is linked to.
async fn react(state: &Arc<AppState>, change: Change) {
    let (session_id, outcome, end) = match change {
        Change::Updated(session) => (
            session.id.clone(),
            observed_outcome(&session),
            reply_end(&session),
        ),
        Change::Forgotten(id) => (id, None, Some(ReplyEnd::Forgotten)),
        Change::Account(_) | Change::Models(_) | Change::Queue(_) | Change::Chat(_) => return,
    };
    let linked = blocking_state(state, move |store, agents| {
        let replied = match &end {
            Some(end) => settle_reply(store, agents, &session_id, end)?,
            None => None,
        };
        let linked = latch_linked(store, &session_id, outcome).map_err(store_error)?;
        Ok((linked, replied))
    })
    .await;
    match linked {
        Ok((linked, replied)) => {
            if let Some(linked) = linked {
                publish_linked(state, &linked);
            }
            if let Some(team) = replied {
                publish_room(state, team);
            }
        }
        Err(error) => report(&error),
    }
}

/// Latches every session the agents domain lists now, and tells the Board about each linked task.
async fn resync(state: &Arc<AppState>) {
    let linked = blocking_state(state, |store, agents| {
        let sessions = agents.list(None).map_err(ErrorBody::from)?;
        let (mut linked, mut replied) = (Vec::new(), Vec::new());
        for session in &sessions {
            if let Some(end) = reply_end(session) {
                replied.extend(settle_reply(store, agents, &session.id, &end)?);
            }
            linked.extend(
                latch_linked(store, &session.id, observed_outcome(session)).map_err(store_error)?,
            );
        }
        Ok((linked, replied))
    })
    .await;
    match linked {
        Ok((linked, replied)) => {
            linked
                .iter()
                .for_each(|linked| publish_linked(state, linked));
            replied
                .into_iter()
                .for_each(|team| publish_room(state, team));
        }
        Err(error) => report(&error),
    }
}

/// The task a session change reached, and the team whose room heard about it.
struct Linked {
    task_id: String,
    room_team: Option<String>,
}

/// Latches `outcome` into the task linked to `session_id`, if there is one, and returns that task. A latched outcome
/// is kept, so a later change of the same session changes nothing. A room task's latch is told in its room by its bot.
fn latch_linked(
    store: &Store,
    session_id: &str,
    outcome: Option<TaskOutcome>,
) -> Result<Option<Linked>, StoreError> {
    let Some(task) = store.find_task_by_session(session_id)? else {
        return Ok(None);
    };
    let mut room_team = None;
    if let Some(outcome) = outcome
        && store.latch_task_outcome(&task.id, session_id, outcome, now_ms())?
    {
        room_team = tell_room(store, &task, outcome)?;
    }
    Ok(Some(Linked {
        task_id: task.id,
        room_team,
    }))
}

/// A room task's bot posts the outcome that just latched, once per run: only the latch that sets the outcome calls
/// this. Returns the team whose room changed.
fn tell_room(
    store: &Store,
    task: &TaskRow,
    outcome: TaskOutcome,
) -> Result<Option<String>, StoreError> {
    // A delegated child reports to its room as a room task does (decision 0128).
    if task.origin != TaskOrigin::Room && store.task_delegation_parent(&task.id)?.is_none() {
        return Ok(None);
    }
    store.insert_team_room_message(&TeamRoomMessageRow {
        id: new_id(),
        team_id: task.team_id.clone(),
        author: "bot".to_owned(),
        bot_id: task.bot_id.clone(),
        text: format!("{}: {}", outcome_word(outcome), task.title),
        task_id: Some(task.id.clone()),
        routine_id: None,
        card: None,
        card_answer: None,
        reply_to: None,
        reply_bot_id: None,
        reply_session: None,
        reply_state: None,
        created_at: now_ms(),
    })?;
    Ok(Some(task.team_id.clone()))
}

/// The word a room line opens with for a latched outcome, as the Board's card states read.
pub(super) fn outcome_word(outcome: TaskOutcome) -> &'static str {
    match outcome {
        TaskOutcome::Success => "Done",
        TaskOutcome::Failure => "Stuck",
        TaskOutcome::Cancelled => "Stopped",
        TaskOutcome::Interrupted => "Interrupted",
    }
}

fn publish_linked(state: &AppState, linked: &Linked) {
    publish_task(state, &linked.task_id);
    if let Some(team) = &linked.room_team {
        publish_room(state, team.clone());
    }
}

fn publish_room(state: &AppState, team: String) {
    state.events.publish(ChangeEvent {
        topic: TOPIC_TEAM_ROOM_CHANGED.to_owned(),
        id: team,
    });
}

/// Latches the outcome a loaded session already shows into `task`, which was just linked to it. The listener finds a
/// task by its session, so a Done that landed before the link would never latch otherwise. Returns whether it latched,
/// and the team whose room heard about it.
pub(crate) fn latch_at_link(
    store: &Store,
    task: &TaskRow,
    session: &Session,
) -> Result<(bool, Option<String>), StoreError> {
    let Session::Known(session) = session else {
        return Ok((false, None));
    };
    let Some(outcome) = observed_outcome(session) else {
        return Ok((false, None));
    };
    if !store.latch_task_outcome(&task.id, &session.id, outcome, now_ms())? {
        return Ok((false, None));
    }
    Ok((true, tell_room(store, task, outcome)?))
}

fn publish_task(state: &AppState, task_id: &str) {
    state.events.publish(ChangeEvent {
        topic: TOPIC_TASK_CHANGED.to_owned(),
        id: task_id.to_owned(),
    });
}

fn report(error: &ErrorBody) {
    tracing::warn!(
        code = %error.code,
        "the task outcome listener could not read or latch a task: {}",
        error.message
    );
}

#[cfg(test)]
mod tests {
    use massorca_core_store::{Store, TaskOrigin, TaskOutcome, TaskRow, TeamRow, new_id, now_ms};
    use massorca_protocol::methods::agents::{
        Activity, ActivitySource, ActivityState, AgentSession, Execution, Outcome,
    };
    use massorca_protocol::methods::terminals::TerminalState;

    use super::{Session, latch_at_link, latch_linked};

    fn store_with_task(session_id: &str) -> (Store, TaskRow) {
        store_with_origin(session_id, TaskOrigin::Cli)
    }

    fn store_with_origin(session_id: &str, origin: TaskOrigin) -> (Store, TaskRow) {
        let store = Store::open_in_memory().expect("in-memory store opens");
        let team = TeamRow {
            id: new_id(),
            name: "alpha".to_owned(),
            purpose: String::new(),
            character: "triangle".to_owned(),
            access: "full".to_owned(),
            workspaces: false,
            created_at: now_ms(),
        };
        store.insert_team(&team).expect("team");
        let task = TaskRow {
            id: new_id(),
            team_id: team.id,
            project_id: None,
            bot_id: None,
            title: "parse".to_owned(),
            body: String::new(),
            origin,
            session_id: Some(session_id.to_owned()),
            outcome: None,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            created_at: 1,
            updated_at: 1,
        };
        store.insert_task(&task).expect("task");
        (store, task)
    }

    fn session(
        id: &str,
        execution: TerminalState,
        state: ActivityState,
        outcome: Option<Outcome>,
    ) -> Session {
        Session::Known(Box::new(AgentSession {
            id: id.to_owned(),
            kind: "claude".to_owned(),
            workspace_id: "w1".to_owned(),
            created_at_ms: 1,
            execution: Execution {
                state: execution,
                exit: None,
            },
            activity: Some(Activity {
                state,
                outcome,
                source: ActivitySource::Hook,
                at_ms: 2,
                seen_at_ms: None,
            }),
            ..Default::default()
        }))
    }

    fn latched(store: &Store, task: &TaskRow) -> Option<TaskOutcome> {
        store
            .get_task(&task.id)
            .expect("store reads")
            .expect("task")
            .outcome
    }

    #[test]
    fn a_session_linked_after_it_finished_latches_the_done_it_shows() {
        let (store, task) = store_with_task("session-1");
        let finished = session(
            "session-1",
            TerminalState::Live,
            ActivityState::Done,
            Some(Outcome::Success),
        );
        assert!(latch_at_link(&store, &task, &finished).unwrap().0);
        assert_eq!(latched(&store, &task), Some(TaskOutcome::Success));
    }

    #[test]
    fn a_room_task_finished_before_its_link_still_tells_its_room() {
        let (store, task) = store_with_origin("session-1", TaskOrigin::Room);
        let finished = session(
            "session-1",
            TerminalState::Live,
            ActivityState::Done,
            Some(Outcome::Success),
        );
        let (did_latch, room) = latch_at_link(&store, &task, &finished).unwrap();
        assert!(did_latch);
        assert_eq!(room.as_deref(), Some(task.team_id.as_str()));
        // The listener's later change of the same session posts nothing more.
        let again = latch_linked(&store, "session-1", Some(TaskOutcome::Success))
            .unwrap()
            .expect("the task is linked");
        assert_eq!(again.room_team, None);
        let texts: Vec<String> = store
            .list_team_room(&task.team_id, None)
            .unwrap()
            .into_iter()
            .map(|line| line.text)
            .collect();
        assert_eq!(texts, ["Done: parse"]);
    }

    #[test]
    fn a_run_the_host_cannot_see_or_a_turn_still_running_latches_nothing_at_the_link() {
        let (store, task) = store_with_task("session-1");
        let unverifiable = session(
            "session-1",
            TerminalState::Unverifiable,
            ActivityState::Done,
            Some(Outcome::Failure),
        );
        let working = session(
            "session-1",
            TerminalState::Live,
            ActivityState::Working,
            None,
        );
        assert!(!latch_at_link(&store, &task, &unverifiable).unwrap().0);
        assert!(!latch_at_link(&store, &task, &working).unwrap().0);
        assert!(!latch_at_link(&store, &task, &Session::Forgotten).unwrap().0);
        assert_eq!(latched(&store, &task), None);
    }

    #[test]
    fn a_change_latches_only_the_task_its_session_is_linked_to_and_keeps_the_first_outcome() {
        let (store, task) = store_with_task("session-1");
        let linked = |session: &str, outcome| {
            latch_linked(&store, session, Some(outcome))
                .unwrap()
                .map(|linked| linked.task_id)
        };
        assert_eq!(linked("session-9", TaskOutcome::Failure), None);
        assert_eq!(
            linked("session-1", TaskOutcome::Failure),
            Some(task.id.clone())
        );
        assert_eq!(latched(&store, &task), Some(TaskOutcome::Failure));

        // The task is still named by its session, and the outcome it latched first stands.
        assert_eq!(
            linked("session-1", TaskOutcome::Success),
            Some(task.id.clone())
        );
        assert_eq!(latched(&store, &task), Some(TaskOutcome::Failure));
        assert!(
            store
                .list_team_room(&task.team_id, None)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_room_task_tells_its_room_once_when_its_outcome_latches() {
        let (store, task) = store_with_origin("session-1", TaskOrigin::Room);
        let first = latch_linked(&store, "session-1", Some(TaskOutcome::Success))
            .unwrap()
            .expect("the task is linked");
        assert_eq!(first.room_team.as_deref(), Some(task.team_id.as_str()));
        let again = latch_linked(&store, "session-1", Some(TaskOutcome::Success))
            .unwrap()
            .expect("the task is linked");
        assert_eq!(again.room_team, None, "a kept outcome is not told again");
        let room = store.list_team_room(&task.team_id, None).unwrap();
        let texts: Vec<&str> = room.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(texts, ["Done: parse"]);
        assert_eq!(room[0].task_id.as_deref(), Some(task.id.as_str()));
    }
}
