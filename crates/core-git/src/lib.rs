//! Synchronous access to the user's `git` binary: repository and worktree
//! queries, worktree add and remove, and branch checks.
//!
//! Each command runs without a shell and with a timeout, and uses only commands
//! and options that exist in Git 2.25 (ADR 0007).

mod error;
mod git;
mod process;
mod refs;
mod repository;
mod version;

pub use error::GitError;
pub use git::{DEFAULT_TIMEOUT, Git};
pub use version::GitVersion;
