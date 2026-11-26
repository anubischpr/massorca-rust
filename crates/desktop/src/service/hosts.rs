//! The hosts this computer paired with (decision 0046), and the calls that add one: an SSH host (decision 0044) or the
//! server of an offer (decision 0041). Each paired host is followed by its live link (REM-07), whatever this
//! computer's service is doing; pairing and an SSH add open their own connection, then hand the host to its link.

use std::path::{Path, PathBuf};

use massorca_client::{
    Client, ClientError, DeviceRevocation, HostRecord, Hosts, default_device_name,
};
use massorca_core_remote::bootstrap::{
    BootstrapError, BootstrapPlan, BootstrapReport, DEFAULT_LISTEN, RemoteMachine,
    install_over_ssh, probe_over_ssh,
};
use massorca_core_remote::ssh::SshError;
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::pairing::Offer;
use tokio::sync::{mpsc, oneshot};

use super::bridge::{BRIDGE_STOPPED, Command, ServiceHandle};
use super::live_hosts::LiveHosts;
use super::refusal::{Refusal, sentence};
use super::update::StoreUpdate;

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

/// What removing a host did (D-HO-03): the device on the host, and where its old files wait when they could not be
/// deleted at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRemoval {
    pub host: String,
    pub device: DeviceRevocation,
    pub leftover: Option<PathBuf>,
}

/// What the remove confirm counts on a host before it is forgotten: its projects and its agent sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLoss {
    pub projects: usize,
    pub sessions: usize,
}

/// Runs a host call on its own task. Host calls need no connection to this computer's service.
pub(super) fn spawn(
    profile: Profile,
    command: Command,
    updates: mpsc::UnboundedSender<StoreUpdate>,
    live: &LiveHosts,
) {
    match command {
        Command::RefreshHosts => follow_paired(&profile, live, &updates),
        Command::ReconnectHost { name } => {
            live.reconnect(&name);
        }
        Command::AddSshHost { host, reply } => {
            let live = live.clone();
            tokio::spawn(async move {
                let result = add_ssh(&profile, host, &updates, &live).await;
                let _ = reply.send(result);
            });
        }
        Command::AcceptOffer { offer, name, reply } => {
            let live = live.clone();
            tokio::spawn(async move {
                let result = accept_offer(&profile, &offer, name, &updates, &live).await;
                let _ = reply.send(result);
            });
        }
        Command::RemoveHost { name, reply } => {
            let live = live.clone();
            tokio::spawn(async move {
                let result = remove(&profile, &name, &updates, &live).await;
                let _ = reply.send(result);
            });
        }
        Command::CountHostLoss { name, reply } => {
            let live = live.clone();
            tokio::spawn(async move {
                let _ = reply.send(count_loss(&live, &name).await);
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
    /// Re-reads the paired hosts and follows each one live. The store changes through the updates, not this call.
    pub fn refresh_hosts(&self) {
        self.send(Command::RefreshHosts);
    }

    /// Tries one paired host again at once (Retry now), leaving the other hosts as they are. A connected host is
    /// connected afresh, which a sleep or a network change may need.
    pub fn reconnect_host(&self, name: String) {
        self.send(Command::ReconnectHost { name });
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

    /// Forgets a host, as `massorca host remove` does: this device is revoked there when the host answers, then the
    /// record and device key are deleted here. Nothing else on the host changes.
    pub async fn remove_host(&self, name: String) -> Result<HostRemoval, Refusal> {
        self.ask(|reply| Command::RemoveHost { name, reply }).await
    }

    /// Counts the projects and sessions on a host, for the remove confirm. A host that does not answer is refused.
    pub async fn count_host_loss(&self, name: String) -> Result<HostLoss, Refusal> {
        self.ask(|reply| Command::CountHostLoss { name, reply })
            .await
    }
}

/// The device is revoked first and a failed revoke never stops the removal, as in `host remove`. The host's live link
/// stops before the revoke, so the connection the revoke ends is not dialed again, and everything that borrowed it
/// stops too. A removal that fails gives the host its link back.
async fn remove(
    profile: &Profile,
    name: &str,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    live: &LiveHosts,
) -> Result<HostRemoval, Refusal> {
    let hosts = Hosts::of(profile);
    hosts.get(name).map_err(|error| Refusal::from(&error))?;
    live.stop(name);
    let device = hosts.revoke_this_device(name, &peer()).await;
    let removed = match hosts.remove(name) {
        Ok(removed) => removed,
        Err(error) => {
            follow_paired(profile, live, updates);
            return Err(Refusal::from(&error));
        }
    };
    if let Ok(records) = hosts.list() {
        let _ = updates.send(StoreUpdate::HostsResync(records));
    }
    Ok(HostRemoval {
        host: name.to_owned(),
        device,
        leftover: removed.leftover,
    })
}

/// Counted on the host's live connection.
async fn count_loss(live: &LiveHosts, name: &str) -> Result<HostLoss, Refusal> {
    let client = live
        .client(name)
        .await
        .map_err(|error| Refusal::from(&*error))?;
    let projects = client
        .project_list()
        .await
        .map_err(|error| Refusal::from(&error))?;
    let sessions = client
        .agent_list(None)
        .await
        .map_err(|error| Refusal::from(&error))?;
    Ok(HostLoss {
        projects: projects.projects.len(),
        sessions: sessions.sessions.len(),
    })
}

/// Lists the hosts and follows each one live: a new host gets its link, a forgotten one loses it, and a link that runs
/// keeps its state. Each link reports its state as it changes.
pub(super) fn follow_paired(
    profile: &Profile,
    live: &LiveHosts,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let Ok(records) = Hosts::of(profile).list() else {
        return;
    };
    let _ = updates.send(StoreUpdate::HostsResync(records.clone()));
    live.follow(records.into_iter().map(|record| record.name));
}

/// Tells the store the hosts as they are now, and has the live link of `name` try again at once: it was just paired,
/// re-added or upgraded. Its row follows that link alone, so a later loss of contact shows.
fn announce(
    hosts: &Hosts,
    name: &str,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    live: &LiveHosts,
) {
    if let Ok(records) = hosts.list() {
        let _ = updates.send(StoreUpdate::HostsResync(records.clone()));
        live.follow_again(records.into_iter().map(|record| record.name), name);
    }
}

async fn accept_offer(
    profile: &Profile,
    text: &str,
    name: Option<String>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    live: &LiveHosts,
) -> Result<HostRecord, Refusal> {
    let offer = Offer::decode(text).map_err(|error| Refusal::from(&error))?;
    let name = name.unwrap_or_else(|| offer.server.clone());
    let hosts = Hosts::of(profile);
    let (client, record) = Client::pair(&hosts, &offer, &name, &default_device_name(), &peer())
        .await
        .map_err(|error| Refusal::from(&error))?;
    drop(client);
    announce(&hosts, &record.name, updates, live);
    Ok(record)
}

async fn add_ssh(
    profile: &Profile,
    host: SshHost,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    live: &LiveHosts,
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
        drop(client);
        let record = hosts.get(&name).map_err(|error| SshFailed {
            refusal: Refusal::from(&error),
            log: log.clone(),
        })?;
        announce(&hosts, &name, updates, live);
        return Ok(SshAdded::Upgraded(record));
    }

    let (client, record) = Client::pair(&hosts, &offer, &name, &device_name, &peer())
        .await
        .map_err(|error| SshFailed {
            refusal: after_install(&host.target, &error),
            log: log.clone(),
        })?;
    drop(client);
    announce(&hosts, &name, updates, live);
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
