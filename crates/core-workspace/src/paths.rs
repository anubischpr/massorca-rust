//! Path spelling rules (ADR 0007): canonical form, UTF-8 text, and the key that decides equality.

use std::path::{Component, Path, PathBuf};

use massorca_protocol::codes;

use crate::error::DomainError;

/// The canonical form of `path`. A path that does not exist yet takes the canonical form of
/// its nearest existing ancestor, so a workspace directory keeps one key before and after
/// it is created or deleted.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    let normalized = normalize_lexically(path);
    if let Ok(resolved) = dunce::canonicalize(&normalized) {
        return resolved;
    }
    let mut missing = Vec::new();
    for ancestor in normalized.ancestors() {
        if let Ok(mut resolved) = dunce::canonicalize(ancestor) {
            for part in missing.iter().rev() {
                resolved.push(part);
            }
            return resolved;
        }
        if let Some(name) = ancestor.file_name() {
            missing.push(name.to_os_string());
        }
    }
    normalized
}

/// The equality key of `path`: its canonical text, lowercased where the filesystem ignores case.
pub(crate) fn path_key(path: &Path) -> String {
    let text = canonical(path).to_string_lossy().into_owned();
    if cfg!(any(windows, target_os = "macos")) {
        text.to_lowercase()
    } else {
        text
    }
}

/// The UTF-8 text stored for `path`; a non-UTF-8 path is `path.not_utf8`.
pub(crate) fn path_text(path: &Path) -> Result<String, DomainError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| DomainError::new(codes::PATH_NOT_UTF8, "path is not valid UTF-8"))
}

/// The canonical form of `path`, which must name an existing directory.
pub(crate) fn existing_directory(path: &Path) -> Result<PathBuf, DomainError> {
    let resolved = dunce::canonicalize(path).map_err(|error| {
        DomainError::new(
            codes::PATH_NOT_FOUND,
            format!("{} does not exist ({error})", path.display()),
        )
    })?;
    if !resolved.is_dir() {
        return Err(DomainError::new(
            codes::PATH_NOT_DIRECTORY,
            format!("{} is not a directory", path.display()),
        ));
    }
    path_text(&resolved)?;
    Ok(resolved)
}

/// A path that must be absolute: relative paths depend on the service's working directory.
pub(crate) fn absolute(text: &str) -> Result<PathBuf, DomainError> {
    let path = PathBuf::from(text);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(DomainError::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("path {text:?} must be absolute"),
        ))
    }
}

/// Why a path does not name a regular file inside a workspace root (decision 0172).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRefusal {
    /// The path holds `..`, is relative with a prefix or root in it, or reaches a file outside the root, through a
    /// symlink or a junction too.
    Outside,
    /// No file can be reached at the path: it does not exist, or it cannot be read.
    Missing,
    /// The path names a directory or another kind of entry.
    NotRegularFile,
    /// The file's path inside the root is not valid UTF-8, so it cannot be written in a prompt.
    NotText,
}

/// A regular file inside a workspace root, as the containment check found it (decision 0172).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedFile {
    /// The canonical path of the file, with symlinks and junctions followed.
    pub path: PathBuf,
    /// The path from the root, with `/` between names: the spelling a prompt mentions the file by.
    pub relative: String,
}

/// The regular file `text` names inside `root` (decision 0172). A relative `text` is taken from the root and holds
/// plain names only. An absolute one may name any place, but it is accepted only when the file it reaches, after
/// symlinks and junctions are followed, lies inside the root.
pub fn regular_file_inside(root: &Path, text: &str) -> Result<ContainedFile, FileRefusal> {
    let given = Path::new(text);
    if given
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(FileRefusal::Outside);
    }
    let root = dunce::canonicalize(root).map_err(|_| FileRefusal::Missing)?;
    let candidate = if given.is_absolute() {
        given.to_path_buf()
    } else {
        if !given
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(FileRefusal::Outside);
        }
        root.join(given)
    };
    let resolved = dunce::canonicalize(&candidate).map_err(|_| FileRefusal::Missing)?;
    if !resolved.starts_with(&root) {
        return Err(FileRefusal::Outside);
    }
    if !resolved.is_file() {
        return Err(FileRefusal::NotRegularFile);
    }
    let mut names = Vec::new();
    for part in resolved
        .strip_prefix(&root)
        .map_err(|_| FileRefusal::Outside)?
        .components()
    {
        let Component::Normal(name) = part else {
            return Err(FileRefusal::Outside);
        };
        names.push(name.to_str().ok_or(FileRefusal::NotText)?);
    }
    let relative = names.join("/");
    Ok(ContainedFile {
        path: resolved,
        relative,
    })
}

/// Removes `.` and resolves `..` against earlier components, without touching the disk.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use massorca_protocol::codes;

    use super::{
        FileRefusal, absolute, existing_directory, path_key, path_text, regular_file_inside,
    };

    #[test]
    fn a_relative_path_is_refused_as_invalid_params() {
        let error = absolute("relative/dir").expect_err("relative");
        assert_eq!(error.code(), codes::REQUEST_INVALID_PARAMS);
    }

    #[test]
    fn an_absolute_path_is_kept_as_given() {
        let dir = tempfile::tempdir().expect("temp dir");
        let text = dir.path().to_str().expect("utf-8");
        assert_eq!(absolute(text).expect("absolute"), dir.path());
    }

    #[test]
    fn a_missing_path_is_not_found_and_a_file_is_not_a_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = existing_directory(&dir.path().join("nope")).expect_err("missing");
        assert_eq!(missing.code(), codes::PATH_NOT_FOUND);
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").expect("write file");
        let not_dir = existing_directory(&file).expect_err("file");
        assert_eq!(not_dir.code(), codes::PATH_NOT_DIRECTORY);
    }

    #[test]
    fn paths_that_name_the_same_place_have_the_same_key() {
        let dir = tempfile::tempdir().expect("temp dir");
        let direct = dir.path().join("child");
        let roundabout = dir.path().join("missing").join("..").join("child");
        assert_eq!(path_key(&direct), path_key(&roundabout));
    }

    /// Links the directory `link` to `target`: a junction on Windows, which needs no privilege, and a symlink elsewhere.
    /// False when the link cannot be made, which fails the test that asked for it.
    fn link_directory(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            massorca_process::command("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|output| output.status.success())
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }

    #[test]
    fn a_relative_file_inside_the_root_resolves_to_its_canonical_path() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(root.path().join("src")).expect("dir");
        let file = root.path().join("src").join("main.rs");
        std::fs::write(&file, "fn main() {}").expect("file");
        let found = regular_file_inside(root.path(), "src/main.rs").expect("inside");
        assert_eq!(found.path, dunce::canonicalize(&file).expect("canonical"));
        assert_eq!(found.relative, "src/main.rs");
    }

    #[test]
    fn a_parent_component_is_outside_whether_or_not_the_file_exists() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::write(root.path().join("a.txt"), "x").expect("file");
        assert_eq!(
            regular_file_inside(root.path(), "../a.txt"),
            Err(FileRefusal::Outside)
        );
        assert_eq!(
            regular_file_inside(root.path(), "sub/../a.txt"),
            Err(FileRefusal::Outside)
        );
        assert_eq!(
            regular_file_inside(root.path(), "../gone.txt"),
            Err(FileRefusal::Outside)
        );
    }

    #[test]
    fn an_absolute_file_outside_the_root_is_outside() {
        let root = tempfile::tempdir().expect("temp dir");
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let file = elsewhere.path().join("notes.txt");
        std::fs::write(&file, "x").expect("file");
        let text = file.to_str().expect("utf-8");
        assert_eq!(
            regular_file_inside(root.path(), text),
            Err(FileRefusal::Outside)
        );
    }

    #[test]
    fn an_absolute_file_inside_the_root_is_accepted() {
        let root = tempfile::tempdir().expect("temp dir");
        let file = root.path().join("notes.txt");
        std::fs::write(&file, "x").expect("file");
        let text = file.to_str().expect("utf-8");
        assert_eq!(
            regular_file_inside(root.path(), text).expect("inside").path,
            dunce::canonicalize(&file).expect("canonical")
        );
    }

    #[test]
    fn a_missing_file_is_missing() {
        let root = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            regular_file_inside(root.path(), "nope.txt"),
            Err(FileRefusal::Missing)
        );
    }

    #[test]
    fn a_directory_is_not_a_regular_file() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(root.path().join("dir")).expect("dir");
        assert_eq!(
            regular_file_inside(root.path(), "dir"),
            Err(FileRefusal::NotRegularFile)
        );
    }

    #[test]
    fn a_link_that_leaves_the_root_is_outside() {
        let root = tempfile::tempdir().expect("temp dir");
        let elsewhere = tempfile::tempdir().expect("temp dir");
        std::fs::write(elsewhere.path().join("secret.txt"), "x").expect("file");
        let link = root.path().join("outside");
        assert!(
            link_directory(elsewhere.path(), &link),
            "a junction or symlink can be made"
        );
        assert!(link.join("secret.txt").is_file(), "the link resolves");
        assert_eq!(
            regular_file_inside(root.path(), "outside/secret.txt"),
            Err(FileRefusal::Outside)
        );
    }

    #[test]
    fn a_link_that_stays_inside_the_root_resolves_to_its_target() {
        let root = tempfile::tempdir().expect("temp dir");
        let real = root.path().join("real");
        std::fs::create_dir(&real).expect("dir");
        std::fs::write(real.join("file.txt"), "x").expect("file");
        let link = root.path().join("alias");
        assert!(
            link_directory(&real, &link),
            "a junction or symlink can be made"
        );
        assert!(link.join("file.txt").is_file(), "the link resolves");
        let found = regular_file_inside(root.path(), "alias/file.txt").expect("inside");
        assert_eq!(
            found.path,
            dunce::canonicalize(real.join("file.txt")).expect("canonical")
        );
        assert_eq!(found.relative, "real/file.txt");
    }

    #[test]
    fn a_utf8_path_is_stored_as_its_text() {
        assert_eq!(path_text(Path::new("some/dir")).expect("utf-8"), "some/dir");
    }
}
