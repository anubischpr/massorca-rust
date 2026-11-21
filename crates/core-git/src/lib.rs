//! Synchronous access to the user's `git` binary: repository and worktree
//! queries, worktree add and remove, and branch checks.
//!
//! Each command runs without a shell and with a timeout, and uses only commands
//! and options that exist in Git 2.25 (ADR 0007).

mod classify;
mod commit;
mod diff;
mod error;
mod git;
mod index;
mod log;
mod process;
mod refs;
mod repository;
mod status;
mod version;
#[cfg(windows)]
mod windows_job_spawn;
mod worktree;
mod worktree_locks;
mod worktree_porcelain;

pub use classify::failure_code;
pub use commit::{CommitError, CommitRequest};
pub use error::GitError;
pub use git::{DEFAULT_TIMEOUT, Git};
pub use index::{DiscardError, DiscardWaivers, PathSet};
pub use log::LogCursor;
pub use repository::WorktreeChanges;
pub use version::GitVersion;
pub use worktree::{WorktreeAdd, WorktreeEntry};
