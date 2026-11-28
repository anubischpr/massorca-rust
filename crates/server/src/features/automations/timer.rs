//! The automation timer (decision 0125, AUTO-03): a check every 60 seconds, and one at start, claims each due automation
//! and starts its run before it claims the next. A claim is a compare-and-swap on the stored next run that also stores
//! the run, so an occurrence runs once even when two checks overlap. Only the latest due occurrence may run, and only
//! within its grace; earlier ones, and a latest one past its grace, are recorded as missed and never replayed.

use std::sync::Arc;
use std::time::Duration;

use massorca_core_agents::{Agents, Change};
use massorca_core_schedule::{ClaimDecision, decide_claim, runs_within_grace};
use massorca_core_store::{AutomationRow, AutomationRunRow, Store, now_ms, run_state};
use massorca_protocol::methods::automations::DEFAULT_GRACE_MINUTES;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_WORKSPACE_CREATED};
use massorca_protocol::{ErrorBody, codes};
use tokio::sync::broadcast::Receiver;
use tokio::time::MissedTickBehavior;

use crate::context::AppState;
use crate::features::teams::{blocking_state, store_error};
use crate::shutdown::ShutdownSignal;

use super::completion::{follow_runs, settle_forgotten, settle_session};
use super::definition::compile;
use super::destination::{Refusal, refusal};
use super::launch::{RUN_REQUEST_PREFIX, dispatch};
use super::runs::{TRIGGER_SCHEDULE, missed_run, placed_run, prune, publish};

/// How often the host checks for due automations (AUTO-03).
const CHECK_EVERY: Duration = Duration::from_secs(60);
const CHECK_EVERY_MS: i64 = 60_000;

const INTERRUPTED: &str = "The service stopped before this run's agent started.";

/// Runs the automation timer and the run listener until shutdown.
pub(crate) async fn follow_automations(
    state: Arc<AppState>,
    changes: Receiver<Change>,
    signal: ShutdownSignal,
) {
    let runs = tokio::spawn(follow_runs(Arc::clone(&state), changes, signal.clone()));
    follow_schedule(state, signal).await;
    if let Err(error) = runs.await {
        tracing::warn!("the automation run listener stopped: {error}");
    }
}

/// Attaches or fails the runs a previous service left dispatching, then checks for due automations every 60 seconds, and once at
/// start, until shutdown. A sweep that fails is tried again before each check until one succeeds.
async fn follow_schedule(state: Arc<AppState>, mut signal: ShutdownSignal) {
    let mut swept = false;
    let mut checks = tokio::time::interval(CHECK_EVERY);
    // After the host slept, the late check runs once and the next one stays on the normal beat.
    checks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            () = signal.wait() => return,
            _ = checks.tick() => {}
        }
        if !swept {
            swept = sweep(&state).await;
        }
        check(&state).await;
    }
}

async fn sweep(state: &Arc<AppState>) -> bool {
    let started_at = state.started_at_ms;
    let settled = blocking_state(state, move |store, agents| {
        let ended = reattach(store, agents, started_at)?;
        let settled = store
            .settle_dispatching_automation_runs(
                started_at,
                codes::AUTOMATION_RUN_INTERRUPTED,
                INTERRUPTED,
                now_ms(),
            )
            .map_err(store_error)?;
        Ok((ended, settled))
    })
    .await;
    match settled {
        Ok((ended, settled)) => {
            for automation_id in ended {
                publish(state, &automation_id);
            }
            if settled > 0 {
                tracing::info!(
                    settled,
                    "automation runs left dispatching by the last stop are failed"
                );
            }
            true
        }
        Err(error) => {
            tracing::warn!(code = %error.code, "the automation runs left dispatching could not be settled: {}", error.message);
            false
        }
    }
}

/// Attaches the runs whose agent started before the stop saved it, and ends those whose session already ended. Returns
/// the automations whose runs ended.
fn reattach(store: &Store, agents: &Agents, started_at: i64) -> Result<Vec<String>, ErrorBody> {
    let mut ended = Vec::new();
    for session in store
        .reattach_dispatching_automation_runs(started_at, RUN_REQUEST_PREFIX)
        .map_err(store_error)?
    {
        let settled = match agents.show(&session) {
            Ok(view) => settle_session(store, &view)?,
            Err(error) if error.code == codes::AGENT_NOT_FOUND => {
                settle_forgotten(store, &session)?
            }
            Err(error) => return Err(ErrorBody::from(error)),
        };
        ended.extend(settled);
    }
    Ok(ended)
}

/// One check on the blocking pool, then the events for what it changed.
async fn check(state: &Arc<AppState>) {
    let now = now_ms();
    let workspaces = Arc::clone(&state.workspaces);
    let outcome = blocking_state(state, move |store, agents| {
        tick(
            store,
            now,
            &mut |automation| refusal(&workspaces, automation),
            &mut |automation, run| dispatch(store, agents, &workspaces, automation, run),
        )
    })
    .await;
    match outcome {
        Ok(changed) => {
            for change in changed {
                if let Some(workspace) = change.workspace {
                    state.events.publish(ChangeEvent {
                        topic: TOPIC_WORKSPACE_CREATED.to_owned(),
                        id: workspace,
                    });
                }
                publish(state, &change.automation_id);
            }
        }
        Err(error) => {
            tracing::warn!(code = %error.code, "the automation check failed: {}", error.message)
        }
    }
}

/// Starts a stored dispatching run; returns the workspace it made. The timer passes `dispatch`.
pub(super) type Launch<'a> = dyn FnMut(&AutomationRow, &AutomationRunRow) -> Option<String> + 'a;
/// The refusal an automation's destination meets now, if any; the timer passes `refusal`.
pub(super) type Refuse<'a> = dyn FnMut(&AutomationRow) -> Option<Refusal> + 'a;

/// What one claimed automation changed.
pub(super) struct Changed {
    pub(super) automation_id: String,
    pub(super) workspace: Option<String>,
}

/// Claims each due automation and starts its run with `launch` before claiming the next. An automation whose claim
/// fails is skipped with a warning and tried again at the next check; the others still run.
pub(super) fn tick(
    store: &Store,
    now: i64,
    refuse: &mut Refuse<'_>,
    launch: &mut Launch<'_>,
) -> Result<Vec<Changed>, ErrorBody> {
    let mut changed = Vec::new();
    for automation in store.due_automations(now).map_err(store_error)? {
        let refused = refuse(&automation);
        let runs = match claim(store, &automation, now, refused) {
            Ok(Some(runs)) => runs,
            Ok(None) => continue,
            Err(error) => {
                tracing::warn!(code = %error.code, "an automation could not be claimed: {}", error.message);
                continue;
            }
        };
        let mut workspace = None;
        for run in runs
            .iter()
            .filter(|run| run.state == run_state::DISPATCHING)
        {
            workspace = launch(&automation, run).or(workspace);
        }
        prune(store, &automation.id);
        changed.push(Changed {
            automation_id: automation.id,
            workspace,
        });
    }
    Ok(changed)
}

/// Claims `automation` when its next run is due and stores the runs the claim records. A claim another caller won is
/// `None`. A schedule that no longer compiles is skipped and tried again at the next check, so a repaired one resumes.
fn claim(
    store: &Store,
    automation: &AutomationRow,
    now: i64,
    refused: Option<Refusal>,
) -> Result<Option<Vec<AutomationRunRow>>, ErrorBody> {
    let Some(stored) = automation.next_run_at else {
        return Ok(None);
    };
    let schedule = compile(automation)?;
    let ClaimDecision::Due {
        scheduled_for,
        next_run_at,
        skipped,
    } = decide_claim(&schedule, Some(stored), now)
    else {
        return Ok(None);
    };
    let mut runs = Vec::new();
    if skipped > 0 {
        runs.push(missed_run(
            automation,
            stored,
            skipped,
            format!(
                "{skipped} earlier occurrence(s) were missed while the host was not checking; they are not replayed."
            ),
            now,
        ));
    }
    let grace = automation.grace_minutes.unwrap_or(DEFAULT_GRACE_MINUTES);
    let latest = if runs_within_grace(scheduled_for, now, grace, CHECK_EVERY_MS) {
        placed_run(automation, TRIGGER_SCHEDULE, scheduled_for, refused, now)
    } else {
        missed_run(
            automation,
            scheduled_for,
            1,
            format!(
                "The occurrence was later than its grace of {grace} minutes; it is not replayed."
            ),
            now,
        )
    };
    let started = (latest.state == run_state::DISPATCHING).then_some(now);
    runs.push(latest);
    let won = store
        .claim_automation_runs(&automation.id, stored, next_run_at, started, &mut runs)
        .map_err(store_error)?;
    Ok(won.then_some(runs))
}

#[cfg(test)]
#[path = "timer_tests.rs"]
mod tests;
