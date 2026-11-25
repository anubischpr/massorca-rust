//! The hosts this computer paired with (decision 0046), and the calls that add one: an SSH host (decision 0044) or the
//! server of an offer (decision 0041). Each call opens its own connection, so the hosts work while this computer's
//! service is down.

use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_client::{Client, ClientError, HostRecord, Hosts, default_device_name};
use massorca_core_remote::bootstrap::{
    BootstrapError, BootstrapPlan, BootstrapReport, DEFAULT_LISTEN, RemoteMachine,
    install_over_ssh, probe_over_ssh,
};
use massorca_core_remote::ssh::SshError;
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::pairing::Offer;
use tokio::sync::{mpsc, oneshot};
use tokio::time::MissedTickBehavior;

use super::bridge::{BRIDGE_STOPPED, Command, ServiceHandle};
use super::host_sessions::{Followed, connected, follow};
use super::refusal::{Refusal, sentence};
use super::update::{HostLink, StoreUpdate};

/// Who this desktop says it is to a host it connects to.
pub(super) fn peer() -> PeerInfo {
    PeerInfo {
        name: "massorca-desktop".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// An SSH host to install the service on, and then pair with (decision 0044).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshHost {
    /// `[user@]host`.
    pub target: String,
    /// Extra ssh options, one entry each: `-p` and then `2222`.
    pub ssh_args: Vec<String>,
    /// The Linux `massorca` program to upload, for x86_64 or aarch64.
    pub binary: PathBuf,
    /// The host's name on this computer. The server's own name when absent.
    pub name: Option<String>,
    /// The name this computer has on the host. This computer's name when absent.
    pub device_name: Option<String>,
}

/// What an SSH add did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SshAdded {
    /// The service was installed and this computer paired with it.
    Paired(HostRecord),
    /// A host of that name already existed: the service was upgraded and nothing was paired again.
    Upgraded(HostRecord),
}

/// An SSH add that did not finish: why, and the steps the install reported, for the connection log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshFailed {
    pub refusal: Refusal,
    pub log: Vec<String>,
}

/// Runs a host call on its own task. Host calls need no connection to this computer's service.
pub(super) fn spawn(
    profile: Profile,
    command: Command,
    updates: mpsc::UnboundedSender<StoreUpdate>,
    followed: &Followed,
) {
    match command {
        Command::RefreshHosts => refresh(&profile, &updates, followed),
        Command::AddSshHost { host, reply } => {
            tokio::spawn(async move {
                let result = add_ssh(&profile, host, &updates).await;
                let _ = reply.send(result);
            });
        }
        Command::AcceptOffer { offer, name, reply } => {
            tokio::spawn(async move {
                let result = accept_offer(&profile, &offer, name, &updates).await;
                let _ = reply.send(result);
            });
        }
        Command::ProbeSshHost {
            target,
            ssh_args,
            reply,
        } => {
            tokio::spawn(async move {
                let result = probe_ssh(target, ssh_args).await;
                let _ = reply.send(result);
            });
        }
        _ => {}
    }
}

impl ServiceHandle {
    /// Re-reads the paired hosts and checks each one. The store changes through the updates, not this call.
    pub fn refresh_hosts(&self) {
        self.send(Command::RefreshHosts);
    }

    /// Installs the service on an SSH host and pairs with it, as `massorca host add-ssh` does.
    pub async fn add_ssh_host(&self, host: SshHost) -> Result<SshAdded, SshFailed> {
        let stopped = || SshFailed {
            refusal: Refusal::unavailable(BRIDGE_STOPPED),
            log: Vec::new(),
        };
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command::AddSshHost { host, reply })
            .map_err(|_| stopped())?;
        answer.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Checks an SSH host's machine with `uname -sm`, installing nothing: the Add host dialog's test.
    pub async fn probe_ssh_host(
        &self,
        target: String,
        ssh_args: Vec<String>,
    ) -> Result<RemoteMachine, SshFailed> {
        let stopped = || SshFailed {
            refusal: Refusal::unavailable(BRIDGE_STOPPED),
            log: Vec::new(),
        };
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command::ProbeSshHost {
                target,
                ssh_args,
                reply,
            })
            .map_err(|_| stopped())?;
        answer.await.unwrap_or_else(|_| Err(stopped()))
    }

    /// Pairs with the server of an offer and keeps it as a host, named `name` or the server's own name.
    pub async fn accept_offer(
        &self,
        offer: String,
        name: Option<String>,
    ) -> Result<HostRecord, Refusal> {
        self.ask(|reply| Command::AcceptOffer { offer, name, reply })
            .await
    }
}

/// Lists the hosts, then checks each one that no task follows at once; each check reports when it finishes. A host
/// a task follows reports through that task's own connection, so it is not checked again here.
fn refresh(profile: &Profile, updates: &mpsc::UnboundedSender<StoreUpdate>, followed: &Followed) {
    let hosts = Hosts::of(profile);
    let Ok(records) = hosts.list() else {
        return;
    };
    let _ = updates.send(StoreUpdate::HostsResync(records.clone()));
    for record in records {
        if let Some(claim) = followed.claim(&record.name) {
            tokio::spawn(follow(profile.clone(), record.name, claim, updates.clone()));
        }
    }
}

/// Checks the paired hosts again every `HOST_CHECK_EVERY` while the bridge runs. This is the fallback for a host
/// with no live event subscription: a host that a task follows is left to that task.
pub(super) fn check_hosts_again(
    profile: Profile,
    updates: mpsc::UnboundedSender<StoreUpdate>,
    followed: Followed,
) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(HOST_CHECK_EVERY);
        every.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            every.tick().await;
            if updates.is_closed() {
                return;
            }
            refresh(&profile, &updates, &followed);
        }
    });
}

/// How often the paired hosts without a live event subscription are checked again (STAT-16, STAT-17).
const HOST_CHECK_EVERY: Duration = Duration::from_secs(30);

/// Tells the store the hosts as they are now, then what this call found about the host it changed.
fn announce(
    hosts: &Hosts,
    name: &str,
    link: HostLink,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    if let Ok(records) = hosts.list() {
        let _ = updates.send(StoreUpdate::HostsResync(records));
    }
    let _ = updates.send(StoreUpdate::HostLinkChanged {
        name: name.to_owned(),
        link,
    });
}

async fn accept_offer(
    profile: &Profile,
    text: &str,
    name: Option<String>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) -> Result<HostRecord, Refusal> {
    let offer = Offer::decode(text).map_err(|error| Refusal::from(&error))?;
    let name = name.unwrap_or_else(|| offer.server.clone());
    let hosts = Hosts::of(profile);
    let (client, record) = Client::pair(&hosts, &offer, &name, &default_device_name(), &peer())
        .await
        .map_err(|error| Refusal::from(&error))?;
    let link = connected(&client);
    drop(client);
    announce(&hosts, &record.name, link, updates);
    Ok(record)
}

async fn add_ssh(
    profile: &Profile,
    host: SshHost,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) -> Result<SshAdded, SshFailed> {
    let device_name = host.device_name.clone().unwrap_or_else(default_device_name);
    let plan = BootstrapPlan::for_ssh_host(
        host.binary.clone(),
        vec![DEFAULT_LISTEN.to_owned()],
        device_name.clone(),
    );
    let (target, ssh_args) = (host.target.clone(), host.ssh_args.clone());
    let (installed, mut log) = tokio::task::spawn_blocking(move || {
        let mut log = Vec::new();
        let installed = install_over_ssh(&target, ssh_args, &plan, &mut |line: &str| {
            log.push(line.to_owned());
        });
        (installed, log)
    })
    .await
    .map_err(|error| SshFailed {
        refusal: Refusal::Failed {
            code: "client.failed".to_owned(),
            message: error.to_string(),
        },
        log: Vec::new(),
    })?;
    let installed = installed.map_err(|error| SshFailed {
        refusal: bootstrap_refusal(&error),
        log: log.clone(),
    })?;
    log.extend(install_lines(&installed, &host.target));
    let offer = Offer::decode(installed.offer.expose()).map_err(|error| SshFailed {
        refusal: Refusal::from(&error),
        log: log.clone(),
    })?;

    let name = host.name.clone().unwrap_or_else(|| offer.server.clone());
    let hosts = Hosts::of(profile);
    if hosts.exists(&name) {
        // A host that exists is checked and not paired again, as `host add-ssh` does.
        log.push(format!(
            "host {name} is already paired: no new pairing needed; the offer made for this run expires unused"
        ));
        let client = Client::connect_host(&hosts, &name, &peer())
            .await
            .map_err(|error| SshFailed {
                refusal: Refusal::from(&error),
                log: log.clone(),
            })?;
        let link = connected(&client);
        drop(client);
        let record = hosts.get(&name).map_err(|error| SshFailed {
            refusal: Refusal::from(&error),
            log: log.clone(),
        })?;
        announce(&hosts, &name, link, updates);
        return Ok(SshAdded::Upgraded(record));
    }

    let (client, record) = Client::pair(&hosts, &offer, &name, &device_name, &peer())
        .await
        .map_err(|error| SshFailed {
            refusal: after_install(&host.target, &error),
            log: log.clone(),
        })?;
    let link = connected(&client);
    drop(client);
    announce(&hosts, &name, link, updates);
    Ok(SshAdded::Paired(record))
}

/// Checks an SSH host over ssh with `uname -sm`, installing nothing. Failures keep the step's own code and sentence.
async fn probe_ssh(target: String, ssh_args: Vec<String>) -> Result<RemoteMachine, SshFailed> {
    let named = target.clone();
    tokio::task::spawn_blocking(move || probe_over_ssh(&target, ssh_args))
        .await
        .map_err(|error| SshFailed {
            refusal: Refusal::Failed {
                code: "client.failed".to_owned(),
                message: error.to_string(),
            },
            log: Vec::new(),
        })?
        .map_err(|error| SshFailed {
            refusal: bootstrap_refusal(&error),
            log: vec![format!("could not check {named}"), error.to_string()],
        })
}

/// The Linux `massorca` program packaged for `arch`: `resources/massorca-linux-<arch>` next to this executable
/// (decision 0075). None when the app has no program for that machine.
pub fn bundled_program(arch: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    bundled_program_in(exe.parent()?, arch)
}

fn bundled_program_in(folder: &Path, arch: &str) -> Option<PathBuf> {
    if !matches!(arch, "x86_64" | "aarch64") {
        return None;
    }
    let program = folder
        .join("resources")
        .join(format!("massorca-linux-{arch}"));
    program.is_file().then_some(program)
}

/// The CLI's lines about an installed service, in the same words.
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

/// A failed install, with the code of the step that failed.
fn bootstrap_refusal(error: &BootstrapError) -> Refusal {
    let code = match error {
        BootstrapError::Ssh(SshError::NotFound) => "ssh.not_found",
        BootstrapError::Ssh(SshError::Refused(_)) => "usage.invalid_ssh_target",
        BootstrapError::Ssh(SshError::Spawn(_)) => "ssh.failed",
        BootstrapError::Ssh(SshError::Failed { .. } | SshError::TimedOut(_)) => "host.unreachable",
        BootstrapError::UnsupportedSystem(_) => "host.unsupported_system",
        BootstrapError::BinaryMismatch(_) => "host.binary_mismatch",
        BootstrapError::Remote(_) => "host.bootstrap_failed",
        BootstrapError::InvalidPlan(_) => "usage.invalid_plan",
        BootstrapError::Io(_) => "io.failed",
    };
    Refusal::Failed {
        code: code.to_owned(),
        message: error.to_string(),
    }
}

/// The CLI's sentence for a pairing that failed after the install worked.
fn after_install(target: &str, error: &ClientError) -> Refusal {
    let refusal = Refusal::from(error);
    let code = refusal.code().unwrap_or("client.failed").to_owned();
    Refusal::Failed {
        code,
        message: sentence(&format!(
            "massorca was installed on {target}, but pairing failed: {}",
            refusal.message()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_program_is_the_one_packaged_for_the_machine() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let resources = folder.path().join("resources");
        std::fs::create_dir(&resources).expect("a resources folder");
        std::fs::write(resources.join("massorca-linux-x86_64"), b"program").expect("a program");
        assert_eq!(
            bundled_program_in(folder.path(), "x86_64"),
            Some(resources.join("massorca-linux-x86_64"))
        );
        assert_eq!(
            bundled_program_in(folder.path(), "aarch64"),
            None,
            "a program for one machine is not offered for another"
        );
    }

    #[test]
    fn no_bundled_program_is_offered_for_a_machine_massorca_does_not_run_on() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        assert_eq!(bundled_program_in(folder.path(), "armv7l"), None);
    }
}
