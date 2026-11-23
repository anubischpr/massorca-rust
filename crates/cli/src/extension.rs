//! `massorca extension ...` (alias `ext`): install, list, enable, disable and remove packages (decision 0062).
//!
//! The CLI never prompts. `install` without `--yes` prints the review and fails with `extension.consent_required`;
//! `--yes` accepts the review it just printed, `--consent <FINGERPRINT>` accepts exactly the review an earlier run
//! printed, and `--yes-run-installer` also accepts running the vendor command.

use std::path::Path;

use clap::{ArgGroup, Args, Subcommand};
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::extensions::{
    CliPlanState, ExtensionInstallParams, ExtensionRemoveParams, ExtensionReview,
    ExtensionSetEnabledParams, ExtensionState,
};

use crate::agent_hook::{ENV_SESSION, ENV_TOKEN};
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::extension_text::{
    command_text, installed_lines, item_lines, item_rows, removed_lines, review_lines, state_text,
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
    /// Lists the built-in catalogue and which entries are installed.
    Catalogue,
    /// Reviews a package: its risk, permissions and what installing would run. Changes nothing.
    Inspect {
        /// catalogue:<id>, dir:<path> or file:<path>, read on the service host.
        source: String,
    },
    /// Installs a package. Without --yes it prints the review and fails with extension.consent_required.
    Install(InstallArgs),
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
    let removed = client.extension_remove(&params(confirmation)).await?;
    output.success(&removed, &removed_lines(&removed));
    Ok(())
}
