//! Synchronous access to the user's `git` binary: repository and worktree
//! queries, worktree add and remove, branch checks, and `--no-ff` merges.
//!
//! Each command runs without a shell and with a timeout, and uses only commands
//! and options that exist in Git 2.25 (ADR 0007).

mod availability;
mod base;
mod branch;
mod classify;
mod commit;
mod config;
mod diff;
mod error;
mod file_content;
#[cfg(windows)]
mod file_handle_path;
mod git;
mod index;
mod log;
mod merge;
mod process;
mod refs;
mod remote;
mod remote_identity;
mod remote_owner;
mod repository;
mod repository_init;
mod status;
mod submodule_pointer;
mod untrusted;
mod version;
mod worktree;
mod worktree_file;
mod worktree_locks;
mod worktree_porcelain;
mod worktree_prune;

pub use availability::GitAvailability;
pub use base::Base;
pub use branch::BranchError;
pub use classify::failure_code;
pub use commit::{CommitError, CommitRequest};
pub use error::GitError;
pub use file_content::{FileContentError, SideContent};
pub use git::{DEFAULT_TIMEOUT, Git, redact_urls};
pub use index::{DiscardError, DiscardWaivers, PathSet};
pub use log::LogCursor;
pub use merge::MergeOutcome;
pub use remote::{FetchTarget, RemoteError};
pub use remote_owner::remote_owner;
pub use repository::WorktreeChanges;
pub use repository_init::discard_new_repository;
pub use version::GitVersion;
pub use worktree::{WorktreeAdd, WorktreeEntry};
