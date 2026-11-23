//! A routine's run: the task it makes, started through the same start a person's Start uses, and the outcome recorded on
//! the run. A failure is kept on the run; a task already made stays Open with its own error, and nothing is retried.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{Store, TeamRoutineRow, TeamRoutineRunRow};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::tasks::{TaskCreateParams, TaskOrigin};
use massorca_protocol::{ErrorBody, codes};

use super::store_error;
use super::task::create_task;
use super::task_start::{Changes, start_task};

/// Why a run receives no vault secrets: its routine was not set up by a host-admin caller.
const WITHHELD: &str =
    "vault secrets go only to routine runs whose routine a host-admin caller set up";

/// The grant a routine's run receives: vault secrets only when its creator held host-admin scope (decision 0071).
pub(super) fn routine_grant(starts_as_admin: bool) -> SecretGrant {
    if starts_as_admin {
        SecretGrant::Provide
    } else {
        SecretGrant::Withhold(WITHHELD)
    }
}

/// Makes the run's task and starts it, then records the task and any failure on `run`. It never fails its caller.
pub(super) fn run_routine(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    routine: &TeamRoutineRow,
    run: &TeamRoutineRunRow,
    grant: SecretGrant,
    changes: &mut Changes,
) {
    let (task_id, error) = match start_routine_task(
        store,
        agents,
        workspaces,
        routine,
        run.scheduled_for,
        grant,
        changes,
    ) {
        Ok(task_id) => (Some(task_id), None),
        Err(failure) => (failure.task, Some(failure.error.message)),
    };
    if let Err(error) = store.set_routine_run_result(&run.id, task_id.as_deref(), error.as_deref())
    {
        tracing::warn!("a routine run's outcome could not be saved: {error}");
    }
}

/// Why a run did not start, and the task it had already made when the start failed. Boxed: it is only the failure path.
struct Failure {
    task: Option<String>,
    error: ErrorBody,
}

fn start_routine_task(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    routine: &TeamRoutineRow,
    scheduled_for: i64,
    grant: SecretGrant,
    changes: &mut Changes,
) -> Result<String, Box<Failure>> {
    let unstarted = |error: ErrorBody| Box::new(Failure { task: None, error });
    let bot_id = bot_of(store, routine).map_err(unstarted)?;
    let task = create_task(
        store,
        TaskCreateParams {
            team: routine.team_id.clone(),
            title: routine.name.clone(),
            body: Some(task_body(routine, scheduled_for)),
            bot: Some(bot_id),
            project: None,
            origin: Some(TaskOrigin::Cli),
        },
    )
    .map_err(unstarted)?;
    let task_id = task.id;
    changes.task = Some(task_id.clone());
    start_task(store, agents, workspaces, &task_id, grant, changes).map_err(|error| {
        Box::new(Failure {
            task: Some(task_id.clone()),
            error,
        })
    })?;
    Ok(task_id)
}

/// The routine's bot, by id. A removed bot leaves the routine with none, and its run says so.
fn bot_of(store: &Store, routine: &TeamRoutineRow) -> Result<String, ErrorBody> {
    let removed = || {
        ErrorBody::new(
            codes::BOT_NOT_FOUND,
            "bot removed: the routine has no bot to run",
        )
    };
    let id = routine.bot_id.as_deref().ok_or_else(removed)?;
    store
        .get_bot(id)
        .map_err(store_error)?
        .map(|bot| bot.id)
        .ok_or_else(removed)
}

/// The routine's prompt and a one-line clock note, so "today" in the prompt means the day the run stands for.
fn task_body(routine: &TeamRoutineRow, scheduled_for: i64) -> String {
    let zone = routine.zone.as_deref().unwrap_or("the host's zone");
    let day = local_day(scheduled_for, routine.zone.as_deref())
        .unwrap_or_else(|| "an unknown day".to_owned());
    format!(
        "{}\n\nClock: this run is for {day} in {zone}. Treat \"today\" as {day}.",
        routine.prompt
    )
}

/// The calendar day `at_ms` falls on in `zone`, or in the host's zone when none is set.
fn local_day(at_ms: i64, zone: Option<&str>) -> Option<String> {
    let zone = match zone {
        Some(name) => TimeZone::get(name).ok()?,
        None => TimeZone::try_system().unwrap_or(TimeZone::UTC),
    };
    let at = Timestamp::from_millisecond(at_ms).ok()?;
    Some(zone.to_datetime(at).date().to_string())
}

#[cfg(test)]
mod tests {
    use massorca_core_agents::SecretGrant;
    use massorca_core_store::TeamRoutineRow;

    use super::{routine_grant, task_body};

    fn routine(zone: &str) -> TeamRoutineRow {
        TeamRoutineRow {
            id: "r1".to_owned(),
            team_id: "t1".to_owned(),
            bot_id: Some("b1".to_owned()),
            name: "Check".to_owned(),
            prompt: "Read the notes.".to_owned(),
            cadence: "daily".to_owned(),
            time: "02:30".to_owned(),
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

    #[test]
    fn a_routine_set_up_by_a_host_admin_receives_vault_secrets() {
        assert_eq!(routine_grant(true), SecretGrant::Provide);
    }

    #[test]
    fn a_routine_set_up_by_another_caller_withholds_vault_secrets() {
        assert!(matches!(routine_grant(false), SecretGrant::Withhold(_)));
    }

    #[test]
    fn the_clock_note_names_the_local_day_of_the_run() {
        // 2026-03-08 07:00 UTC is 03:00 EDT on the day the clocks change in America/New_York.
        let body = task_body(&routine("America/New_York"), 1_772_953_200_000);
        assert_eq!(
            body,
            "Read the notes.\n\nClock: this run is for 2026-03-08 in America/New_York. Treat \"today\" as 2026-03-08."
        );
    }
}
