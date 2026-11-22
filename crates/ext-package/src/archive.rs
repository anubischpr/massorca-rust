//! The `.mxt` archive check. The pinned SHA-256 is verified first; then every entry is checked and read into memory, and
//! the manifest and the files it names are validated. Nothing is written to disk.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use sha2::{Digest, Sha256};

use crate::error::Error;
use crate::manifest::{self, Manifest, check_files_present};
use crate::package_path;

/// Entries an archive may hold (directories included). Conservative: a package is a manifest, views and a few files.
pub const DEFAULT_MAX_ENTRIES: usize = 2_000;

/// Total uncompressed bytes an archive may expand to, 64 MiB. It bounds memory and a zip bomb alike.
pub const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;

/// The limits an archive is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub max_entries: usize,
    pub max_uncompressed_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            max_uncompressed_bytes: DEFAULT_MAX_UNCOMPRESSED_BYTES,
        }
    }
}

/// One file entry of a verified archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub path: String,
    pub size: u64,
}

/// A verified package, held in memory.
#[derive(Debug)]
pub struct Package {
    pub manifest: Manifest,
    /// File entries in archive order; directories are not listed.
    pub entries: Vec<EntryInfo>,
    /// Every file's bytes, keyed by its package path.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Verifies that `bytes` hash to the pinned SHA-256 (64 hex characters, either case).
pub fn verify_sha256(bytes: &[u8], pinned: &str) -> Result<(), Error> {
    if pinned.len() != 64 || !pinned.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::InvalidHash(pinned.to_owned()));
    }
    let expected = pinned.to_ascii_lowercase();
    let actual: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual == expected {
        Ok(())
    } else {
        Err(Error::HashMismatch { expected, actual })
    }
}

/// Verifies and opens an archive: the hash, then each entry, then the manifest and the files it names.
pub fn open(bytes: &[u8], pinned_sha256: &str, limits: ArchiveLimits) -> Result<Package, Error> {
    verify_sha256(bytes, pinned_sha256)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| Error::InvalidArchive(format!("not a zip archive: {error}")))?;
    if archive.len() > limits.max_entries {
        return Err(Error::LimitExceeded(format!(
            "{} entries, the limit is {}",
            archive.len(),
            limits.max_entries
        )));
    }

    let mut files = BTreeMap::new();
    let mut entries = Vec::new();
    let mut exact = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            Error::InvalidArchive(format!("entry {index} cannot be read: {error}"))
        })?;
        let name = file
            .name()
            .map_err(|error| {
                Error::InvalidArchive(format!("entry {index} has an unreadable name: {error}"))
            })?
            .into_owned();
        if file.is_dir() {
            check_name(&name, true)?;
            continue;
        }
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
        {
            return Err(Error::UnsafeEntry {
                path: name,
                reason: "symbolic link",
            });
        }
        check_name(&name, false)?;
        if !exact.insert(name.clone()) {
            return Err(Error::UnsafeEntry {
                path: name,
                reason: "duplicate name",
            });
        }
        if !folded.insert(name.to_ascii_lowercase()) {
            return Err(Error::UnsafeEntry {
                path: name,
                reason: "differs from another entry only in case",
            });
        }
        let remaining = limits.max_uncompressed_bytes.saturating_sub(total);
        if file.size() > remaining {
            return Err(limit_error(limits));
        }
        let mut contents = Vec::new();
        (&mut file)
            .take(remaining.saturating_add(1))
            .read_to_end(&mut contents)
            .map_err(|error| Error::InvalidArchive(format!("{name} cannot be read: {error}")))?;
        let size = contents.len() as u64;
        if size > remaining {
            return Err(limit_error(limits));
        }
        total += size;
        entries.push(EntryInfo {
            path: name.clone(),
            size,
        });
        files.insert(name, contents);
    }

    let manifest = {
        let bytes = files.get(manifest::MANIFEST_FILE).ok_or_else(|| {
            Error::InvalidArchive(format!("no {} at the root", manifest::MANIFEST_FILE))
        })?;
        manifest::load(bytes)?
    };
    check_files_present(&manifest, files.keys().map(String::as_str))?;
    Ok(Package {
        manifest,
        entries,
        files,
    })
}

/// Checks one entry name. Glob characters are allowed in names; traversal, absolute paths and separators are not.
fn check_name(name: &str, is_dir: bool) -> Result<(), Error> {
    let trimmed = if is_dir {
        name.strip_suffix('/').unwrap_or(name)
    } else {
        name
    };
    package_path::check(trimmed, true).map_err(|reason| Error::UnsafePath {
        path: name.to_owned(),
        reason,
    })
}

fn limit_error(limits: ArchiveLimits) -> Error {
    Error::LimitExceeded(format!(
        "more than {} uncompressed bytes",
        limits.max_uncompressed_bytes
    ))
}
