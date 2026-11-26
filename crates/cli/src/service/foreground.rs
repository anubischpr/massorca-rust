//! `massorca service run`: the service in this process, until it is stopped.

use std::io::Write;
use std::path::Path;

use massorca_profile::{Profile, RootAccess, ensure_dir, secure_root};
use massorca_protocol::ErrorBody;
use massorca_server::{
    ListenSpec, NetworkConfig, RunningService, START_TIMING_ENV, Service, ServiceConfig,
    ServiceError, ShutdownHandle,
};
use tokio::runtime::{Builder, Runtime};
use tracing_appender::non_blocking::WorkerGuard;

use crate::args::RunArgs;
use crate::error::CliError;
use crate::logging::init_service_logging;
use crate::output::Output;
use crate::readiness::{ReadinessRecord, lines};
use crate::service::parent_watch;

/// Runs the service of `profile` until a stop request or a stop signal arrives. A failure before the service accepts
/// connections is also written to `--start-status`, where the client that started it reads it (ADR 0073).
pub fn run(profile: &Profile, args: &RunArgs, output: Output) -> Result<(), CliError> {
    let started = start(profile, args).inspect_err(|error| {
        if let Some(path) = &args.start_status {
            write_start_status(path, error);
        }
    })?;
    let Started {
        runtime,
        running,
        _logging,
    } = started;
    runtime.block_on(serve(running, output))
}

/// A service that accepts connections, and what must live as long as it.
struct Started {
    runtime: Runtime,
    running: RunningService,
    _logging: WorkerGuard,
}

fn start(profile: &Profile, args: &RunArgs) -> Result<Started, CliError> {
    // Checked before anything is written into the profile, logs included (decision 0073).
    let checking = std::time::Instant::now();
    let access = secure_root(profile).map_err(ServiceError::from)?;
    let root_check = checking.elapsed();
    let network = network_config(args)?;
    ensure_dir(profile)?;
    let logging = init_service_logging(profile, args.log_stderr)?;
    if access == RootAccess::Repaired {
        tracing::warn!(path = %profile.dir().display(), "profile directory access was repaired: it is now private to its owner");
    }
    if args.repair_command {
        crate::cli_install::repair_on_service_start();
    }
    tracing::info!(took_ms = root_check.as_millis(), "profile root checked");
    if std::env::var_os(START_TIMING_ENV).is_some() {
        let _ = writeln!(
            std::io::stderr(),
            "profile root checked in {}ms",
            root_check.as_millis()
        );
    }
    let runtime = Builder::new_multi_thread().enable_all().build()?;
    let config = ServiceConfig {
        profile: profile.clone(),
        server_name: "massorca".to_owned(),
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    // The root was checked above, before the logs were written; the service does not check it again.
    // Logged here, while the log file is still open: the guard below flushes it when this function returns.
    let running = runtime
        .block_on(Service::start_checked(config, network, access))
        .inspect_err(|error| tracing::error!(%error, "service did not start"))?;
    Ok(Started {
        runtime,
        running,
        _logging: logging,
    })
}

/// The error as the client shows it: one `ErrorBody`. A start fails either way, so a write error is ignored.
fn write_start_status(path: &Path, error: &CliError) {
    let body = ErrorBody {
        code: error.code.clone(),
        message: error.message.clone(),
        data: error.data.as_deref().cloned(),
    };
    if let Ok(json) = serde_json::to_vec(&body) {
        let _ = std::fs::write(path, json);
    }
}

/// The `--listen` addresses, checked as decision 0045 says. None means the local socket only.
fn network_config(args: &RunArgs) -> Result<NetworkConfig, CliError> {
    let listen: Vec<ListenSpec> = args
        .listen
        .iter()
        .map(|address| ListenSpec::parse(address, args.allow_all_interfaces))
        .collect::<Result<_, _>>()?;
    Ok(NetworkConfig { listen })
}

/// Prints the readiness record once the listeners have settled (never before they are bound), then serves until stopped.
async fn serve(running: RunningService, output: Output) -> Result<(), CliError> {
    let info = running.info();
    tracing::info!(profile = %info.profile, pid = info.pid, "service started");

    stop_on_signal(running.shutdown_handle());
    if let Some(pid) = parent_watch::parent_pid() {
        parent_watch::stop_with_parent(pid, running.shutdown_handle());
    }
    if let Some(ready) = running.ready().await {
        let record = ReadinessRecord::new(info, ready);
        output.success(&record, &lines(&record));
    }
    running.wait().await?;
    tracing::info!("service stopped");
    Ok(())
}

/// Requests a graceful shutdown on Ctrl+C, and on SIGTERM where the platform has it.
fn stop_on_signal(handle: ShutdownHandle) {
    tokio::spawn(async move {
        match wait_for_stop_signal().await {
            Ok(()) => {
                tracing::info!("stop signal received");
                handle.shutdown();
            }
            Err(error) => tracing::warn!(
                %error,
                "cannot listen for stop signals; use `massorca service stop`"
            ),
        }
    });
}

#[cfg(unix)]
async fn wait_for_stop_signal() -> std::io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result,
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_stop_signal() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}
