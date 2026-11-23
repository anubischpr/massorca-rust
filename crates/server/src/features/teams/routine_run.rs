//! A routine's run: the task it makes, stored with the run in one transaction, then started through the same start a
//! person's Start uses, and the outcome recorded on the run. A failure is kept on the run with its code; a task already
//! made stays Open with its own error, and nothing is retried. A run whose outcome was never recorded is settled at the
//! next service start from what its task shows.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{Store, TaskRow, TeamRoutineRow, TeamRoutineRunRow, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::tasks::{TaskCreateParams, TaskOrigin};
use massorca_protocol::{ErrorBody, codes};

use super::store_error;
use super::task::new_task_row;
use super::task_start::{Changes, start_task};

/// Why a run receives no vault secrets: its routine was not set up by a host-admin caller.
const WITHHELD: &str =
    "vault secrets go only to routine runs whose routine a host-admin caller set up";

/// The message of a run the service stopped before its start recorded an outcome.
pub(super) const INTERRUPTED: &str = "the service stopped before this run started";

/// The grant a routine's run receives: vault secrets only when its creator held host-admin scope (decision 0071).
pub(super) fn routine_grant(starts_as_admin: bool) -> SecretGrant {
    if starts_as_admin {
        SecretGrant::Provide
    } else {
        SecretGrant::Withhold(WITHHELD)
    }
}

/// A run about to be stored, and the task it makes. A run that cannot make its task, such as one whose bot was
/// removed, is already settled with the reason and has no task.
pub(super) struct Prepared {
    pub(super) run: TeamRoutineRunRow,
    pub(super) task: Option<TaskRow>,
}

/// Builds the run for the occurrence `scheduled_for` and the task it makes, neither stored yet.
pub(super) fn prepare_run(
    store: &Store,
    routine: &TeamRoutineRow,
    scheduled_for: i64,
    now: i64,
) -> Prepared {
    match routine_task(store, routine, scheduled_for) {
        Ok(task) => {
            let mut run = pending_run(routine, scheduled_for, now);
            run.task_id = Some(task.id.clone());
            Prepared {
                run,
                task: Some(task),
            }
        }
        Err(error) => failed_run(routine, scheduled_for, now, &error),
    }
}

/// A run that ends before it makes a task, with the code and message of `error`.
pub(super) fn failed_run(
    routine: &TeamRoutineRow,
    scheduled_for: i64,
    now: i64,
    error: &ErrorBody,
) -> Prepared {
    let mut run = pending_run(routine, scheduled_for, now);
    run.error = Some(error.message.clone());
    run.error_code = Some(error.code.clone());
    run.settled_at = Some(now);
    Prepared { run, task: None }
}

fn pending_run(routine: &TeamRoutineRow, scheduled_for: i64, now: i64) -> TeamRoutineRunRow {
    TeamRoutineRunRow {
        id: new_id(),
        routine_id: routine.id.clone(),
        scheduled_for,
        task_id: None,
        error: None,
        error_code: None,
        settled_at: None,
        created_at: now,
    }
}

/// Starts the task a stored run made, then records the outcome on the run. A settled run, or one without a task, starts
/// nothing. It never fails its caller.
pub(super) fn start_run(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    run: &TeamRoutineRunRow,
    grant: SecretGrant,
    changes: &mut Changes,
) {
    let Some(task_id) = run.task_id.as_deref() else {
        return;
    };
    if run.settled_at.is_some() {
        return;
    }
    changes.task = Some(task_id.to_owned());
    let failure = start_task(store, agents, workspaces, task_id, grant, changes).err();
    record_outcome(store, &run.id, failure.as_ref());
}

/// Records a run's outcome, trying once more when the write fails. A run whose outcome still could not be written stays
/// pending, and the next service start settles it from its task: started when its session is linked, the task's start
/// error when it kept one, interrupted otherwise. Its state never lives only in the log.
fn record_outcome(store: &Store, run_id: &str, failure: Option<&ErrorBody>) {
    let error = failure.map(|error| (error.code.as_str(), error.message.as_str()));
    for attempt in 1..=2 {
        match store.set_routine_run_result(run_id, error, now_ms()) {
            Ok(_) => return,
            Err(write) => tracing::warn!(
                attempt,
                "a routine run's outcome could not be saved; the next start settles it from its task: {write}"
            ),
        }
    }
}

/// The task a run makes for the routine's bot, checked as `task.create` checks it.
fn routine_task(
    store: &Store,
    routine: &TeamRoutineRow,
    scheduled_for: i64,
) -> Result<TaskRow, ErrorBody> {
    let bot_id = bot_of(store, routine)?;
    new_task_row(
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
