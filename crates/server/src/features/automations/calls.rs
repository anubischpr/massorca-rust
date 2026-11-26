//! The `automation.*` handlers. A create reads or writes the store on the blocking pool, and resolves its project the way
//! `project.show` does, through the workspace domain (decision 0123).

use massorca_core_store::{AutomationRow, StoreError, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::automations::{Automation, AutomationCreateParams};

use super::definition::{
    HostZone, cadence_name, check, check_grace, keep_for, next_run, time_kept, weekday_name,
};
use super::view::automation_view;
use crate::context::RequestCtx;
use crate::features::teams::{blocking, non_blank, optional_text, store_error};
use crate::features::workspace_domain::shared;

pub(super) async fn create(
    ctx: RequestCtx,
    params: AutomationCreateParams,
) -> Result<Automation, ErrorBody> {
    let host = HostZone::read();
    let domain = shared(&ctx);
    blocking(&ctx, move |store, agents| {
        let key = optional_text(params.creation_key.clone());
        if let Some(key) = key.as_deref()
            && let Some(existing) = store
                .find_automation_by_creation_key(key)
                .map_err(store_error)?
        {
            return automation_view(store, &existing, &host);
        }
        let now = now_ms();
        let row = new_row(&domain, &params, now, key)?;
        let compiled = check(&row, agents, &host)?;
        let row = AutomationRow {
            next_run_at: next_run(&row, &compiled, now),
            ..row
        };
        match store.insert_automation(&row) {
            Ok(()) => automation_view(store, &row, &host),
            // A concurrent create with the same key was first: its automation is the answer.
            Err(StoreError::Conflict { constraint }) => match row.creation_key.as_deref() {
                Some(key) => match store
                    .find_automation_by_creation_key(key)
                    .map_err(store_error)?
                {
                    Some(existing) => automation_view(store, &existing, &host),
                    None => Err(store_error(StoreError::Conflict { constraint })),
                },
                None => Err(store_error(StoreError::Conflict { constraint })),
            },
            Err(error) => Err(store_error(error)),
        }
    })
    .await
}

/// A new automation from the create params. Its project is resolved here; the caller sets its next run.
fn new_row(
    domain: &Workspaces,
    params: &AutomationCreateParams,
    now: i64,
    key: Option<String>,
) -> Result<AutomationRow, ErrorBody> {
    let (project_id, _) = project_of(domain, &params.project)?;
    let cadence = cadence_name(params.cadence)?;
    let weekday = params
        .weekday
        .map(weekday_name)
        .transpose()?
        .map(str::to_owned);
    let cron = params.cron.as_deref().map(|text| text.trim().to_owned());
    let (weekday, cron) = keep_for(cadence, weekday, cron);
    Ok(AutomationRow {
        id: new_id(),
        project_id: Some(project_id),
        name: non_blank(&params.name, "the automation name")?,
        prompt: non_blank(&params.prompt, "the automation prompt")?,
        agent_kind: non_blank(&params.agent, "the agent kind")?,
        cadence: cadence.to_owned(),
        time: time_kept(cadence, &params.time),
        weekday,
        cron,
        zone: optional_text(params.zone.clone()),
        grace_minutes: check_grace(params.grace_minutes)?,
        enabled: true,
        creation_key: key,
        next_run_at: None,
        last_run_at: None,
        created_at: now,
        updated_at: now,
    })
}

/// The project a selector names, as `project.show` resolves it: its id and its name.
fn project_of(domain: &Workspaces, selector: &str) -> Result<(String, String), ErrorBody> {
    let project = domain.show_project(selector)?;
    Ok((project.id, project.name))
}
