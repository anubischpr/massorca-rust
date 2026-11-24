//! The local trace log files every MassOrca process writes into its profile's logs folder (decision 0006): one file a
//! day per writer, `<prefix>.<date>.log`, the newest seven kept. Nothing is sent anywhere.

use std::fs;
use std::path::Path;

use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};

/// File prefix of the service's log: `service.2026-11-24.log`.
pub const SERVICE_LOG_PREFIX: &str = "service";
/// File prefix of the desktop app's log: `desktop.2026-11-24.log`.
pub const DESKTOP_LOG_PREFIX: &str = "desktop";
/// Daily files kept per prefix: today's and the six before it.
pub const KEPT_LOG_FILES: usize = 7;
const LOG_SUFFIX: &str = "log";

/// The daily writer of the `prefix` log in `logs_dir`. Building it prunes that prefix's files beyond `KEPT_LOG_FILES`.
pub fn daily_log_file(logs_dir: &Path, prefix: &str) -> Result<RollingFileAppender, InitError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(prefix)
        .filename_suffix(LOG_SUFFIX)
        .max_log_files(KEPT_LOG_FILES)
        .build(logs_dir)
}

/// The name of the newest `prefix` log file in `logs_dir`, or `None` when it holds none (or cannot be read).
pub fn newest_log_file(logs_dir: &Path, prefix: &str) -> Option<String> {
    let head = format!("{prefix}.");
    let tail = format!(".{LOG_SUFFIX}");
    fs::read_dir(logs_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.strip_prefix(&head)
                .and_then(|rest| rest.strip_suffix(&tail))
                .is_some_and(|date| !date.is_empty() && !date.contains('.'))
        })
        // Dates are ISO, so the greatest name is the newest day.
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_file_of_a_prefix_is_the_latest_day_and_ignores_other_prefixes() {
        let temp = tempfile::TempDir::new().expect("a temporary folder");
        for name in [
            "service.2026-11-22.log",
            "service.2026-11-24.log",
            "desktop.2026-11-23.log",
            "service.notes.txt",
        ] {
            fs::write(temp.path().join(name), "x\n").expect("a log file");
        }
        assert_eq!(
            newest_log_file(temp.path(), SERVICE_LOG_PREFIX).as_deref(),
            Some("service.2026-11-24.log")
        );
        assert_eq!(
            newest_log_file(temp.path(), DESKTOP_LOG_PREFIX).as_deref(),
            Some("desktop.2026-11-23.log")
        );
        assert_eq!(
            newest_log_file(&temp.path().join("missing"), "service"),
            None
        );
    }

    #[test]
    fn only_the_newest_seven_daily_files_of_a_prefix_are_kept() {
        let temp = tempfile::TempDir::new().expect("a temporary folder");
        for day in 1..=9 {
            fs::write(
                temp.path().join(format!("service.2026-11-{day:02}.log")),
                "old\n",
            )
            .expect("an old log");
        }
        fs::write(temp.path().join("desktop.2026-11-01.log"), "old\n").expect("a desktop log");
        let _writer = daily_log_file(temp.path(), SERVICE_LOG_PREFIX).expect("the log opens");
        let service_files = fs::read_dir(temp.path())
            .expect("the logs folder")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("service."))
            .count();
        assert_eq!(service_files, KEPT_LOG_FILES);
        assert!(temp.path().join("desktop.2026-11-01.log").exists());
    }
}
