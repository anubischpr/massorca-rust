//! `massorca host list|show|remove`: the hosts this client keeps, and forgetting one (decision 0046).

use massorca_client::{DeviceRevocation, HostRecord, Hosts};
use massorca_core_remote::bootstrap::{self, DEFAULT_REMOTE_DIR};
use massorca_core_remote::ssh::SystemSsh;
use massorca_profile::Profile;
use serde::Serialize;

use crate::args::HostRemoveArgs;
use crate::connect::peer;
use crate::error::CliError;
use crate::output::Output;
use crate::time_text;
use crate::workspace_text;

/// The hosts of this client profile.
#[derive(Debug, Serialize)]
pub struct HostList {
    pub hosts: Vec<HostRecord>,
}

/// What `host remove` did: the device on the server, and the SSH uninstall when one was asked for.
#[derive(Debug, Serialize)]
pub struct Removal {
    pub host: String,
    pub device: DeviceOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uninstalled_from: Option<String>,
    /// Where the host's old files wait when they could not be deleted at once; the next host change deletes them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leftover: Option<String>,
}

/// What became of this device on its server.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeviceOutcome {
    /// The server revoked this device.
    Revoked,
    /// `--keep-device`: the device stays paired on the server.
    Kept,
    /// The server still lists this device as paired: the host did not answer, or refused it.
    NotRevoked { reason: String },
}

/// `host list`: every host with its addresses, device and pairing time.
pub fn list(profile: &Profile, output: Output) -> Result<(), CliError> {
    let hosts = Hosts::of(profile).list()?;
    let lines = if hosts.is_empty() {
        vec!["no hosts".to_owned()]
    } else {
        host_table(&hosts)
    };
    output.success(&HostList { hosts }, &lines);
    Ok(())
}

/// `host show`: one host in full.
pub fn show(profile: &Profile, name: &str, output: Output) -> Result<(), CliError> {
    let record = Hosts::of(profile).get(name)?;
    let lines = record_lines(&record);
    output.success(&record, &lines);
    Ok(())
}

/// `host remove`: forgets a host. Unless `--keep-device`, this device is revoked on the server first, when the host answers.
pub async fn remove(
    profile: &Profile,
    args: &HostRemoveArgs,
    output: Output,
) -> Result<(), CliError> {
    let hosts = Hosts::of(profile);
    let record = hosts.get(&args.name)?;
    let device = if args.keep_device {
        DeviceOutcome::Kept
    } else {
        match hosts.revoke_this_device(&record.name, &peer()).await {
            DeviceRevocation::Revoked => DeviceOutcome::Revoked,
            DeviceRevocation::NotRevoked(reason) => DeviceOutcome::NotRevoked { reason },
        }
    };
    // A failed uninstall returns before the host is forgotten, so the same command can be run again.
    let uninstalled_from = match (args.uninstall, &args.ssh) {
        (true, Some(target)) => {
            uninstall_over_ssh(target).await?;
            Some(target.clone())
        }
        _ => None,
    };
    let removed = hosts.remove(&args.name)?;
    let removal = Removal {
        host: args.name.clone(),
        device,
        uninstalled_from,
        leftover: removed.leftover.map(|path| path.display().to_string()),
    };
    let lines = removal_lines(&removal);
    output.success(&removal, &lines);
    Ok(())
}

/// A host in full, as text lines.
pub fn record_lines(record: &HostRecord) -> Vec<String> {
    let mut lines = vec![
        format!("host: {}", record.name),
        format!("server: {}", record.server),
    ];
    lines.extend(
        record
            .addresses
            .iter()
            .map(|address| format!("address: {address}")),
    );
    lines.push(format!("pin: {}", record.pin));
    lines.push(format!(
        "device: {} ({})",
        record.device_name, record.device_id
    ));
    lines.push(format!("scopes: {}", record.scopes.join(", ")));
    lines.push(format!(
        "paired: {}",
        time_text::utc_minute(record.paired_at_ms)
    ));
    lines
}

fn host_table(hosts: &[HostRecord]) -> Vec<String> {
    let rows = hosts
        .iter()
        .map(|host| {
            vec![
                host.name.clone(),
                host.addresses.join(", "),
                host.device_name.clone(),
                time_text::utc_minute(host.paired_at_ms),
            ]
        })
        .collect();
    workspace_text::table(&["NAME", "ADDRESSES", "DEVICE", "PAIRED"], rows)
}

fn removal_lines(removal: &Removal) -> Vec<String> {
    let mut lines = vec![format!("removed host {}", removal.host)];
    lines.push(match &removal.device {
        DeviceOutcome::Revoked => "device revoked on the server".to_owned(),
        DeviceOutcome::Kept => "device kept on the server (--keep-device)".to_owned(),
        DeviceOutcome::NotRevoked { reason } => {
            format!("device not revoked on the server: {reason}")
        }
    });
    if let Some(target) = &removal.uninstalled_from {
        lines.push(format!("service uninstalled over ssh: {target}"));
    }
    if let Some(leftover) = &removal.leftover {
        lines.push(format!(
            "its old files could not be deleted yet; they were moved to {leftover}, which the next host change deletes"
        ));
    }
    lines
}

/// Stops the service on the SSH host and removes its install directory (decision 0045).
async fn uninstall_over_ssh(target: &str) -> Result<(), CliError> {
    let target = target.to_owned();
    tokio::task::spawn_blocking(move || -> Result<(), CliError> {
        let shell = SystemSsh::new(&target, Vec::new())?;
        bootstrap::uninstall(&shell, DEFAULT_REMOTE_DIR)?;
        Ok(())
    })
    .await?
}
