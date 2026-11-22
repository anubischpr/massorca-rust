//! Service logging: daily files in the profile's logs directory, filtered by `MASSORCA_LOG`.

use massorca_profile::Profile;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

use crate::error::{CliError, exit};

/// Environment variable holding the log filter, in `tracing` directive syntax.
pub const FILTER_ENV: &str = "MASSORCA_LOG";
const DEFAULT_FILTER: &str = "info";
/// Daily files kept: today's file and the six before it.
const KEPT_FILES: usize = 7;

/// Starts logging for the service of `profile`. Keep the guard until the process ends: dropping it flushes queued lines.
pub fn init_service_logging(profile: &Profile, to_stderr: bool) -> Result<WorkerGuard, CliError> {
    let filter = EnvFilter::try_new(filter_directives()).map_err(|error| {
        CliError::new(
            "usage.invalid_log_filter",
            format!("invalid {FILTER_ENV}: {error}"),
            exit::USAGE,
        )
    })?;
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("service")
        .filename_suffix("log")
        .max_log_files(KEPT_FILES)
        .build(profile.logs_dir())
        .map_err(|error| log_setup_error(&error.to_string()))?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    let stderr_layer =
        to_stderr.then(|| fmt::layer().with_writer(std::io::stderr).with_ansi(false));
    tracing_subscriber::registry()
        .with(filter)
        .with(massorca_net::frame_dump_guard())
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .with(stderr_layer)
        .try_init()
        .map_err(|error| log_setup_error(&error.to_string()))?;
    Ok(guard)
}

fn filter_directives() -> String {
    match std::env::var(FILTER_ENV) {
        Ok(text) if !text.trim().is_empty() => text,
        _ => DEFAULT_FILTER.to_owned(),
    }
}

fn log_setup_error(reason: &str) -> CliError {
    CliError::new(
        "service.log_setup",
        format!("cannot set up the service log: {reason}"),
        exit::ERROR,
    )
}
