//! The rules an automation must meet: its cadence, time, weekday, cron expression, zone, grace and agent (decision
//! 0123). core-schedule checks the schedule itself; this module maps each refusal to its protocol code.

use massorca_core_agents::Agents;
use massorca_core_schedule::{
    Cadence, CompiledSchedule, CronSchedule, Schedule, ScheduleError, Weekday, ZoneSource,
    host_zone_used, next_run_from,
};
use massorca_core_store::AutomationRow;
use massorca_protocol::methods::automations::{self, AutomationCadence, AutomationWeekday};
use massorca_protocol::{ErrorBody, codes};

/// The host's zone, which an automation without a zone runs in.
pub(super) struct HostZone {
    name: Option<String>,
    known: bool,
}

impl HostZone {
    pub(super) fn read() -> Self {
        let (name, source) = host_zone_used();
        Self {
            name,
            known: source != ZoneSource::HostUnknownUsedUtc,
        }
    }

    /// The zone an automation without a zone shows as in use.
    pub(super) fn label(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| "the host's zone".to_owned())
    }

    /// Refuses an automation left without a zone while the host's zone cannot be read: it would silently run at UTC.
    fn allow(&self, zone: Option<&str>) -> Result<(), ErrorBody> {
        if zone.is_none() && !self.known {
            return Err(ErrorBody::new(
                codes::AUTOMATION_ZONE_INVALID,
                "the host's time zone cannot be read, so an automation needs a zone, such as Europe/Madrid",
            ));
        }
        Ok(())
    }
}

const WEEKDAYS: [(&str, AutomationWeekday, Weekday); 7] = [
    ("monday", AutomationWeekday::Monday, Weekday::Monday),
    ("tuesday", AutomationWeekday::Tuesday, Weekday::Tuesday),
    (
        "wednesday",
        AutomationWeekday::Wednesday,
        Weekday::Wednesday,
    ),
    ("thursday", AutomationWeekday::Thursday, Weekday::Thursday),
    ("friday", AutomationWeekday::Friday, Weekday::Friday),
    ("saturday", AutomationWeekday::Saturday, Weekday::Saturday),
    ("sunday", AutomationWeekday::Sunday, Weekday::Sunday),
];

pub(super) fn schedule_invalid(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(codes::AUTOMATION_SCHEDULE_INVALID, message)
}

/// The stored name of a cadence; a value this build does not know is refused.
pub(super) fn cadence_name(cadence: AutomationCadence) -> Result<&'static str, ErrorBody> {
    match cadence {
        AutomationCadence::Hourly => Ok("hourly"),
        AutomationCadence::Daily => Ok("daily"),
        AutomationCadence::Weekdays => Ok("weekdays"),
        AutomationCadence::Weekly => Ok("weekly"),
        AutomationCadence::Cron => Ok("cron"),
        AutomationCadence::Unknown => Err(schedule_invalid(
            "the cadence must be hourly, daily, weekdays, weekly or cron",
        )),
    }
}

pub(super) fn cadence_of(name: &str) -> AutomationCadence {
    match name {
        "hourly" => AutomationCadence::Hourly,
        "daily" => AutomationCadence::Daily,
        "weekdays" => AutomationCadence::Weekdays,
        "weekly" => AutomationCadence::Weekly,
        "cron" => AutomationCadence::Cron,
        _ => AutomationCadence::Unknown,
    }
}

pub(super) fn weekday_name(day: AutomationWeekday) -> Result<&'static str, ErrorBody> {
    WEEKDAYS
        .iter()
        .find(|(_, wire, _)| *wire == day)
        .map(|(name, _, _)| *name)
        .ok_or_else(|| schedule_invalid("the weekday must be monday to sunday"))
}

pub(super) fn weekday_of(name: &str) -> AutomationWeekday {
    WEEKDAYS
        .iter()
        .find(|(known, _, _)| *known == name)
        .map_or(AutomationWeekday::Unknown, |(_, wire, _)| *wire)
}

/// A weekly automation keeps its weekday and a cron one its expression; other cadences keep neither.
pub(super) fn keep_for(
    cadence: &str,
    weekday: Option<String>,
    cron: Option<String>,
) -> (Option<String>, Option<String>) {
    let weekday = if cadence == "weekly" { weekday } else { None };
    let cron = if cadence == "cron" { cron } else { None };
    (weekday, cron)
}

/// A cron automation's times come from its expression, so it keeps no time of its own.
pub(super) fn time_kept(cadence: &str, time: &str) -> String {
    if cadence == "cron" {
        String::new()
    } else {
        time.trim().to_owned()
    }
}

/// The grace a missed run may keep, when the automation sets one, within a week.
pub(super) fn check_grace(grace: Option<i64>) -> Result<Option<i64>, ErrorBody> {
    match grace {
        None => Ok(None),
        Some(minutes) if (0..=automations::MAX_GRACE_MINUTES).contains(&minutes) => {
            Ok(Some(minutes))
        }
        Some(minutes) => Err(schedule_invalid(format!(
            "the grace is {minutes} minutes; it must be 0 to {} minutes",
            automations::MAX_GRACE_MINUTES
        ))),
    }
}

/// The agent must be a kind this build declares.
pub(super) fn check_agent(agents: &Agents, agent: &str) -> Result<(), ErrorBody> {
    if agents.kinds().kinds.iter().any(|kind| kind.id == agent) {
        return Ok(());
    }
    Err(ErrorBody::new(
        codes::AGENT_KIND_NOT_FOUND,
        format!("no agent kind {agent}"),
    ))
}

/// Refuses a definition the rules do not accept, and returns its compiled schedule.
pub(super) fn check(
    row: &AutomationRow,
    agents: &Agents,
    host: &HostZone,
) -> Result<CompiledSchedule, ErrorBody> {
    check_agent(agents, &row.agent_kind)?;
    host.allow(row.zone.as_deref())?;
    compile(row)
}

/// The schedule a stored automation means. core-schedule refuses a cron that can never occur, a bad time, a weekday
/// it does not know and a zone it cannot resolve.
pub(super) fn compile(row: &AutomationRow) -> Result<CompiledSchedule, ErrorBody> {
    let zone = row.zone.clone();
    let compiled = if row.cadence == "cron" {
        CronSchedule {
            expression: row.cron.clone().unwrap_or_default(),
            zone,
        }
        .compile()
    } else {
        Schedule {
            cadence: core_cadence(&row.cadence)?,
            weekday: row.weekday.as_deref().map(core_weekday).transpose()?,
            day_of_month: None,
            date: None,
            time: row.time.clone(),
            zone,
        }
        .compile()
    };
    compiled.map_err(schedule_error)
}

/// The next run from `now`: none while the automation is disabled.
pub(super) fn next_run(row: &AutomationRow, compiled: &CompiledSchedule, now: i64) -> Option<i64> {
    if row.enabled {
        next_run_from(compiled, now)
    } else {
        None
    }
}

fn core_cadence(name: &str) -> Result<Cadence, ErrorBody> {
    match name {
        "hourly" => Ok(Cadence::Hourly),
        "daily" => Ok(Cadence::Daily),
        "weekdays" => Ok(Cadence::Weekdays),
        "weekly" => Ok(Cadence::Weekly),
        other => Err(schedule_invalid(format!(
            "the cadence {other:?} is not hourly, daily, weekdays, weekly or cron"
        ))),
    }
}

fn core_weekday(name: &str) -> Result<Weekday, ErrorBody> {
    WEEKDAYS
        .iter()
        .find(|(known, _, _)| *known == name)
        .map(|(_, _, day)| *day)
        .ok_or_else(|| schedule_invalid(format!("the weekday {name:?} is not a day of the week")))
}

/// A zone refusal stays a zone refusal; every other schedule refusal is a schedule one.
fn schedule_error(error: ScheduleError) -> ErrorBody {
    let code = match error {
        ScheduleError::UnknownZone(_) | ScheduleError::ZoneTooLong(_) => {
            codes::AUTOMATION_ZONE_INVALID
        }
        _ => codes::AUTOMATION_SCHEDULE_INVALID,
    };
    ErrorBody::new(code, error.to_string())
}
