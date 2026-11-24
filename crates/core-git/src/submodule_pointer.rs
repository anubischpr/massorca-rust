//! The pointer diff of a submodule (a gitlink, mode 160000): the commit each side records, and whether a checked-out
//! submodule has changes. Its files are never read; git reads them only when the submodule itself is asked.

use std::path::Path;

use massorca_protocol::methods::git::diff::SubmodulePointer;

use crate::diff::ZERO_OID;
use crate::error::GitError;
use crate::git::Git;

/// The mode git gives a submodule entry.
pub(crate) const GITLINK_MODE: &str = "160000";

impl Git {
    /// The pointer of one changed submodule. `old` and `new` are the commits recorded on the sides that are
    /// submodules; `reads_worktree` means the new side is the working tree, whose commit is the submodule's HEAD.
    pub(crate) fn submodule_pointer(
        &self,
        dir: &Path,
        path: &str,
        old: Option<&str>,
        new: Option<&str>,
        reads_worktree: bool,
    ) -> Result<SubmodulePointer, GitError> {
        let checkout = dir.join(path);
        let (new_commit, dirty) = if !reads_worktree {
            let recorded = new.filter(|oid| *oid != ZERO_OID).map(str::to_owned);
            (recorded, None)
        } else if let Some(head) = self.checkout_head(&checkout)? {
            (Some(head), self.checkout_dirty(&checkout)?)
        } else {
            (None, None)
        };
        let old_commit = old.map(str::to_owned);
        Ok(SubmodulePointer {
            summary: summary(old_commit.as_deref(), new_commit.as_deref()),
            old_commit,
            new_commit,
            dirty,
        })
    }

    /// The HEAD of the submodule checked out at `checkout`; `None` when nothing is checked out there. Git's own
    /// discovery would walk up to the parent repository, so the checkout must be the top of its own repository.
    fn checkout_head(&self, checkout: &Path) -> Result<Option<String>, GitError> {
        if !is_checkout_root(checkout) {
            return Ok(None);
        }
        let prefix = self.run(Some(checkout), ["rev-parse", "--show-prefix"])?;
        if !prefix.succeeded() || !prefix.text()?.trim().is_empty() {
            return Ok(None);
        }
        let head = self.run(Some(checkout), ["rev-parse", "--verify", "-q", "HEAD"])?;
        if !head.succeeded() {
            return Ok(None);
        }
        Ok(Some(head.text()?.trim().to_owned()))
    }

    /// Whether the submodule checked out at `checkout` has changes, untracked files included.
    fn checkout_dirty(&self, checkout: &Path) -> Result<Option<bool>, GitError> {
        let status = self.run(Some(checkout), ["status", "--porcelain", "-z"])?;
        Ok(status.succeeded().then(|| !status.bytes().is_empty()))
    }
}

/// `dir` is a directory holding its own `.git`, so a git run there never reads a parent repository.
fn is_checkout_root(dir: &Path) -> bool {
    std::fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.is_dir())
        && dir.join(".git").exists()
}

/// Git's one-line form of the pointer change; with one side unknown it names the other.
fn summary(old: Option<&str>, new: Option<&str>) -> String {
    match (old, new) {
        (Some(old), Some(new)) => format!("Subproject commit {old}..{new}"),
        (Some(commit), None) | (None, Some(commit)) => format!("Subproject commit {commit}"),
        (None, None) => "Subproject commit unknown".to_owned(),
    }
}
