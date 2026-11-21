//! Staging, unstaging and discarding (decisions 0010 and 0013). Paths are literal pathspecs, passed in chunks
//! because `--pathspec-from-file` needs Git 2.26 (decision 0014).

use std::collections::BTreeSet;
use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::index::GitDiscarded;
use massorca_protocol::methods::git::status::StatusEntry;

use crate::error::GitError;
use crate::git::Git;

/// Path bytes per git call, well under the Windows command-line limit of 32,767 characters.
const CHUNK_BYTES: usize = 24 * 1024;

/// The files a request names: listed paths, or every changed file.
#[derive(Debug, Clone, Copy)]
pub enum PathSet<'a> {
    Paths(&'a [String]),
    All,
}

/// The waivers of a discard; each allows one more kind of loss (decision 0013).
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscardWaivers {
    pub delete_untracked: bool,
    pub include_staged: bool,
}

/// Why a discard did not run, or a git failure while it ran.
#[derive(Debug, thiserror::Error)]
pub enum DiscardError {
    /// A refusal under decision 0013: `code` names it, `paths` are the files that caused it.
    #[error("{message}")]
    Refused {
        code: &'static str,
        message: String,
        paths: Vec<String>,
    },
    #[error(transparent)]
    Git(#[from] GitError),
}

impl Git {
    /// Stages the named files (additions, edits and deletions alike), or every change.
    pub fn stage(&self, dir: &Path, paths: PathSet<'_>) -> Result<(), GitError> {
        match paths {
            PathSet::All => self.mutate(dir, &["add", "-A"], &[]),
            PathSet::Paths(paths) => self.mutate(dir, &["add", "-A"], paths),
        }
    }

    /// Unstages the named files, or everything, keeping the working files.
    pub fn unstage(&self, dir: &Path, paths: PathSet<'_>) -> Result<(), GitError> {
        let born = self.ref_exists(dir, "HEAD")?;
        // Before the first commit there is no HEAD to reset to; the index entries are simply removed.
        let command: &[&str] = if born {
            &["reset", "-q"]
        } else {
            &["rm", "--cached", "-r", "-q", "--ignore-unmatch"]
        };
        match paths {
            PathSet::Paths(paths) => self.mutate(dir, command, paths),
            PathSet::All if born => self.mutate(dir, command, &[]),
            PathSet::All => self.mutate(dir, command, &[".".to_owned()]),
        }
    }

    /// Discards changes to the files `paths` names under `waivers`, after refusing anything the waivers do not cover.
    pub fn discard(
        &self,
        dir: &Path,
        paths: PathSet<'_>,
        waivers: DiscardWaivers,
        dry_run: bool,
    ) -> Result<GitDiscarded, DiscardError> {
        let status = self.status(dir, usize::MAX)?;
        let born = status.branch.oid.is_some();
        let entries: Vec<&StatusEntry> = match paths {
            PathSet::All => status.entries.iter().collect(),
            PathSet::Paths(paths) => {
                let mut picked = Vec::new();
                let mut directories = Vec::new();
                for path in paths {
                    match status.entries.iter().find(|entry| &entry.path == path) {
                        Some(entry) => picked.push(entry),
                        None if dir.join(path).is_dir() => directories.push(path.clone()),
                        // An unchanged file has nothing to discard.
                        None => {}
                    }
                }
                refuse_if(
                    codes::GIT_PATH_IS_DIRECTORY,
                    "discard acts on files, not directories",
                    directories,
                )?;
                picked
            }
        };
        let plan = plan(&entries, born, waivers)?;
        if !dry_run {
            self.apply(dir, &plan)?;
        }
        Ok(GitDiscarded { dry_run, ..plan })
    }

    fn apply(&self, dir: &Path, plan: &GitDiscarded) -> Result<(), GitError> {
        if !plan.restored_from_index.is_empty() {
            self.mutate(dir, &["checkout", "-q"], &plan.restored_from_index)?;
        }
        if !plan.restored_from_head.is_empty() {
            self.mutate(dir, &["checkout", "-q", "HEAD"], &plan.restored_from_head)?;
        }
        if !plan.deleted.is_empty() {
            self.mutate(
                dir,
                &["rm", "--cached", "-q", "--ignore-unmatch"],
                &plan.deleted,
            )?;
            for path in &plan.deleted {
                remove_inside(dir, path)?;
            }
        }
        Ok(())
    }

    /// Runs `git --literal-pathspecs <command> -- <paths>` once per chunk of paths; no paths runs it once bare.
    fn mutate(&self, dir: &Path, command: &[&str], paths: &[String]) -> Result<(), GitError> {
        let run = |chunk: &[String]| -> Result<(), GitError> {
            let mut args: Vec<&str> = vec!["--literal-pathspecs"];
            args.extend_from_slice(command);
            if !chunk.is_empty() {
                args.push("--");
                args.extend(chunk.iter().map(String::as_str));
            }
            self.run_mutation(Some(dir), args)?.require_success()
        };
        if paths.is_empty() {
            return run(&[]);
        }
        let mut start = 0;
        let mut bytes = 0;
        for (index, path) in paths.iter().enumerate() {
            if bytes + path.len() > CHUNK_BYTES && index > start {
                run(&paths[start..index])?;
                start = index;
                bytes = 0;
            }
            bytes += path.len() + 1;
        }
        run(&paths[start..])
    }
}

/// Sorts each entry into what restores it, refusing what the waivers do not allow.
fn plan(
    entries: &[&StatusEntry],
    born: bool,
    waivers: DiscardWaivers,
) -> Result<GitDiscarded, DiscardError> {
    let mut plan = GitDiscarded::default();
    let mut conflicted = Vec::new();
    let mut untracked = Vec::new();
    let mut staged_only = Vec::new();
    let mut head = BTreeSet::new();
    let mut deleted = BTreeSet::new();
    for entry in entries {
        let path = entry.path.clone();
        if entry.conflict.is_some() {
            conflicted.push(path);
        } else if entry.untracked {
            untracked.push(path.clone());
            deleted.insert(path);
        } else if waivers.include_staged && entry.staged.is_some() {
            match entry.staged {
                // Only the index has these files; restoring from HEAD means deleting them.
                Some(ChangeKind::Added | ChangeKind::Copied) => {
                    untracked.push(path.clone());
                    deleted.insert(path);
                }
                Some(ChangeKind::Renamed) => {
                    untracked.push(path.clone());
                    deleted.insert(path);
                    if let Some(original) = &entry.original_path {
                        head.insert(original.clone());
                    }
                }
                _ if !born => {
                    untracked.push(path.clone());
                    deleted.insert(path);
                }
                _ => {
                    head.insert(path);
                }
            }
        } else if entry.unstaged.is_some() {
            plan.restored_from_index.push(path);
        } else if entry.staged.is_some() {
            staged_only.push(path);
        }
    }
    refuse_if(
        codes::GIT_PATH_CONFLICTED,
        "unmerged files cannot be discarded; resolve or abort first",
        conflicted,
    )?;
    if !waivers.delete_untracked {
        refuse_if(
            codes::GIT_DISCARD_UNTRACKED,
            "discarding these deletes files git has no copy of; pass delete_untracked",
            untracked,
        )?;
    }
    refuse_if(
        codes::GIT_DISCARD_STAGED,
        "these files have only staged changes; pass include_staged to drop them",
        staged_only,
    )?;
    plan.restored_from_head = head.into_iter().collect();
    plan.deleted = deleted.into_iter().collect();
    Ok(plan)
}

fn refuse_if(code: &'static str, message: &str, paths: Vec<String>) -> Result<(), DiscardError> {
    if paths.is_empty() {
        return Ok(());
    }
    Err(DiscardError::Refused {
        code,
        message: format!("{message}: {}", paths.join(", ")),
        paths,
    })
}

/// Deletes `path` under `dir`, refusing a target whose parent resolves outside the work tree.
fn remove_inside(dir: &Path, path: &str) -> Result<(), GitError> {
    let full = dir.join(path);
    let unreadable = |source| GitError::File {
        path: full.clone(),
        source,
    };
    let root = dunce::canonicalize(dir).map_err(unreadable)?;
    let parent = full
        .parent()
        .map(dunce::canonicalize)
        .transpose()
        .map_err(unreadable)?;
    if !parent.is_some_and(|parent| parent.starts_with(&root)) {
        return Err(GitError::File {
            path: full.clone(),
            source: std::io::Error::other("the file is outside the work tree"),
        });
    }
    match std::fs::remove_file(&full) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(unreadable(error)),
        _ => Ok(()),
    }
}
