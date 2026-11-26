//! Automation runs as the store keeps them and the protocol shows them (decision 0125): the rows a claim or a manual
//! run records, retention, and the `automation.changed` event.

use massorca_core_store::{AutomationRow, AutomationRunRow, Store, new_id, run_state};
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{
    AutomationRun, AutomationRunOutcome, AutomationRunState, AutomationRunTrigger,
    AutomationRunUsage, AutomationUsageStatus, TOPIC_AUTOMATION_CHANGED,
};
use massorca_protocol::methods::events::ChangeEvent;

use crate::context::AppState;

/// Final runs kept per automation; runs in progress are never evicted (AUTO-03, as pv96 keeps them).
pub(super) const KEPT_FINAL_RUNS: usize = 100;

const NO_PROJECT: &str = "The automation has no project, so the run did not start.";
const NO_USAGE: &str = "the agent session reports no token usage";

pub(super) const TRIGGER_SCHEDULE: &str = "schedule";
pub(super) const TRIGGER_MANUAL: &str = "manual";

/// A run about to be stored for `automation`: dispatching when it has a project, otherwise skipped as unavailable.
pub(super) fn new_run(
    automation: &AutomationRow,
    trigger: &str,
    scheduled_for: i64,
    now: i64,
) -> AutomationRunRow {
    let run = AutomationRunRow {
        id: new_id(),
        automation_id: automation.id.clone(),
        trigger: trigger.to_owned(),
        scheduled_for,
        occurrences: 1,
        state: run_state::DISPATCHING.to_owned(),
        created_at: now,
        ..AutomationRunRow::default()
    };
    if automation.project_id.is_some() {
        return run;
    }
    skipped(
        run,
        run_state::SKIPPED_UNAVAILABLE,
        codes::AUTOMATION_NO_PROJECT,
        NO_PROJECT,
        now,
    )
}

fn skipped(
    run: AutomationRunRow,
    state: &str,
    code: &str,
    reason: &str,
    now: i64,
) -> AutomationRunRow {
    AutomationRunRow {
        state: state.to_owned(),
        reason_code: Some(code.to_owned()),
        reason: Some(reason.to_owned()),
        ended_at: Some(now),
        ..run
    }
}

/// Drops the final runs beyond retention. A failure is only logged: the next run prunes again.
pub(super) fn prune(store: &Store, automation_id: &str) {
    if let Err(error) = store.prune_automation_runs(automation_id, KEPT_FINAL_RUNS) {
        tracing::warn!("an automation's old runs could not be pruned: {error}");
    }
}

/// Tells clients that the runs, or the last and next runs, of `automation_id` changed.
pub(super) fn publish(state: &AppState, automation_id: &str) {
    state.events.publish(ChangeEvent {
        topic: TOPIC_AUTOMATION_CHANGED.to_owned(),
        id: automation_id.to_owned(),
    });
}

pub(super) fn run_view(row: &AutomationRunRow) -> AutomationRun {
    let known = row.input_tokens.is_some() || row.output_tokens.is_some();
    AutomationRun {
        id: row.id.clone(),
        automation_id: row.automation_id.clone(),
        number: row.number,
        trigger: match row.trigger.as_str() {
            TRIGGER_SCHEDULE => AutomationRunTrigger::Schedule,
            TRIGGER_MANUAL => AutomationRunTrigger::Manual,
            _ => AutomationRunTrigger::Unknown,
        },
        state: state_of(&row.state),
        scheduled_for_ms: row.scheduled_for,
        occurrences: u32::try_from(row.occurrences).unwrap_or(u32::MAX),
        outcome: row.outcome.as_deref().map(outcome_of),
        session_id: row.session_id.clone(),
        workspace_id: row.workspace_id.clone(),
        reason: row.reason.clone(),
        reason_code: row.reason_code.clone(),
        result: row.result.clone(),
        usage: AutomationRunUsage {
            status: if known {
                AutomationUsageStatus::Known
            } else {
                AutomationUsageStatus::Unavailable
            },
            reason: (!known).then(|| NO_USAGE.to_owned()),
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
        },
        started_at_ms: row.started_at,
        ended_at_ms: row.ended_at,
        created_at_ms: row.created_at,
    }
}

fn state_of(state: &str) -> AutomationRunState {
    match state {
        "pending" => AutomationRunState::Pending,
        run_state::DISPATCHING => AutomationRunState::Dispatching,
        run_state::DISPATCHED => AutomationRunState::Dispatched,
        run_state::COMPLETED => AutomationRunState::Completed,
        "skipped_precheck" => AutomationRunState::SkippedPrecheck,
        run_state::SKIPPED_MISSED => AutomationRunState::SkippedMissed,
        run_state::SKIPPED_UNAVAILABLE => AutomationRunState::SkippedUnavailable,
        "skipped_needs_interactive_auth" => AutomationRunState::SkippedNeedsInteractiveAuth,
        run_state::DISPATCH_FAILED => AutomationRunState::DispatchFailed,
        _ => AutomationRunState::Unknown,
    }
}

fn outcome_of(outcome: &str) -> AutomationRunOutcome {
    match outcome {
        "success" => AutomationRunOutcome::Success,
        "failure" => AutomationRunOutcome::Failure,
        "cancelled" => AutomationRunOutcome::Cancelled,
        "interrupted" => AutomationRunOutcome::Interrupted,
        _ => AutomationRunOutcome::Unknown,
    }
}
