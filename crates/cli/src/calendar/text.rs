//! The lines `massorca calendar` prints: one run per line, earliest first.

use massorca_protocol::methods::calendar::{CalendarKind, CalendarOccurrence};

/// One line per run: its date, time and zone on the run's own zone, its kind, its name, and `done` for a done run.
pub fn occurrence_lines(runs: &[CalendarOccurrence]) -> Vec<String> {
    runs.iter().map(occurrence_line).collect()
}

fn occurrence_line(run: &CalendarOccurrence) -> String {
    let zone = run.zone.as_deref().unwrap_or("host zone");
    let done = if run.done { "  done" } else { "" };
    format!(
        "{} {} {zone}  {:<10} {}{done}",
        run.local_date,
        run.local_time,
        kind_word(run.kind),
        run.name
    )
}

fn kind_word(kind: CalendarKind) -> &'static str {
    match kind {
        CalendarKind::Routine => "routine",
        CalendarKind::Automation => "automation",
        CalendarKind::Unknown => "run",
    }
}
