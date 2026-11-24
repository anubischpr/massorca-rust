//! The bounded walk that finds the `SKILL.md` files under one root (pv96's `skill-root-file-walk.ts`).

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SKILL_FILE_NAME: &str = "SKILL.md";

/// What one walk found: the `SKILL.md` files, and whether the entry budget ran out before the walk finished.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Walk {
    pub files: Vec<PathBuf>,
    pub truncated: bool,
}

/// The `SKILL.md` files below `root`, at most `max_file_depth` folders deep with the file counted. Each directory entry
/// the walk examines takes one unit of `budget`; when none is left the walk stops and reports itself truncated. `Err`
/// only when `root` itself cannot be listed; a folder below it that cannot be read is skipped.
pub fn find_skill_files(
    root: &Path,
    max_file_depth: usize,
    budget: &mut usize,
) -> io::Result<Walk> {
    fs::read_dir(root)?;
    let mut walk = Walk::default();
    let mut visited = HashSet::new();
    visit(root, 0, max_file_depth - 1, &mut visited, budget, &mut walk);
    Ok(walk)
}

fn visit(
    folder: &Path,
    depth: usize,
    max_folder_depth: usize,
    visited: &mut HashSet<PathBuf>,
    budget: &mut usize,
    walk: &mut Walk,
) {
    if walk.truncated {
        return;
    }
    // A symlinked folder can point back up the tree, so each real folder is walked once.
    let real = fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    if !visited.insert(real) {
        return;
    }
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            walk.truncated = true;
            return;
        }
        *budget -= 1;
        let path = entry.path();
        if entry.file_name() == SKILL_FILE_NAME {
            if fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
                walk.files.push(path);
            }
            continue;
        }
        let is_folder = match entry.file_type() {
            Ok(kind) if kind.is_symlink() => fs::metadata(&path).is_ok_and(|meta| meta.is_dir()),
            Ok(kind) => kind.is_dir(),
            Err(_) => false,
        };
        if is_folder && depth < max_folder_depth {
            visit(&path, depth + 1, max_folder_depth, visited, budget, walk);
        }
    }
}
