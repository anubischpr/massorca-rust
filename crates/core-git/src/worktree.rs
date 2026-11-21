//! Linked worktrees: listing, adding and removing them.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::GitError;
use crate::git::Git;
use crate::worktree_porcelain::parse_worktree_list;

/// One worktree from `git worktree list --porcelain`. The first entry is the main worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    /// Commit at HEAD; absent for a bare repository.
    pub head: Option<String>,
    /// Short branch name such as `main`; absent when HEAD is detached.
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    /// Lock reason; `Some("")` when the worktree is locked without a reason.
    pub locked: Option<String>,
    /// Why git could prune this worktree; `Some("")` when no reason is given.
    pub prunable: Option<String>,
}

impl WorktreeEntry {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            head: None,
            branch: None,
            bare: false,
            detached: false,
            locked: None,
            prunable: None,
        }
    }
}

/// How `worktree_add` obtains the branch it checks out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeAdd<'a> {
    /// Creates `branch` starting at `base`, or at HEAD when `base` is `None`.
    NewBranch {
        branch: &'a str,
        base: Option<&'a str>,
    },
    /// Checks out a local branch that already exists.
    ExistingBranch { branch: &'a str },
}

impl Git {
    /// Lists the main worktree and every linked worktree of `repo`.
    pub fn worktree_list(&self, repo: &Path) -> Result<Vec<WorktreeEntry>, GitError> {
        let captured = self.run(Some(repo), ["worktree", "list", "--porcelain"])?;
        captured.require_success()?;
        let text = captured.text()?;
        parse_worktree_list(text).map_err(|reason| captured.invalid_output(reason))
    }

    /// Adds a worktree at `path`; a relative `path` resolves against `repo`.
    pub fn worktree_add(
        &self,
        repo: &Path,
        path: &Path,
        add: WorktreeAdd<'_>,
    ) -> Result<(), GitError> {
        // `--` ends option parsing, so a path or base that starts with a dash is not an option.
        let mut args = vec![OsString::from("worktree"), OsString::from("add")];
        match add {
            WorktreeAdd::NewBranch { branch, base } => {
                args.push(OsString::from("-b"));
                args.push(OsString::from(branch));
                args.push(OsString::from("--"));
                args.push(path.into());
                args.extend(base.map(OsString::from));
            }
            WorktreeAdd::ExistingBranch { branch } => {
                args.push(OsString::from("--"));
                args.push(path.into());
                args.push(OsString::from(branch));
            }
        }
        self.run(Some(repo), args)?.require_success()
    }

    /// Removes the worktree at `path`. A dirty worktree is refused unless `force`; a locked
    /// one is refused either way, because git would need `--force` twice to remove it.
    pub fn worktree_remove(&self, repo: &Path, path: &Path, force: bool) -> Result<(), GitError> {
        let mut args = vec![OsString::from("worktree"), OsString::from("remove")];
        if force {
            args.push(OsString::from("--force"));
        }
        args.push(OsString::from("--"));
        args.push(path.into());
        self.run(Some(repo), args)?.require_success()
    }
}
