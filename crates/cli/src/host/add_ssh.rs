//! `massorca host add-ssh`: installs or upgrades the service on a Linux host over SSH, then pairs with it (decision 0044).

use std::path::PathBuf;

use massorca_client::{
    Client, HostRecord, Hosts, SshInstall, default_device_name, validate_host_name,
};
use massorca_core_remote::bootstrap::{BootstrapPlan, BootstrapReport, install_over_ssh};
use massorca_profile::Profile;
use massorca_protocol::pairing::Offer;
use serde::Serialize;

use crate::args::HostAddSshArgs;
use crate::connect::peer;
use crate::cwd_path;
use crate::error::CliError;
use crate::host::records::record_lines;
use crate::host::status::{self, HostStatus};
use crate::output::Output;
use crate::service::unit::listen_addresses;

/// What `host add-ssh` did: the installed binary and unit, and the host it paired or upgraded.
#[derive(Debug, Serialize)]
pub struct InstallReport {
    pub target: String,
    pub remote_os: String,
    pub remote_arch: String,
    pub remote_binary: String,
    pub unit_path: String,
    pub server_version: String,
    /// `paired` for a new host, `upgraded` when a host of that name already existed.
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<HostRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<HostStatus>,
}

/// Installs or upgrades massorca on the SSH host, then pairs this computer with it as host `--name`.
/// A host that already exists is not paired again: it is checked and reported as upgraded.
pub async fn execute(
    profile: &Profile,
    args: &HostAddSshArgs,
    output: Output,
) -> Result<(), CliError> {
    if let Some(name) = &args.name {
        validate_host_name(name)?;
    }
    let listen = listen_addresses(&args.listen)?;
    let device_name = args.device_name.clone().unwrap_or_else(default_device_name);
    let plan = BootstrapPlan::for_ssh_host(
        PathBuf::from(cwd_path::absolute(&args.binary)?),
        listen,
        device_name.clone(),
    );
    let installed = run_bootstrap(args.target.clone(), args.ssh_arg.clone(), plan, output).await?;
    let offer = Offer::decode(installed.offer.expose())?;
    let name = args.name.clone().unwrap_or_else(|| offer.server.clone());
    let hosts = Hosts::of(profile);
    if hosts.exists(&name) {
        return upgrade(&hosts, &name, args, &installed, output).await;
    }
    let paired = Client::pair(&hosts, &offer, &name, &device_name, &peer()).await;
    let (client, record) = paired.map_err(|error| {
        let mut error = CliError::from(error);
        error.message = format!(
            "massorca was installed on {}, but pairing failed: {}",
            args.target, error.message
        );
        error
    })?;
    drop(client);
    note_install(&hosts, &name, args, output);
    let mut lines = install_lines(&installed, &args.target);
    lines.extend(record_lines(&record));
    let report = install_report(&args.target, &installed, "paired", Some(record), None);
    output.success(&report, &lines);
    Ok(())
}

/// The host already exists: it is checked, and nothing is paired again.
async fn upgrade(
    hosts: &Hosts,
    name: &str,
    args: &HostAddSshArgs,
    installed: &BootstrapReport,
    output: Output,
) -> Result<(), CliError> {
    match status::check(hosts, name).await {
        Ok(connected) => {
            note_install(hosts, name, args, output);
            let state = status::status_of(name, Ok(connected));
            let mut lines = install_lines(installed, &args.target);
            lines.push(format!(
                "host {name} is already paired: no new pairing needed; the offer made for this run expires unused"
            ));
            lines.push(status::status_line(&state));
            let report = install_report(&args.target, installed, "upgraded", None, Some(state));
            output.success(&report, &lines);
            Ok(())
        }
        Err(error) => {
            let mut error = CliError::from(error);
            error.message = format!(
                "massorca was upgraded on {}, but host {name} did not answer: {}",
                args.target, error.message
            );
            Err(error)
        }
    }
}

/// Records how host `name` was installed, for the self-test's log hint. A note that cannot be written is only a warning:
/// the host is installed and paired either way.
fn note_install(hosts: &Hosts, name: &str, args: &HostAddSshArgs, output: Output) {
    let install = SshInstall {
        ssh_target: args.target.clone(),
        ssh_args: args.ssh_arg.clone(),
    };
    if let Err(error) = hosts.note_ssh_install(name, &install) {
        output.progress(&format!(
            "warning: the install target of host {name} was not recorded: {error}"
        ));
    }
}

/// Installs or upgrades the service on the host, on a blocking thread. Progress goes to stderr in text mode.
async fn run_bootstrap(
    target: String,
    ssh_args: Vec<String>,
    plan: BootstrapPlan,
    output: Output,
) -> Result<BootstrapReport, CliError> {
    tokio::task::spawn_blocking(move || {
        let mut progress = |line: &str| output.progress(line);
        install_over_ssh(&target, ssh_args, &plan, &mut progress)
    })
    .await?
    .map_err(CliError::from)
}

fn install_lines(installed: &BootstrapReport, target: &str) -> Vec<String> {
    let version = if installed.server_version.is_empty() {
        "unknown"
    } else {
        installed.server_version.as_str()
    };
    vec![
        format!(
            "installed massorca on {target} ({} {})",
            installed.remote_os, installed.remote_arch
        ),
        format!("binary: {}", installed.remote_binary),
        format!("unit: {}", installed.unit_path),
        format!("service version: {version}"),
    ]
}

fn install_report(
    target: &str,
    installed: &BootstrapReport,
    outcome: &'static str,
    host: Option<HostRecord>,
    status: Option<HostStatus>,
) -> InstallReport {
    InstallReport {
        target: target.to_owned(),
        remote_os: installed.remote_os.clone(),
        remote_arch: installed.remote_arch.clone(),
        remote_binary: installed.remote_binary.clone(),
        unit_path: installed.unit_path.clone(),
        server_version: installed.server_version.clone(),
        outcome,
        host,
        status,
    }
}
