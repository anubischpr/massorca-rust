//! The routine timer (decision 0072 amendment, decision 0082): a check every 30 seconds, and one at start, claims the
//! routines that are due and runs each claimed occurrence. A claim is a compare-and-swap on the stored next run, so an
//! occurrence runs once even when two checks overlap, and a check that finds several missed occurrences runs the latest.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use massorca_core_agents::Agents;
use massorca_core_schedule::{ClaimDecision, decide_claim};
use massorca_core_store::{Store, TeamRoutineRow, TeamRoutineRunRow, new_id, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TEAM_CHANGED};
use massorca_protocol::{ErrorBody, codes};
use serde_json::Value;
use tokio::time::MissedTickBehavior;

use crate::context::AppState;
use crate::features::config::WORKSPACES_DEFAULT_ROOT;
use crate::owner::Owner;
use crate::shutdown::ShutdownSignal;

use super::routine_run::{routine_grant, run_routine};
use super::routines::{schedule_error, schedule_of};
use super::task_start::{Changes, publish_changes};
use super::{blocking_state, store_error};

/// How often the host checks for due routines.
const CHECK_EVERY: Duration = Duration::from_secs(30);

/// Checks for due routines every 30 seconds, and once at start, until shutdown.
pub(crate) async fn follow_routines(state: Arc<AppState>, mut signal: ShutdownSignal) {
    let mut checks = tokio::time::interval(CHECK_EVERY);
    // After the host slept, the late check runs once and the next one stays on the normal beat.
    checks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            () = signal.wait() => return,
            _ = checks.tick() => {}
        }
        check(&state).await;
    }
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
        tick(store, agents, &workspaces, now)
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

/// Claims the due occurrences and runs each one. Returns, per claimed routine, its team and what its run changed.
fn tick(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    now: i64,
) -> Result<Vec<(String, Changes)>, ErrorBody> {
    let mut outcomes = Vec::new();
    for claimed in claim_due(store, now)? {
        let mut changes = Changes::default();
        // A claim whose schedule no longer compiles is already finished with its reason, so it does not start.
        if claimed.run.error.is_none() {
            let grant = routine_grant(claimed.routine.starts_as_admin);
            run_routine(
                store,
                agents,
                workspaces,
                &claimed.routine,
                &claimed.run,
                grant,
                &mut changes,
            );
        }
        outcomes.push((claimed.routine.team_id.clone(), changes));
    }
    Ok(outcomes)
}

/// An occurrence this check claimed: its routine as it was, and the run recorded for it.
struct Claimed {
    routine: TeamRoutineRow,
    run: TeamRoutineRunRow,
}

/// Claims every routine whose next run is due. Only the latest missed occurrence is claimed, and a claim another caller
/// won is skipped.
fn claim_due(store: &Store, now: i64) -> Result<Vec<Claimed>, ErrorBody> {
    let mut claimed = Vec::new();
    for routine in store.due_routines(now).map_err(store_error)? {
        let Some(stored) = routine.next_run_at else {
            continue;
        };
        let (scheduled_for, next, error) = match schedule_of(&routine)
            .and_then(|schedule| schedule.compile().map_err(schedule_error))
        {
            Ok(compiled) => match decide_claim(&compiled, Some(stored), now) {
                ClaimDecision::Due {
                    scheduled_for,
                    next_run_at,
                    ..
                } => (scheduled_for, next_run_at, None),
                ClaimDecision::NotDue => continue,
            },
            // The schedule no longer compiles, such as a zone the database lost: the routine ends with the reason.
            Err(error) => (
                stored,
                None,
                Some(format!(
                    "the schedule is no longer valid: {}",
                    error.message
                )),
            ),
        };
        let run = TeamRoutineRunRow {
            id: new_id(),
            routine_id: routine.id.clone(),
            scheduled_for,
            task_id: None,
            error,
            created_at: now,
        };
        if store
            .claim_routine_run(stored, next, &run)
            .map_err(store_error)?
        {
            claimed.push(Claimed { routine, run });
        }
    }
    Ok(claimed)
}

#[cfg(test)]
mod tests {
    use massorca_core_store::{BotRow, Store, TeamRoutineRow, TeamRow, new_id};

    use super::claim_due;

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

    /// 2026-11-20 08:00 UTC is 09:00 CET in Europe/Madrid, the first of the three occurrences that were missed.
    const FIRST_MISSED: i64 = 1_795_161_600_000;
    /// 2026-11-22 08:00 UTC, the latest missed occurrence; the check runs at 12:00 UTC that day.
    const LATEST_MISSED: i64 = 1_795_334_400_000;
    const CHECK_AT: i64 = 1_795_348_800_000;
    /// 2026-11-23 08:00 UTC, the occurrence after the check.
    const NEXT: i64 = 1_795_420_800_000;

    #[test]
    fn a_missed_routine_is_claimed_once_for_its_latest_occurrence() {
        let (store, routine_id) = store_with_routine("Europe/Madrid", Some(FIRST_MISSED));

        let claimed = claim_due(&store, CHECK_AT).unwrap();

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].run.scheduled_for, LATEST_MISSED);
        assert_eq!(claimed[0].run.error, None);
        assert_eq!(
            store.get_routine(&routine_id).unwrap().unwrap().next_run_at,
            Some(NEXT)
        );
        assert_eq!(store.list_routine_runs(&routine_id, 10).unwrap().len(), 1);
    }

    #[test]
    fn a_check_after_a_claim_finds_nothing_due() {
        let (store, _) = store_with_routine("Europe/Madrid", Some(FIRST_MISSED));
        claim_due(&store, CHECK_AT).unwrap();

        assert!(claim_due(&store, CHECK_AT).unwrap().is_empty());
    }

    #[test]
    fn a_paused_routine_is_never_claimed() {
        let (store, _) = store_with_routine("Europe/Madrid", None);

        assert!(claim_due(&store, CHECK_AT).unwrap().is_empty());
    }

    #[test]
    fn a_schedule_that_no_longer_compiles_is_finished_with_its_reason_and_not_retried() {
        let (store, routine_id) = store_with_routine("Not/AZone", Some(FIRST_MISSED));

        let claimed = claim_due(&store, CHECK_AT).unwrap();

        assert_eq!(claimed.len(), 1);
        assert!(
            claimed[0]
                .run
                .error
                .as_deref()
                .unwrap()
                .contains("no longer valid")
        );
        assert_eq!(
            store.get_routine(&routine_id).unwrap().unwrap().next_run_at,
            None
        );
        assert!(claim_due(&store, CHECK_AT).unwrap().is_empty());
    }
}
