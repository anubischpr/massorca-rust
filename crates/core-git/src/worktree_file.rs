//! The working-tree side of `git.file_content` (decision 0103): a file on disk, read only once the file that was opened
//! is known to be inside the work tree. Windows checks the handle's own final path; Unix checks the identity of the file
//! the path names after the open, which leaves a window that decision 0103 records.

use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use massorca_protocol::codes;

use crate::file_content::{FileContentError, SideContent, is_absent, outside, refused, unreadable};
use crate::git::Git;

/// The bytes of `path` in the work tree `dir`, for a `path` that `normalize_request` has already spelled.
pub(crate) fn worktree_content(
    git: &Git,
    dir: &Path,
    path: &str,
    max_bytes: usize,
) -> Result<SideContent, FileContentError> {
    let root = dunce::canonicalize(dir).map_err(|source| unreadable(path, source))?;
    let joined = dir.join(path);
    let Some(location) = resolve(&joined, path)? else {
        return Ok(SideContent::Absent);
    };
    let folders = git.git_folders(dir, path)?;
    check_location(&root, &folders, &location, path)?;
    let named = fs::metadata(&location).map_err(|source| unreadable(path, source))?;
    refuse_unless_regular(&named, path)?;
    let file = match open_nonblocking(&location) {
        Ok(file) => file,
        Err(source) if is_absent(&source) => return Ok(SideContent::Absent),
        Err(source) => return Err(unreadable(path, source)),
    };
    let opened = file.metadata().map_err(|source| unreadable(path, source))?;
    refuse_unless_regular(&opened, path)?;
    confirm_opened(&file, &opened, &root, &folders, &joined, path)?;
    let cap = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if opened.len() > cap {
        return Ok(SideContent::TooLarge { size: opened.len() });
    }
    read_bounded(&file, cap, || {
        file.metadata().map(|metadata| metadata.len())
    })
    .map_err(|source| unreadable(path, source))
}

impl Git {
    /// This worktree's own git folder and the common folder that linked worktrees share, canonical (decision 0103).
    fn git_folders(&self, dir: &Path, path: &str) -> Result<Vec<PathBuf>, FileContentError> {
        let captured = self.run(
            Some(dir),
            ["rev-parse", "--absolute-git-dir", "--git-common-dir"],
        )?;
        captured.require_success()?;
        captured
            .text()?
            .lines()
            .map(|line| {
                dunce::canonicalize(dir.join(line)).map_err(|source| unreadable(path, source))
            })
            .collect()
    }
}

/// The canonical location of `joined`, or `None` when nothing exists there.
fn resolve(joined: &Path, path: &str) -> Result<Option<PathBuf>, FileContentError> {
    match dunce::canonicalize(joined) {
        Ok(location) => Ok(Some(location)),
        Err(source) if is_absent(&source) => Ok(None),
        Err(source) => Err(unreadable(path, source)),
    }
}

/// A location must lie inside the work tree, outside the repository's git folders, and carry no `.git` part.
fn check_location(
    root: &Path,
    folders: &[PathBuf],
    location: &Path,
    path: &str,
) -> Result<(), FileContentError> {
    if !is_within(root, location) {
        return Err(outside(path));
    }
    if folders.iter().any(|folder| is_within(folder, location)) || has_git_part(root, location) {
        return Err(refused(
            codes::REQUEST_INVALID_PARAMS,
            format!("`{path}` is inside git's own folder, not a workspace file"),
        ));
    }
    Ok(())
}

fn refuse_unless_regular(metadata: &Metadata, path: &str) -> Result<(), FileContentError> {
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
    Ok(())
}

/// Opens for reading without waiting: a FIFO swapped in after the check opens at once, and the type of the handle
/// then refuses it.
#[cfg(unix)]
fn open_nonblocking(location: &Path) -> io::Result<File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(location)
}

#[cfg(not(unix))]
fn open_nonblocking(location: &Path) -> io::Result<File> {
    File::open(location)
}

/// Checks that the handle that was opened is a file inside the work tree: on Windows by the handle's final path, which
/// the handle itself names, so no later swap of a path can change it.
#[cfg(windows)]
fn confirm_opened(
    file: &File,
    _opened: &Metadata,
    root: &Path,
    folders: &[PathBuf],
    _joined: &Path,
    path: &str,
) -> Result<(), FileContentError> {
    let location =
        crate::file_handle_path::final_path(file).map_err(|source| unreadable(path, source))?;
    check_location(root, folders, &location, path)
}

/// Checks that the handle that was opened is a file inside the work tree: on Unix by the identity of the file the path
/// names now. A window between this check and the read remains; decision 0103 records it.
#[cfg(unix)]
fn confirm_opened(
    _file: &File,
    opened: &Metadata,
    root: &Path,
    folders: &[PathBuf],
    joined: &Path,
    path: &str,
) -> Result<(), FileContentError> {
    use std::os::unix::fs::MetadataExt;
    let location = resolve(joined, path)?
        .ok_or_else(|| unreadable(path, io::Error::from(io::ErrorKind::NotFound)))?;
    check_location(root, folders, &location, path)?;
    let named = fs::metadata(&location).map_err(|source| unreadable(path, source))?;
    if named.dev() == opened.dev() && named.ino() == opened.ino() {
        Ok(())
    } else {
        Err(outside(path))
    }
}

/// Reads at most `cap` bytes. Past the cap it reports the size the file has now, which is never less than what was
/// read, and no partial bytes.
fn read_bounded(
    reader: impl Read,
    cap: u64,
    size_now: impl FnOnce() -> io::Result<u64>,
) -> io::Result<SideContent> {
    let mut bytes = Vec::new();
    reader.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > cap {
        return Ok(SideContent::TooLarge {
            size: size_now()?.max(read),
        });
    }
    Ok(SideContent::Bytes(bytes))
}

/// Whether `location` is `root` or lies below it. Both are compared without a verbatim (`\\?\`) prefix, which Windows
/// keeps on a long path and drops from a short one.
pub(crate) fn is_within(root: &Path, location: &Path) -> bool {
    strip_verbatim(location).starts_with(strip_verbatim(root))
}

/// Whether a part of `location` below `root` is named `.git` in any case; a link can lead to git's own file.
fn has_git_part(root: &Path, location: &Path) -> bool {
    strip_verbatim(location)
        .strip_prefix(strip_verbatim(root))
        .is_ok_and(|below| {
            below
                .components()
                .any(|part| part.as_os_str().eq_ignore_ascii_case(".git"))
        })
}

/// The path with its verbatim prefix replaced by the ordinary spelling: `\\?\C:\x` becomes `C:\x`, and
/// `\\?\UNC\server\share` becomes `\\server\share`.
fn strip_verbatim(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    text.strip_prefix(r"\\?\")
        .map_or_else(|| path.to_path_buf(), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;

    use massorca_protocol::codes;

    use super::{check_location, is_within, read_bounded, strip_verbatim};
    use crate::file_content::{FileContentError, SideContent};

    #[test]
    fn a_location_is_within_its_root_and_not_within_a_sibling_that_shares_a_prefix() {
        let root = Path::new("work");
        assert!(is_within(root, Path::new("work")));
        assert!(is_within(
            root,
            Path::new("work").join("a").join("b").as_path()
        ));
        assert!(!is_within(root, Path::new("workshop").join("a").as_path()));
        assert!(!is_within(root, Path::new("other").join("a").as_path()));
    }

    #[test]
    fn a_read_past_the_cap_reports_the_size_the_file_has_now() {
        let fits = read_bounded(Cursor::new(vec![7_u8; 10]), 10, || panic!("not asked"));
        assert_eq!(fits.expect("read"), SideContent::Bytes(vec![7; 10]));

        let grown = read_bounded(Cursor::new(vec![7_u8; 100]), 10, || Ok(100));
        assert_eq!(grown.expect("read"), SideContent::TooLarge { size: 100 });
    }

    #[test]
    fn a_location_under_a_git_folder_or_named_git_is_refused_and_one_beside_it_is_not() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dunce::canonicalize(dir.path()).expect("root");
        let folder = root.join(".git");
        std::fs::create_dir_all(folder.join("refs")).expect("create folder");
        let folders = vec![folder.clone()];
        let refused_code = |location: &Path| match check_location(&root, &folders, location, "x") {
            Err(FileContentError::Refused { code, .. }) => Some(code),
            Err(other) => panic!("unexpected error {other:?}"),
            Ok(()) => None,
        };

        assert_eq!(
            refused_code(&folder.join("config")),
            Some(codes::REQUEST_INVALID_PARAMS)
        );
        assert_eq!(
            refused_code(&root.join("sub").join(".GIT")),
            Some(codes::REQUEST_INVALID_PARAMS)
        );
        assert_eq!(refused_code(&root.join("README.md")), None);
        let sibling = dir.path().parent().expect("a parent").join("elsewhere");
        assert_eq!(
            refused_code(&sibling),
            Some(codes::GIT_PATH_OUTSIDE_WORKTREE)
        );
    }

    #[test]
    fn a_verbatim_prefix_is_dropped_for_comparison_on_both_sides() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\ws\a")),
            Path::new(r"C:\ws\a")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\server\share\ws")),
            Path::new(r"\\server\share\ws")
        );
        assert_eq!(strip_verbatim(Path::new(r"C:\ws")), Path::new(r"C:\ws"));
    }

    #[cfg(windows)]
    #[test]
    fn a_long_verbatim_location_is_within_a_short_root_and_a_long_root_contains_a_short_location() {
        let long = format!(
            r"\\?\C:\ws\{}\file.txt",
            "a-long-folder-name-0123456789\\".repeat(10)
        );
        assert!(is_within(Path::new(r"C:\ws"), Path::new(&long)));
        assert!(!is_within(Path::new(r"C:\wsx"), Path::new(&long)));
        assert!(is_within(
            Path::new(r"\\?\C:\ws\a-long-folder-name-0123456789"),
            Path::new(r"C:\ws\a-long-folder-name-0123456789\file.txt")
        ));
    }
}
