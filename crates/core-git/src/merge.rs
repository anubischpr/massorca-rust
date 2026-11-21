//! Merges into the checked-out branch, for bring-back (ADR 0055): a `--no-ff` merge that
//! is aborted when it stops on conflicts.

use std::path::Path;

use crate::error::GitError;
use crate::git::Git;

/// What `Git::merge_no_ff` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The branch is merged, or was already contained in HEAD; `commit` is the new HEAD.
    Merged { commit: String },
    /// The merge stopped on conflicts in `paths`. It was aborted, so HEAD and the worktree are as they were.
    Conflict { paths: Vec<String> },
}

impl Git {
    /// Merges `branch` into HEAD in `dir` with `git merge --no-ff --no-edit`. A merge that
    /// stops on conflicts is aborted, and HEAD is checked to be where it was before the
    /// merge; the conflicting paths are returned. Any other failure is an error.
    pub fn merge_no_ff(&self, dir: &Path, branch: &str) -> Result<MergeOutcome, GitError> {
        // `git merge` takes no `--` before the branch, so a dash-led name is refused here.
        if branch.starts_with('-') {
            return Err(GitError::Failed {
                args: vec!["merge".to_owned(), "--no-ff".to_owned()],
                code: None,
                stderr: format!("refusing branch name `{branch}`: it starts with a dash"),
            });
        }
        let before = self.head(dir)?;
        let captured = self.run_mutation(Some(dir), ["merge", "--no-ff", "--no-edit", branch])?;
        if captured.succeeded() {
            return Ok(MergeOutcome::Merged {
                commit: self.head(dir)?,
            });
        }
        let paths = self.unmerged_paths(dir)?;
        if paths.is_empty() {
            return Err(captured.failure());
        }
        self.run_mutation(Some(dir), ["merge", "--abort"])?
            .require_success()?;
        let after = self.head(dir)?;
        if after != before {
            return Err(GitError::InvalidOutput {
                args: vec!["merge".to_owned(), "--abort".to_owned()],
                reason: format!("HEAD is {after} after the abort, not {before}"),
            });
        }
        Ok(MergeOutcome::Conflict { paths })
    }

    /// The commit HEAD points to in `dir`; an unborn HEAD has nothing to merge into.
    fn head(&self, dir: &Path) -> Result<String, GitError> {
        self.resolve_commit(dir, "HEAD")?
            .ok_or_else(|| GitError::InvalidOutput {
                args: vec!["rev-parse".to_owned(), "HEAD".to_owned()],
                reason: "HEAD names no commit".to_owned(),
            })
    }

    /// The paths git reports as unmerged in `dir`, read NUL-separated so no path is quoted.
    fn unmerged_paths(&self, dir: &Path) -> Result<Vec<String>, GitError> {
        let captured = self.run(Some(dir), ["diff", "--name-only", "-z", "--diff-filter=U"])?;
        captured.require_success()?;
        Ok(captured
            .text()?
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect())
    }
}
