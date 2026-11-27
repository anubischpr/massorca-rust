//! Copies a skill folder and digests it. Links and other non-file entries are refused, so a placed copy can never reach
//! outside its folder, and a copy is staged next to its target and renamed into place, so a half-copied folder never
//! shows under a skill name.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::placement_error::PlacementError;

/// The most files one placed skill may hold.
const MAX_FILES: usize = 2_000;
/// The most bytes one placed skill may hold.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// How many folders a skill may nest.
const MAX_DEPTH: usize = 16;
/// How old a staging folder must be before a sweep takes it for a crash's leftover.
const STALE_STAGING: Duration = Duration::from_secs(10 * 60);

/// The entries of a skill folder, sorted by their path below it.
#[derive(Default)]
struct Survey {
    dirs: Vec<PathBuf>,
    files: Vec<(PathBuf, PathBuf)>,
}

/// Copies `source` to `target`, which must not exist. The copy is staged beside `target` and renamed into place.
pub(super) fn place_copy(source: &Path, target: &Path) -> Result<(), PlacementError> {
    let parent = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_at("cannot create", parent))?;
    let staging = parent.join(staging_name(target));
    if let Err(error) = copy_tree(source, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if fs::symlink_metadata(target).is_ok() {
        let _ = fs::remove_dir_all(&staging);
        return Err(PlacementError::Io {
            message: "a folder is already there",
            path: target.to_path_buf(),
            source: io::Error::from(io::ErrorKind::AlreadyExists),
        });
    }
    fs::rename(&staging, target).map_err(|source| {
        let _ = fs::remove_dir_all(&staging);
        io_at("cannot move into place", target)(source)
    })
}

/// A digest of a skill folder: every directory and file, by its path and bytes. Two folders with the same digest hold the
/// same skill.
pub(super) fn tree_digest(root: &Path) -> Result<String, PlacementError> {
    let survey = survey(root)?;
    let mut hasher = Sha256::new();
    for dir in &survey.dirs {
        hasher.update(b"dir\0");
        hasher.update(relative(dir).as_bytes());
        hasher.update(b"\0");
    }
    for (rel, absolute) in &survey.files {
        let bytes = fs::read(absolute).map_err(io_at("cannot read", absolute))?;
        hasher.update(b"file\0");
        hasher.update(relative(rel).as_bytes());
        hasher.update(b"\0");
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Whether `path` is a real folder: not a link, and not a file.
pub(super) fn is_plain_folder(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && !is_link(&meta))
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), PlacementError> {
    let survey = survey(source)?;
    fs::create_dir(target).map_err(io_at("cannot create", target))?;
    for dir in &survey.dirs {
        let to = target.join(dir);
        fs::create_dir(&to).map_err(io_at("cannot create", &to))?;
    }
    for (rel, absolute) in &survey.files {
        let to = target.join(rel);
        fs::copy(absolute, &to).map_err(io_at("cannot copy", absolute))?;
    }
    Ok(())
}

fn survey(root: &Path) -> Result<Survey, PlacementError> {
    if !is_plain_folder(root) {
        return Err(unsupported(root, "it is not a plain folder"));
    }
    let mut survey = Survey::default();
    let mut bytes = 0u64;
    let mut pending = vec![(root.to_path_buf(), PathBuf::new(), 0usize)];
    while let Some((folder, below, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(unsupported(root, "it nests too deeply"));
        }
        let entries = fs::read_dir(&folder).map_err(io_at("cannot read", &folder))?;
        for entry in entries {
            let entry = entry.map_err(io_at("cannot read", &folder))?;
            let path = entry.path();
            let rel = below.join(entry.file_name());
            let meta = fs::symlink_metadata(&path).map_err(io_at("cannot read", &path))?;
            if is_link(&meta) {
                return Err(unsupported(&path, "links are not copied"));
            }
            if meta.is_dir() {
                survey.dirs.push(rel.clone());
                pending.push((path, rel, depth + 1));
            } else if meta.is_file() {
                bytes = bytes.saturating_add(meta.len());
                if survey.files.len() >= MAX_FILES || bytes > MAX_BYTES {
                    return Err(unsupported(root, "it is too large to place"));
                }
                survey.files.push((rel, path));
            } else {
                return Err(unsupported(&path, "only files and folders are copied"));
            }
        }
    }
    survey.dirs.sort();
    survey.files.sort();
    Ok(survey)
}

/// A symbolic link, or on Windows a junction or other reparse point.
pub(super) fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn relative(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn staging_name(target: &Path) -> String {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!(".{name}.placing-{}-{}", process::id(), now_nanos())
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default()
}

/// The creation time a staging name's `<pid>-<nanos>` tail records, when the tail has exactly that shape.
fn staging_stamp(tail: &str) -> Option<u128> {
    let (pid, nanos) = tail.split_once('-')?;
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(pid) || !digits(nanos) {
        return None;
    }
    pid.parse::<u32>().ok()?;
    nanos.parse().ok()
}

fn unsupported(path: &Path, reason: &'static str) -> PlacementError {
    PlacementError::Unsupported {
        path: path.to_path_buf(),
        reason,
    }
}

fn io_at(message: &'static str, path: &Path) -> impl FnOnce(io::Error) -> PlacementError {
    let path = path.to_path_buf();
    move |source| PlacementError::Io {
        message,
        path,
        source,
    }
}

/// Removes the staging folders a crash left beside `folder` for the skill `name`. Only plain folders named exactly
/// `.<name>.placing-<pid>-<nanos>` go, and only once older than `STALE_STAGING`, so another process's copy in progress,
/// a link and a folder the user made are never touched.
pub(super) fn sweep_staging(folder: &Path, name: &str) {
    let prefix = format!(".{name}.placing-");
    let now = now_nanos();
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(stamp) = file_name
            .to_str()
            .and_then(|text| text.strip_prefix(&prefix))
            .and_then(staging_stamp)
        else {
            continue;
        };
        // A stamp ahead of the clock counts as young.
        if now.saturating_sub(stamp) < STALE_STAGING.as_nanos() {
            continue;
        }
        let path = entry.path();
        if is_plain_folder(&path) {
            let _ = fs::remove_dir_all(path);
        }
    }
}
