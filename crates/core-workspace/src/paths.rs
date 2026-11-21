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

    use super::{absolute, existing_directory, path_key, path_text};

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

    #[test]
    fn a_utf8_path_is_stored_as_its_text() {
        assert_eq!(path_text(Path::new("some/dir")).expect("utf-8"), "some/dir");
    }
}
