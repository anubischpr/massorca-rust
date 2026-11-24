//! Service logging: daily files in the profile's logs directory, filtered by `MASSORCA_LOG`.

use massorca_net::{RedactedFormat, SERVICE_LOG_PREFIX, daily_log_file};
use massorca_profile::Profile;
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

use crate::error::{CliError, exit};

/// Environment variable holding the log filter, in `tracing` directive syntax.
pub const FILTER_ENV: &str = "MASSORCA_LOG";
const DEFAULT_FILTER: &str = "info";

/// Starts logging for the service of `profile`. Keep the guard until the process ends: dropping it flushes queued lines.
pub fn init_service_logging(profile: &Profile, to_stderr: bool) -> Result<WorkerGuard, CliError> {
    let filter = EnvFilter::try_new(filter_directives()).map_err(|error| {
        CliError::new(
            "usage.invalid_log_filter",
            format!("invalid {FILTER_ENV}: {error}"),
            exit::USAGE,
        )
    })?;
    let file = daily_log_file(&profile.logs_dir(), SERVICE_LOG_PREFIX)
        .map_err(|error| log_setup_error(&error.to_string()))?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    service_subscriber(filter, writer, to_stderr)
        .try_init()
        .map_err(|error| log_setup_error(&error.to_string()))?;
    Ok(guard)
}

/// The service's subscriber: `filter`, frame dumps capped, and each line written to `writer` (and stderr on request)
/// with its secret-looking values redacted.
fn service_subscriber<W>(
    filter: EnvFilter,
    writer: W,
    to_stderr: bool,
) -> impl Subscriber + Send + Sync + 'static
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let stderr_layer = to_stderr.then(|| {
        fmt::layer()
            .with_ansi(false)
            .event_format(RedactedFormat(fmt::format().with_ansi(false)))
            .with_writer(std::io::stderr)
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(massorca_net::frame_dump_guard())
        .with(
            fmt::layer()
                .with_ansi(false)
                .event_format(RedactedFormat(fmt::format().with_ansi(false)))
                .with_writer(writer),
        )
        .with(stderr_layer)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use massorca_net::{SERVICE_LOG_PREFIX, daily_log_file};
    use tracing::info;
    use tracing_subscriber::EnvFilter;

    use super::service_subscriber;

    /// Logs what `emit` logs into a fresh service log in `dir`, flushes it, and returns the written text.
    fn logged_text(dir: &Path, emit: impl FnOnce()) -> String {
        let file = daily_log_file(dir, SERVICE_LOG_PREFIX).expect("the log opens");
        let (writer, guard) = tracing_appender::non_blocking(file);
        tracing::subscriber::with_default(
            service_subscriber(EnvFilter::new("info"), writer, false),
            emit,
        );
        drop(guard);
        fs::read_dir(dir)
            .expect("the logs folder")
            .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("a log file"))
            .collect()
    }

    #[test]
    fn secrets_never_reach_the_written_service_log_file() {
        let temp = tempfile::TempDir::new().expect("a temporary folder");
        let secrets = [
            "hunter2-placeholder",
            "abc123tokenvalue",
            "dXNlcjpwbGFjZWhvbGRlcg",
            "sk-ant-api03-PLACEHOLDER0123456789",
            "ghp_PLACEHOLDER0123456789abcdef",
            "mop1.eyJwbGFjZWhvbGRlciI6dHJ1ZX0",
        ];
        let text = logged_text(temp.path(), || {
            info!(token = secrets[1], "sign-in sent password={}", secrets[0]);
            info!("request header Authorization: Bearer {}", secrets[2]);
            info!(key = secrets[3], github = secrets[4], "vendor keys loaded");
            info!(offer = ?secrets[5], "pairing offer made");
        });
        assert!(text.contains("vendor keys loaded"), "{text}");
        assert!(text.contains("[redacted]"), "{text}");
        for secret in secrets {
            assert!(!text.contains(secret), "{secret} in the log: {text}");
        }
    }

    #[test]
    fn ordinary_lines_are_written_unchanged() {
        let temp = tempfile::TempDir::new().expect("a temporary folder");
        let text = logged_text(temp.path(), || info!(workspaces = 3, "service started"));
        assert!(text.contains("service started workspaces=3"), "{text}");
        assert!(!text.contains("[redacted]"), "{text}");
    }
}
