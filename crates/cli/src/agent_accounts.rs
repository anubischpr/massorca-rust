//! `massorca agent accounts ...`: a kind's accounts (decision 0059): sign-in, import, activation, checks and removal.
//!
//! Only account ids, labels, states and identities are printed, never credential contents.

use std::path::PathBuf;

use clap::Subcommand;
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AccountState, AccountSummary, AccountsActivateParams, AccountsAddParams, AccountsAdded,
    AccountsCheckParams, AccountsImportParams, AccountsImported, AccountsList,
    AccountsRemoveParams, AccountsRemoved, AccountsSignInParams, ImportCandidates, ImportResult,
};

use crate::cwd_path;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;
use crate::terminal_attach;
use crate::workspace_text;

#[derive(Debug, Subcommand)]
pub enum AccountsCommand {
    /// Lists the kind's accounts: label, state, identity, whether active, and the projects that chose each.
    List {
        /// Agent kind, for example claude.
        kind: String,
    },
    /// Signs a new account in: runs the kind's login in a terminal, then attaches to it unless `--no-attach`.
    Add {
        /// Agent kind, for example claude.
        kind: String,
        /// The account's label, for example work.
        #[arg(long)]
        label: Option<String>,
        /// Prints the account and its sign-in terminal, and returns without attaching.
        #[arg(long)]
        no_attach: bool,
    },
    /// Signs an existing account in again: runs the kind's login in a terminal with the account's own home, then attaches
    /// to it unless `--no-attach`. The account is kept whatever the login ends with.
    SignIn {
        /// Agent kind, for example claude.
        kind: String,
        /// The account id, as `accounts list` prints it.
        account: String,
        /// Prints the account and its sign-in terminal, and returns without attaching.
        #[arg(long)]
        no_attach: bool,
    },
    /// Imports the kind's declared login files into a new account, then checks it. Without `--from`, each file is copied
    /// from where the kind's CLI keeps it (see `accounts candidates`), and the files it will copy are printed first.
    Import {
        /// Agent kind, for example claude.
        kind: String,
        /// A directory holding every declared login file, instead of where the kind's CLI keeps each one.
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
        /// The account's label, for example work.
        #[arg(long)]
        label: Option<String>,
    },
    /// Shows where an import without `--from` would copy each kind's login files from: per kind its folder, and each
    /// file's name, whether it is required, whether it was found, and its path. Nothing is read or copied.
    Candidates {
        /// Agent kind, for example claude; every kind that declares login files when absent.
        kind: Option<String>,
    },
    /// Makes an account the kind's active one, or the account a project uses with `--project`.
    Activate {
        /// Agent kind, for example claude.
        kind: String,
        /// The account id, as `accounts list` prints it.
        account: String,
        /// A project id; the choice then applies to that project only.
        #[arg(long, value_name = "PROJECT")]
        project: Option<String>,
    },
    /// Deletes an account. Refused while a live run uses it; the kind's sign-out runs first when it declares one.
    Remove {
        /// Agent kind, for example claude.
        kind: String,
        /// The account id.
        account: String,
    },
    /// Checks an account now, or the kind's active account when none is named, and prints its state.
    Check {
        /// Agent kind, for example claude.
        kind: String,
        /// The account id; the active account when absent.
        account: Option<String>,
    },
}

/// Runs one `agent accounts` subcommand. `json` is the output mode: sign-in attaches to a terminal, which has no JSON
/// form, so it needs `--no-attach` there.
pub async fn execute(
    target: &Target,
    client: &Client,
    command: AccountsCommand,
    output: Output,
    json: bool,
) -> Result<(), CliError> {
    match command {
        AccountsCommand::List { kind } => {
            let listed = client.agent_accounts_list(&kind).await?;
            output.success(&listed, &list_lines(&listed));
        }
        AccountsCommand::Add {
            kind,
            label,
            no_attach,
        } => {
            check_attach(json, no_attach)?;
            let added = client
                .agent_accounts_add(&AccountsAddParams { kind, label })
                .await?;
            report_sign_in(target, client, output, &added, no_attach).await?;
        }
        AccountsCommand::SignIn {
            kind,
            account,
            no_attach,
        } => {
            check_attach(json, no_attach)?;
            let added = client
                .agent_accounts_sign_in(&AccountsSignInParams { kind, account })
                .await?;
            report_sign_in(target, client, output, &added, no_attach).await?;
        }
        AccountsCommand::Import { kind, from, label } => {
            let from = match from {
                Some(from) => Some(cwd_path::absolute(&from)?),
                None => {
                    announce_detected(client, output, &kind).await?;
                    None
                }
            };
            let imported = client
                .agent_accounts_import(&AccountsImportParams { kind, from, label })
                .await?;
            output.success(&imported, &import_lines(&imported));
        }
        AccountsCommand::Candidates { kind } => {
            let candidates = client
                .agent_accounts_import_candidates(kind.as_deref())
                .await?;
            output.success(&candidates, &candidates_lines(&candidates));
        }
        AccountsCommand::Activate {
            kind,
            account,
            project,
        } => {
            let summary = client
                .agent_accounts_activate(&AccountsActivateParams {
                    kind,
                    account,
                    project,
                })
                .await?;
            output.success(&summary, &summary_lines(&summary));
        }
        AccountsCommand::Remove { kind, account } => {
            let removed = client
                .agent_accounts_remove(&AccountsRemoveParams { kind, account })
                .await?;
            output.success(&removed, &[removed_line(&removed)]);
        }
        AccountsCommand::Check { kind, account } => {
            let checked = client
                .agent_accounts_check(&AccountsCheckParams { kind, account })
                .await?;
            output.success(&checked, &summary_lines(&checked));
        }
    }
    Ok(())
}

/// Before an import without `--from`: prints each file it will copy, or refuses with the missing required files and
/// where each was looked for, so nothing is sent for a kind that is not ready.
async fn announce_detected(client: &Client, output: Output, kind: &str) -> Result<(), CliError> {
    let detected = client.agent_accounts_import_candidates(Some(kind)).await?;
    let Some(candidate) = detected.candidates.iter().find(|found| found.kind == kind) else {
        return Err(CliError::new(
            codes::AGENT_CREDENTIALS_UNSUPPORTED,
            format!("agent kind {kind} declares no login files to import"),
            crate::error::exit::ERROR,
        ));
    };
    if !candidate.ready {
        let missing: Vec<String> = candidate
            .files
            .iter()
            .filter(|file| file.required && !file.found)
            .map(|file| match &file.path {
                Some(path) => format!("{} (looked for at {})", file.name, printable(path)),
                None => format!("{} (the kind declares no location for it)", file.name),
            })
            .collect();
        return Err(CliError::new(
            codes::AGENT_CREDENTIALS_INVALID,
            format!(
                "nothing was imported for agent kind {kind}; a required file is missing: {}; sign in with the vendor's \
                 CLI or pass --from DIR",
                missing.join(", ")
            ),
            crate::error::exit::ERROR,
        ));
    }
    for file in candidate.files.iter().filter(|file| file.found) {
        output.progress(&format!(
            "copying {} from {}",
            file.name,
            printable(file.path.as_deref().unwrap_or("its usual place"))
        ));
    }
    for file in candidate.files.iter().filter(|file| !file.found) {
        match &file.reason {
            Some(reason) => output.progress(&format!(
                "skipping {} (optional, not usable: {reason})",
                file.name
            )),
            None => output.progress(&format!("skipping {} (optional, not found)", file.name)),
        }
    }
    Ok(())
}

/// Per kind its readiness and folder, then each declared file: required or optional, found or missing, and its path.
fn candidates_lines(candidates: &ImportCandidates) -> Vec<String> {
    if candidates.candidates.is_empty() {
        return vec!["no agent kind declares login files to import".to_owned()];
    }
    let mut lines = Vec::new();
    for candidate in &candidates.candidates {
        let folder = candidate
            .folder
            .as_deref()
            .map(|folder| format!(", folder {}", printable(folder)))
            .unwrap_or_default();
        lines.push(format!(
            "{}: {}{folder}",
            candidate.kind,
            if candidate.ready {
                "ready"
            } else {
                "not ready"
            }
        ));
        for file in &candidate.files {
            let path = file
                .path
                .as_deref()
                .map_or_else(|| "no location".to_owned(), printable);
            let env = file
                .env
                .as_deref()
                .map(|env| format!(" (from {env})"))
                .unwrap_or_default();
            lines.push(format!(
                "  {}: {}, {}, {path}{env}",
                file.name,
                if file.required {
                    "required"
                } else {
                    "optional"
                },
                if file.found { "found" } else { "missing" },
            ));
        }
    }
    lines
}

/// The attach rule of a sign-in, shared by `add` and `sign-in`: it attaches to a terminal, which has no `--json` form, so
/// `--json` needs `--no-attach`. Checked before the sign-in starts, so a refused attach leaves the account as it was.
fn check_attach(json: bool, no_attach: bool) -> Result<(), CliError> {
    if no_attach {
        return Ok(());
    }
    if json {
        return Err(CliError::new(
            "usage.invalid_arguments",
            "sign-in attaches to a terminal, which has no --json form; pass --no-attach",
            crate::error::exit::USAGE,
        ));
    }
    terminal_attach::require_terminal()
}

/// Prints a started sign-in, then attaches to its terminal unless `--no-attach`.
async fn report_sign_in(
    target: &Target,
    client: &Client,
    output: Output,
    added: &AccountsAdded,
    no_attach: bool,
) -> Result<(), CliError> {
    output.success(added, &added_lines(added));
    if !no_attach {
        terminal_attach::attach(target, client, &added.terminal_id).await?;
    }
    Ok(())
}

/// The wire name of a state, as the docs and the tests spell it.
pub fn state_word(state: AccountState) -> &'static str {
    match state {
        AccountState::SignedIn => "signed_in",
        AccountState::Expired => "expired",
        AccountState::NeedsSignIn => "needs_sign_in",
        AccountState::NotSetUp => "not_set_up",
        AccountState::Migrated => "migrated",
        AccountState::Conflict => "conflict",
        AccountState::Unknown => "unknown",
    }
}

fn list_lines(listed: &AccountsList) -> Vec<String> {
    if listed.accounts.is_empty() {
        return vec!["no accounts; sign in with `agent accounts add <kind>`".to_owned()];
    }
    let rows = listed
        .accounts
        .iter()
        .map(|account| {
            vec![
                account.id.clone(),
                printable(&account.label),
                state_word(account.state).to_owned(),
                printable(&account.identity.clone().unwrap_or_default()),
                if account.active { "yes" } else { "no" }.to_owned(),
                account.projects.join(", "),
            ]
        })
        .collect();
    workspace_text::table(
        &[
            "ACCOUNT", "LABEL", "STATE", "IDENTITY", "ACTIVE", "PROJECTS",
        ],
        rows,
    )
}

fn added_lines(added: &AccountsAdded) -> Vec<String> {
    let mut lines = summary_lines(&added.account);
    lines.push(format!("sign-in terminal {}", added.terminal_id));
    lines
}

fn summary_lines(summary: &AccountSummary) -> Vec<String> {
    let identity = summary
        .identity
        .as_deref()
        .map(|identity| format!(", {}", printable(identity)))
        .unwrap_or_default();
    vec![format!(
        "account {} ({}): {}{identity}{}",
        printable(&summary.label),
        summary.id,
        state_word(summary.state),
        if summary.active { ", active" } else { "" }
    )]
}

fn import_lines(imported: &AccountsImported) -> Vec<String> {
    match (imported.result, &imported.account, &imported.reason) {
        (ImportResult::Imported, Some(account), _) => {
            let mut lines = Vec::new();
            if let Some(updated) = &imported.updated {
                // The login was already this account (decision 0098): it has the new files, and no account was added.
                lines.push(format!(
                    "updated account {updated} with the imported files instead of adding one"
                ));
            }
            lines.extend(summary_lines(account));
            lines
        }
        (ImportResult::NotFound, _, reason) => vec![format!(
            "not imported: {}",
            reason
                .as_deref()
                .unwrap_or("a required file is missing from the source")
        )],
        (_, _, reason) => vec![format!(
            "not imported: {}",
            reason.as_deref().unwrap_or("the import failed")
        )],
    }
}

fn removed_line(removed: &AccountsRemoved) -> String {
    if removed.removed {
        format!("removed account {} of {}", removed.account, removed.kind)
    } else {
        format!(
            "there was no account {} of {}",
            removed.account, removed.kind
        )
    }
}

/// Text as the terminal shows it: control characters removed, so a label or identity cannot move the cursor or clear the
/// screen. Labels are at most 256 characters, and identities are checked by the service the same way.
fn printable(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}
