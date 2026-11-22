//! `massorca agent credentials ...`: a kind's login files copied into this profile (decision 0056).
//!
//! Only file names and sizes are printed, never contents.

use std::path::PathBuf;

use clap::Subcommand;
use massorca_client::Client;
use massorca_protocol::methods::agents::{
    CredentialsImportParams, CredentialsImported, CredentialsRemoved, CredentialsShown,
};

use crate::cwd_path;
use crate::error::CliError;
use crate::output::Output;
use crate::workspace_text;

/// Said after every import: the copied login and its source must not both stay in use.
pub const SINGLE_USE_NOTE: &str = "Stop using the source login now: its refresh token is single use, so only one copy can keep working. Use the agent through MassOrca from now on, or log in again at the source.";

#[derive(Debug, Subcommand)]
pub enum CredentialsCommand {
    /// Copies the kind's declared login files from a directory into this profile. Prints names and sizes only.
    Import {
        /// Agent kind, for example claude.
        kind: String,
        /// The directory holding the kind's login files, for example the agent's own config directory.
        #[arg(long, value_name = "DIR")]
        from: PathBuf,
        /// Replace the profile's existing home for the kind.
        #[arg(long)]
        replace: bool,
    },
    /// Shows whether the profile has a home for the kind, and which declared files are in it.
    Show {
        /// Agent kind, for example claude.
        kind: String,
    },
    /// Deletes the profile's home for the kind; its runs use the agent's usual home again.
    Remove {
        /// Agent kind, for example claude.
        kind: String,
    },
}

/// Runs one `agent credentials` subcommand.
pub async fn execute(
    client: &Client,
    command: CredentialsCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        CredentialsCommand::Import {
            kind,
            from,
            replace,
        } => {
            let imported = client
                .agent_credentials_import(&CredentialsImportParams {
                    kind,
                    from: cwd_path::absolute(&from)?,
                    replace,
                })
                .await?;
            output.success(&imported, &imported_lines(&imported));
        }
        CredentialsCommand::Show { kind } => {
            let shown = client.agent_credentials_show(&kind).await?;
            output.success(&shown, &shown_lines(&shown));
        }
        CredentialsCommand::Remove { kind } => {
            let removed = client.agent_credentials_remove(&kind).await?;
            output.success(&removed, &[removed_line(&removed)]);
        }
    }
    Ok(())
}

fn imported_lines(imported: &CredentialsImported) -> Vec<String> {
    let mut lines = vec![format!(
        "imported credentials for {} into {}",
        imported.kind, imported.home
    )];
    let rows = imported
        .files
        .iter()
        .map(|file| vec![file.name.clone(), file.bytes.to_string()])
        .collect();
    lines.extend(workspace_text::table(&["FILE", "BYTES"], rows));
    lines.push(SINGLE_USE_NOTE.to_owned());
    lines
}

fn shown_lines(shown: &CredentialsShown) -> Vec<String> {
    let state = if shown.exists {
        "exists"
    } else {
        "not imported; runs use the agent's usual home"
    };
    let mut lines = vec![format!(
        "credential home for {}: {} ({state})",
        shown.kind, shown.home
    )];
    if shown.files.is_empty() {
        lines.push("the kind declares no credential files".to_owned());
        return lines;
    }
    let rows = shown
        .files
        .iter()
        .map(|file| {
            vec![
                file.name.clone(),
                if file.present { "yes" } else { "no" }.to_owned(),
                file.bytes
                    .map(|bytes| bytes.to_string())
                    .unwrap_or_default(),
            ]
        })
        .collect();
    lines.extend(workspace_text::table(&["FILE", "PRESENT", "BYTES"], rows));
    lines
}

fn removed_line(removed: &CredentialsRemoved) -> String {
    if removed.removed {
        format!("removed the credential home for {}", removed.kind)
    } else {
        format!("there was no credential home for {}", removed.kind)
    }
}
