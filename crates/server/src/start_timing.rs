//! How long each phase of a service start takes, so a slow start says where its time went. The phases are logged once
//! at info level when the service accepts connections, and also printed on stderr when `MASSORCA_START_TIMING` is set.

use std::io::Write;
use std::time::{Duration, Instant};

/// Prints the phases on stderr too when set, for timing a start from outside.
pub const START_TIMING_ENV: &str = "MASSORCA_START_TIMING";

#[derive(Debug)]
pub(crate) struct StartTimings {
    started: Instant,
    last: Instant,
    phases: Vec<(&'static str, Duration)>,
}

impl StartTimings {
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            last: now,
            phases: Vec::new(),
        }
    }

    /// Ends the phase `name` at this moment; the next phase starts now.
    pub(crate) fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.phases.push((name, now - self.last));
        self.last = now;
    }

    /// One line: the total and each phase in milliseconds, in order.
    pub(crate) fn summary(&self) -> String {
        let phases = self
            .phases
            .iter()
            .map(|(name, took)| format!("{name} {}ms", took.as_millis()))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}ms in total: {phases}",
            self.started.elapsed().as_millis()
        )
    }

    pub(crate) fn report(&self) {
        let summary = self.summary();
        tracing::info!(phases = %summary, "service start phases");
        if std::env::var_os(START_TIMING_ENV).is_some() {
            let _ = writeln!(std::io::stderr(), "service start: {summary}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StartTimings;

    #[test]
    fn the_summary_names_every_phase_in_order() {
        let mut timings = StartTimings::new();
        timings.mark("root check");
        timings.mark("store");

        let summary = timings.summary();

        let root = summary.find("root check ").expect("the first phase");
        let store = summary.find("store ").expect("the second phase");
        assert!(root < store, "{summary}");
        assert!(summary.contains("ms in total: "), "{summary}");
    }
}
