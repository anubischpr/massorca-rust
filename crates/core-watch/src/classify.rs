//! Which part of a repository a changed path belongs to (decision 0012).

use std::path::{Path, PathBuf};

/// What changed; a client refreshes only the views that depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WatchKind {
    /// A file under the working tree.
    Worktree,
    /// The index (`.git/index`).
    Index,
    /// `HEAD`, or the state of a merge, rebase, cherry-pick or revert.
    Head,
    /// Branches and tags: `refs/`, `packed-refs`, `FETCH_HEAD`.
    Refs,
}

/// The directories of one work tree, all absolute and canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchSpec {
    pub worktree: PathBuf,
    /// The work tree's own git directory (`<common>/worktrees/<id>` for a linked worktree).
    pub git_dir: PathBuf,
    /// Where refs and objects live; the same as `git_dir` for a main worktree.
    pub common_dir: PathBuf,
}

/// Files in a git directory whose change means HEAD or an operation in progress changed.
const HEAD_FILES: &[&str] = &[
    "HEAD",
    "ORIG_HEAD",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "rebase-merge",
    "rebase-apply",
];

impl WatchSpec {
    /// The kind of change at `path`, or `None` for a path no view depends on (objects, logs, lock files).
    pub fn classify(&self, path: &Path) -> Option<WatchKind> {
        if path
            .extension()
            .is_some_and(|extension| extension == "lock")
        {
            return None;
        }
        if let Ok(inside) = path.strip_prefix(&self.git_dir) {
            return git_dir_kind(inside, true);
        }
        if let Ok(inside) = path.strip_prefix(&self.common_dir) {
            // Another worktree's own directory, under `worktrees/`, is not ours.
            if inside.starts_with("worktrees") {
                return None;
            }
            return git_dir_kind(inside, false);
        }
        let inside = path.strip_prefix(&self.worktree).ok()?;
        // A nested `.git` (a submodule, or a git directory that is not ours) is not the working tree.
        if inside
            .components()
            .any(|component| component.as_os_str() == ".git")
        {
            return None;
        }
        Some(WatchKind::Worktree)
    }
}

/// `own` is true inside the work tree's own git directory, where the index and HEAD live.
fn git_dir_kind(inside: &Path, own: bool) -> Option<WatchKind> {
    let first = inside.components().next()?.as_os_str().to_str()?;
    match first {
        "refs" | "packed-refs" | "FETCH_HEAD" => Some(WatchKind::Refs),
        "index" if own => Some(WatchKind::Index),
        name if own && HEAD_FILES.contains(&name) => Some(WatchKind::Head),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{WatchKind, WatchSpec};

    fn main_worktree() -> WatchSpec {
        let root = PathBuf::from("/w/repo");
        WatchSpec {
            git_dir: root.join(".git"),
            common_dir: root.join(".git"),
            worktree: root,
        }
    }

    fn linked_worktree() -> WatchSpec {
        WatchSpec {
            worktree: PathBuf::from("/w/feature"),
            git_dir: PathBuf::from("/w/repo/.git/worktrees/feature"),
            common_dir: PathBuf::from("/w/repo/.git"),
        }
    }

    #[test]
    fn a_main_worktree_sorts_its_paths() {
        let spec = main_worktree();
        let kind = |path: &str| spec.classify(&PathBuf::from(path));
        assert_eq!(kind("/w/repo/src/a.rs"), Some(WatchKind::Worktree));
        assert_eq!(kind("/w/repo/.git/index"), Some(WatchKind::Index));
        assert_eq!(kind("/w/repo/.git/HEAD"), Some(WatchKind::Head));
        assert_eq!(
            kind("/w/repo/.git/rebase-merge/done"),
            Some(WatchKind::Head)
        );
        assert_eq!(kind("/w/repo/.git/refs/heads/main"), Some(WatchKind::Refs));
        assert_eq!(kind("/w/repo/.git/packed-refs"), Some(WatchKind::Refs));
        assert_eq!(kind("/w/repo/.git/index.lock"), None);
        assert_eq!(kind("/w/repo/.git/objects/ab/cdef"), None);
        assert_eq!(kind("/w/repo/.git/logs/HEAD"), None);
        assert_eq!(kind("/w/repo/vendor/sub/.git/index"), None);
        assert_eq!(kind("/elsewhere/file"), None);
    }

    #[test]
    fn a_linked_worktree_ignores_the_other_worktrees_directories() {
        let spec = linked_worktree();
        let kind = |path: &str| spec.classify(&PathBuf::from(path));
        assert_eq!(kind("/w/feature/a.txt"), Some(WatchKind::Worktree));
        assert_eq!(
            kind("/w/repo/.git/worktrees/feature/index"),
            Some(WatchKind::Index)
        );
        assert_eq!(
            kind("/w/repo/.git/worktrees/feature/HEAD"),
            Some(WatchKind::Head)
        );
        assert_eq!(kind("/w/repo/.git/worktrees/other/index"), None);
        assert_eq!(
            kind("/w/repo/.git/refs/heads/feature"),
            Some(WatchKind::Refs)
        );
        // The main worktree's index and HEAD are not this worktree's.
        assert_eq!(kind("/w/repo/.git/index"), None);
        assert_eq!(kind("/w/repo/.git/HEAD"), None);
        assert_eq!(kind("/w/repo/README.md"), None);
    }
}
