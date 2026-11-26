//! `massorca service status|info|stop|restart`: controlling the service of a profile.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::{Client, ClientError};
use massorca_profile::Profile;
use massorca_protocol::methods::service::{GitStatus, ServiceInfo, ToolInfo, ToolState};
use serde::Serialize;

use crate::connect::{connect, connect_local, connect_running};
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;

/// How long a stopping service may take to release its profile.
const STOP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(50);

/// Result of `service status`.
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub running: bool,
    pub info: Option<ServiceInfo>,
}

/// Result of `service stop`: whether a running service was stopped.
#[derive(Debug, Serialize)]
pub struct StopOutcome {
    pub stopped: bool,
}

/// Reports whether the service runs. Never starts it.
pub async fn status(profile: &Profile, output: Output) -> Result<(), CliError> {
    let status = match connect_running(profile).await? {
        Some(client) => ServiceStatus {
            running: true,
            info: Some(client.service_info().await?),
        },
        None => ServiceStatus {
            running: false,
            info: None,
        },
    };
    let lines = match &status.info {
        Some(info) => std::iter::once("running".to_owned())
            .chain(info_lines(info))
            .collect(),
        None => vec!["not running".to_owned()],
    };
    output.success(&status, &lines);
    Ok(())
}

/// Shows the developer tools on the service's host, one line each. The service detects them, so `--host` picks whose.
pub async fn tools(
    target: &Target,
    autostart: bool,
    refresh: bool,
    output: Output,
) -> Result<(), CliError> {
    let list = connect(target, autostart)
        .await?
        .service_tools(refresh)
        .await?;
    let lines: Vec<String> = list.tools.iter().map(tool_line).collect();
    output.success(&list, &lines);
    Ok(())
}

/// One tool as `id: state version (path)`, with the version and the path only when the service reported them.
fn tool_line(tool: &ToolInfo) -> String {
    let state = match tool.state {
        ToolState::Present => "present",
        ToolState::Missing => "missing",
        ToolState::TooOld => "too_old",
        ToolState::Unknown => "unknown",
        ToolState::Other => "other",
    };
    let mut line = format!("{}: {state}", tool.id);
    if let Some(version) = &tool.version {
        line.push(' ');
        line.push_str(version);
    }
    if let Some(path) = &tool.path {
        line.push_str(&format!(" ({path})"));
    }
    line
}

/// Shows the service's identity, starting the service first unless autostart is off.
pub async fn info(target: &Target, autostart: bool, output: Output) -> Result<(), CliError> {
    let info = connect(target, autostart).await?.service_info().await?;
    output.success(&info, &info_lines(&info));
    Ok(())
}

/// Stops the service. Succeeds when it is not running.
pub async fn stop(profile: &Profile, output: Output) -> Result<(), CliError> {
    let stopped = stop_service(profile).await?;
    let line = if stopped {
        "service stopped"
    } else {
        "not running"
    };
    output.success(&StopOutcome { stopped }, &[line.to_owned()]);
    Ok(())
}

/// Stops the service if it runs, then starts it again.
pub async fn restart(profile: &Profile, output: Output) -> Result<(), CliError> {
    stop_service(profile).await?;
    let info = connect_local(profile, true).await?.service_info().await?;
    output.success(&info, &info_lines(&info));
    Ok(())
}

/// Asks a running service to shut down and waits until its profile lock is free.
/// Returns `false` when nothing was running.
async fn stop_service(profile: &Profile) -> Result<bool, CliError> {
    let deadline = Instant::now() + STOP_TIMEOUT;
    let mut requested = false;
    loop {
        if !requested && let Some(client) = connect_running(profile).await? {
            request_shutdown(&client).await?;
            requested = true;
        }
        if lock_is_free(&profile.lock_path())? {
            return Ok(requested);
        }
        if Instant::now() >= deadline {
            return Err(CliError::new(
                "service.stop_timeout",
                "the service did not stop within 15 seconds",
                exit::ERROR,
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Sends `service.shutdown`. A connection the service drops while it stops still counts as accepted.
async fn request_shutdown(client: &Client) -> Result<(), CliError> {
    match client.shutdown().await {
        Ok(()) | Err(ClientError::Disconnected) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// `true` when no service holds the profile lock. A service releases it after it stops listening.
fn lock_is_free(path: &Path) -> Result<bool, CliError> {
    let file: File = match OpenOptions::new().read(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(error.into()),
    };
    match file.try_lock() {
        Ok(()) => {
            file.unlock()?;
            Ok(true)
        }
        Err(TryLockError::WouldBlock) => Ok(false),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

/// The lines `service info` prints: one `key: value` per line.
fn info_lines(info: &ServiceInfo) -> Vec<String> {
    vec![
        format!("profile: {}", info.profile),
        format!("pid: {}", info.pid),
        format!("version: {}", info.version),
        format!("protocol: {}", info.protocol),
        format!("capabilities: {}", info.capabilities.join(", ")),
        format!("started_at_ms: {}", info.started_at_ms),
        format!("git: {}", git_line(info.git.as_ref())),
    ]
}

/// The host's git in one phrase. A service that predates the field is named, so a reply without it is not read as "no git".
fn git_line(git: Option<&GitStatus>) -> String {
    match git {
        Some(GitStatus::Present { version }) => format!("present {version}"),
        Some(GitStatus::Missing) => "missing".to_owned(),
        Some(GitStatus::TooOld { version, required }) => {
            format!("too_old {version} (requires {required})")
        }
        Some(GitStatus::Unknown { reason }) => format!("unknown ({reason})"),
        None => "not reported (this service predates git status; update the service)".to_owned(),
    }
}
