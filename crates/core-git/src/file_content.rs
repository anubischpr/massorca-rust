//! One file's bytes on one side of a comparison: the working tree, the index or a revision (decision 0103).
//! The working tree is read from disk behind the checks in `worktree_file`; the other sides come from git's object store.

use std::io;
use std::path::{Component, Path};

use massorca_protocol::codes;
use massorca_protocol::methods::git::diff::FileSide;

use crate::error::GitError;
use crate::git::{Captured, Git};
use crate::worktree_file::worktree_content;

/// Bytes of an index listing kept: the first record of a path decides whether it is a file or a directory.
const MAX_LISTING_BYTES: usize = 64 * 1024;

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
    /// A file could not be read. `path` is the workspace-relative path the caller asked for, never a host path.
    #[error("`{path}` could not be read")]
    Unreadable {
        path: String,
        #[source]
        source: io::Error,
    },
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
    /// The bytes of `path` on `side`, read only when they fit `max_bytes`. The path is spelled once first, so every
    /// side reads the same file for the same request. A path that leaves the work tree, names git's own folder, or
    /// names a directory or submodule is refused.
    pub fn file_content(
        &self,
        dir: &Path,
        side: &FileSide,
        path: &str,
        max_bytes: usize,
    ) -> Result<SideContent, FileContentError> {
        let path = normalize_request(path)?;
        if path.is_empty() {
            return Err(refused(
                codes::GIT_PATH_IS_DIRECTORY,
                "the work tree itself is a directory, not a file".to_owned(),
            ));
        }
        match side {
            FileSide::Worktree => worktree_content(self, dir, &path, max_bytes),
            FileSide::Index => {
                let captured = self.run_capped(
                    Some(dir),
                    [
                        "--literal-pathspecs",
                        "ls-files",
                        "-s",
                        "-z",
                        "--",
                        path.as_str(),
                    ],
                    MAX_LISTING_BYTES,
                )?;
                if !captured.truncated() {
                    captured.require_success()?;
                }
                let listed = index_listed(&captured, &path)?;
                self.content_of(dir, listed, &path, max_bytes)
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
                        path.as_str(),
                    ],
                )?;
                captured.require_success()?;
                let listed = tree_listed(&captured, &path)?;
                self.content_of(dir, listed, &path, max_bytes)
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
    if captured.truncated() && matches!(listed, Listed::Absent) {
        return Err(captured.invalid_output("the index listing was cut short before the path"));
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

/// The request's path spelled once: relative, with `/` separators and no `.` or empty part (decision 0103). A rooted
/// path, a `..` part, or a `.git` part in any case is refused. On Windows a backslash separates parts too.
fn normalize_request(path: &str) -> Result<String, FileContentError> {
    if path.contains('\0') {
        return Err(refused(
            codes::REQUEST_INVALID_PARAMS,
            "the path contains a NUL byte".to_owned(),
        ));
    }
    if is_rooted(path) {
        return Err(outside(path));
    }
    let spelled = if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };
    let mut parts = Vec::new();
    for part in spelled.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(outside(path)),
            name if name.eq_ignore_ascii_case(".git") => {
                return Err(refused(
                    codes::REQUEST_INVALID_PARAMS,
                    format!("`{path}` is inside git's own folder, not a workspace file"),
                ));
            }
            name => parts.push(name),
        }
    }
    Ok(parts.join("/"))
}

/// Whether the path starts at a root or a drive, which a workspace-relative path never does.
fn is_rooted(path: &str) -> bool {
    Path::new(path)
        .components()
        .next()
        .is_some_and(|part| matches!(part, Component::RootDir | Component::Prefix(_)))
}

/// Whether a failed lookup means the path does not exist, rather than that it could not be read.
pub(crate) fn is_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

pub(crate) fn refused(code: &'static str, message: String) -> FileContentError {
    FileContentError::Refused { code, message }
}

pub(crate) fn outside(path: &str) -> FileContentError {
    refused(
        codes::GIT_PATH_OUTSIDE_WORKTREE,
        format!("`{path}` is not a path inside the workspace"),
    )
}

pub(crate) fn unreadable(path: &str, source: io::Error) -> FileContentError {
    FileContentError::Unreadable {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::codes;

    use super::{FileContentError, normalize_request};

    fn code_of(path: &str) -> &'static str {
        match normalize_request(path) {
            Err(FileContentError::Refused { code, .. }) => code,
            other => panic!("{path} should be refused, got {other:?}"),
        }
    }

    #[test]
    fn spellings_of_one_path_normalize_to_the_same_relative_path() {
        assert_eq!(normalize_request("./README.md").expect("path"), "README.md");
        assert_eq!(normalize_request("a//b/./c/").expect("path"), "a/b/c");
        assert_eq!(
            normalize_request("docs/./guide.md").expect("path"),
            "docs/guide.md"
        );
        assert_eq!(normalize_request(".").expect("path"), "");
    }

    #[test]
    fn a_climbing_or_rooted_path_is_outside_the_workspace() {
        for path in ["../x", "a/../../x", "/etc/passwd", "a/..", ".."] {
            assert_eq!(code_of(path), codes::GIT_PATH_OUTSIDE_WORKTREE, "{path}");
        }
    }

    #[test]
    fn a_git_part_is_refused_in_any_case() {
        for path in [".git", ".GIT/config", "x/.Git/HEAD", "./.git/config"] {
            assert_eq!(code_of(path), codes::REQUEST_INVALID_PARAMS, "{path}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn backslashes_separate_parts_and_rooted_or_drive_spellings_are_outside() {
        assert_eq!(
            normalize_request(r"docs\\guide.md").expect("path"),
            "docs/guide.md"
        );
        assert_eq!(
            normalize_request(r"docs/sub\guide.md").expect("path"),
            "docs/sub/guide.md"
        );
        for path in [r"..\x", r"\\?\C:\x", r"C:x", r"\x", r"\\server\share\x"] {
            assert_eq!(code_of(path), codes::GIT_PATH_OUTSIDE_WORKTREE, "{path}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn a_backslash_is_an_ordinary_character_off_windows() {
        assert_eq!(
            normalize_request(r"docs\guide.md").expect("path"),
            r"docs\guide.md"
        );
    }
}
