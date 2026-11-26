//! The window `calendar.occurrences` reads: the runs of routines and automations that fall in it, earliest first.

use massorca_core_schedule::{WallClock, host_zone_used, wall_clock_in};
use massorca_core_store::Store;
use massorca_protocol::methods::calendar::{
    CalendarKind, CalendarOccurrenceList, CalendarOccurrencesParams, MAX_OCCURRENCES,
    MAX_WINDOW_DAYS,
};
use massorca_protocol::{ErrorBody, codes};

use super::routine_runs;

const DAY_MS: i64 = 86_400_000;

/// What one read covers: the window, the instant of the read, and whether done routine runs are listed.
#[derive(Debug, Clone, Copy)]
pub(super) struct Window {
    pub(super) from_ms: i64,
    pub(super) to_ms: i64,
    pub(super) now_ms: i64,
    pub(super) include_done: bool,
}

impl Window {
    /// Upcoming runs are the ones at or after now: a run before now has happened, and is done or was missed.
    pub(super) fn upcoming_from(&self) -> i64 {
        self.from_ms.max(self.now_ms)
    }
}

/// The runs in the window at `now_ms`, earliest first, at most [`MAX_OCCURRENCES`] of them.
pub(super) fn read(
    store: &Store,
    now_ms: i64,
    params: &CalendarOccurrencesParams,
) -> Result<CalendarOccurrenceList, ErrorBody> {
    check_window(params)?;
    if params.kind == Some(CalendarKind::Unknown) {
        return Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            "the calendar kind is neither routine nor automation",
        ));
    }
    let window = Window {
        from_ms: params.from_ms,
        to_ms: params.to_ms,
        now_ms,
        include_done: params.include_done,
    };
    let host_zone = host_zone_used().0;
    let mut runs = Vec::new();
    if includes(params.kind, CalendarKind::Routine) {
        runs.extend(routine_runs::read(store, &window, host_zone.as_deref())?);
    }
    runs.sort_by(|a, b| {
        a.at_ms
            .cmp(&b.at_ms)
            .then_with(|| a.source_id.cmp(&b.source_id))
            .then_with(|| a.done.cmp(&b.done))
    });
    let truncated = runs.len() > MAX_OCCURRENCES;
    runs.truncate(MAX_OCCURRENCES);
    Ok(CalendarOccurrenceList {
        occurrences: runs,
        truncated,
    })
}

/// A window ends after it starts and spans at most [`MAX_WINDOW_DAYS`].
fn check_window(params: &CalendarOccurrencesParams) -> Result<(), ErrorBody> {
    if params.to_ms <= params.from_ms {
        return Err(window_error("the window ends before it starts"));
    }
    match params.to_ms.checked_sub(params.from_ms) {
        Some(span) if span <= MAX_WINDOW_DAYS * DAY_MS => Ok(()),
        _ => Err(window_error("the window spans more than 62 days")),
    }
}

fn window_error(message: &str) -> ErrorBody {
    ErrorBody::new(codes::CALENDAR_WINDOW_INVALID, message)
}

fn includes(kind: Option<CalendarKind>, this: CalendarKind) -> bool {
    kind.is_none_or(|kind| kind == this)
}

/// The local date and time of `at_ms` on `zone`, the host's zone when `None`. An instant the engine produced is always
/// in range, so an out-of-range one is an internal error.
pub(super) fn placed(zone: Option<&str>, at_ms: i64) -> Result<WallClock, ErrorBody> {
    wall_clock_in(zone, at_ms)
        .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?
        .ok_or_else(|| {
            ErrorBody::new(
                codes::INTERNAL,
                "a run's time is outside the calendar's range",
            )
        })
}

#[cfg(test)]
mod tests {
    use massorca_core_store::{BotRow, Store, TeamRoutineRow, TeamRoutineRunRow, TeamRow};
    use massorca_protocol::codes;
    use massorca_protocol::methods::calendar::{CalendarKind, CalendarOccurrencesParams};

    use super::read;

    const DAY: i64 = 86_400_000;
    const HOUR: i64 = 3_600_000;
    /// 2026-11-23 12:00 UTC.
    const NOW: i64 = 1_795_435_200_000;
    /// 2026-11-25 00:00 UTC.
    const NOV_25: i64 = 1_795_564_800_000;
    /// 2026-11-24 00:00 UTC.
    const NOV_24: i64 = NOV_25 - DAY;

    fn store() -> Store {
        let store = Store::open_in_memory().expect("in-memory store opens");
        store
            .insert_team(&TeamRow {
                id: "team".to_owned(),
                name: "Desarrollo".to_owned(),
                purpose: String::new(),
                character: "triangle".to_owned(),
                access: "full".to_owned(),
                workspaces: false,
                created_at: 1,
            })
            .expect("team inserts");
        store
            .insert_bot(&BotRow {
                id: "bot".to_owned(),
                team_id: "team".to_owned(),
                name: "Mara".to_owned(),
                job: "Writes the parser".to_owned(),
                kind: "claude".to_owned(),
                model: None,
                effort: None,
                lead: false,
                created_at: 1,
            })
            .expect("bot inserts");
        store
    }

    fn routine(id: &str, time: &str, zone: Option<&str>) -> TeamRoutineRow {
        TeamRoutineRow {
            id: id.to_owned(),
            team_id: "team".to_owned(),
            bot_id: Some("bot".to_owned()),
            name: format!("Routine {id}"),
            prompt: "Read the notes.".to_owned(),
            cadence: "daily".to_owned(),
            time: time.to_owned(),
            weekday: None,
            month_day: None,
            run_date: None,
            zone: zone.map(str::to_owned),
            paused: false,
            starts_as_admin: true,
            next_run_at: None,
            created_at: 1,
        }
    }

    fn params(from_ms: i64, to_ms: i64) -> CalendarOccurrencesParams {
        CalendarOccurrencesParams {
            from_ms,
            to_ms,
            kind: None,
            include_done: false,
        }
    }

    #[test]
    fn a_window_of_62_days_is_read_and_a_longer_one_is_refused() {
        let store = store();
        assert!(read(&store, NOW, &params(NOV_25, NOV_25 + 62 * DAY)).is_ok());
        let error = read(&store, NOW, &params(NOV_25, NOV_25 + 62 * DAY + 1))
            .expect_err("a window longer than 62 days is refused");
        assert_eq!(error.code, codes::CALENDAR_WINDOW_INVALID);
    }

    #[test]
    fn a_window_that_ends_before_it_starts_is_refused() {
        let store = store();
        let error = read(&store, NOW, &params(NOV_25, NOV_25 - 1))
            .expect_err("a reversed window is refused");
        assert_eq!(error.code, codes::CALENDAR_WINDOW_INVALID);
    }

    #[test]
    fn a_routine_is_placed_by_its_instant_on_its_own_zone() {
        let store = store();
        store
            .insert_routine(&routine("madrid", "09:00", Some("Europe/Madrid")))
            .expect("routine inserts");
        store
            .insert_routine(&routine("chicago", "09:00", Some("America/Chicago")))
            .expect("routine inserts");

        let listed = read(&store, NOW, &params(NOV_25, NOV_25 + DAY)).expect("window reads");

        let placed: Vec<(&str, i64, &str, &str)> = listed
            .occurrences
            .iter()
            .map(|run| {
                (
                    run.source_id.as_str(),
                    run.at_ms,
                    run.local_date.as_str(),
                    run.local_time.as_str(),
                )
            })
            .collect();
        assert_eq!(
            placed,
            vec![
                ("madrid", NOV_25 + 8 * HOUR, "2026-11-25", "09:00"),
                ("chicago", NOV_25 + 15 * HOUR, "2026-11-25", "09:00"),
            ]
        );
        assert_eq!(listed.occurrences[0].zone.as_deref(), Some("Europe/Madrid"));
        assert!(!listed.truncated);
    }

    #[test]
    fn a_routine_created_by_a_bot_in_the_conference_room_is_listed_with_the_others() {
        let store = store();
        store
            .insert_routine(&routine("page", "09:00", Some("UTC")))
            .expect("routine inserts");
        let mut by_bot = routine("room", "10:00", Some("UTC"));
        by_bot.starts_as_admin = false;
        store.insert_routine(&by_bot).expect("routine inserts");

        let listed = read(&store, NOW, &params(NOV_25, NOV_25 + DAY)).expect("window reads");

        let mut ids: Vec<&str> = listed
            .occurrences
            .iter()
            .map(|run| run.source_id.as_str())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["page", "room"]);
    }

    #[test]
    fn the_kind_filter_keeps_only_the_kind_asked_for() {
        let store = store();
        store
            .insert_routine(&routine("daily", "09:00", Some("UTC")))
            .expect("routine inserts");

        let routines_only = read(
            &store,
            NOW,
            &CalendarOccurrencesParams {
                kind: Some(CalendarKind::Routine),
                ..params(NOV_25, NOV_25 + DAY)
            },
        )
        .expect("window reads");
        assert_eq!(routines_only.occurrences.len(), 1);

        let automations_only = read(
            &store,
            NOW,
            &CalendarOccurrencesParams {
                kind: Some(CalendarKind::Automation),
                ..params(NOV_25, NOV_25 + DAY)
            },
        )
        .expect("window reads");
        assert!(automations_only.occurrences.is_empty());
    }

    #[test]
    fn done_runs_are_hidden_unless_asked_for_and_a_pending_run_is_never_done() {
        let store = store();
        store
            .insert_routine(&routine("daily", "09:00", Some("Europe/Madrid")))
            .expect("routine inserts");
        let run = |id: &str, settled_at: Option<i64>| TeamRoutineRunRow {
            id: id.to_owned(),
            routine_id: "daily".to_owned(),
            scheduled_for: NOV_24 + 8 * HOUR,
            task_id: None,
            error: None,
            error_code: None,
            settled_at,
            created_at: NOV_24,
        };
        store
            .insert_routine_run(&run("done", Some(NOV_24 + 9 * HOUR)), None)
            .expect("run inserts");
        let window = |include_done| CalendarOccurrencesParams {
            include_done,
            ..params(NOV_24, NOV_25)
        };

        let hidden = read(&store, NOV_25, &window(false)).expect("window reads");
        assert!(hidden.occurrences.is_empty());

        let shown = read(&store, NOV_25, &window(true)).expect("window reads");
        assert_eq!(shown.occurrences.len(), 1);
        assert!(shown.occurrences[0].done);
        assert_eq!(shown.occurrences[0].local_time, "09:00");

        store
            .insert_routine_run(&run("pending", None), None)
            .expect("run inserts");
        let with_pending = read(&store, NOV_25, &window(true)).expect("window reads");
        assert_eq!(with_pending.occurrences.len(), 1);
    }

    #[test]
    fn only_occurrences_from_now_on_are_upcoming_and_a_paused_routine_has_none() {
        let store = store();
        store
            .insert_routine(&routine("daily", "09:00", Some("Europe/Madrid")))
            .expect("routine inserts");
        let mut paused = routine("paused", "09:00", Some("Europe/Madrid"));
        paused.paused = true;
        store.insert_routine(&paused).expect("routine inserts");

        // Madrid's 09:00 on 25 November is 08:00 UTC, before the clock at 12:00 UTC; the 26th is upcoming.
        let listed = read(
            &store,
            NOV_25 + 12 * HOUR,
            &params(NOV_24, NOV_25 + 2 * DAY),
        )
        .expect("window reads");

        let upcoming: Vec<(&str, i64)> = listed
            .occurrences
            .iter()
            .map(|run| (run.source_id.as_str(), run.at_ms))
            .collect();
        assert_eq!(upcoming, vec![("daily", NOV_25 + DAY + 8 * HOUR)]);
    }
}
