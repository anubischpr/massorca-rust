//! The `automation.*` handlers. Each reads or writes the store on the blocking pool. A create or an edit resolves its
//! project the way `project.show` does, through the workspace domain (decision 0123).

use massorca_core_store::{AutomationRow, StoreError, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::automations::{
    Automation, AutomationCreateParams, AutomationList, AutomationRef, AutomationUpdateParams,
};
use massorca_protocol::{ErrorBody, codes};

use super::definition::{
    HostZone, cadence_name, check, check_grace, keep_for, next_run, time_kept, weekday_name,
};
use super::destination::{self, Stored};
use super::selectors::automation;
use super::view::automation_view;
use crate::context::RequestCtx;
use crate::features::NoParams;
use crate::features::teams::{blocking, non_blank, optional_text, store_error};
use crate::features::workspace_domain::shared;

pub(super) async fn list(ctx: RequestCtx, _params: NoParams) -> Result<AutomationList, ErrorBody> {
    let host = HostZone::read();
    blocking(&ctx, move |store, _agents| {
        let rows = store.list_automations().map_err(store_error)?;
        let automations = rows
            .iter()
            .map(|row| automation_view(store, row, &host))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AutomationList { automations })
    })
    .await
}

pub(super) async fn show(ctx: RequestCtx, params: AutomationRef) -> Result<Automation, ErrorBody> {
    let host = HostZone::read();
    blocking(&ctx, move |store, _agents| {
        let row = automation(store, &params.automation)?;
        automation_view(store, &row, &host)
    })
    .await
}

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

pub(super) async fn update(
    ctx: RequestCtx,
    params: AutomationUpdateParams,
) -> Result<Automation, ErrorBody> {
    let host = HostZone::read();
    let domain = shared(&ctx);
    blocking(&ctx, move |store, agents| {
        let old = automation(store, &params.automation)?;
        let now = now_ms();
        let mut row = old.clone();
        let schedule_moved = apply_edit(&mut row, &params, &domain, now)?;
        let compiled = check(&row, agents, &host)?;
        row.next_run_at = match (row.enabled, schedule_moved || old.next_run_at.is_none()) {
            (false, _) => None,
            (true, true) => next_run(&row, &compiled, now),
            (true, false) => old.next_run_at,
        };
        if !store.update_automation(&row).map_err(store_error)? {
            return Err(removed_meanwhile());
        }
        automation_view(store, &row, &host)
    })
    .await
}

pub(super) async fn remove(
    ctx: RequestCtx,
    params: AutomationRef,
) -> Result<Automation, ErrorBody> {
    let host = HostZone::read();
    blocking(&ctx, move |store, _agents| {
        let row = automation(store, &params.automation)?;
        let removed = automation_view(store, &row, &host)?;
        if store.delete_automation(&row.id).map_err(store_error)? {
            Ok(removed)
        } else {
            Err(removed_meanwhile())
        }
    })
    .await
}

pub(super) async fn enable(
    ctx: RequestCtx,
    params: AutomationRef,
) -> Result<Automation, ErrorBody> {
    set_enabled(ctx, params, true).await
}

pub(super) async fn disable(
    ctx: RequestCtx,
    params: AutomationRef,
) -> Result<Automation, ErrorBody> {
    set_enabled(ctx, params, false).await
}

/// Enabling schedules the next run from now and checks the definition still holds; disabling clears the next run.
async fn set_enabled(
    ctx: RequestCtx,
    params: AutomationRef,
    enabled: bool,
) -> Result<Automation, ErrorBody> {
    let host = HostZone::read();
    blocking(&ctx, move |store, agents| {
        let mut row = automation(store, &params.automation)?;
        if row.enabled == enabled {
            return automation_view(store, &row, &host);
        }
        let now = now_ms();
        row.enabled = enabled;
        row.updated_at = now;
        row.next_run_at = if enabled {
            let compiled = check(&row, agents, &host)?;
            next_run(&row, &compiled, now)
        } else {
            None
        };
        if !store.update_automation(&row).map_err(store_error)? {
            return Err(removed_meanwhile());
        }
        automation_view(store, &row, &host)
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
    let stored = destination::check(domain, &project_id, params.destination.as_ref())?;
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
        destination: stored.kind.to_owned(),
        destination_workspace_id: stored.workspace_id,
        destination_path: stored.path,
        base_branch: stored.base_branch,
        created_at: now,
        updated_at: now,
    })
}

/// Applies the fields an edit gives. Returns whether the schedule moved (its cadence, time, weekday, cron expression or
/// zone), which recomputes the next run.
fn apply_edit(
    row: &mut AutomationRow,
    params: &AutomationUpdateParams,
    domain: &Workspaces,
    now: i64,
) -> Result<bool, ErrorBody> {
    let before = schedule_fields(row);
    if let Some(selector) = params.project.as_deref() {
        row.project_id = Some(project_of(domain, selector)?.0);
        // A destination names a workspace or repository of the old project, so it is set again with the new one.
        if params.destination.is_none() && row.destination != Stored::project_default().kind {
            return Err(ErrorBody::new(
                codes::AUTOMATION_DESTINATION_INVALID,
                "changing the project needs its destination given again",
            ));
        }
    }
    if let Some(name) = &params.name {
        row.name = non_blank(name, "the automation name")?;
    }
    if let Some(prompt) = &params.prompt {
        row.prompt = non_blank(prompt, "the automation prompt")?;
    }
    if let Some(agent) = &params.agent {
        row.agent_kind = non_blank(agent, "the agent kind")?;
    }
    let cadence = match params.cadence {
        Some(cadence) => cadence_name(cadence)?.to_owned(),
        None => row.cadence.clone(),
    };
    let weekday = match params.weekday {
        Some(day) => Some(weekday_name(day)?.to_owned()),
        None => row.weekday.clone(),
    };
    let cron = match params.cron.as_deref() {
        Some(text) => Some(text.trim().to_owned()),
        None => row.cron.clone(),
    };
    let time = match params.time.as_deref() {
        Some(text) => text.to_owned(),
        None => row.time.clone(),
    };
    let (weekday, cron) = keep_for(&cadence, weekday, cron);
    row.time = time_kept(&cadence, &time);
    row.cadence = cadence;
    row.weekday = weekday;
    row.cron = cron;
    if let Some(zone) = &params.zone {
        row.zone = optional_text(Some(zone.clone()));
    }
    if params.grace_minutes.is_some() {
        row.grace_minutes = check_grace(params.grace_minutes)?;
    }
    if let Some(given) = &params.destination {
        let project_id = row.project_id.clone().ok_or_else(|| {
            ErrorBody::new(
                codes::AUTOMATION_NO_PROJECT,
                "the automation has no project, so it cannot take a destination",
            )
        })?;
        let stored = destination::check(domain, &project_id, Some(given))?;
        row.destination = stored.kind.to_owned();
        row.destination_workspace_id = stored.workspace_id;
        row.destination_path = stored.path;
        row.base_branch = stored.base_branch;
    }
    row.updated_at = now;
    Ok(before != schedule_fields(row))
}

fn schedule_fields(
    row: &AutomationRow,
) -> (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    (
        row.cadence.clone(),
        row.time.clone(),
        row.weekday.clone(),
        row.cron.clone(),
        row.zone.clone(),
    )
}

/// The project a selector names, as `project.show` resolves it: its id and its name.
fn project_of(domain: &Workspaces, selector: &str) -> Result<(String, String), ErrorBody> {
    let project = domain.show_project(selector)?;
    Ok((project.id, project.name))
}

fn removed_meanwhile() -> ErrorBody {
    ErrorBody::new(
        codes::AUTOMATION_NOT_FOUND,
        "the automation was removed by another call",
    )
}
