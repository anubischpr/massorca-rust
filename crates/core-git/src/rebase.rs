//! Rebasing a branch onto a base (decision 0181): `git rebase <base>` in a worktree, with a conflict aborted and reported.

use std::path::Path;

use crate::error::GitError;
use crate::git::Git;

/// What a rebase onto a base did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseOutcome {
    /// The branch now sits on the base; `head` is its new commit.
    Rebased { head: String },
    /// The base is already in the branch's history; nothing changed.
    UpToDate,
    /// The rebase stopped on conflicts in `paths`. It was aborted, so HEAD and the worktree are as they were.
    Conflict { paths: Vec<String> },
}

impl Git {
    /// Rebases the branch checked out in `dir` onto `base`, a revision git resolves. A rebase that stops on conflicts
    /// is aborted and its paths returned, as `merge_no_ff` does; any other failure is an error, and a rebase it left in
    /// progress is aborted too. Either way HEAD is checked to be where it was before the rebase.
    pub fn rebase_onto(&self, dir: &Path, base: &str) -> Result<RebaseOutcome, GitError> {
        // `git rebase` would read a dash-led name as an option, so it is refused here.
        if base.starts_with('-') {
            return Err(GitError::Failed {
                args: vec!["rebase".to_owned()],
                code: None,
                stderr: format!("refusing base `{base}`: it starts with a dash"),
            });
        }
        let before = self.head(dir)?;
        if self.is_ancestor(dir, base, "HEAD")? {
            return Ok(RebaseOutcome::UpToDate);
        }
        let captured = self.run_mutation(Some(dir), ["rebase", base])?;
        if captured.succeeded() {
            return Ok(RebaseOutcome::Rebased {
                head: self.head(dir)?,
            });
        }
        let paths = self.unmerged_paths(dir)?;
        if self.operation_in_progress(dir)?.is_some() {
            self.run_mutation(Some(dir), ["rebase", "--abort"])?
                .require_success()?;
        }
        let after = self.head(dir)?;
        if after != before {
            return Err(GitError::InvalidOutput {
                args: vec!["rebase".to_owned(), "--abort".to_owned()],
                reason: format!("HEAD is {after} after the failed rebase, not {before}"),
            });
        }
        if paths.is_empty() {
            return Err(captured.failure());
        }
        Ok(RebaseOutcome::Conflict { paths })
    }
}
