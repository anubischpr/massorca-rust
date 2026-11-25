//! `massorca git show-file`: one file's bytes on the working tree, the index or a revision (decision 0099).

use std::path::PathBuf;

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::diff::{
    FileSide, GitFileContent, GitFileContentParams, MAX_CONTENT_BYTES,
};

use super::WorkspaceArg;
use crate::error::{CliError, exit};
use crate::output::Output;

#[derive(Debug, Args)]
pub struct ShowFileArgs {
    /// The file, relative to the workspace root.
    pub path: String,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    #[command(flatten)]
    pub side: SideArgs,
    /// Writes the file's bytes to this file, and fails when there are none.
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,
}

/// Which side to show; with no flag, the file in the working tree.
#[derive(Debug, Clone, Args)]
#[group(multiple = false)]
pub struct SideArgs {
    /// The file as the index holds it: what a commit would record.
    #[arg(long)]
    pub index: bool,
    /// The file as this revision holds it, such as `HEAD` or a branch.
    #[arg(long, value_name = "REV")]
    pub rev: Option<String>,
}

fn side(args: SideArgs) -> FileSide {
    match args {
        SideArgs { index: true, .. } => FileSide::Index,
        SideArgs { rev: Some(rev), .. } => FileSide::Revision { rev },
        _ => FileSide::Worktree,
    }
}

pub(super) async fn run(
    client: &Client,
    args: ShowFileArgs,
    output: Output,
) -> Result<(), CliError> {
    let side = side(args.side);
    let content = client
        .git_file_content(&GitFileContentParams {
            workspace: args.workspace.selector()?,
            side: side.clone(),
            path: args.path,
            max_bytes: None,
        })
        .await?;
    let mut lines = vec![summary(&content, &side)];
    if let Some(out) = &args.out {
        let bytes = content.decoded().ok_or_else(|| no_bytes(&content, &side))?;
        std::fs::write(out, &bytes)?;
        lines.push(format!("written to {}", out.display()));
    }
    output.success(&content, &lines);
    Ok(())
}

/// The one-line summary: the size, the type of a previewable image, and whether the file is too large to show.
fn summary(content: &GitFileContent, side: &FileSide) -> String {
    let mime = content
        .mime
        .as_deref()
        .map(|mime| format!(", {mime}"))
        .unwrap_or_default();
    match content.size {
        None => format!("{}: no file in {}", content.path, side_label(side)),
        Some(size) if content.too_large => {
            format!("{}: {size} bytes{mime}, too large to show", content.path)
        }
        Some(size) => format!("{}: {size} bytes{mime}", content.path),
    }
}

fn side_label(side: &FileSide) -> String {
    match side {
        FileSide::Worktree => "the working tree".to_owned(),
        FileSide::Index => "the index".to_owned(),
        FileSide::Revision { rev } => format!("revision {rev}"),
    }
}

fn no_bytes(content: &GitFileContent, side: &FileSide) -> CliError {
    let reason = if content.too_large {
        format!("it is over the {} MiB limit", MAX_CONTENT_BYTES >> 20)
    } else {
        format!("there is no file in {}", side_label(side))
    };
    CliError::new(
        "git.no_bytes",
        format!("cannot write `{}`: {reason}", content.path),
        exit::ERROR,
    )
}
