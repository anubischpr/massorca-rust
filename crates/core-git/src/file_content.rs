//! One file's bytes on one side of a comparison: the working tree, the index or a revision (decision 0099).
//! The working tree is read from disk behind a containment check; the other sides come from git's object store.

use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use massorca_protocol::codes;
use massorca_protocol::methods::git::diff::FileSide;

use crate::error::GitError;
use crate::git::{Captured, Git};

/// What one side holds at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideContent {
    /// The side has no file at the path.
    Absent,
    /// The file is larger than the cap; its bytes were not read.
    TooLarge { size: u64 },
    /// The file's bytes, all of them.
    Bytes(Vec<u8>),
}

/// Why a file was not read, or a git failure while it was.
#[derive(Debug, thiserror::Error)]
pub enum FileContentError {
    /// A refusal: `code` names it.
    #[error("{message}")]
    Refused { code: &'static str, message: String },
    #[error(transparent)]
    Git(#[from] GitError),
}

/// What git lists at a path on the index or at a revision.
enum Listed {
    Absent,
    Blob(String),
    /// A directory, or a submodule (a gitlink), which holds no file of its own.
    Directory,
    /// Only unmerged entries: the index holds no single version of the file.
    Conflicted,
}

impl Git {
    /// The bytes of `path` on `side`, read only when they fit `max_bytes`. A path that leaves the work tree, names
    /// git's own folder, or names a directory or submodule is refused.
    pub fn file_content(
        &self,
        dir: &Path,
        side: &FileSide,
        path: &str,
        max_bytes: usize,
    ) -> Result<SideContent, FileContentError> {
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            return Err(refused(
                codes::GIT_PATH_IS_DIRECTORY,
                "the work tree itself is a directory, not a file".to_owned(),
            ));
        }
        if !is_inside_worktree(path) {
            return Err(outside(path));
        }
        match side {
            FileSide::Worktree => worktree_content(dir, path, max_bytes),
            FileSide::Index => {
                let captured = self.run(
                    Some(dir),
                    ["--literal-pathspecs", "ls-files", "-s", "-z", "--", path],
                )?;
                captured.require_success()?;
                self.content_of(dir, index_listed(&captured, path)?, path, max_bytes)
            }
            FileSide::Revision { rev } => {
                if !self.ref_exists(dir, rev)? {
                    return Err(refused(
                        codes::GIT_REVISION_NOT_FOUND,
                        format!("`{rev}` does not name a commit"),
                    ));
                }
                let captured = self.run(
                    Some(dir),
                    [
                        "--literal-pathspecs",
                        "ls-tree",
                        "-z",
                        rev.as_str(),
                        "--",
                        path,
                    ],
                )?;
                captured.require_success()?;
                self.content_of(dir, tree_listed(&captured, path)?, path, max_bytes)
            }
        }
    }

    fn content_of(
        &self,
        dir: &Path,
        listed: Listed,
        path: &str,
        max_bytes: usize,
    ) -> Result<SideContent, FileContentError> {
        match listed {
            Listed::Absent => Ok(SideContent::Absent),
            Listed::Blob(object) => Ok(self.blob_content(dir, &object, max_bytes)?),
            Listed::Directory => Err(refused(
                codes::GIT_PATH_IS_DIRECTORY,
                format!("`{path}` names a directory or a submodule, not a file"),
            )),
            Listed::Conflicted => Err(refused(
                codes::GIT_PATH_CONFLICTED,
                format!("`{path}` is conflicted in the index"),
            )),
        }
    }

    /// A blob's bytes, or only its size when it is over the cap: `cat-file -s` decides before any byte is read.
    fn blob_content(
        &self,
        dir: &Path,
        object: &str,
        max_bytes: usize,
    ) -> Result<SideContent, GitError> {
        let sized = self.run(Some(dir), ["cat-file", "-s", object])?;
        sized.require_success()?;
        let size: u64 = sized
            .text()?
            .trim()
            .parse()
            .map_err(|_| sized.invalid_output("cat-file printed no size"))?;
        if size > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
            return Ok(SideContent::TooLarge { size });
        }
        let blob = self.run(Some(dir), ["cat-file", "blob", object])?;
        blob.require_success()?;
        Ok(SideContent::Bytes(blob.bytes().to_vec()))
    }
}

fn worktree_content(
    dir: &Path,
    path: &str,
    max_bytes: usize,
) -> Result<SideContent, FileContentError> {
    let Some((full, metadata)) = worktree_file(dir, path)? else {
        return Ok(SideContent::Absent);
    };
    let cap = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if metadata.len() > cap {
        return Ok(SideContent::TooLarge {
            size: metadata.len(),
        });
    }
    let mut bytes = Vec::new();
    File::open(&full)
        .and_then(|file| file.take(cap.saturating_add(1)).read_to_end(&mut bytes))
        .map_err(|source| unreadable(&full, source))?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > cap {
        // The file grew after it was measured; no partial bytes are returned.
        return Ok(SideContent::TooLarge { size: read });
    }
    Ok(SideContent::Bytes(bytes))
}

/// The canonical path of `path` under `dir` with its metadata, or `None` when nothing exists there. A symlink is
/// followed only to a target that is still inside the work tree.
fn worktree_file(dir: &Path, path: &str) -> Result<Option<(PathBuf, Metadata)>, FileContentError> {
    let root = dunce::canonicalize(dir).map_err(|source| unreadable(dir, source))?;
    let joined = dir.join(path);
    let full = match dunce::canonicalize(&joined) {
        Ok(full) => full,
        Err(error) if is_absent(&error) => return Ok(None),
        Err(source) => return Err(unreadable(&joined, source)),
    };
    let Ok(inside) = full.strip_prefix(&root) else {
        return Err(outside(path));
    };
    if inside
        .components()
        .any(|part| part.as_os_str().eq_ignore_ascii_case(".git"))
    {
        return Err(refused(
            codes::REQUEST_INVALID_PARAMS,
            format!("`{path}` is inside git's own folder, not a workspace file"),
        ));
    }
    let metadata = std::fs::metadata(&full).map_err(|source| unreadable(&full, source))?;
    if metadata.is_dir() {
        return Err(refused(
            codes::GIT_PATH_IS_DIRECTORY,
            format!("`{path}` names a directory, not a file"),
        ));
    }
    if !metadata.is_file() {
        return Err(refused(
            codes::REQUEST_INVALID_PARAMS,
            format!("`{path}` is not a regular file"),
        ));
    }
    Ok(Some((full, metadata)))
}

/// The index entry of `path` from `ls-files -s -z`, one `<mode> <object> <stage>\t<path>` record per NUL.
fn index_listed(captured: &Captured, path: &str) -> Result<Listed, GitError> {
    let folder = format!("{path}/");
    let mut listed = Listed::Absent;
    for (meta, name) in records(captured) {
        if name.starts_with(&folder) {
            return Ok(Listed::Directory);
        }
        if name != path {
            continue;
        }
        let mut fields = meta.split(' ');
        let (Some(mode), Some(object), Some(stage)) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(captured.invalid_output("an index record has too few fields"));
        };
        if stage == "0" {
            listed = if mode == "160000" {
                Listed::Directory
            } else {
                Listed::Blob(object.to_owned())
            };
        } else if matches!(listed, Listed::Absent) {
            listed = Listed::Conflicted;
        }
    }
    Ok(listed)
}

/// The tree entry of `path` from `ls-tree -z`, one `<mode> <type> <object>\t<path>` record per NUL.
fn tree_listed(captured: &Captured, path: &str) -> Result<Listed, GitError> {
    for (meta, name) in records(captured) {
        if name != path {
            continue;
        }
        let mut fields = meta.split(' ');
        let (Some(_), Some(kind), Some(object)) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(captured.invalid_output("a tree record has too few fields"));
        };
        return Ok(if kind == "blob" {
            Listed::Blob(object.to_owned())
        } else {
            Listed::Directory
        });
    }
    Ok(Listed::Absent)
}

/// The `<metadata>\t<path>` records of a NUL-separated listing; names are read lossily, so a non-UTF-8 name never
/// equals a request.
fn records(captured: &Captured) -> Vec<(String, String)> {
    String::from_utf8_lossy(captured.bytes())
        .split('\0')
        .filter_map(|record| record.split_once('\t'))
        .map(|(meta, name)| (meta.to_owned(), name.to_owned()))
        .collect()
}

/// Whether `path` is spelled as a path inside the work tree: relative, with no `..`, drive or root.
fn is_inside_worktree(path: &str) -> bool {
    Path::new(path)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

/// Whether a failed lookup means the path does not exist, rather than that it could not be read.
fn is_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

fn refused(code: &'static str, message: String) -> FileContentError {
    FileContentError::Refused { code, message }
}

fn outside(path: &str) -> FileContentError {
    refused(
        codes::GIT_PATH_OUTSIDE_WORKTREE,
        format!("`{path}` is not a path inside the workspace"),
    )
}

fn unreadable(path: &Path, source: io::Error) -> FileContentError {
    GitError::File {
        path: path.to_path_buf(),
        source,
    }
    .into()
}
