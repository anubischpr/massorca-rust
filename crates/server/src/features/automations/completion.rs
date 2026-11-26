//! Ends automation runs from their agent sessions (decision 0125, AUTO-07). A run completes when its agent reports Done
//! from a run the host can see, with the agent's result text; an agent that exits without Done fails the run; a session
//! the agents domain forgot strands it, also when its event was missed. A run whose session is unverifiable stays dispatched: loss of contact is never
//! evidence that the agent ended.

use std::sync::Arc;

use massorca_core_agents::Change;
use massorca_core_store::{Store, TaskOutcome, now_ms, run_state};
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::{ErrorBody, codes};
use tokio::sync::broadcast::{Receiver, error::RecvError};

use crate::context::AppState;
use crate::features::teams::{blocking_state, observed_outcome, store_error};
use crate::shutdown::ShutdownSignal;

use super::runs::{capped_result, prune, publish};

const EXITED: &str = "The agent exited without reporting completion.";
const STRANDED: &str = "The run's agent session is gone, so how it ended is not known; it is not reported as completed.";

/// Follows the agents domain until shutdown. It reads every session at start and after a lag, because a Done that
/// arrived while nothing listened is gone.
pub(super) async fn follow_runs(
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
            Ok(Change::Updated(session)) => {
                let ended = blocking_state(&state, move |store, _agents| {
                    settle_session(store, &session)
                })
                .await;
                announce(&state, ended.map(|ended| ended.into_iter().collect()));
            }
            Ok(Change::Forgotten(id)) => {
                let ended =
                    blocking_state(&state, move |store, _agents| settle_forgotten(store, &id))
                        .await;
                announce(&state, ended.map(|ended| ended.into_iter().collect()));
            }
            Ok(_) => {}
            Err(RecvError::Lagged(_)) => resync(&state).await,
            Err(RecvError::Closed) => return,
        }
    }
}

async fn resync(state: &Arc<AppState>) {
    let ended = blocking_state(state, |store, agents| {
        let mut ended = Vec::new();
        for session in agents.list(None).map_err(ErrorBody::from)? {
            ended.extend(settle_session(store, &session)?);
        }
        // A run whose session is gone missed its Forgotten event; one whose session exists, even unverifiable, stays.
        ended.extend(
            store
                .fail_automation_runs_without_session(
                    codes::AUTOMATION_RUN_STRANDED,
                    STRANDED,
                    now_ms(),
                )
                .map_err(store_error)?,
        );
        Ok(ended)
    })
    .await;
    announce(state, ended);
}

fn announce(state: &AppState, ended: Result<Vec<String>, ErrorBody>) {
    match ended {
        Ok(ended) => ended
            .iter()
            .for_each(|automation_id| publish(state, automation_id)),
        Err(error) => tracing::warn!(
            code = %error.code,
            "the automation run listener could not read or end a run: {}",
            error.message
        ),
    }
}

/// Ends the dispatched run of `session` when its turn ended. Returns the automation whose run ended.
pub(super) fn settle_session(
    store: &Store,
    session: &AgentSession,
) -> Result<Option<String>, ErrorBody> {
    let ended = match observed_outcome(session).map(TaskOutcome::as_str) {
        Some(outcome) => {
            let result = session.result.as_deref().map(capped_result);
            store.end_automation_run_by_session(
                &session.id,
                run_state::COMPLETED,
                Some(outcome),
                result.as_deref(),
                None,
                now_ms(),
            )
        }
        None if session.execution.state == TerminalState::Exited => store
            .end_automation_run_by_session(
                &session.id,
                run_state::DISPATCH_FAILED,
                None,
                None,
                Some((codes::AUTOMATION_RUN_EXITED, EXITED)),
                now_ms(),
            ),
        None => return Ok(None),
    }
    .map_err(store_error)?;
    Ok(ended.map(|run| {
        prune(store, &run.automation_id);
        run.automation_id
    }))
}

pub(super) fn settle_forgotten(
    store: &Store,
    session_id: &str,
) -> Result<Option<String>, ErrorBody> {
    let ended = store
        .end_automation_run_by_session(
            session_id,
            run_state::DISPATCH_FAILED,
            None,
            None,
            Some((codes::AUTOMATION_RUN_STRANDED, STRANDED)),
            now_ms(),
        )
        .map_err(store_error)?;
    Ok(ended.map(|run| {
        prune(store, &run.automation_id);
        run.automation_id
    }))
}
