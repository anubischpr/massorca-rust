//! The `automation.*` handlers. Each reads or writes the store on the blocking pool. A create or an edit resolves its
//! project the way `project.show` does, through the workspace domain (decision 0123).

use massorca_core_schedule::CompiledSchedule;
use massorca_core_store::{AutomationRow, Store, StoreError, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::automations::{
    Automation, AutomationCreateParams, AutomationList, AutomationRef, AutomationUpdateParams,
};
use massorca_protocol::{ErrorBody, codes};

use super::definition::{
    HostZone, cadence_name, check, check_grace, keep_for, next_run, time_kept, weekday_name,
};
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
        let recompute = !row.enabled || schedule_moved || old.next_run_at.is_none();
        let written = write_edit(store, &row, recompute.then_some(&compiled), now)?;
        automation_view(store, &written, &host)
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
        let compiled = check(&row, agents, &host)?;
        let written = write_edit(store, &row, Some(&compiled), now)?;
        automation_view(store, &written, &host)
    })
    .await
}

/// Tries of the next-run compare-and-swap before an edit gives up; each miss means a claim moved it meanwhile.
const NEXT_RUN_ATTEMPTS: usize = 5;

/// Writes an edit's definition, never its next or last run. With `recompute`, it then moves the next run from `now`
/// by a compare-and-swap on the stored next run, read again on each miss, so a claim made between the edit's read and
/// its write is never undone and its occurrence never runs twice. Returns the automation as stored.
fn write_edit(
    store: &Store,
    row: &AutomationRow,
    recompute: Option<&CompiledSchedule>,
    now: i64,
) -> Result<AutomationRow, ErrorBody> {
    if !store
        .update_automation_definition(row)
        .map_err(store_error)?
    {
        return Err(removed_meanwhile());
    }
    let Some(compiled) = recompute else {
        return stored_row(store, &row.id);
    };
    let target = next_run(row, compiled, now);
    for _ in 0..NEXT_RUN_ATTEMPTS {
        let stored = stored_row(store, &row.id)?;
        if store
            .set_automation_next_run_if(&row.id, stored.next_run_at, target)
            .map_err(store_error)?
        {
            return stored_row(store, &row.id);
        }
    }
    Err(ErrorBody::new(
        codes::INTERNAL,
        "the automation's next run kept changing while it was edited; try again",
    ))
}

fn stored_row(store: &Store, id: &str) -> Result<AutomationRow, ErrorBody> {
    store
        .get_automation(id)
        .map_err(store_error)?
        .ok_or_else(removed_meanwhile)
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

#[cfg(test)]
mod tests {
    use massorca_core_store::{AutomationRow, ProjectKind, ProjectRow, Store, new_id};

    use super::super::definition::compile;
    use super::super::timer::tick;
    use super::write_edit;

    const MINUTE: i64 = 60_000;
    const DAY: i64 = 24 * 60 * MINUTE;
    /// 2026-11-25 09:00 UTC, an occurrence of a daily 09:00 automation in UTC.
    const DUE: i64 = 1_795_597_200_000;

    fn store_with_due_automation() -> (Store, AutomationRow) {
        let store = Store::open_in_memory().expect("in-memory store opens");
        let project = ProjectRow {
            id: new_id(),
            kind: ProjectKind::Folder,
            path: "/work/app".to_owned(),
            path_key: "/work/app".to_owned(),
            name: "app".to_owned(),
            workspaces_root: "/work/app-workspaces".to_owned(),
            added_at: 1,
            base_ref: None,
            icon: None,
        };
        store.insert_project(&project).unwrap();
        let row = AutomationRow {
            id: new_id(),
            project_id: Some(project.id),
            name: "Morning PRs".to_owned(),
            prompt: "Summarise the open pull requests".to_owned(),
            agent_kind: "claude".to_owned(),
            cadence: "daily".to_owned(),
            time: "09:00".to_owned(),
            weekday: None,
            cron: None,
            zone: Some("UTC".to_owned()),
            grace_minutes: None,
            enabled: true,
            creation_key: None,
            next_run_at: Some(DUE),
            last_run_at: None,
            created_at: 1,
            updated_at: 1,
        };
        store.insert_automation(&row).unwrap();
        (store, row)
    }

    /// Runs one timer check; returns how many runs it launched.
    fn claims(store: &Store, now: i64) -> usize {
        let mut launched = 0;
        tick(store, now, &mut |_automation, _run| {
            launched += 1;
            None
        })
        .unwrap();
        launched
    }

    #[test]
    fn an_edit_that_keeps_the_schedule_never_undoes_a_claim_made_after_its_read() {
        let (store, read) = store_with_due_automation();
        let now = DUE + MINUTE;
        // The edit read the row; the timer claims the occurrence before the edit writes.
        assert_eq!(claims(&store, now), 1);
        let edited = AutomationRow {
            prompt: "Summarise the merged pull requests".to_owned(),
            ..read
        };

        let written = write_edit(&store, &edited, None, now).unwrap();

        assert_eq!(written.prompt, "Summarise the merged pull requests");
        assert_eq!(written.next_run_at, Some(DUE + DAY));
        assert_eq!(written.last_run_at, Some(now));
        assert_eq!(claims(&store, now), 0, "the occurrence does not run twice");
    }

    #[test]
    fn a_schedule_change_after_a_claim_moves_the_next_run_from_now_without_a_second_run() {
        let (store, read) = store_with_due_automation();
        let now = DUE + MINUTE;
        assert_eq!(claims(&store, now), 1);
        let edited = AutomationRow {
            time: "09:30".to_owned(),
            ..read
        };
        let compiled = compile(&edited).unwrap();

        let written = write_edit(&store, &edited, Some(&compiled), now).unwrap();

        assert_eq!(written.next_run_at, Some(DUE + 30 * MINUTE));
        assert_eq!(claims(&store, now), 0);
        assert_eq!(
            store
                .list_automation_runs(&written.id, 10, None)
                .unwrap()
                .len(),
            1
        );
    }
}
