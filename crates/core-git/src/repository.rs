//! Repository queries: the top of a work tree and whether it has changes.

use std::path::{Path, PathBuf};

use crate::error::GitError;
use crate::git::{Git, path_from_git};

/// Changes in a work tree, counted by kind. Ignored files are counted apart: `git status`
/// does not list them as changes, yet removing the tree deletes them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorktreeChanges {
    /// Tracked files with staged or unstaged changes.
    pub uncommitted: usize,
    /// Untracked files and directories, as git lists them.
    pub untracked: usize,
    /// Ignored files and directories, as git lists them.
    pub ignored: usize,
}

impl WorktreeChanges {
    /// True when there is nothing in the tree that git would report, ignored files included.
    pub fn is_clean(&self) -> bool {
        self.uncommitted == 0 && self.untracked == 0 && self.ignored == 0
    }
}

impl Git {
    /// The top of the work tree containing `dir`, or `None` when `dir` is outside every repository.
    pub fn toplevel(&self, dir: &Path) -> Result<Option<PathBuf>, GitError> {
        let captured = self.run(Some(dir), ["rev-parse", "--show-toplevel"])?;
        // Git exits 128 for this and for other failures alike; only the message tells them
        // apart (LC_ALL=C keeps it English). ADR 0008 records this exception.
        if !captured.succeeded() && captured.stderr_contains("not a git repository") {
            return Ok(None);
        }
        captured.require_success()?;
        let line = captured.text()?.trim_end_matches(['\r', '\n']);
        Ok(Some(path_from_git(line)))
    }

    /// Whether the work tree containing `dir` has staged, unstaged or untracked changes.
    pub fn status_dirty(&self, dir: &Path) -> Result<bool, GitError> {
        let captured = self.run(
            Some(dir),
            ["status", "--porcelain", "--untracked-files=normal"],
        )?;
        captured.require_success()?;
        Ok(!captured.bytes().is_empty())
    }

    /// Counts the changes in the work tree containing `dir`, ignored files included.
    pub fn status_changes(&self, dir: &Path) -> Result<WorktreeChanges, GitError> {
        let captured = self.run(
            Some(dir),
            [
                "status",
                "--porcelain",
                "--ignored",
                "--untracked-files=normal",
            ],
        )?;
        captured.require_success()?;
        let mut changes = WorktreeChanges::default();
        for line in captured.text()?.lines() {
            if line.starts_with("?? ") {
                changes.untracked += 1;
            } else if line.starts_with("!! ") {
                changes.ignored += 1;
            } else if !line.is_empty() {
                changes.uncommitted += 1;
            }
        }
        Ok(changes)
    }
}
