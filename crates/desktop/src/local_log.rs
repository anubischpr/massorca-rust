//! The desktop's own log, beside the service's: one file a day in the profile's logs folder, the newest seven kept. It
//! follows `MASSORCA_LOG` as the service does, drops frame dumps and redacts secret-looking values. Nothing is sent anywhere.

use std::path::Path;

use massorca_profile::{Profile, RootAccess, ensure_dir, secure_root};
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

use crate::log_redaction::RedactedFormat;
use crate::service::LaunchOptions;

/// The environment variable holding the level, in `tracing` directive syntax, as the service reads it.
pub const FILTER_ENV: &str = "MASSORCA_LOG";
const DEFAULT_FILTER: &str = "info";
/// Files start with this, then the day: `desktop.2026-11-24.log`.
pub const FILE_PREFIX: &str = "desktop";
/// Daily files kept, today's included, as the service keeps its own.
pub const KEPT_FILES: usize = 7;

/// Starts the local log of the profile the launch names. `None` when that profile does not resolve, since the connection
/// reports that. Keep the guard until the app quits: dropping it flushes the lines still queued.
pub fn start_for_launch(options: &LaunchOptions) -> Result<Option<WorkerGuard>, String> {
    match options.resolve_profile() {
        Ok(profile) => start(&profile, |key| std::env::var(key).ok()).map(Some),
        Err(_) => Ok(None),
    }
}

/// Starts logging into the logs folder of `profile`, at the level `env` gives for `MASSORCA_LOG` (`info` when unset).
pub fn start(
    profile: &Profile,
    env: impl Fn(&str) -> Option<String>,
) -> Result<WorkerGuard, String> {
    // Checked before anything is written into the profile, logs included (decision 0073), as the service does.
    let access = secure_root(profile)
        .map_err(|error| format!("cannot check the profile folder: {error}"))?;
    ensure_dir(profile).map_err(|error| format!("cannot create the logs folder: {error}"))?;
    let filter = EnvFilter::try_new(filter_directives(&env))
        .map_err(|error| format!("invalid {FILTER_ENV}: {error}"))?;
    let file = daily_file(&profile.logs_dir())
        .map_err(|error| format!("cannot open the local log: {error}"))?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    subscriber(filter, writer)
        .try_init()
        .map_err(|error| format!("the local log cannot start: {error}"))?;
    if access == RootAccess::Repaired {
        tracing::warn!(
            path = %profile.dir().display(),
            "profile directory access was repaired: it is now private to its owner"
        );
    }
    Ok(guard)
}

/// The daily writer of the desktop's log in `logs_dir`. Building it prunes the older files beyond `KEPT_FILES`.
pub fn daily_file(logs_dir: &Path) -> Result<RollingFileAppender, InitError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix("log")
        .max_log_files(KEPT_FILES)
        .build(logs_dir)
}

/// The subscriber the desktop logs through: `filter` for every target, frame dumps capped, and each line written with
/// its secret-looking values redacted.
pub fn subscriber<W>(filter: EnvFilter, writer: W) -> impl Subscriber + Send + Sync + 'static
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::registry()
        .with(filter)
        .with(massorca_net::frame_dump_guard())
        .with(
            fmt::layer()
                .with_ansi(false)
                .event_format(RedactedFormat(fmt::format().with_ansi(false)))
                .with_writer(writer),
        )
}

fn filter_directives(env: &impl Fn(&str) -> Option<String>) -> String {
    match env(FILTER_ENV) {
        Some(text) if !text.trim().is_empty() => text,
        _ => DEFAULT_FILTER.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;
    use tracing::{Level, event, info};
    use tracing_subscriber::EnvFilter;

    use super::{KEPT_FILES, daily_file, subscriber};

    /// Logs what `emit` logs, at `level` and up, into a fresh log in `dir`, and flushes it.
    fn log_into(dir: &Path, level: &str, emit: impl FnOnce()) {
        let (writer, guard) =
            tracing_appender::non_blocking(daily_file(dir).expect("the log opens"));
        tracing::subscriber::with_default(subscriber(EnvFilter::new(level), writer), emit);
        drop(guard);
    }

    /// The text of every file in `dir`.
    fn log_text(dir: &Path) -> String {
        fs::read_dir(dir)
            .expect("the logs folder")
            .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("a log file"))
            .collect()
    }

    #[test]
    fn the_log_is_one_daily_desktop_file_in_the_logs_folder() {
        let temp = TempDir::new().expect("a temporary folder");
        log_into(temp.path(), "info", || info!("connected"));
        let names: Vec<String> = fs::read_dir(temp.path())
            .expect("the logs folder")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(
            names[0].starts_with("desktop.") && names[0].ends_with(".log"),
            "{names:?}"
        );
        assert!(log_text(temp.path()).contains("connected"));
    }

    #[test]
    fn a_secret_looking_value_is_written_redacted() {
        let temp = TempDir::new().expect("a temporary folder");
        log_into(temp.path(), "info", || {
            info!(token = "abc123", "sign-in sent password: hunter2");
        });
        let text = log_text(temp.path());
        assert!(text.contains("token=\"[redacted]\""), "{text}");
        assert!(text.contains("password: [redacted]"), "{text}");
        assert!(
            !text.contains("abc123") && !text.contains("hunter2"),
            "{text}"
        );
    }

    #[test]
    fn frame_dumps_stay_out_of_the_log_even_at_trace() {
        let temp = TempDir::new().expect("a temporary folder");
        log_into(temp.path(), "trace", || {
            event!(target: "tungstenite::protocol", Level::TRACE, "frame with token=zz");
            event!(target: "massorca_desktop", Level::TRACE, "a trace line");
        });
        let text = log_text(temp.path());
        assert!(text.contains("a trace line"), "{text}");
        assert!(!text.contains("frame with"), "{text}");
    }

    #[test]
    fn only_the_newest_seven_daily_files_are_kept() {
        let temp = TempDir::new().expect("a temporary folder");
        for day in 1..=9 {
            fs::write(
                temp.path().join(format!("desktop.2026-11-{day:02}.log")),
                "old\n",
            )
            .expect("an old log");
        }
        let _writer = daily_file(temp.path()).expect("the log opens");
        assert_eq!(
            fs::read_dir(temp.path()).expect("the logs folder").count(),
            KEPT_FILES
        );
    }
}
