//! Enable, disable and remove, with the remove guards and cleanups other features register.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use massorca_ext_package::manifest::Manifest;
use serde::Serialize;

use crate::error::{Error, HookError, codes};
use crate::hooks::{HookHandle, HookKind, HookRun, RemovedPackage};
use crate::host::INSTALLER_TIMEOUT;
use crate::install_log_tail::last_log_lines_after;
use crate::lockfile::{LockCli, LockEntry};
use crate::review::uninstall_fingerprint;
use crate::run_log::RunAction;
use crate::run_log::log_len;
use crate::service::{Extensions, describe};
use massorca_ext_package::manifest::agent::CliOs;

/// The result of `set_enabled`. `changed` is false when the package was already in that state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnabledOutcome {
    pub id: String,
    pub enabled: bool,
    pub changed: bool,
}

/// The result of `remove`. The package is gone; anything listed in `failures` was not undone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Removed {
    pub id: String,
    pub version: String,
    pub failures: Vec<Failure>,
    pub cli: Vec<CliRemoval>,
}

/// A step after the lockfile commit that failed: a file deletion (owner `extensions`) or a registered cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failure {
    pub owner: String,
    pub code: String,
    pub message: String,
}

/// What removal did about one agent CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliRemoval {
    pub agent: String,
    pub binary: String,
    #[serde(flatten)]
    pub action: CliAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CliAction {
    /// The package did not install this CLI, so it is left alone.
    LeftAlone,
    /// The package installed it, but declares no uninstall command for this OS.
    NoCommand,
    /// The uninstall command was not asked for; the Danger Zone can offer it.
    Offered { command: Vec<String> },
    /// The uninstall command ran and exited 0.
    Ran {
        command: Vec<String>,
        exit_code: Option<i32>,
    },
    /// The uninstall command ran and failed. Nothing is rolled back.
    Failed {
        command: Vec<String>,
        message: String,
    },
}

impl Extensions {
    /// Registers a guard that may refuse disabling or removing a package, such as `agent.busy`. It must have no side
    /// effects.
    pub fn register_remove_guard(
        &self,
        owner: impl Into<String>,
        guard: impl Fn(&str) -> Result<(), HookError> + Send + Sync + 'static,
    ) -> HookHandle {
        self.hooks
            .register(owner.into(), HookRun::Guard(Arc::new(guard)))
    }

    /// Registers a cleanup that runs after a removal, only when `delete_agent_homes` is asked for. It runs after the
    /// package's files are deleted, so it is given what the package provided (decision 0098).
    pub fn register_remove_cleanup(
        &self,
        owner: impl Into<String>,
        cleanup: impl Fn(&RemovedPackage) -> Result<(), HookError> + Send + Sync + 'static,
    ) -> HookHandle {
        self.hooks
            .register(owner.into(), HookRun::Cleanup(Arc::new(cleanup)))
    }

    /// Registers a listener told of every committed removal, whatever `delete_agent_homes` says, such as the extension
    /// points dropping what the package registered (decision 0153). It runs after the package's files are deleted.
    pub fn register_remove_listener(
        &self,
        owner: impl Into<String>,
        listener: impl Fn(&RemovedPackage) + Send + Sync + 'static,
    ) -> HookHandle {
        self.hooks
            .register(owner.into(), HookRun::Listener(Arc::new(listener)))
    }

    pub(crate) fn run_guards(&self, id: &str) -> Result<(), Error> {
        for (owner, run) in self.hooks.of(HookKind::Guard) {
            if let HookRun::Guard(guard) = run {
                guard(id).map_err(|error| Error::Refused { owner, error })?;
            }
        }
        Ok(())
    }

    /// Enables or disables an installed package. Disabling asks the remove guards first; files and data stay.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<EnabledOutcome, Error> {
        let _changes = self.lock_changes();
        let mut lockfile = self.read_lockfile()?;
        self.refuse_if_busy(id)?;
        let entry = lockfile
            .get_mut(id)
            .ok_or_else(|| Error::NotInstalled(id.to_owned()))?;
        let changed = entry.enabled != enabled;
        if changed {
            if !enabled {
                self.run_guards(id)?;
            }
            entry.enabled = enabled;
            lockfile.write(&self.layout.lock_path())?;
        }
        Ok(EnabledOutcome {
            id: id.to_owned(),
            enabled,
            changed,
        })
    }

    /// Removes a package: guards, then the lockfile entry and its files, then the cleanups when `delete_agent_homes`,
    /// then the uninstall command of each CLI this package installed when `uninstall_cli`.
    pub fn remove(
        &self,
        id: &str,
        delete_agent_homes: bool,
        uninstall_cli: bool,
        uninstall_consent: Option<&str>,
    ) -> Result<Removed, Error> {
        // The change lock covers the lockfile and the files. The uninstall commands run after it is released, with the
        // package busy, so other packages can change meanwhile.
        let (entry, manifest, failures, _busy) = {
            let _changes = self.lock_changes();
            self.refuse_if_busy(id)?;
            let mut lockfile = self.read_lockfile()?;
            let entry = lockfile
                .get(id)
                .cloned()
                .ok_or_else(|| Error::NotInstalled(id.to_owned()))?;
            // A package whose files are damaged can still be removed; its CLI uninstall is then unknown.
            let manifest = self.installed_manifest(&entry);
            // When the files cannot be read, the kinds they provided are not guessed: their accounts are left (0098).
            let unread = manifest.as_ref().err().map(|error| error.code().to_owned());
            let manifest = manifest.ok();
            if uninstall_cli {
                // The uninstall runs only for the commands the person accepted (0062, V2): nothing changes without them.
                let commands = pending_uninstalls(&entry, manifest.as_ref(), self.os);
                if !commands.is_empty() {
                    let fingerprint = uninstall_fingerprint(&commands);
                    match uninstall_consent {
                        None => {
                            return Err(Error::UninstallConsentRequired {
                                commands,
                                fingerprint,
                            });
                        }
                        Some(given) if given != fingerprint => return Err(Error::ConsentMismatch),
                        Some(_) => {}
                    }
                }
            }
            self.run_guards(id)?;
            // Read before the files go: the cleanups run after, and need the kinds this package provided (0098).
            let removed = RemovedPackage {
                id: id.to_owned(),
                agent_kinds: provided_kinds(&entry, manifest.as_ref()),
            };
            lockfile.packages.retain(|package| package.id != id);
            lockfile.write(&self.layout.lock_path())?;
            let failures = self.delete_package(&removed, delete_agent_homes, unread.as_deref());
            for (_, run) in self.hooks.of(HookKind::Listener) {
                if let HookRun::Listener(listener) = run {
                    listener(&removed);
                }
            }
            let busy = self.mark_busy(id, Vec::new())?;
            (entry, manifest, failures, busy)
        };
        let uninstall_log = self.layout.uninstall_log(id);
        let offset = log_len(&uninstall_log);
        let cli: Vec<CliRemoval> = entry
            .cli
            .iter()
            .map(|cli| self.remove_cli(id, cli, manifest.as_ref(), uninstall_cli))
            .collect();
        let ok = failures.is_empty()
            && cli
                .iter()
                .all(|removal| !matches!(removal.action, CliAction::Failed { .. }));
        let output = last_log_lines_after(&uninstall_log, offset);
        self.log_run(
            id,
            RunAction::Remove,
            Some(entry.version.clone()),
            None,
            ok,
            output,
        );
        Ok(Removed {
            id: id.to_owned(),
            version: entry.version,
            failures,
            cli,
        })
    }

    /// Deletes a removed package's folders and, when asked, runs the cleanups. A failure is reported, not rolled back.
    fn delete_package(
        &self,
        removed: &RemovedPackage,
        delete_agent_homes: bool,
        unread: Option<&str>,
    ) -> Vec<Failure> {
        let id = removed.id.as_str();
        let mut failures = Vec::new();
        for dir in [self.layout.package_root(id), self.layout.data_dir(id)] {
            if let Err(error) = remove_tree(&dir) {
                failures.push(Failure {
                    owner: "extensions".to_owned(),
                    code: codes::IO.to_owned(),
                    message: format!("cannot delete {}: {error}", dir.display()),
                });
            }
        }
        if delete_agent_homes && let Some(code) = unread {
            failures.push(Failure {
                owner: "extensions".to_owned(),
                code: code.to_owned(),
                message: format!(
                    "the files of {id} could not be read, so the accounts of its agent kinds were left; remove them with `agent accounts remove`"
                ),
            });
        }
        if delete_agent_homes {
            for (owner, run) in self.hooks.of(HookKind::Cleanup) {
                let HookRun::Cleanup(cleanup) = run else {
                    continue;
                };
                if let Err(error) = cleanup(removed) {
                    failures.push(Failure {
                        owner,
                        code: error.code,
                        message: error.message,
                    });
                }
            }
        }
        failures
    }

    fn remove_cli(
        &self,
        id: &str,
        cli: &LockCli,
        manifest: Option<&Manifest>,
        uninstall_cli: bool,
    ) -> CliRemoval {
        let action = if !cli.installed_by_package {
            CliAction::LeftAlone
        } else {
            let command = manifest
                .and_then(|manifest| manifest.cli_of(&cli.agent))
                .and_then(|spec| spec.uninstall.iter().find(|step| Some(step.os) == self.os))
                .map(|step| step.command.clone());
            match command {
                None => CliAction::NoCommand,
                Some(command) if !uninstall_cli => CliAction::Offered { command },
                Some(command) => self.run_uninstall(id, command, &cli.binary),
            }
        };
        CliRemoval {
            agent: cli.agent.clone(),
            binary: cli.binary.clone(),
            action,
        }
    }

    fn run_uninstall(&self, id: &str, command: Vec<String>, binary: &str) -> CliAction {
        if !massorca_ext_package::manifest::agent::is_uninstall_program(&command, binary) {
            return CliAction::Failed {
                message: "its program is not on the vendor allowlist".to_owned(),
                command,
            };
        }
        let log = self.layout.uninstall_log(id);
        let ran = log
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| self.runner.run(&command, &log, INSTALLER_TIMEOUT));
        match ran {
            Ok(exit) if exit.success() => CliAction::Ran {
                command,
                exit_code: exit.code,
            },
            Ok(exit) => CliAction::Failed {
                message: describe(exit),
                command,
            },
            Err(error) => CliAction::Failed {
                message: error.to_string(),
                command,
            },
        }
    }
}

fn remove_tree(dir: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// The uninstall commands a removal would run for the CLIs this package installed, for this OS.
fn pending_uninstalls(
    entry: &LockEntry,
    manifest: Option<&Manifest>,
    os: Option<CliOs>,
) -> Vec<Vec<String>> {
    entry
        .cli
        .iter()
        .filter(|cli| cli.installed_by_package)
        .filter_map(|cli| {
            manifest?
                .cli_of(&cli.agent)?
                .uninstall
                .iter()
                .find(|step| Some(step.os) == os)
                .map(|step| step.command.clone())
        })
        .collect()
}

/// The agent kinds a package provides, read from its manifest: none for a first-party id the catalogue does not vouch
/// for, and none when the manifest cannot be read (the removal then reports that the accounts were left).
fn provided_kinds(entry: &LockEntry, manifest: Option<&Manifest>) -> Vec<String> {
    if crate::catalogue::is_unvouched_first_party(entry) {
        return Vec::new();
    }
    manifest
        .map(|manifest| {
            manifest
                .contributes
                .agent_adapters
                .iter()
                .map(|adapter| adapter.id.clone())
                .collect()
        })
        .unwrap_or_default()
}
