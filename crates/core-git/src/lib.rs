//! Synchronous access to the user's `git` binary: repository and worktree
//! queries, worktree add and remove, branch checks, and `--no-ff` merges.
//!
//! Each command runs without a shell and with a timeout, and uses only commands
//! and options that exist in Git 2.25 (ADR 0007).

mod base;
mod branch;
mod classify;
mod commit;
mod diff;
mod error;
mod git;
mod index;
mod log;
mod merge;
mod process;
mod refs;
mod remote;
mod repository;
mod status;
mod untrusted;
mod version;
mod worktree;
mod worktree_locks;
mod worktree_porcelain;

pub use base::Base;
pub use branch::BranchError;
pub use classify::failure_code;
pub use commit::{CommitError, CommitRequest};
pub use error::GitError;
pub use git::{DEFAULT_TIMEOUT, Git, redact_urls};
pub use index::{DiscardError, DiscardWaivers, PathSet};
pub use log::LogCursor;
pub use merge::MergeOutcome;
pub use remote::{FetchTarget, RemoteError};
pub use repository::WorktreeChanges;
pub use version::GitVersion;
pub use worktree::{WorktreeAdd, WorktreeEntry};
