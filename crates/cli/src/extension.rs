//! `massorca extension ...` (alias `ext`): install, list, enable, disable and remove packages (decision 0062).
//!
//! The CLI never prompts. `install` without `--yes` prints the review and fails with `extension.consent_required`;
//! `--yes` accepts the review it just printed, `--consent <FINGERPRINT>` accepts exactly the review an earlier run
//! printed, and `--yes-run-installer` also accepts running the vendor command. `update` and `rollback` take the same consent.

use std::path::Path;

use clap::{ArgGroup, Args, Subcommand};
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::extensions::{
    CliPlanState, ExtensionInstallParams, ExtensionRemoveParams, ExtensionReview,
    ExtensionRollbackParams, ExtensionSetEnabledParams, ExtensionState, ExtensionUpdateParams,
    ProgramUpdateParams, SourceKind, SourceRef,
};
use serde_json::json;

use crate::agent_hook::{ENV_SESSION, ENV_TOKEN};
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::extension_text::{
    UNINSTALL_CLI_WARNING, command_text, doctor_lines, installed_lines, item_lines, item_rows,
    points_lines, removed_lines, review_lines, run_lines, state_text,
};
use crate::output::Output;
use crate::target::Target;

#[derive(Debug, Subcommand)]
pub enum ExtensionCommand {
    /// Lists the installed extensions.
    List,
    /// Shows one extension, installed or from the catalogue.
    Show {
        /// Package id, such as massorca.claude.
        id: String,
    },
    /// Shows what detection sees for a package's CLIs: the path found or not found, and the PATH it searched.
    Doctor {
        /// Package id, such as massorca.codex.
        id: String,
    },
    /// Lists the built-in catalogue and which entries are installed.
    Catalogue,
    /// Reviews a package: its risk, permissions and what installing would run. Changes nothing.
    Inspect {
        /// catalogue:<id>, dir:<path> or file:<path>, read on the service host.
        source: String,
    },
    /// Installs a package. Without --yes it prints the review and fails with extension.consent_required.
    Install(InstallArgs),
    /// Updates an installed package to its source's current version, in place. Without --yes it prints the new review
    /// and fails with extension.consent_required. Runs no vendor command; an unchanged package is already up to date.
    Update(UpdateArgs),
    /// Rolls an extension back to the version its last update replaced. Without --yes it prints the review of that
    /// version and fails with extension.consent_required; the built-in catalogue needs none.
    Rollback(RollbackArgs),
    /// Enables an installed extension.
    Enable {
        /// Package id.
        id: String,
    },
    /// Disables an installed extension. Its files and data stay.
    Disable {
        /// Package id.
        id: String,
    },
    /// Removes an installed extension, its files and its data.
    Remove(RemoveArgs),
    /// Updates a host program a tool package installed, such as Git, with the vendor's update command. Without --yes it
    /// prints what would run and fails with extension.consent_required. A program MassOrca did not install is refused.
    ProgramUpdate(ProgramUpdateArgs),
    /// Lists what each owner registered on the tool, skill, view, theme and status-rule points, and the enabled
    /// extensions that registered nothing.
    Points,
    /// Lists the recent install, update, rollback and remove runs of an extension, oldest first.
    Log {
        /// Package id, such as massorca.claude.
        id: String,
    },
}

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("accept").args(["yes", "consent"])))]
pub struct InstallArgs {
    /// catalogue:<id>, dir:<path> or file:<path>, read on the service host.
    pub source: String,
    /// Required for file:<path>: the archive's SHA-256, as published.
    #[arg(long, value_name = "HEX")]
    pub sha256: Option<String>,
    /// Accepts the review this command prints: the risk and the permissions.
    #[arg(long)]
    pub yes: bool,
    /// Accepts the review an earlier run printed, by its consent fingerprint. The install fails if anything changed.
    #[arg(long, value_name = "FINGERPRINT")]
    pub consent: Option<String>,
    /// Also accepts running the vendor installer the review lists. Needs --yes or --consent.
    #[arg(long, requires = "accept")]
    pub yes_run_installer: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Package id.
    pub id: String,
    /// Also deletes the credential homes of the package's agents.
    #[arg(long)]
    pub delete_agent_homes: bool,
    /// Also runs the uninstall command of each agent CLI the package installed. Needs --yes or --uninstall-consent.
    #[arg(long)]
    pub uninstall_cli: bool,
    /// Accepts the uninstall command the refusal printed, by its fingerprint. The removal fails if it changed.
    #[arg(long, value_name = "FINGERPRINT")]
    pub uninstall_consent: Option<String>,
    /// Accepts the uninstall command of --uninstall-cli, as printed, in this run.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("accept").args(["yes", "consent"])))]
pub struct UpdateArgs {
    /// Package id, such as massorca.claude.
    pub id: String,
    /// Required for a file: package: the archive's SHA-256, as published.
    #[arg(long, value_name = "HEX")]
    pub sha256: Option<String>,
    /// Accepts the review this command prints: the risk and the permissions of the new version.
    #[arg(long)]
    pub yes: bool,
    /// Accepts the review an earlier run printed, by its consent fingerprint. The update fails if anything changed.
    #[arg(long, value_name = "FINGERPRINT")]
    pub consent: Option<String>,
}

/// Params of `massorca extension rollback`: the package, and the accepted review of the version it restores.
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("accept").args(["yes", "consent"])))]
pub struct RollbackArgs {
    /// Package id, such as massorca.claude.
    pub id: String,
    /// Accepts the review of the version this command restores, as printed.
    #[arg(long)]
    pub yes: bool,
    /// Accepts the review an earlier run printed, by its consent fingerprint. The rollback fails if anything changed.
    #[arg(long, value_name = "FINGERPRINT")]
    pub consent: Option<String>,
}

/// Params of `massorca extension program-update`: the package, its host program, and the accepted update review.
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("accept").args(["yes", "consent"])))]
pub struct ProgramUpdateArgs {
    /// Package id, such as massorca.git.
    pub id: String,
    /// The host program's id in the package, such as git.
    pub program: String,
    /// Accepts the update command this command prints.
    #[arg(long)]
    pub yes: bool,
    /// Accepts the update review an earlier run printed, by its consent fingerprint. The update fails if it changed.
    #[arg(long, value_name = "FINGERPRINT")]
    pub consent: Option<String>,
}

/// Runs one `extension` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: ExtensionCommand,
    output: Output,
) -> Result<(), CliError> {
    if let Some(refusal) = agent_refusal(&command) {
        return Err(refusal);
    }
    let client = connect(target, autostart).await?;
    match command {
        ExtensionCommand::List => {
            let list = client.extension_list().await?;
            output.success(
                &list,
                &item_rows(&list.extensions, "no extensions installed"),
            );
        }
        ExtensionCommand::Show { id } => {
            let item = client.extension_get(&id).await?;
            output.success(&item, &item_lines(&item));
        }
        ExtensionCommand::Doctor { id } => {
            let item = client.extension_get(&id).await?;
            output.success(&item, &doctor_lines(&item));
        }
        ExtensionCommand::Catalogue => {
            let list = client.extension_catalogue().await?;
            output.success(
                &list,
                &item_rows(&list.extensions, "the catalogue is empty"),
            );
        }
        ExtensionCommand::Inspect { source } => {
            let review = client
                .extension_inspect(&source_for(&source, target))
                .await?;
            output.success(&review, &review_lines(&review));
        }
        ExtensionCommand::Install(args) => install(&client, target, args, output).await?,
        ExtensionCommand::Update(args) => update(&client, args, output).await?,
        ExtensionCommand::Rollback(args) => rollback(&client, args, output).await?,
        ExtensionCommand::ProgramUpdate(args) => program_update(&client, args, output).await?,
        ExtensionCommand::Points => {
            let points = client.extension_points().await?;
            output.success(&points, &points_lines(&points));
        }
        ExtensionCommand::Log { id } => {
            let log = client.extension_log(&id).await?;
            output.success(&log, &run_lines(&log, "no runs yet"));
        }
        ExtensionCommand::Enable { id } => set_enabled(&client, id, true, output).await?,
        ExtensionCommand::Disable { id } => set_enabled(&client, id, false, output).await?,
        ExtensionCommand::Remove(args) => remove(&client, args, output).await?,
    }
    Ok(())
}

async fn install(
    client: &Client,
    target: &Target,
    args: InstallArgs,
    output: Output,
) -> Result<(), CliError> {
    let source = source_for(&args.source, target);
    if let Some(consent) = args.consent {
        // The person reviewed in an earlier run; the service refuses if the package or plan changed since.
        let installed = client
            .extension_install(&ExtensionInstallParams {
                source,
                consent: Some(consent),
                run_installer: args.yes_run_installer,
                sha256: args.sha256,
                agent_token: agent_token(),
            })
            .await?;
        output.success(&installed, &installed_lines(&installed));
        return Ok(());
    }
    let review = client.extension_inspect(&source).await?;
    if let Some(pinned) = &args.sha256
        && !pinned.eq_ignore_ascii_case(&review.sha256)
    {
        return Err(CliError::new(
            codes::EXTENSION_HASH_MISMATCH,
            format!("the package hashes to {}, not {pinned}", review.sha256),
            exit::ERROR,
        ));
    }
    if !args.yes {
        let hint = if review.runs_installer {
            format!(
                "review it above, then re-run with --consent {} --yes-run-installer (or --yes --yes-run-installer) to accept exactly this review and run the vendor installer",
                review.consent
            )
        } else {
            format!(
                "review it above, then re-run with --consent {} (or --yes) to accept exactly this review",
                review.consent
            )
        };
        return Err(refusal(
            codes::EXTENSION_CONSENT_REQUIRED,
            format!("the install review was not accepted; {hint}"),
            &review,
            output,
        ));
    }
    if review.runs_installer && !args.yes_run_installer {
        let commands: Vec<String> = review
            .cli
            .iter()
            .filter_map(|plan| match &plan.state {
                CliPlanState::WillRun { command, .. } => Some(command_text(command)),
                _ => None,
            })
            .collect();
        return Err(refusal(
            codes::EXTENSION_INSTALLER_CONSENT_REQUIRED,
            format!(
                "installing runs a vendor installer ({}); re-run with --consent {} --yes-run-installer to allow it",
                commands.join("; "),
                review.consent
            ),
            &review,
            output,
        ));
    }
    for line in review_lines(&review) {
        output.progress(&line);
    }
    let installed = client
        .extension_install(&ExtensionInstallParams {
            source,
            consent: Some(review.consent),
            run_installer: args.yes_run_installer,
            sha256: args.sha256,
            agent_token: agent_token(),
        })
        .await?;
    output.success(&installed, &installed_lines(&installed));
    Ok(())
}

async fn set_enabled(
    client: &Client,
    id: String,
    enabled: bool,
    output: Output,
) -> Result<(), CliError> {
    let outcome = client
        .extension_set_enabled(&ExtensionSetEnabledParams { id, enabled })
        .await?;
    let line = if outcome.changed {
        format!("{} is now {}", outcome.id, state_text(state_of(enabled)))
    } else {
        format!(
            "{} was already {}",
            outcome.id,
            state_text(state_of(enabled))
        )
    };
    output.success(&outcome, &[line]);
    Ok(())
}

fn state_of(enabled: bool) -> ExtensionState {
    if enabled {
        ExtensionState::Enabled
    } else {
        ExtensionState::Disabled
    }
}

/// A refusal that carries the review: in text mode the review is printed first, in JSON mode it is the error's data.
fn refusal(code: &str, message: String, review: &ExtensionReview, output: Output) -> CliError {
    for line in review_lines(review) {
        output.progress(&line);
    }
    let mut error = CliError::new(code, message, exit::ERROR);
    error.data = serde_json::to_value(review).ok().map(Box::new);
    error
}

/// The source as the service should read it. A relative `dir:` or `file:` path is made absolute here, because the
/// service runs in another folder; for a paired host the path means a path on that host and is left as typed.
fn source_for(source: &str, target: &Target) -> String {
    if !target.is_local() {
        return source.to_owned();
    }
    let Some((kind, path)) = source.split_once(':') else {
        return source.to_owned();
    };
    if !matches!(kind, "dir" | "file") || path.is_empty() || Path::new(path).is_absolute() {
        return source.to_owned();
    }
    match std::env::current_dir() {
        Ok(cwd) => format!("{kind}:{}", cwd.join(path).display()),
        Err(_) => source.to_owned(),
    }
}

/// The run token this CLI runs under, when an agent's run started it. The service refuses a vendor command that carries
/// one (0062, second review).
fn agent_token() -> Option<String> {
    std::env::var(ENV_TOKEN)
        .ok()
        .filter(|token| !token.is_empty())
}

/// Refuses, before anything connects, a vendor installer or uninstaller asked for from an agent's run: a person must
/// run it. The check reads the run's environment, which an agent that strips it can avoid; the service still refuses
/// any request that carries a run token.
fn agent_refusal(command: &ExtensionCommand) -> Option<CliError> {
    let message = match command {
        ExtensionCommand::Install(args) if args.yes_run_installer => {
            "an agent cannot run a vendor installer; ask the person to run the command"
        }
        ExtensionCommand::Update(_) => {
            "an agent cannot update an extension; ask the person to run the command"
        }
        ExtensionCommand::Rollback(_) => {
            "an agent cannot roll back an extension; ask the person to run the command"
        }
        ExtensionCommand::ProgramUpdate(_) => {
            "an agent cannot update a program; ask the person to run the command"
        }
        ExtensionCommand::Remove(args) if args.uninstall_cli => {
            "an agent cannot run a vendor uninstaller; ask the person to run the command"
        }
        _ => return None,
    };
    let in_agent_run = [ENV_SESSION, ENV_TOKEN]
        .into_iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.is_empty()));
    in_agent_run.then(|| {
        CliError::new(
            codes::EXTENSION_NEEDS_PERSON,
            message.to_owned(),
            exit::ERROR,
        )
    })
}

/// Removes a package. An uninstall runs only after its confirmation: `--yes` accepts the command the service names
/// first, `--uninstall-consent` accepts it by its fingerprint, and without either the removal changes nothing (0062, V2).
async fn remove(client: &Client, args: RemoveArgs, output: Output) -> Result<(), CliError> {
    let params = |uninstall_consent: Option<String>| ExtensionRemoveParams {
        id: args.id.clone(),
        delete_agent_homes: args.delete_agent_homes,
        uninstall_cli: args.uninstall_cli,
        agent_token: agent_token(),
        uninstall_consent,
    };
    let confirmation = match args.uninstall_consent.clone() {
        Some(consent) => Some(consent),
        None if args.uninstall_cli && args.yes => {
            match client.extension_remove(&params(None)).await {
                Ok(removed) => {
                    output.success(&removed, &removed_lines(&removed));
                    return Ok(());
                }
                Err(error) => {
                    let refusal = CliError::from(error);
                    let fingerprint = refusal
                        .data
                        .as_deref()
                        .and_then(|data| data["fingerprint"].as_str().map(str::to_owned));
                    match fingerprint {
                        Some(fingerprint) if refusal.code == codes::EXTENSION_CONSENT_REQUIRED => {
                            Some(fingerprint)
                        }
                        _ => return Err(refusal),
                    }
                }
            }
        }
        None => None,
    };
    let removed = client
        .extension_remove(&params(confirmation))
        .await
        .map_err(|error| uninstall_confirmation(CliError::from(error), args.uninstall_cli))?;
    output.success(&removed, &removed_lines(&removed));
    Ok(())
}

/// The refusal that asks for an uninstall's confirmation says what the uninstall does to the CLI, as the offer does.
fn uninstall_confirmation(error: CliError, uninstall_cli: bool) -> CliError {
    if uninstall_cli && error.code == codes::EXTENSION_CONSENT_REQUIRED {
        return CliError {
            message: format!("{} {UNINSTALL_CLI_WARNING}", error.message),
            ..error
        };
    }
    error
}

/// Updates an installed package in place, from the source it came from. The new review is accepted as an install's is:
/// `--yes` accepts what this run prints, `--consent` accepts an earlier run's review. An unchanged package is up to date.
async fn update(client: &Client, args: UpdateArgs, output: Output) -> Result<(), CliError> {
    if let Some(consent) = args.consent {
        // The service checks the consent against the package as it is now, so a stale fingerprint is refused.
        return send_update(client, &args.id, Some(consent), args.sha256, output).await;
    }
    let item = client.extension_get(&args.id).await?;
    let Some(installed) = item.installed else {
        return Err(CliError::new(
            codes::EXTENSION_NOT_INSTALLED,
            format!("extension {} is not installed", args.id),
            exit::ERROR,
        ));
    };
    let review = client
        .extension_inspect(&source_text(&installed.source)?)
        .await?;
    if review.sha256.eq_ignore_ascii_case(&installed.sha256) {
        return report_up_to_date(&args.id, output);
    }
    if let Some(pinned) = &args.sha256
        && !pinned.eq_ignore_ascii_case(&review.sha256)
    {
        return Err(CliError::new(
            codes::EXTENSION_HASH_MISMATCH,
            format!("the package hashes to {}, not {pinned}", review.sha256),
            exit::ERROR,
        ));
    }
    if !args.yes {
        return Err(refusal(
            codes::EXTENSION_CONSENT_REQUIRED,
            format!(
                "the update review was not accepted; review it above, then re-run with --consent {} (or --yes) to accept exactly this review",
                review.consent
            ),
            &review,
            output,
        ));
    }
    for line in review_lines(&review) {
        output.progress(&line);
    }
    send_update(client, &args.id, Some(review.consent), args.sha256, output).await
}

/// `massorca extension rollback`: restores the version the last update replaced. Without --yes or --consent it fails
/// with the consent of that version's review, which the refusal carries.
async fn rollback(client: &Client, args: RollbackArgs, output: Output) -> Result<(), CliError> {
    if let Some(consent) = args.consent {
        return send_rollback(client, &args.id, Some(consent), output).await;
    }
    let params = ExtensionRollbackParams {
        id: args.id.clone(),
        consent: None,
        agent_token: agent_token(),
    };
    match client.extension_rollback(&params).await {
        Ok(restored) => {
            output.success(&restored, &installed_lines(&restored));
            Ok(())
        }
        Err(error) => {
            let refusal = CliError::from(error);
            let consent = refusal
                .data
                .as_deref()
                .and_then(|data| data["consent"].as_str().map(str::to_owned));
            match consent {
                Some(consent) if args.yes && refusal.code == codes::EXTENSION_CONSENT_REQUIRED => {
                    send_rollback(client, &args.id, Some(consent), output).await
                }
                _ => Err(refusal),
            }
        }
    }
}

async fn program_update(
    client: &Client,
    args: ProgramUpdateArgs,
    output: Output,
) -> Result<(), CliError> {
    let consent = match args.consent {
        Some(consent) => consent,
        None => {
            let review = client
                .extension_program_update_review(&args.id, &args.program)
                .await?;
            if !args.yes {
                output.progress(&format!(
                    "{} of {} updates with: {}",
                    review.program,
                    review.id,
                    command_text(&review.command)
                ));
                let mut error = CliError::new(
                    codes::EXTENSION_CONSENT_REQUIRED,
                    format!(
                        "the update was not accepted; re-run with --consent {} (or --yes) to run exactly this command",
                        review.consent
                    ),
                    exit::ERROR,
                );
                error.data = serde_json::to_value(&review).ok().map(Box::new);
                return Err(error);
            }
            review.consent
        }
    };
    let updated = client
        .extension_program_update(&ProgramUpdateParams {
            id: args.id,
            program: args.program,
            consent: Some(consent),
            agent_token: agent_token(),
        })
        .await?;
    let line = format!(
        "{} of {} updated: {}",
        updated.program,
        updated.id,
        command_text(&updated.command)
    );
    output.success(&updated, &[line]);
    Ok(())
}

async fn send_rollback(
    client: &Client,
    id: &str,
    consent: Option<String>,
    output: Output,
) -> Result<(), CliError> {
    let params = ExtensionRollbackParams {
        id: id.to_owned(),
        consent,
        agent_token: agent_token(),
    };
    let restored = client.extension_rollback(&params).await?;
    output.success(&restored, &installed_lines(&restored));
    Ok(())
}

/// Sends the update. A package already in place is reported as up to date, whichever side noticed it first.
async fn send_update(
    client: &Client,
    id: &str,
    consent: Option<String>,
    sha256: Option<String>,
    output: Output,
) -> Result<(), CliError> {
    let params = ExtensionUpdateParams {
        id: id.to_owned(),
        consent,
        sha256,
        agent_token: agent_token(),
    };
    match client.extension_update(&params).await {
        Ok(updated) => {
            output.success(&updated, &installed_lines(&updated));
            Ok(())
        }
        Err(error) => {
            let error = too_old_to_update(CliError::from(error));
            if error.code == codes::EXTENSION_UP_TO_DATE {
                return report_up_to_date(id, output);
            }
            Err(error)
        }
    }
}

fn report_up_to_date(id: &str, output: Output) -> Result<(), CliError> {
    output.success(
        &json!({ "id": id, "up_to_date": true }),
        &[format!("{id} is already up to date")],
    );
    Ok(())
}

/// `extensions.update` came after `extensions/1`, so an older host answers it as an unknown method.
fn too_old_to_update(error: CliError) -> CliError {
    if error.code == codes::METHOD_NOT_FOUND {
        return CliError {
            message: "this host is too old to update extensions; update it".to_owned(),
            ..error
        };
    }
    error
}

/// The source an installed package came from, written the way `extension inspect` takes it.
fn source_text(source: &SourceRef) -> Result<String, CliError> {
    let kind = match source.kind {
        SourceKind::Catalogue => "catalogue",
        SourceKind::Dir => "dir",
        SourceKind::File => "file",
        SourceKind::Unknown => {
            return Err(CliError::new(
                codes::EXTENSION_INVALID_SOURCE,
                "this package was installed from a source this CLI does not know; update the CLI",
                exit::ERROR,
            ));
        }
    };
    Ok(format!("{kind}:{}", source.reference))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_update_method_says_the_host_is_too_old() {
        let old = too_old_to_update(CliError::new(
            codes::METHOD_NOT_FOUND,
            "unknown method extensions.update",
            exit::ERROR,
        ));
        assert_eq!(old.code, codes::METHOD_NOT_FOUND);
        assert_eq!(
            old.message,
            "this host is too old to update extensions; update it"
        );

        let other = too_old_to_update(CliError::new(
            codes::EXTENSION_BUSY,
            "extension acme.kit is busy",
            exit::ERROR,
        ));
        assert_eq!(other.message, "extension acme.kit is busy");
    }
}
