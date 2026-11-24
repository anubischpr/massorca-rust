use std::io;
use std::path::PathBuf;

use crate::version::GitVersion;

/// Why a git command did not produce the answer its caller asked for.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// No `git` executable was found on PATH or at `MASSORCA_GIT`.
    #[error("git executable not found (install git or set MASSORCA_GIT)")]
    NotFound,
    /// The binary is older than the baseline, so the command was not run (ADR 0014).
    #[error("git {found} is older than the required {required}")]
    TooOld {
        found: GitVersion,
        required: GitVersion,
    },
    /// The process could not be started, or its output could not be read.
    #[error("failed to run git: {0}")]
    Spawn(#[source] io::Error),
    /// The command ran past its timeout and was killed.
    #[error("git {} timed out", .args.join(" "))]
    Timeout { args: Vec<String> },
    /// The command exited with a failure status; `stderr` is git's own message.
    #[error("git {} failed: {stderr}", .args.join(" "))]
    Failed {
        args: Vec<String>,
        code: Option<i32>,
        stderr: String,
    },
    /// The command succeeded but its output did not have the expected shape.
    #[error("unexpected output from git {}: {reason}", .args.join(" "))]
    InvalidOutput { args: Vec<String>, reason: String },
    /// A working-tree file a diff needed could not be read.
    #[error("cannot read {path:?}: {source}")]
    File {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The path has a line break, which `git worktree list --porcelain` cannot report
    /// back unambiguously before Git 2.36.
    #[error("unsupported worktree path {path:?}: it contains a line break")]
    UnsupportedPath { path: PathBuf },
    /// The run would act for a caller without `workspace` permission and cannot be made safe (decision 0078).
    #[error("git refused for a caller without workspace permission: {reason}")]
    UnsafeForCaller { reason: String },
}
