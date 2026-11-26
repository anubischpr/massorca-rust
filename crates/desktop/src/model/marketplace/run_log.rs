//! Which version a rollback of an installed package restores, read from its run log (decision 0100). The lockfile keeps
//! the replaced version, but the protocol does not list it, so the run log is the record the Marketplace reads.

use massorca_protocol::methods::extensions::{
    ExtensionRun, ExtensionRunAction, ExtensionRunOutcome,
};

/// The version a rollback restores: the version the newest run that worked replaced, when that run was an update or a
/// rollback. An install or a removal since then leaves nothing kept, and a run that failed changed nothing.
#[must_use]
pub fn kept_version(runs: &[ExtensionRun]) -> Option<String> {
    let newest = runs.iter().rev().find(|run| {
        run.outcome == ExtensionRunOutcome::Ok && run.action != ExtensionRunAction::Unknown
    })?;
    match newest.action {
        ExtensionRunAction::Update | ExtensionRunAction::Rollback => newest.from_version.clone(),
        ExtensionRunAction::Install | ExtensionRunAction::Remove | ExtensionRunAction::Unknown => {
            None
        }
    }
}

/// One run as the log view shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRow {
    /// When the run started, in UTC to the minute.
    pub when: String,
    pub action: &'static str,
    /// The versions the run moved between, such as "1.0.0 to 2.0.0"; empty when it named none.
    pub versions: String,
    pub outcome: &'static str,
    /// The output tail the run wrote, redacted and capped by the service; empty when no vendor command ran.
    pub output: Vec<String>,
}

impl RunRow {
    /// The run's line: when, what it did, the versions it moved and how it ended.
    #[must_use]
    pub fn summary(&self) -> String {
        [
            self.when.as_str(),
            self.action,
            self.versions.as_str(),
            self.outcome,
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    }
}

/// The log view's row for one run.
#[must_use]
pub fn run_row(run: &ExtensionRun) -> RunRow {
    RunRow {
        when: utc_minute(run.at_ms),
        action: match run.action {
            ExtensionRunAction::Install => "Installed",
            ExtensionRunAction::Update => "Updated",
            ExtensionRunAction::Rollback => "Rolled back",
            ExtensionRunAction::Remove => "Removed",
            ExtensionRunAction::Unknown => "Other change",
        },
        versions: versions(run.from_version.as_deref(), run.to_version.as_deref()),
        outcome: match run.outcome {
            ExtensionRunOutcome::Ok => "Done",
            ExtensionRunOutcome::Failed => "Failed",
            ExtensionRunOutcome::Unknown => "Unknown",
        },
        output: run.output.clone(),
    }
}

/// The whole log as text, for Copy details: each run's line then its output, and a blank line between runs.
#[must_use]
pub fn run_log_text(runs: &[ExtensionRun]) -> String {
    runs.iter()
        .map(|run| {
            let row = run_row(run);
            std::iter::once(row.summary())
                .chain(row.output)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The versions a run moved between, as the log names them.
fn versions(from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => format!("{from} to {to}"),
        (Some(version), None) | (None, Some(version)) => version.to_owned(),
        (None, None) => String::new(),
    }
}

/// The minute of a time in milliseconds since 1970, in UTC: the desktop keeps no time zone of its own.
fn utc_minute(at_ms: u64) -> String {
    let minutes = at_ms / 60_000;
    let days = i64::try_from(minutes / 1_440).unwrap_or(i64::MAX);
    let (year, month, day) = civil_from_days(days);
    let minute_of_day = minutes % 1_440;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        minute_of_day / 60,
        minute_of_day % 60
    )
}

/// The civil date of a day count from 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        action: ExtensionRunAction,
        outcome: ExtensionRunOutcome,
        from: Option<&str>,
        to: Option<&str>,
    ) -> ExtensionRun {
        ExtensionRun {
            action,
            from_version: from.map(str::to_owned),
            to_version: to.map(str::to_owned),
            outcome,
            ..crate::service::fixtures::base_extension_run()
        }
    }

    fn installed() -> ExtensionRun {
        run(
            ExtensionRunAction::Install,
            ExtensionRunOutcome::Ok,
            None,
            Some("1.0.0"),
        )
    }

    fn updated() -> ExtensionRun {
        run(
            ExtensionRunAction::Update,
            ExtensionRunOutcome::Ok,
            Some("1.0.0"),
            Some("2.0.0"),
        )
    }

    #[test]
    fn an_empty_log_or_a_fresh_install_keeps_nothing() {
        assert_eq!(kept_version(&[]), None);
        assert_eq!(kept_version(&[installed()]), None);
    }

    #[test]
    fn an_update_keeps_the_version_it_replaced() {
        assert_eq!(
            kept_version(&[installed(), updated()]),
            Some("1.0.0".to_owned())
        );
    }

    #[test]
    fn a_rollback_keeps_the_version_it_rolled_back_from_so_a_second_one_goes_forward() {
        let rolled_back = run(
            ExtensionRunAction::Rollback,
            ExtensionRunOutcome::Ok,
            Some("2.0.0"),
            Some("1.0.0"),
        );
        assert_eq!(
            kept_version(&[installed(), updated(), rolled_back]),
            Some("2.0.0".to_owned())
        );
    }

    #[test]
    fn a_failed_run_changes_nothing_about_what_is_kept() {
        let failed = run(
            ExtensionRunAction::Update,
            ExtensionRunOutcome::Failed,
            Some("2.0.0"),
            Some("3.0.0"),
        );
        assert_eq!(
            kept_version(&[installed(), updated(), failed]),
            Some("1.0.0".to_owned())
        );
    }

    #[test]
    fn a_removal_since_the_last_update_leaves_nothing_kept() {
        let removed = run(
            ExtensionRunAction::Remove,
            ExtensionRunOutcome::Ok,
            Some("2.0.0"),
            None,
        );
        assert_eq!(kept_version(&[installed(), updated(), removed]), None);
    }

    #[test]
    fn a_run_reads_as_its_time_action_versions_and_outcome_with_its_output() {
        let mut update = updated();
        update.at_ms = 1_700_000_000_000;
        update.output = vec!["npm WARN deprecated".to_owned()];
        let row = run_row(&update);
        assert_eq!(
            row.summary(),
            "2023-11-14 22:13 UTC · Updated · 1.0.0 to 2.0.0 · Done"
        );
        assert_eq!(row.output, ["npm WARN deprecated"]);
    }

    #[test]
    fn a_failed_removal_names_the_version_it_removed() {
        let removed = run(
            ExtensionRunAction::Remove,
            ExtensionRunOutcome::Failed,
            Some("2.0.0"),
            None,
        );
        assert_eq!(
            run_row(&removed).summary(),
            "2023-11-14 22:13 UTC · Removed · 2.0.0 · Failed"
        );
    }

    #[test]
    fn the_time_is_utc_to_the_minute_across_leap_days() {
        assert_eq!(utc_minute(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc_minute(951_782_400_000), "2000-02-29 00:00 UTC");
        assert_eq!(utc_minute(1_700_000_000_000), "2023-11-14 22:13 UTC");
    }

    #[test]
    fn the_copied_log_gives_each_run_its_line_then_its_output() {
        let mut update = updated();
        update.output = vec!["npm WARN deprecated".to_owned()];
        assert_eq!(
            run_log_text(&[installed(), update]),
            "2023-11-14 22:13 UTC · Installed · 1.0.0 · Done\n\n2023-11-14 22:13 UTC · Updated · 1.0.0 to 2.0.0 · Done\nnpm WARN deprecated"
        );
    }
}
