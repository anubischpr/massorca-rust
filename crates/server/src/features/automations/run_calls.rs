//! `automation.run_now` and `automation.runs` (decision 0125, AUTO-04). A manual run starts at once and does not move
//! the schedule; one that cannot start is still written to the history, so the person sees why.

use massorca_core_store::{now_ms, run_state};
use massorca_protocol::methods::automations::{
    AutomationRef, AutomationRun, AutomationRunList, AutomationRunsParams, DEFAULT_RUNS_LIMIT,
    MAX_RUNS_LIMIT,
};
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_WORKSPACE_CREATED};
use massorca_protocol::{ErrorBody, codes};

use super::launch::dispatch;
use super::runs::{TRIGGER_MANUAL, new_run, prune, publish, run_view};
use super::selectors::automation;
use crate::context::RequestCtx;
use crate::features::teams::{blocking, store_error};
use crate::features::workspace_domain::for_mutation;

pub(super) async fn run_now(
    ctx: RequestCtx,
    params: AutomationRef,
) -> Result<AutomationRun, ErrorBody> {
    let workspaces = for_mutation(&ctx).await?;
    let (run, made) = blocking(&ctx, move |store, agents| {
        let row = automation(store, &params.automation)?;
        let now = now_ms();
        let mut run = new_run(&row, TRIGGER_MANUAL, now, now);
        let starts = run.state == run_state::DISPATCHING;
        store
            .insert_automation_run(&mut run, starts.then_some(now))
            .map_err(store_error)?;
        let made = if starts {
            dispatch(store, agents, &workspaces, &row, &run)
        } else {
            None
        };
        prune(store, &row.id);
        let stored = store
            .get_automation_run(&run.id)
            .map_err(store_error)?
            .ok_or_else(|| {
                ErrorBody::new(
                    codes::AUTOMATION_NOT_FOUND,
                    "the automation was removed while it ran",
                )
            })?;
        Ok((run_view(&stored), made))
    })
    .await?;
    if let Some(workspace) = made {
        ctx.state.events.publish(ChangeEvent {
            topic: TOPIC_WORKSPACE_CREATED.to_owned(),
            id: workspace,
        });
    }
    publish(&ctx.state, &run.automation_id);
    Ok(run)
}

pub(super) async fn runs(
    ctx: RequestCtx,
    params: AutomationRunsParams,
) -> Result<AutomationRunList, ErrorBody> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_RUNS_LIMIT)
        .clamp(1, MAX_RUNS_LIMIT);
    let limit = usize::try_from(limit).unwrap_or(1);
    blocking(&ctx, move |store, _agents| {
        let row = automation(store, &params.automation)?;
        let mut rows = store
            .list_automation_runs(&row.id, limit + 1, params.before)
            .map_err(store_error)?;
        let more = rows.len() > limit;
        rows.truncate(limit);
        Ok(AutomationRunList {
            next_before: more.then(|| rows.last().map(|run| run.number)).flatten(),
            runs: rows.iter().map(run_view).collect(),
        })
    })
    .await
}
