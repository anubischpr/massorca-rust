//! Lock state read from git's own admin files, for git before 2.31: its porcelain list
//! prints no `locked` line, but every lock is a `locked` file beside the worktree's record.

use std::path::{Path, PathBuf};

use crate::git::path_from_git;
use crate::worktree::WorktreeEntry;

/// Fills `locked` of each linked worktree that the porcelain list left empty. `entries` starts
/// with the main worktree, whose `.git` directory holds the admin records of all the others.
/// An unreadable admin directory leaves the entries unchanged.
pub(crate) fn fill_lock_reasons(entries: &mut [WorktreeEntry]) {
    let Some(main) = entries.first() else {
        return;
    };
    let common = main.path.join(".git");
    let admin_root = if common.is_dir() {
        common.join("worktrees")
    } else {
        main.path.join("worktrees")
    };
    let Ok(records) = std::fs::read_dir(admin_root) else {
        return;
    };
    for record in records.flatten() {
        let Some(lock) = read_lock(&record.path()) else {
            continue;
        };
        let Some(worktree) = worktree_of(&record.path()) else {
            continue;
        };
        for entry in entries.iter_mut().skip(1) {
            if entry.locked.is_none() && same_directory(&entry.path, &worktree) {
                entry.locked = Some(lock.clone());
            }
        }
    }
}

/// The reason in `record/locked`: empty when locked without one, `None` when not locked.
fn read_lock(record: &Path) -> Option<String> {
    let text = std::fs::read_to_string(record.join("locked")).ok()?;
    Some(text.trim_end_matches(['\r', '\n']).to_owned())
}

/// The worktree directory a record belongs to: its `gitdir` file names `<worktree>/.git`.
fn worktree_of(record: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(record.join("gitdir")).ok()?;
    let dot_git = path_from_git(text.trim());
    dot_git.parent().map(Path::to_path_buf)
}

/// Whether two spellings name one directory. A missing directory cannot be canonicalised, so
/// its text is compared, with separators and case folded.
fn same_directory(a: &Path, b: &Path) -> bool {
    let canonical = (dunce::canonicalize(a), dunce::canonicalize(b));
    matches!(canonical, (Ok(ref x), Ok(ref y)) if x == y) || folded(a) == folded(b)
}

fn folded(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/');
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.to_owned()
    }
}
