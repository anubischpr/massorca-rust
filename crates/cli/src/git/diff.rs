//! `massorca git diff` (the changed files) and `massorca git patch` (one file's patch).

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::diff::{
    DiffTarget, GitDiff, GitDiffFile, GitDiffFileParams, GitDiffParams,
};

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

/// Which sides to compare; with no flag, the index against the working tree (unstaged changes).
#[derive(Debug, Clone, Args)]
#[group(multiple = false)]
pub struct TargetArgs {
    /// HEAD against the index: what a commit would record.
    #[arg(long)]
    pub staged: bool,
    /// HEAD against the working tree: everything not committed.
    #[arg(long)]
    pub head: bool,
    /// Compare this revision with `--to` (default HEAD).
    #[arg(long, value_name = "REV")]
    pub from: Option<String>,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    #[command(flatten)]
    pub target: TargetArgs,
    /// With `--from`, the revision to compare against.
    #[arg(long, value_name = "REV", requires = "from")]
    pub to: Option<String>,
    /// List at most this many files.
    #[arg(long)]
    pub limit: Option<u32>,
}

#[derive(Debug, Args)]
pub struct PatchArgs {
    /// The file, relative to the workspace root.
    pub path: String,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    #[command(flatten)]
    pub target: TargetArgs,
    #[arg(long, value_name = "REV", requires = "from")]
    pub to: Option<String>,
    /// For a renamed file, its old path, so the patch shows the rename.
    #[arg(long, value_name = "PATH")]
    pub original_path: Option<String>,
    /// Return at most this many bytes of patch (the service caps it at 4 MiB).
    #[arg(long)]
    pub max_bytes: Option<u32>,
}

fn target(args: TargetArgs, to: Option<String>) -> DiffTarget {
    match args {
        TargetArgs {
            from: Some(from), ..
        } => DiffTarget::Range { from, to },
        TargetArgs { staged: true, .. } => DiffTarget::Staged,
        TargetArgs { head: true, .. } => DiffTarget::Head,
        _ => DiffTarget::Worktree,
    }
}

pub(super) async fn run_list(
    client: &Client,
    args: DiffArgs,
    output: Output,
) -> Result<(), CliError> {
    let diff = client
        .git_diff(&GitDiffParams {
            workspace: args.workspace.selector()?,
            target: target(args.target, args.to),
            limit: args.limit,
        })
        .await?;
    output.success(&diff, &list_lines(&diff));
    Ok(())
}

pub(super) async fn run_patch(
    client: &Client,
    args: PatchArgs,
    output: Output,
) -> Result<(), CliError> {
    let patch = client
        .git_diff_file(&GitDiffFileParams {
            workspace: args.workspace.selector()?,
            target: target(args.target, args.to),
            path: args.path,
            original_path: args.original_path,
            max_bytes: args.max_bytes,
        })
        .await?;
    output.success(&patch, &patch_lines(&patch));
    Ok(())
}

fn list_lines(diff: &GitDiff) -> Vec<String> {
    if diff.files.is_empty() {
        return vec!["no changes".to_owned()];
    }
    let mut lines: Vec<String> = diff
        .files
        .iter()
        .map(|file| {
            let counts = if file.lines.binary {
                "binary".to_owned()
            } else {
                format!("+{} -{}", file.lines.added, file.lines.removed)
            };
            match &file.original_path {
                Some(original) => format!("{counts}\t{original} -> {}", file.path),
                None => format!("{counts}\t{}", file.path),
            }
        })
        .collect();
    let totals = diff.totals;
    lines.push(format!(
        "{} files, +{} -{}{}",
        totals.files,
        totals.added,
        totals.removed,
        if diff.truncated {
            " (list truncated)"
        } else {
            ""
        }
    ));
    lines
}

fn patch_lines(patch: &GitDiffFile) -> Vec<String> {
    if let Some(pointer) = &patch.submodule {
        let mut lines = vec![format!("submodule {}: {}", patch.path, pointer.summary)];
        match pointer.dirty {
            Some(true) => lines.push("working tree dirty".to_owned()),
            Some(false) => lines.push("working tree clean".to_owned()),
            None => {}
        }
        return lines;
    }
    if patch.binary {
        let size = |side: Option<u64>| {
            side.map_or_else(|| "none".to_owned(), |bytes| format!("{bytes} bytes"))
        };
        return vec![format!(
            "binary file {}: {} -> {}",
            patch.path,
            size(patch.old_size),
            size(patch.new_size)
        )];
    }
    let Some(text) = &patch.patch else {
        return vec![format!("no changes in {}", patch.path)];
    };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if patch.truncated {
        lines.push("(patch truncated; ask for more with --max-bytes)".to_owned());
    }
    lines
}
