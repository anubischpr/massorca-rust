//! Prune state of linked worktrees for git before 2.31, whose porcelain list prints no `prunable` line. Git prunes a
//! linked worktree whose folder is gone and that is not locked, so the core asks the folder instead.

use std::io::ErrorKind;
use std::path::Path;

use crate::worktree::WorktreeEntry;

/// Git's own wording for a folder that is gone, so the same state reads the same on every git version.
pub(crate) const MISSING_FOLDER_REASON: &str = "gitdir file points to non-existent location";

/// Marks each linked worktree prunable when git has not said so, it is not locked and its folder is gone. `entries`
/// starts with the main worktree, which is never marked.
pub(crate) fn fill_prune_reasons(entries: &mut [WorktreeEntry]) {
    for entry in entries.iter_mut().skip(1) {
        if entry.bare || entry.locked.is_some() || entry.prunable.is_some() {
            continue;
        }
        if folder_is_gone(&entry.path) {
            entry.prunable = Some(MISSING_FOLDER_REASON.to_owned());
        }
    }
}

/// Only a not-found answer counts: a folder that cannot be read is not gone.
fn folder_is_gone(path: &Path) -> bool {
    matches!(std::fs::metadata(path), Err(error) if error.kind() == ErrorKind::NotFound)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{MISSING_FOLDER_REASON, fill_prune_reasons};
    use crate::worktree::WorktreeEntry;

    /// A folder that exists: this crate's directory.
    fn present() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// A folder that does not exist.
    fn missing() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-worktree-folder")
    }

    /// The main worktree followed by `linked`.
    fn listing(linked: Vec<WorktreeEntry>) -> Vec<WorktreeEntry> {
        let mut entries = vec![WorktreeEntry::new(present())];
        entries.extend(linked);
        entries
    }

    #[test]
    fn a_linked_worktree_whose_folder_is_gone_is_prunable() {
        let mut entries = listing(vec![WorktreeEntry::new(missing())]);
        fill_prune_reasons(&mut entries);
        assert_eq!(entries[1].prunable.as_deref(), Some(MISSING_FOLDER_REASON));
    }

    #[test]
    fn a_linked_worktree_whose_folder_exists_is_not_prunable() {
        let mut entries = listing(vec![WorktreeEntry::new(present())]);
        fill_prune_reasons(&mut entries);
        assert_eq!(entries[1].prunable, None);
    }

    #[test]
    fn a_locked_worktree_is_never_prunable() {
        let mut locked = WorktreeEntry::new(missing());
        locked.locked = Some("agent is running".to_owned());
        let mut entries = listing(vec![locked]);
        fill_prune_reasons(&mut entries);
        assert_eq!(entries[1].prunable, None);
    }

    #[test]
    fn a_reason_git_already_printed_is_kept() {
        let mut reported = WorktreeEntry::new(missing());
        reported.prunable = Some("reported by git".to_owned());
        let mut entries = listing(vec![reported]);
        fill_prune_reasons(&mut entries);
        assert_eq!(entries[1].prunable.as_deref(), Some("reported by git"));
    }

    #[test]
    fn the_main_worktree_is_never_marked() {
        let mut entries = vec![WorktreeEntry::new(missing())];
        fill_prune_reasons(&mut entries);
        assert_eq!(entries[0].prunable, None);
    }

    #[test]
    fn a_bare_entry_is_never_marked() {
        let mut bare = WorktreeEntry::new(missing());
        bare.bare = true;
        let mut entries = listing(vec![bare]);
        fill_prune_reasons(&mut entries);
        assert_eq!(entries[1].prunable, None);
    }
}
