//! Latches a task's outcome from its agent session, and tells the Board when a linked session changes (decision 0072).
//! Only a Done the host can see latches; loss of contact never does.

use std::sync::Arc;

use massorca_core_agents::Change;
use massorca_core_store::{Store, StoreError, TaskOutcome, now_ms};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TASK_CHANGED};
use tokio::sync::broadcast::{Receiver, error::RecvError};

use crate::context::AppState;
use crate::shutdown::ShutdownSignal;

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
    let (session_id, outcome) = match change {
        Change::Updated(session) => (session.id.clone(), observed_outcome(&session)),
        Change::Forgotten(id) => (id, None),
        Change::Account(_) | Change::Queue(_) => return,
    };
    let linked = blocking_state(state, move |store, _agents| {
        latch_linked(store, &session_id, outcome).map_err(store_error)
    })
    .await;
    match linked {
        Ok(Some(task_id)) => publish_task(state, &task_id),
        Ok(None) => {}
        Err(error) => report(&error),
    }
}

/// Latches every session the agents domain lists now, and tells the Board about each linked task.
async fn resync(state: &Arc<AppState>) {
    let linked = blocking_state(state, |store, agents| {
        let sessions = agents.list(None).map_err(ErrorBody::from)?;
        let mut task_ids = Vec::new();
        for session in &sessions {
            task_ids.extend(
                latch_linked(store, &session.id, observed_outcome(session)).map_err(store_error)?,
            );
        }
        Ok(task_ids)
    })
    .await;
    match linked {
        Ok(task_ids) => task_ids
            .iter()
            .for_each(|task_id| publish_task(state, task_id)),
        Err(error) => report(&error),
    }
}

/// Latches `outcome` into the task linked to `session_id`, if there is one, and returns that task's id. A latched
/// outcome is kept, so a later change of the same session changes nothing.
fn latch_linked(
    store: &Store,
    session_id: &str,
    outcome: Option<TaskOutcome>,
) -> Result<Option<String>, StoreError> {
    let Some(task) = store.find_task_by_session(session_id)? else {
        return Ok(None);
    };
    if let Some(outcome) = outcome {
        store.latch_task_outcome(&task.id, session_id, outcome, now_ms())?;
    }
    Ok(Some(task.id))
}

/// Latches the outcome a loaded session already shows into `task_id`, for a task that was just linked to it. The
/// listener finds a task by its session, so a Done that landed before the link would never latch otherwise.
pub(crate) fn latch_at_link(
    store: &Store,
    task_id: &str,
    session: &Session,
) -> Result<bool, StoreError> {
    let Session::Known(session) = session else {
        return Ok(false);
    };
    let Some(outcome) = observed_outcome(session) else {
        return Ok(false);
    };
    store.latch_task_outcome(task_id, &session.id, outcome, now_ms())
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
            origin: TaskOrigin::Cli,
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
            title: None,
            read_only: false,
            parent_id: None,
            brought_back_at_ms: None,
            agent_ref: None,
            terminal_id: None,
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
            }),
            result: None,
            holds_secrets: false,
            secrets_withheld: None,
            permission_mode: None,
            mode_confirmed: false,
            mode_source: None,
            pending_permission_mode: None,
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
        assert!(latch_at_link(&store, &task.id, &finished).unwrap());
        assert_eq!(latched(&store, &task), Some(TaskOutcome::Success));
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
        assert!(!latch_at_link(&store, &task.id, &unverifiable).unwrap());
        assert!(!latch_at_link(&store, &task.id, &working).unwrap());
        assert!(!latch_at_link(&store, &task.id, &Session::Forgotten).unwrap());
        assert_eq!(latched(&store, &task), None);
    }

    #[test]
    fn a_change_latches_only_the_task_its_session_is_linked_to_and_keeps_the_first_outcome() {
        let (store, task) = store_with_task("session-1");
        assert_eq!(
            latch_linked(&store, "session-9", Some(TaskOutcome::Failure)).unwrap(),
            None
        );
        assert_eq!(
            latch_linked(&store, "session-1", Some(TaskOutcome::Failure)).unwrap(),
            Some(task.id.clone())
        );
        assert_eq!(latched(&store, &task), Some(TaskOutcome::Failure));

        // The task is still named by its session, and the outcome it latched first stands.
        assert_eq!(
            latch_linked(&store, "session-1", Some(TaskOutcome::Success)).unwrap(),
            Some(task.id.clone())
        );
        assert_eq!(latched(&store, &task), Some(TaskOutcome::Failure));
    }
}
