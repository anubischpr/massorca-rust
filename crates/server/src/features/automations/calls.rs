//! The `automation.*` handlers. Each reads or writes the store on the blocking pool. A create or an edit resolves its
//! project the way `project.show` does, through the workspace domain (decision 0123).

use massorca_core_git::Git;
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
    let git = ctx.state.git.clone();
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
        let row = new_row(&domain, &git, &params, now, key)?;
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
    let git = ctx.state.git.clone();
    blocking(&ctx, move |store, agents| {
        let old = automation(store, &params.automation)?;
        let now = now_ms();
        let mut row = old.clone();
        let schedule_moved = apply_edit(&mut row, &params, &domain, &git, now)?;
        let compiled = check(&row, agents, &host)?;
        let next = if !row.enabled {
            NextRun::Clear
        } else if schedule_moved || old.next_run_at.is_none() {
            NextRun::From(&compiled)
        } else {
            NextRun::Keep
        };
        let written = write_edit(store, &row, next, &mut now_ms)?;
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

async fn set_enabled(
    ctx: RequestCtx,
    params: AutomationRef,
    enabled: bool,
) -> Result<Automation, ErrorBody> {
    let host = HostZone::read();
    blocking(&ctx, move |store, agents| {
        let row = automation(store, &params.automation)?;
        let written = switch(store, row, enabled, &mut now_ms, |row| {
            check(row, agents, &host)
        })?;
        automation_view(store, &written, &host)
    })
    .await
}

/// Enabling checks the definition still holds and schedules the next run from now; disabling checks nothing, so an
/// automation whose agent kind or zone is no longer accepted can still be switched off, and clears the next run.
fn switch(
    store: &Store,
    mut row: AutomationRow,
    enabled: bool,
    clock: &mut dyn FnMut() -> i64,
    check: impl FnOnce(&AutomationRow) -> Result<CompiledSchedule, ErrorBody>,
) -> Result<AutomationRow, ErrorBody> {
    if row.enabled == enabled {
        return Ok(row);
    }
    row.enabled = enabled;
    row.updated_at = clock();
    if !enabled {
        return write_edit(store, &row, NextRun::Clear, clock);
    }
    let compiled = check(&row)?;
    write_edit(store, &row, NextRun::From(&compiled), clock)
}

/// What an edit does with the stored next run.
enum NextRun<'a> {
    Keep,
    Clear,
    /// The next occurrence of this schedule after the time of the write.
    From(&'a CompiledSchedule),
}

/// Tries of the next-run compare-and-swap before an edit gives up; each miss means a claim moved it meanwhile.
const NEXT_RUN_ATTEMPTS: usize = 5;

/// Writes an edit's definition, never its next or last run, then moves the next run as `next` says by a
/// compare-and-swap on the stored next run. Each try reads the stored row, then the clock, so its target is after any
/// claim it saw, and a claim made after its read makes the swap miss and try again: a claimed occurrence is never
/// scheduled again. Returns the automation as stored.
fn write_edit(
    store: &Store,
    row: &AutomationRow,
    next: NextRun<'_>,
    clock: &mut dyn FnMut() -> i64,
) -> Result<AutomationRow, ErrorBody> {
    if !store
        .update_automation_definition(row)
        .map_err(store_error)?
    {
        return Err(removed_meanwhile());
    }
    if matches!(next, NextRun::Keep) {
        return stored_row(store, &row.id);
    }
    for _ in 0..NEXT_RUN_ATTEMPTS {
        let stored = stored_row(store, &row.id)?;
        let target = match next {
            NextRun::From(compiled) => next_run(row, compiled, clock()),
            NextRun::Keep | NextRun::Clear => None,
        };
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
    git: &Git,
    params: &AutomationCreateParams,
    now: i64,
    key: Option<String>,
) -> Result<AutomationRow, ErrorBody> {
    let (project_id, _) = project_of(domain, &params.project)?;
    let stored = destination::check(domain, git, &project_id, params.destination.as_ref())?;
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
    git: &Git,
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
        let stored = destination::check(domain, git, &project_id, Some(given))?;
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

#[cfg(test)]
mod tests {
    use massorca_core_store::{AutomationRow, ProjectKind, ProjectRow, Store, new_id};

    use massorca_protocol::{ErrorBody, codes};

    use super::super::definition::compile;
    use super::super::timer::tick;
    use super::{NextRun, switch, write_edit};

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
            destination: massorca_core_store::destination::PROJECT_DEFAULT.to_owned(),
            destination_workspace_id: None,
            destination_path: None,
            base_branch: None,
            created_at: 1,
            updated_at: 1,
        };
        store.insert_automation(&row).unwrap();
        (store, row)
    }

    /// Runs one timer check; returns how many runs it launched.
    fn claims(store: &Store, now: i64) -> usize {
        let mut launched = 0;
        tick(
            store,
            now,
            &mut |_automation| None,
            &mut |_automation, _run| {
                launched += 1;
                None
            },
        )
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

        let written = write_edit(&store, &edited, NextRun::Keep, &mut || now).unwrap();

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

        let written = write_edit(&store, &edited, NextRun::From(&compiled), &mut || now).unwrap();

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

    #[test]
    fn a_schedule_edit_read_just_before_a_claim_targets_after_the_claim_and_does_not_run_it_again()
    {
        let (store, read) = store_with_due_automation();
        // The edit read the row at 08:59:59.990, a weekday; the timer claims 09:00 at 09:00:00.400; the edit writes at
        // 09:00:00.500 and must not schedule 09:00 again.
        let edited = AutomationRow {
            cadence: "weekdays".to_owned(),
            ..read
        };
        let compiled = compile(&edited).unwrap();
        assert_eq!(claims(&store, DUE + 400), 1);

        let written =
            write_edit(&store, &edited, NextRun::From(&compiled), &mut || DUE + 500).unwrap();

        assert_eq!(written.next_run_at, Some(DUE + DAY), "Thursday 09:00");
        assert_eq!(claims(&store, DUE + 600), 0);
    }

    #[test]
    fn disabling_skips_the_definition_check_and_enabling_keeps_it() {
        let (store, row) = store_with_due_automation();
        let refused = |_: &AutomationRow| {
            Err(ErrorBody::new(
                codes::AGENT_KIND_NOT_FOUND,
                "no agent kind claude",
            ))
        };

        let disabled = switch(&store, row, false, &mut || DUE, refused).unwrap();

        assert!(!disabled.enabled);
        assert_eq!(disabled.next_run_at, None);
        let error = switch(&store, disabled, true, &mut || DUE, refused).unwrap_err();
        assert_eq!(error.code, codes::AGENT_KIND_NOT_FOUND);
        assert!(
            !store.list_automations().unwrap()[0].enabled,
            "the refused enable wrote nothing"
        );
    }
}
