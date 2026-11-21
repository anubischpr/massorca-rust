//! Repository queries: the top of a work tree and whether it has changes.

use std::path::{Path, PathBuf};

use crate::error::GitError;
use crate::git::{Git, path_from_git};

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
}
