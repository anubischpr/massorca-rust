//! The routine timer (decision 0072 amendment, decision 0082): a check every 30 seconds, and one at start, claims each
//! due routine and runs that occurrence before it claims the next routine. A claim is a compare-and-swap on the stored
//! next run that also stores the run and its task, so an occurrence runs once even when two checks overlap, a check
//! that finds several missed occurrences runs the latest, and a crash after a claim leaves a run the next start marks
//! interrupted rather than a run that silently never started.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use massorca_core_agents::SecretGrant;
use massorca_core_schedule::{ClaimDecision, decide_claim};
use massorca_core_store::{Store, TeamRoutineRow, TeamRoutineRunRow, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TEAM_CHANGED};
use massorca_protocol::{ErrorBody, codes};
use serde_json::Value;
use tokio::time::MissedTickBehavior;

use crate::context::AppState;
use crate::features::config::WORKSPACES_DEFAULT_ROOT;
use crate::owner::Owner;
use crate::shutdown::ShutdownSignal;

use super::routine_run::{
    INTERRUPTED, Prepared, failed_run, prepare_run, routine_grant, start_run,
};
use super::routines::{schedule_error, schedule_of};
use super::task_start::{Changes, publish_changes};
use super::{blocking_state, store_error};

/// How often the host checks for due routines.
const CHECK_EVERY: Duration = Duration::from_secs(30);

/// Marks the runs a previous service left pending as interrupted, then checks for due routines every 30 seconds, and
/// once at start, until shutdown.
pub(crate) async fn follow_routines(state: Arc<AppState>, mut signal: ShutdownSignal) {
    sweep(&state).await;
    let mut checks = tokio::time::interval(CHECK_EVERY);
    // After the host slept, the late check runs once and the next one stays on the normal beat.
    checks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            // Shutdown first, so no check starts once the signal is up.
            biased;
            () = signal.wait() => return,
            _ = checks.tick() => {}
        }
        check(&state).await;
    }
}

/// Marks every run created before this service started that never recorded an outcome as interrupted.
async fn sweep(state: &Arc<AppState>) {
    let started_at = state.started_at_ms;
    match blocking_state(state, move |store, _agents| {
        interrupt_pending(store, started_at, now_ms())
    })
    .await
    {
        Ok(0) => {}
        Ok(marked) => tracing::info!(
            marked,
            "routine runs left pending by the last stop are marked interrupted"
        ),
        Err(error) => tracing::warn!(
            code = %error.code,
            "the routine runs left pending could not be marked interrupted: {}",
            error.message
        ),
    }
}

/// Marks the runs created before `started_at` whose start never recorded an outcome as interrupted. Nothing retries
/// them.
pub(super) fn interrupt_pending(
    store: &Store,
    started_at: i64,
    now: i64,
) -> Result<usize, ErrorBody> {
    store
        .interrupt_pending_routine_runs(
            started_at,
            codes::TEAM_ROUTINE_RUN_INTERRUPTED,
            INTERRUPTED,
            now,
        )
        .map_err(store_error)
}

/// One check on the blocking pool, then the events for what it changed.
async fn check(state: &Arc<AppState>) {
    let now = now_ms();
    let shared = Arc::clone(&state.workspaces);
    let fallback = state
        .registries
        .settings
        .find(&Owner::Core, WORKSPACES_DEFAULT_ROOT)
        .map(|decl| decl.default);
    let outcome = blocking_state(state, move |store, agents| {
        let workspaces = default_root_workspaces(store, &shared, fallback)?;
        tick(store, now, &mut |run, grant, changes| {
            start_run(store, agents, &workspaces, run, grant, changes);
        })
    })
    .await;
    match outcome {
        Ok(outcomes) => {
            for (team_id, changes) in outcomes {
                state.events.publish(ChangeEvent {
                    topic: TOPIC_TEAM_CHANGED.to_owned(),
                    id: team_id,
                });
                publish_changes(state, &changes);
            }
        }
        Err(error) => tracing::warn!(
            code = %error.code,
            "the routine check could not claim or run a routine: {}",
            error.message
        ),
    }
}

/// The Workspaces handle with the default root the setting holds now, as a call's `for_mutation` gives it.
fn default_root_workspaces(
    store: &Store,
    shared: &Workspaces,
    fallback: Option<Value>,
) -> Result<Workspaces, ErrorBody> {
    let value = match store
        .get_setting("core", WORKSPACES_DEFAULT_ROOT)
        .map_err(store_error)?
    {
        Some(json) => serde_json::from_str::<Value>(&json).map_err(|error| {
            ErrorBody::new(
                codes::INTERNAL,
                format!("a stored setting is not valid JSON: {error}"),
            )
        })?,
        None => fallback.ok_or_else(|| {
            ErrorBody::new(codes::INTERNAL, "workspaces.default_root is not declared")
        })?,
    };
    let root = value.as_str().map(PathBuf::from).ok_or_else(|| {
        ErrorBody::new(
            codes::CONFIG_INVALID_VALUE,
            "workspaces.default_root must be a path",
        )
    })?;
    Ok(shared.with_default_root(root))
}

/// Starts the task of a stored run and records its outcome; the timer passes `start_run`.
type StartRun<'a> = dyn FnMut(&TeamRoutineRunRow, SecretGrant, &mut Changes) + 'a;

/// Claims each due routine and runs its occurrence with `start` before claiming the next. Returns, per claimed routine,
/// its team and what its run changed. A routine whose claim fails is skipped with a warning; the others still run.
fn tick(
    store: &Store,
    now: i64,
    start: &mut StartRun<'_>,
) -> Result<Vec<(String, Changes)>, ErrorBody> {
    let mut outcomes = Vec::new();
    for routine in store.due_routines(now).map_err(store_error)? {
        let claimed = match claim(store, routine, now) {
            Ok(Some(claimed)) => claimed,
            Ok(None) => continue,
            Err(error) => {
                tracing::warn!(code = %error.code, "a routine could not be claimed: {}", error.message);
                continue;
            }
        };
        let mut changes = Changes {
            task: claimed.run.task_id.clone(),
            ..Changes::default()
        };
        start(
            &claimed.run,
            routine_grant(claimed.routine.starts_as_admin),
            &mut changes,
        );
        outcomes.push((claimed.routine.team_id, changes));
    }
    Ok(outcomes)
}

/// An occurrence this check claimed: its routine as it was, and the run stored for it.
struct Claimed {
    routine: TeamRoutineRow,
    run: TeamRoutineRunRow,
}

/// Claims `routine` when its next run is due: stores its run and the task the run makes, and moves its next run, in one
/// transaction. Only the latest missed occurrence is claimed, and a claim another caller won is `None`.
fn claim(store: &Store, routine: TeamRoutineRow, now: i64) -> Result<Option<Claimed>, ErrorBody> {
    let Some(stored) = routine.next_run_at else {
        return Ok(None);
    };
    let (prepared, next): (Prepared, Option<i64>) = match schedule_of(&routine)
        .and_then(|schedule| schedule.compile().map_err(schedule_error))
    {
        Ok(compiled) => match decide_claim(&compiled, Some(stored), now) {
            ClaimDecision::Due {
                scheduled_for,
                next_run_at,
                ..
            } => (
                prepare_run(store, &routine, scheduled_for, now),
                next_run_at,
            ),
            ClaimDecision::NotDue => return Ok(None),
        },
        // The schedule no longer compiles, such as a zone the database lost: the routine ends with the reason.
        Err(error) => {
            let reason = ErrorBody::new(
                error.code,
                format!("the schedule is no longer valid: {}", error.message),
            );
            (failed_run(&routine, stored, now, &reason), None)
        }
    };
    let won = store
        .claim_routine_run(stored, next, &prepared.run, prepared.task.as_ref())
        .map_err(store_error)?;
    Ok(won.then_some(Claimed {
        routine,
        run: prepared.run,
    }))
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use massorca_core_store::{BotRow, Store, TeamRoutineRow, TeamRoutineRunRow, TeamRow, new_id};
    use massorca_protocol::codes;

    use super::{interrupt_pending, tick};
    use crate::features::teams::routine_run::INTERRUPTED;

    /// A team with one bot and one daily 09:00 routine in `zone`, next due at `next_run_at`.
    fn store_with_routine(zone: &str, next_run_at: Option<i64>) -> (Store, String) {
        let store = Store::open_in_memory().expect("in-memory store opens");
        let team = TeamRow {
            id: new_id(),
            name: "Desarrollo".to_owned(),
            purpose: String::new(),
            character: "triangle".to_owned(),
            access: "full".to_owned(),
            workspaces: false,
            created_at: 1,
        };
        store.insert_team(&team).unwrap();
        let bot = BotRow {
            id: new_id(),
            team_id: team.id.clone(),
            name: "Mara".to_owned(),
            job: "Writes the parser".to_owned(),
            kind: "claude".to_owned(),
            model: None,
            effort: None,
            lead: false,
            created_at: 1,
        };
        store.insert_bot(&bot).unwrap();
        let routine = TeamRoutineRow {
            id: new_id(),
            team_id: team.id,
            bot_id: Some(bot.id),
            name: "Morning check".to_owned(),
            prompt: "Read the open issues.".to_owned(),
            cadence: "daily".to_owned(),
            time: "09:00".to_owned(),
            weekday: None,
            month_day: None,
            run_date: None,
            zone: Some(zone.to_owned()),
            paused: false,
            starts_as_admin: true,
            next_run_at,
            created_at: 1,
        };
        store.insert_routine(&routine).unwrap();
        (store, routine.id)
    }

    /// Runs one check whose start step only notes the runs it was given; returns those runs.
    fn check(store: &Store, now: i64) -> Vec<TeamRoutineRunRow> {
        let mut started = Vec::new();
        tick(store, now, &mut |run, _grant, _changes| {
            started.push(run.clone())
        })
        .unwrap();
        started
    }

    fn runs(store: &Store, routine_id: &str) -> Vec<TeamRoutineRunRow> {
        store.list_routine_runs(routine_id, 10).unwrap()
    }

    /// 2026-11-20 08:00 UTC is 09:00 CET in Europe/Madrid, the first of the three occurrences that were missed.
    const FIRST_MISSED: i64 = 1_795_161_600_000;
    /// 2026-11-22 08:00 UTC, the latest missed occurrence; the check runs at 12:00 UTC that day.
    const LATEST_MISSED: i64 = 1_795_334_400_000;
    const CHECK_AT: i64 = 1_795_348_800_000;
    /// 2026-11-23 08:00 UTC, the occurrence after the check.
    const NEXT: i64 = 1_795_420_800_000;

    #[test]
    fn a_missed_routine_is_claimed_once_for_its_latest_occurrence_with_its_task() {
        let (store, routine_id) = store_with_routine("Europe/Madrid", Some(FIRST_MISSED));

        let started = check(&store, CHECK_AT);

        assert_eq!(started.len(), 1);
        assert_eq!(started[0].scheduled_for, LATEST_MISSED);
        assert_eq!(started[0].error, None);
        let task_id = started[0]
            .task_id
            .as_deref()
            .expect("the claim made a task");
        assert!(store.get_task(task_id).unwrap().is_some());
        assert_eq!(
            store.get_routine(&routine_id).unwrap().unwrap().next_run_at,
            Some(NEXT)
        );
        assert_eq!(runs(&store, &routine_id).len(), 1);
    }

    #[test]
    fn a_check_after_a_claim_finds_nothing_due() {
        let (store, _) = store_with_routine("Europe/Madrid", Some(FIRST_MISSED));
        check(&store, CHECK_AT);

        assert!(check(&store, CHECK_AT).is_empty());
    }

    #[test]
    fn a_paused_routine_is_never_claimed() {
        let (store, _) = store_with_routine("Europe/Madrid", None);

        assert!(check(&store, CHECK_AT).is_empty());
    }

    #[test]
    fn a_schedule_that_no_longer_compiles_is_finished_with_its_reason_and_not_retried() {
        let (store, routine_id) = store_with_routine("Not/AZone", Some(FIRST_MISSED));

        let started = check(&store, CHECK_AT);

        assert_eq!(started.len(), 1);
        let run = &runs(&store, &routine_id)[0];
        assert!(run.error.as_deref().unwrap().contains("no longer valid"));
        assert_eq!(
            run.error_code.as_deref(),
            Some(codes::REQUEST_INVALID_PARAMS)
        );
        assert_eq!(run.task_id, None);
        assert!(run.settled_at.is_some());
        assert_eq!(
            store.get_routine(&routine_id).unwrap().unwrap().next_run_at,
            None
        );
        assert!(check(&store, CHECK_AT).is_empty());
    }

    #[test]
    fn a_fault_between_claim_and_start_leaves_an_interrupted_run_and_the_next_occurrence_still_fires()
     {
        let (store, routine_id) = store_with_routine("Europe/Madrid", Some(FIRST_MISSED));

        // The service dies after the claim committed and before the start recorded anything.
        let crashed = catch_unwind(AssertUnwindSafe(|| {
            tick(&store, CHECK_AT, &mut |_run, _grant, _changes| {
                panic!("the service stops between the claim and the start")
            })
        }));
        assert!(crashed.is_err());
        let pending = runs(&store, &routine_id);
        assert_eq!(pending.len(), 1);
        let task_id = pending[0]
            .task_id
            .clone()
            .expect("the claim stored the task");
        assert!(store.get_task(&task_id).unwrap().is_some());

        // The next service start marks it interrupted and does not retry it.
        assert_eq!(
            interrupt_pending(&store, CHECK_AT + 1, CHECK_AT + 2).unwrap(),
            1
        );
        assert!(check(&store, CHECK_AT + 3).is_empty());
        let interrupted = &runs(&store, &routine_id)[0];
        assert_eq!(
            interrupted.error_code.as_deref(),
            Some(codes::TEAM_ROUTINE_RUN_INTERRUPTED)
        );
        assert_eq!(interrupted.error.as_deref(), Some(INTERRUPTED));
        assert_eq!(interrupted.task_id.as_deref(), Some(task_id.as_str()));

        let started = check(&store, NEXT);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].scheduled_for, NEXT);
        assert_eq!(runs(&store, &routine_id).len(), 2);
    }

    #[test]
    fn a_routine_whose_bot_was_removed_records_the_code_and_makes_no_task() {
        let (store, routine_id) = store_with_routine("Europe/Madrid", Some(FIRST_MISSED));
        let bot_id = store
            .get_routine(&routine_id)
            .unwrap()
            .unwrap()
            .bot_id
            .unwrap();
        store.delete_bot(&bot_id).unwrap();

        check(&store, CHECK_AT);

        let run = &runs(&store, &routine_id)[0];
        assert_eq!(run.task_id, None);
        assert_eq!(run.error_code.as_deref(), Some(codes::BOT_NOT_FOUND));
        assert!(run.settled_at.is_some());
        // A settled run is never marked interrupted.
        assert_eq!(interrupt_pending(&store, NEXT, NEXT).unwrap(), 0);
    }
}
