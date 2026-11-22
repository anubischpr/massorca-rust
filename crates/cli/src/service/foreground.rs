//! `massorca service run`: the service in this process, until it is stopped.

use massorca_profile::{Profile, RootAccess, ensure_dir, secure_root};
use massorca_server::{Service, ServiceConfig, ServiceError, ShutdownHandle};
use tokio::runtime::Builder;

use crate::args::RunArgs;
use crate::error::CliError;
use crate::logging::init_service_logging;
use crate::output::Output;

/// Runs the service of `profile` until a stop request or a stop signal arrives.
pub fn run(profile: &Profile, args: &RunArgs, output: Output) -> Result<(), CliError> {
    // Checked before anything is written into the profile, logs included (decision 0073).
    let access = secure_root(profile).map_err(ServiceError::from)?;
    ensure_dir(profile)?;
    let _logging = init_service_logging(profile, args.log_stderr)?;
    if access == RootAccess::Repaired {
        tracing::warn!(path = %profile.dir().display(), "profile directory access was repaired: it is now private to its owner");
    }
    let runtime = Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(serve(profile, output))
}

async fn serve(profile: &Profile, output: Output) -> Result<(), CliError> {
    let config = ServiceConfig {
        profile: profile.clone(),
        server_name: "massorca".to_owned(),
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let running = Service::start(config).await?;
    let info = running.info();
    tracing::info!(profile = %info.profile, pid = info.pid, "service started");
    output.success(
        &info,
        &[format!(
            "service running for profile {} (pid {})",
            info.profile, info.pid
        )],
    );

    stop_on_signal(running.shutdown_handle());
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
