//! `team.routine_list`, `team.routine_add`, `team.routine_update`, `team.routine_rm`, `team.routine_pause`,
//! `team.routine_resume`, `team.routine_runs` and `team.routine_run` (capability `teams/1`, decision 0072 amendment).
//! The occurrences are decided by `massorca-core-schedule` (decision 0082); `routine_timer.rs` runs the due ones.

use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_schedule::{Cadence, Schedule, ScheduleError, Weekday, next_run_from};
use massorca_core_store::{Store, TeamRoutineRow, TeamRoutineRunRow, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::TOPIC_TEAM_CHANGED;
use massorca_protocol::methods::teams::{
    self, Routine, RoutineAddParams, RoutineCadence, RoutineList, RoutineRef, RoutineRun,
    RoutineRunList, RoutineRunsParams, RoutineUpdateParams, RoutineWeekday, TeamRef,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::agents::secret_grant;
use crate::features::workspace_domain::{for_mutation, publish};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::routine_run::{prepare_run, routine_grant, start_run};
use super::task_start::{Changes, publish_changes};
use super::{blocking, invalid, non_blank, optional_text, selectors, store_error};

/// The runs `team.routine_runs` returns when no limit is given, and the most it returns.
const DEFAULT_RUN_LIMIT: usize = 50;
const MAX_RUN_LIMIT: usize = 500;

/// Each weekday as the store names it, the protocol's value and core-schedule's value.
const WEEKDAYS: [(&str, RoutineWeekday, Weekday); 7] = [
    ("monday", RoutineWeekday::Monday, Weekday::Monday),
    ("tuesday", RoutineWeekday::Tuesday, Weekday::Tuesday),
    ("wednesday", RoutineWeekday::Wednesday, Weekday::Wednesday),
    ("thursday", RoutineWeekday::Thursday, Weekday::Thursday),
    ("friday", RoutineWeekday::Friday, Weekday::Friday),
    ("saturday", RoutineWeekday::Saturday, Weekday::Saturday),
    ("sunday", RoutineWeekday::Sunday, Weekday::Sunday),
];

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            teams::ROUTINE_LIST,
            capabilities::TEAMS,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_ADD,
            capabilities::TEAMS,
            Permission::Workspace,
            add,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_UPDATE,
            capabilities::TEAMS,
            Permission::Workspace,
            update,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_REMOVE,
            capabilities::TEAMS,
            Permission::Workspace,
            remove,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_PAUSE,
            capabilities::TEAMS,
            Permission::Workspace,
            pause,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_RESUME,
            capabilities::TEAMS,
            Permission::Workspace,
            resume,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_RUNS,
            capabilities::TEAMS,
            Permission::Read,
            runs,
        )?,
        methods.register(
            Owner::Core,
            teams::ROUTINE_RUN,
            capabilities::TEAMS,
            Permission::Workspace,
            run,
        )?,
    ])
}

async fn list(ctx: RequestCtx, params: TeamRef) -> Result<RoutineList, ErrorBody> {
    blocking(&ctx, move |store, _agents| {
        let team = selectors::team(store, &params.team)?;
        let rows = store.list_routines(&team.id).map_err(store_error)?;
        Ok(RoutineList {
            routines: rows.iter().map(routine_view).collect(),
        })
    })
    .await
}

async fn add(ctx: RequestCtx, params: RoutineAddParams) -> Result<Routine, ErrorBody> {
    let admin = is_admin(&ctx);
    let row = blocking(&ctx, move |store, _agents| {
        add_routine(store, params, admin, now_ms())
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(routine_view(&row))
}

async fn update(ctx: RequestCtx, params: RoutineUpdateParams) -> Result<Routine, ErrorBody> {
    let admin = is_admin(&ctx);
    let row = blocking(&ctx, move |store, _agents| {
        update_routine(store, params, admin, now_ms())
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(routine_view(&row))
}

async fn remove(ctx: RequestCtx, params: RoutineRef) -> Result<Routine, ErrorBody> {
    let row = blocking(&ctx, move |store, _agents| remove_routine(store, &params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(routine_view(&row))
}

async fn pause(ctx: RequestCtx, params: RoutineRef) -> Result<Routine, ErrorBody> {
    let row = blocking(&ctx, move |store, _agents| {
        set_paused(store, &params, true, now_ms())
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(routine_view(&row))
}

async fn resume(ctx: RequestCtx, params: RoutineRef) -> Result<Routine, ErrorBody> {
    let row = blocking(&ctx, move |store, _agents| {
        set_paused(store, &params, false, now_ms())
    })
    .await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(routine_view(&row))
}

async fn runs(ctx: RequestCtx, params: RoutineRunsParams) -> Result<RoutineRunList, ErrorBody> {
    blocking(&ctx, move |store, _agents| {
        let team = selectors::team(store, &params.team)?;
        let routine = selectors::routine(store, &team, &params.routine)?;
        let limit = params.limit.unwrap_or(DEFAULT_RUN_LIMIT).min(MAX_RUN_LIMIT);
        let rows = store
            .list_routine_runs(&routine.id, limit)
            .map_err(store_error)?;
        Ok(RoutineRunList {
            runs: rows.iter().map(run_view).collect(),
        })
    })
    .await
}

/// `team.routine_run`: one run now. It does not move the schedule.
async fn run(ctx: RequestCtx, params: RoutineRef) -> Result<RoutineRun, ErrorBody> {
    let workspaces = for_mutation(&ctx).await?;
    // A manual run receives vault secrets only when the caller and the routine's creator both held host-admin scope.
    let caller_admin = is_admin(&ctx);
    let (changes, run) = blocking(&ctx, move |store, agents| {
        run_now(store, agents, &workspaces, &params, caller_admin, now_ms())
    })
    .await?;
    publish_changes(&ctx.state, &changes);
    Ok(run)
}

/// Whether the caller holds host-admin scope, the scope vault secrets need (decision 0071).
fn is_admin(ctx: &RequestCtx) -> bool {
    matches!(secret_grant(ctx), SecretGrant::Provide)
}

fn add_routine(
    store: &Store,
    params: RoutineAddParams,
    admin: bool,
    now: i64,
) -> Result<TeamRoutineRow, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let bot = selectors::bot(store, &team, &params.bot)?;
    let cadence = cadence_name(params.cadence)?;
    let weekday = params
        .weekday
        .map(weekday_name)
        .transpose()?
        .map(str::to_owned);
    let (weekday, month_day, run_date) = keep_for(
        cadence,
        weekday,
        params.month_day.map(i64::from),
        optional_text(params.run_date),
    );
    let mut row = TeamRoutineRow {
        id: new_id(),
        team_id: team.id,
        bot_id: Some(bot.id),
        name: non_blank(&params.name, "the routine name")?,
        prompt: non_blank(&params.prompt, "the routine prompt")?,
        cadence: cadence.to_owned(),
        time: params.time.trim().to_owned(),
        weekday,
        month_day,
        run_date,
        zone: optional_text(params.zone),
        paused: false,
        starts_as_admin: admin,
        next_run_at: None,
        created_at: now,
    };
    row.next_run_at = next_run_after(&row, now)?;
    store.insert_routine(&row).map_err(store_error)?;
    Ok(row)
}

fn update_routine(
    store: &Store,
    params: RoutineUpdateParams,
    admin: bool,
    now: i64,
) -> Result<TeamRoutineRow, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let mut row = selectors::routine(store, &team, &params.routine)?;
    if let Some(selector) = params.bot {
        row.bot_id = Some(selectors::bot(store, &team, &selector)?.id);
    }
    if let Some(name) = params.name {
        row.name = non_blank(&name, "the routine name")?;
    }
    if let Some(prompt) = params.prompt {
        row.prompt = non_blank(&prompt, "the routine prompt")?;
    }
    if let Some(time) = params.time {
        row.time = time.trim().to_owned();
    }
    if let Some(zone) = params.zone {
        row.zone = optional_text(Some(zone));
    }
    let cadence = match params.cadence {
        Some(cadence) => cadence_name(cadence)?.to_owned(),
        None => row.cadence.clone(),
    };
    let weekday = match params.weekday {
        Some(day) => Some(weekday_name(day)?.to_owned()),
        None => row.weekday.clone(),
    };
    let month_day = params.month_day.map(i64::from).or(row.month_day);
    let run_date = params
        .run_date
        .map(|date| date.trim().to_owned())
        .or_else(|| row.run_date.clone());
    let (weekday, month_day, run_date) = keep_for(&cadence, weekday, month_day, run_date);
    row.cadence = cadence;
    row.weekday = weekday;
    row.month_day = month_day;
    row.run_date = run_date;
    // An edit makes the routine's scheduled runs act with the scope of the caller that made it.
    row.starts_as_admin = admin;
    row.next_run_at = if row.paused {
        None
    } else {
        next_run_after(&row, now)?
    };
    if !store.update_routine(&row).map_err(store_error)? {
        return Err(vanished());
    }
    Ok(row)
}

fn remove_routine(store: &Store, params: &RoutineRef) -> Result<TeamRoutineRow, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = selectors::routine(store, &team, &params.routine)?;
    if !store.delete_routine(&row.id).map_err(store_error)? {
        return Err(vanished());
    }
    Ok(row)
}

/// Pauses or resumes a routine. Resuming schedules the next occurrence from now, so nothing missed is made up.
fn set_paused(
    store: &Store,
    params: &RoutineRef,
    paused: bool,
    now: i64,
) -> Result<TeamRoutineRow, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let mut row = selectors::routine(store, &team, &params.routine)?;
    row.paused = paused;
    row.next_run_at = if paused {
        None
    } else {
        next_run_after(&row, now)?
    };
    if !store.update_routine(&row).map_err(store_error)? {
        return Err(vanished());
    }
    Ok(row)
}

fn run_now(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    params: &RoutineRef,
    caller_admin: bool,
    now: i64,
) -> Result<(Changes, RoutineRun), ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let routine = selectors::routine(store, &team, &params.routine)?;
    let prepared = prepare_run(store, &routine, now, now);
    store
        .insert_routine_run(&prepared.run, prepared.task.as_ref())
        .map_err(store_error)?;
    let mut changes = Changes::default();
    let grant = routine_grant(routine.starts_as_admin && caller_admin);
    start_run(
        store,
        agents,
        workspaces,
        &prepared.run,
        grant,
        &mut changes,
    );
    let saved = store
        .get_routine_run(&prepared.run.id)
        .map_err(store_error)?
        .ok_or_else(vanished)?;
    Ok((changes, run_view(&saved)))
}

/// The next run of a routine from `now`, ignoring its pause. A one-time routine whose date has passed is refused,
/// since it could never run.
pub(super) fn next_run_after(row: &TeamRoutineRow, now: i64) -> Result<Option<i64>, ErrorBody> {
    let compiled = schedule_of(row)?.compile().map_err(schedule_error)?;
    let next = next_run_from(&compiled, now);
    if next.is_none() && row.cadence == "once" {
        return Err(invalid(format!(
            "the one-time run on {} has already passed",
            row.run_date.as_deref().unwrap_or_default()
        )));
    }
    Ok(next)
}

/// The schedule a stored routine means. core-schedule checks it when it is compiled.
pub(super) fn schedule_of(row: &TeamRoutineRow) -> Result<Schedule, ErrorBody> {
    Ok(Schedule {
        cadence: core_cadence(&row.cadence)?,
        weekday: row.weekday.as_deref().map(core_weekday).transpose()?,
        day_of_month: row
            .month_day
            .map(|day| {
                u8::try_from(day)
                    .map_err(|_| invalid(format!("day of the month {day} is outside 1 to 28")))
            })
            .transpose()?,
        date: row.run_date.clone(),
        time: row.time.clone(),
        zone: row.zone.clone(),
    })
}

pub(super) fn schedule_error(error: ScheduleError) -> ErrorBody {
    invalid(error.to_string())
}

/// Keeps only the schedule fields the cadence uses, so a cadence change leaves no stale day behind.
fn keep_for(
    cadence: &str,
    weekday: Option<String>,
    month_day: Option<i64>,
    run_date: Option<String>,
) -> (Option<String>, Option<i64>, Option<String>) {
    match cadence {
        "weekly" => (weekday, None, None),
        "monthly" => (None, month_day, None),
        "once" => (None, None, run_date),
        _ => (None, None, None),
    }
}

fn vanished() -> ErrorBody {
    ErrorBody::new(
        codes::TEAM_ROUTINE_NOT_FOUND,
        "the routine was removed by another call",
    )
}

/// A routine as the protocol shows it.
pub(super) fn routine_view(row: &TeamRoutineRow) -> Routine {
    Routine {
        id: row.id.clone(),
        team_id: row.team_id.clone(),
        bot_id: row.bot_id.clone(),
        name: row.name.clone(),
        prompt: row.prompt.clone(),
        cadence: cadence_of(&row.cadence),
        time: row.time.clone(),
        weekday: row.weekday.as_deref().map(weekday_of),
        month_day: row.month_day.and_then(|day| u8::try_from(day).ok()),
        run_date: row.run_date.clone(),
        zone: row.zone.clone(),
        paused: row.paused,
        starts_as_admin: row.starts_as_admin,
        next_run_at_ms: row.next_run_at,
        created_at_ms: row.created_at,
    }
}

pub(super) fn run_view(row: &TeamRoutineRunRow) -> RoutineRun {
    RoutineRun {
        id: row.id.clone(),
        routine_id: row.routine_id.clone(),
        scheduled_for_ms: row.scheduled_for,
        task_id: row.task_id.clone(),
        error: row.error.clone(),
        error_code: row.error_code.clone(),
        created_at_ms: row.created_at,
    }
}

fn cadence_name(cadence: RoutineCadence) -> Result<&'static str, ErrorBody> {
    match cadence {
        RoutineCadence::Daily => Ok("daily"),
        RoutineCadence::Weekly => Ok("weekly"),
        RoutineCadence::Monthly => Ok("monthly"),
        RoutineCadence::Once => Ok("once"),
        RoutineCadence::Unknown => Err(invalid(
            "the cadence must be daily, weekly, monthly or once",
        )),
    }
}

fn cadence_of(name: &str) -> RoutineCadence {
    match name {
        "daily" => RoutineCadence::Daily,
        "weekly" => RoutineCadence::Weekly,
        "monthly" => RoutineCadence::Monthly,
        "once" => RoutineCadence::Once,
        _ => RoutineCadence::Unknown,
    }
}

fn core_cadence(name: &str) -> Result<Cadence, ErrorBody> {
    match name {
        "daily" => Ok(Cadence::Daily),
        "weekly" => Ok(Cadence::Weekly),
        "monthly" => Ok(Cadence::Monthly),
        "once" => Ok(Cadence::Once),
        other => Err(invalid(format!(
            "the cadence {other:?} is not daily, weekly, monthly or once"
        ))),
    }
}

fn weekday_name(day: RoutineWeekday) -> Result<&'static str, ErrorBody> {
    WEEKDAYS
        .iter()
        .find(|(_, wire, _)| *wire == day)
        .map(|(name, _, _)| *name)
        .ok_or_else(|| invalid("the weekday must be monday to sunday"))
}

fn weekday_of(name: &str) -> RoutineWeekday {
    WEEKDAYS
        .iter()
        .find(|(known, _, _)| *known == name)
        .map_or(RoutineWeekday::Unknown, |(_, wire, _)| *wire)
}

fn core_weekday(name: &str) -> Result<Weekday, ErrorBody> {
    WEEKDAYS
        .iter()
        .find(|(known, _, _)| *known == name)
        .map(|(_, _, day)| *day)
        .ok_or_else(|| invalid(format!("the weekday {name:?} is not a day of the week")))
}

#[cfg(test)]
mod tests {
    use massorca_core_store::TeamRoutineRow;

    use super::{keep_for, next_run_after};

    /// A daily routine at `time` in `zone`.
    fn daily(time: &str, zone: &str) -> TeamRoutineRow {
        TeamRoutineRow {
            id: "r1".to_owned(),
            team_id: "t1".to_owned(),
            bot_id: Some("b1".to_owned()),
            name: "Check".to_owned(),
            prompt: "Read the notes.".to_owned(),
            cadence: "daily".to_owned(),
            time: time.to_owned(),
            weekday: None,
            month_day: None,
            run_date: None,
            zone: Some(zone.to_owned()),
            paused: false,
            starts_as_admin: true,
            next_run_at: None,
            created_at: 1,
        }
    }

    /// 2026-03-08 02:30 does not exist in America/New_York: the clock jumps from 02:00 EST to 03:00 EDT at 07:00 UTC.
    #[test]
    fn a_wall_time_skipped_by_daylight_saving_runs_at_the_end_of_the_gap() {
        let row = daily("02:30", "America/New_York");
        // 2026-03-07 12:00 UTC is after that morning's 02:30 EST has passed.
        let next = next_run_after(&row, 1_772_884_800_000).unwrap();
        assert_eq!(next, Some(1_772_953_200_000));
    }

    /// 2026-11-01 01:30 happens twice in America/New_York; it fires once, at 05:30 UTC (the earlier, EDT, instant).
    #[test]
    fn a_wall_time_repeated_by_daylight_saving_fires_once_at_the_earlier_instant() {
        let row = daily("01:30", "America/New_York");
        let first = next_run_after(&row, 1_793_448_000_000).unwrap();
        assert_eq!(first, Some(1_793_511_000_000));
        // The repeat at 06:30 UTC is not an occurrence: the next run is 2026-11-02 01:30 EST.
        let second = next_run_after(&row, first.unwrap()).unwrap();
        assert_eq!(second, Some(1_793_601_000_000));
    }

    #[test]
    fn a_cadence_change_clears_the_days_it_does_not_use() {
        let monthly = keep_for(
            "monthly",
            Some("monday".to_owned()),
            Some(15),
            Some("2026-11-24".to_owned()),
        );
        assert_eq!(monthly, (None, Some(15), None));
        let daily = keep_for("daily", Some("monday".to_owned()), Some(15), None);
        assert_eq!(daily, (None, None, None));
    }

    #[test]
    fn a_one_time_routine_whose_date_has_passed_is_refused() {
        let mut row = daily("09:00", "Europe/Madrid");
        row.cadence = "once".to_owned();
        row.run_date = Some("2026-11-01".to_owned());
        assert!(next_run_after(&row, 1_793_600_000_000).is_err());
    }
}
