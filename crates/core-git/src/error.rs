use std::io;

/// Why a git command did not produce the answer its caller asked for.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// No `git` executable was found on PATH or at `MASSORCA_GIT`.
    #[error("git executable not found (install git or set MASSORCA_GIT)")]
    NotFound,
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
}
