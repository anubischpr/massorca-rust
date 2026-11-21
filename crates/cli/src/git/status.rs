//! `massorca git status`.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::status::{
    BranchState, ConflictKind, GitOperation, GitStatus, GitStatusParams, StatusEntry,
};
use massorca_protocol::methods::git::{ChangeKind, LineCounts};

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    /// List at most this many files (the service caps it at 10,000).
    #[arg(long)]
    pub limit: Option<u32>,
}

pub(super) async fn run(client: &Client, args: StatusArgs, output: Output) -> Result<(), CliError> {
    let status = client
        .git_status(&GitStatusParams {
            workspace: args.workspace.selector()?,
            limit: args.limit,
        })
        .await?;
    output.success(&status, &lines(&status));
    Ok(())
}

/// Text form: the branch line, any operation in progress, then one line per file.
pub(crate) fn lines(status: &GitStatus) -> Vec<String> {
    let mut lines = vec![branch_line(&status.branch)];
    if let Some(operation) = status.operation {
        lines.push(format!("{} in progress", operation_name(operation)));
    }
    if status.entries.is_empty() {
        lines.push("nothing changed".to_owned());
    }
    lines.extend(status.entries.iter().map(entry_line));
    if status.truncated {
        let totals = status.totals;
        lines.push(format!(
            "(more files not shown: {} staged, {} unstaged, {} untracked, {} conflicted in total)",
            totals.staged, totals.unstaged, totals.untracked, totals.conflicted
        ));
    }
    lines
}

fn branch_line(branch: &BranchState) -> String {
    let mut line = match (&branch.head, &branch.oid) {
        (Some(head), Some(_)) => format!("on {head}"),
        (Some(head), None) => format!("on {head} (no commits yet)"),
        (None, Some(oid)) => format!("detached at {}", &oid[..oid.len().min(12)]),
        (None, None) => "detached".to_owned(),
    };
    if let Some(upstream) = &branch.upstream {
        if branch.upstream_gone {
            line.push_str(&format!(", upstream {upstream} is gone"));
        } else {
            let ahead = branch.ahead.unwrap_or(0);
            let behind = branch.behind.unwrap_or(0);
            line.push_str(&format!(", {upstream}: ahead {ahead}, behind {behind}"));
        }
    }
    line
}

/// Two status letters as git prints them (`M `, ` M`, `??`, `UU`), the path, then line counts.
fn entry_line(entry: &StatusEntry) -> String {
    let code = if entry.untracked {
        "??".to_owned()
    } else if let Some(conflict) = entry.conflict {
        conflict_code(conflict).to_owned()
    } else {
        format!("{}{}", letter(entry.staged), letter(entry.unstaged))
    };
    let mut line = match &entry.original_path {
        Some(original) => format!("{code} {original} -> {}", entry.path),
        None => format!("{code} {}", entry.path),
    };
    for counts in [entry.staged_lines, entry.unstaged_lines]
        .into_iter()
        .flatten()
    {
        line.push_str(&counts_text(counts));
    }
    line
}

fn counts_text(counts: LineCounts) -> String {
    if counts.binary {
        "  (binary)".to_owned()
    } else {
        format!("  +{} -{}", counts.added, counts.removed)
    }
}

fn letter(kind: Option<ChangeKind>) -> char {
    match kind {
        None => ' ',
        Some(ChangeKind::Added) => 'A',
        Some(ChangeKind::Modified) => 'M',
        Some(ChangeKind::Deleted) => 'D',
        Some(ChangeKind::Renamed) => 'R',
        Some(ChangeKind::Copied) => 'C',
        Some(ChangeKind::TypeChanged) => 'T',
        Some(ChangeKind::Unknown) => '?',
    }
}

fn conflict_code(kind: ConflictKind) -> &'static str {
    match kind {
        ConflictKind::BothModified => "UU",
        ConflictKind::BothAdded => "AA",
        ConflictKind::BothDeleted => "DD",
        ConflictKind::AddedByUs => "AU",
        ConflictKind::AddedByThem => "UA",
        ConflictKind::DeletedByUs => "DU",
        ConflictKind::DeletedByThem => "UD",
        ConflictKind::Unknown => "U?",
    }
}

fn operation_name(operation: GitOperation) -> &'static str {
    match operation {
        GitOperation::Merge => "merge",
        GitOperation::Rebase => "rebase",
        GitOperation::CherryPick => "cherry-pick",
        GitOperation::Revert => "revert",
        GitOperation::ApplyMailbox => "am",
        GitOperation::Unknown => "an operation",
    }
}
