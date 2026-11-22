//! Install sources: `catalogue:<id>`, `dir:<path>` and `file:<path>`, each read into memory and checked with the
//! package format's own rules before anything is written.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use massorca_ext_package::archive::{self, ArchiveLimits};
use massorca_ext_package::manifest::agent::reserves_identity;
use massorca_ext_package::manifest::{self, Manifest, check_files_present};
use massorca_ext_package::package_path;
use sha2::{Digest, Sha256};

use crate::catalogue;
use crate::error::Error;
use crate::lockfile::{LockSource, SourceKind};

/// A package's files, keyed by package path.
pub type Files = BTreeMap<String, Vec<u8>>;

/// Where a package comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Catalogue(String),
    Dir(PathBuf),
    File(PathBuf),
}

impl Source {
    /// Reads `catalogue:<id>`, `dir:<path>` or `file:<path>`.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let invalid = || Error::InvalidSource(text.to_owned());
        let (kind, rest) = text.split_once(':').ok_or_else(invalid)?;
        if rest.is_empty() {
            return Err(invalid());
        }
        if kind != "catalogue" && is_network_or_device_path(rest) {
            return Err(invalid());
        }
        match kind {
            "catalogue" => Ok(Self::Catalogue(rest.to_owned())),
            "dir" => Ok(Self::Dir(PathBuf::from(rest))),
            "file" => Ok(Self::File(PathBuf::from(rest))),
            _ => Err(invalid()),
        }
    }

    /// The lockfile record of this source.
    pub fn lock_source(&self) -> LockSource {
        let (kind, reference) = match self {
            Self::Catalogue(id) => (SourceKind::Catalogue, id.clone()),
            Self::Dir(path) => (SourceKind::Dir, path.to_string_lossy().into_owned()),
            Self::File(path) => (SourceKind::File, path.to_string_lossy().into_owned()),
        };
        LockSource {
            kind,
            reference,
            extra: Default::default(),
        }
    }
}

/// A UNC share or a device namespace path (`\\?\`, `\\.\`, `\??\`): reading one could reach another machine or a
/// device, so a source never names one. Windows reads any two leading separators (`\\`, `//`, `\/`, `/\`) as a share.
fn is_network_or_device_path(path: &str) -> bool {
    let mut leading = path.chars();
    let separator = |c: Option<char>| matches!(c, Some('\\' | '/'));
    path.starts_with(r"\\")
        || path.starts_with(r"\??\")
        || (cfg!(windows) && separator(leading.next()) && separator(leading.next()))
}

/// A checked package, held in memory.
#[derive(Debug)]
pub struct LoadedPackage {
    pub manifest: Manifest,
    pub files: Files,
    /// The archive's SHA-256, or the canonical hash of a folder or catalogue entry.
    pub sha256: String,
    pub source: LockSource,
}

/// Loads and checks a package. With a `pinned` hash the package must match it; an archive without one is hashed as
/// read, which only a review may do (an install requires the pin).
pub fn load(source: &Source, pinned: Option<&str>) -> Result<LoadedPackage, Error> {
    let lock_source = source.lock_source();
    let (manifest, files, sha256) = match source {
        Source::File(path) => {
            let bytes = read_archive(path)?;
            let sha256 = sha256_hex(&bytes);
            let package =
                archive::open(&bytes, pinned.unwrap_or(&sha256), ArchiveLimits::default())?;
            (package.manifest, package.files, sha256)
        }
        Source::Catalogue(id) => {
            let files = catalogue::find(id)
                .ok_or_else(|| Error::NotFound(id.clone()))?
                .files();
            checked(files, pinned)?
        }
        Source::Dir(path) => checked(read_dir(path)?, pinned)?,
    };
    if !matches!(source, Source::Catalogue(_))
        && reserves_identity(&manifest.id, &manifest.publisher)
    {
        return Err(Error::ReservedIdentity {
            id: manifest.id.clone(),
            reference: lock_source.reference.clone(),
        });
    }
    Ok(LoadedPackage {
        manifest,
        files,
        sha256,
        source: lock_source,
    })
}

/// Validates the manifest and its files, and checks the canonical hash against the pin.
fn checked(files: Files, pinned: Option<&str>) -> Result<(Manifest, Files, String), Error> {
    let stream = canonical_stream(&files);
    if let Some(pinned) = pinned {
        archive::verify_sha256(&stream, pinned)?;
    }
    let manifest = manifest_of(&files)?;
    Ok((manifest, files, sha256_hex(&stream)))
}

/// Parses and validates `extension.json`, then checks every file it names is present.
pub(crate) fn manifest_of(files: &Files) -> Result<Manifest, Error> {
    let bytes = files.get(manifest::MANIFEST_FILE).ok_or_else(|| {
        massorca_ext_package::Error::InvalidArchive(format!(
            "no {} at the root",
            manifest::MANIFEST_FILE
        ))
    })?;
    let manifest = manifest::load(bytes)?;
    check_files_present(&manifest, files.keys().map(String::as_str))?;
    Ok(manifest)
}

/// The canonical hash of a folder or catalogue entry: SHA-256 over the sorted list of path, NUL, length, NUL, bytes.
pub fn canonical_sha256(files: &Files) -> String {
    sha256_hex(&canonical_stream(files))
}

fn canonical_stream(files: &Files) -> Vec<u8> {
    let mut stream = Vec::new();
    for (path, bytes) in files {
        stream.extend_from_slice(path.as_bytes());
        stream.push(0);
        stream.extend_from_slice(bytes.len().to_string().as_bytes());
        stream.push(0);
        stream.extend_from_slice(bytes);
    }
    stream
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_archive(path: &Path) -> Result<Vec<u8>, Error> {
    let limit = ArchiveLimits::default().max_uncompressed_bytes;
    read_regular_file(path, limit, path)?.ok_or_else(|| {
        massorca_ext_package::Error::InvalidArchive("the source is not a regular file".to_owned())
            .into()
    })
}

/// Opens `path` once and reads it, if the opened handle is a regular file; `None` for anything else (a FIFO, a
/// socket, a device). More than `limit` bytes is refused while reading, whatever the size the file reported. An I/O
/// error names `shown`, the path the caller sent, and never a deeper host path.
fn read_regular_file(path: &Path, limit: u64, shown: &Path) -> Result<Option<Vec<u8>>, Error> {
    // Opening a FIFO blocks until a writer comes, so the path is checked before it is opened.
    if !fs::symlink_metadata(path)
        .map_err(Error::io("cannot read", shown))?
        .is_file()
    {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(Error::io("cannot read", shown))?;
    if !file
        .metadata()
        .map_err(Error::io("cannot read", shown))?
        .is_file()
    {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(Error::io("cannot read", shown))?;
    if bytes.len() as u64 > limit {
        return Err(
            massorca_ext_package::Error::LimitExceeded(format!("more than {limit} bytes")).into(),
        );
    }
    Ok(Some(bytes))
}

/// Reads a package folder with the archive's rules: safe names, no symbolic links, no case-only duplicates, only
/// regular files and folders, and the same entry and size limits, enforced while reading.
pub(crate) fn read_dir(root: &Path) -> Result<Files, Error> {
    let limits = ArchiveLimits::default();
    let mut files = BTreeMap::new();
    let mut folded = BTreeSet::new();
    let mut total: u64 = 0;
    let mut pending = vec![(root.to_path_buf(), String::new())];
    let mut entries = 0usize;
    let cannot_read = || Error::io("cannot read", root);
    while let Some((dir, prefix)) = pending.pop() {
        for item in fs::read_dir(&dir).map_err(cannot_read())? {
            let item = item.map_err(cannot_read())?;
            entries += 1;
            if entries > limits.max_entries {
                return Err(massorca_ext_package::Error::LimitExceeded(format!(
                    "more than {} entries",
                    limits.max_entries
                ))
                .into());
            }
            let name = item.file_name().into_string().map_err(|name| {
                massorca_ext_package::Error::UnsafePath {
                    path: name.to_string_lossy().into_owned(),
                    reason: "not valid UTF-8",
                }
            })?;
            let rel = format!("{prefix}{name}");
            package_path::check(&rel, true).map_err(|reason| {
                massorca_ext_package::Error::UnsafePath {
                    path: rel.clone(),
                    reason,
                }
            })?;
            let kind = item.file_type().map_err(cannot_read())?;
            if kind.is_symlink() {
                return Err(unsafe_entry(rel, "symbolic link"));
            }
            if kind.is_dir() {
                pending.push((item.path(), format!("{rel}/")));
                continue;
            }
            if !kind.is_file() {
                return Err(unsafe_entry(rel, "not a regular file or folder"));
            }
            if !folded.insert(rel.to_ascii_lowercase()) {
                return Err(unsafe_entry(rel, "differs from another entry only in case"));
            }
            let left = limits.max_uncompressed_bytes - total;
            let bytes = read_regular_file(&item.path(), left, root)
                .map_err(|error| match error {
                    Error::Package(massorca_ext_package::Error::LimitExceeded(_)) => {
                        massorca_ext_package::Error::LimitExceeded(format!(
                            "more than {} bytes",
                            limits.max_uncompressed_bytes
                        ))
                        .into()
                    }
                    other => other,
                })?
                .ok_or_else(|| unsafe_entry(rel.clone(), "not a regular file or folder"))?;
            total += bytes.len() as u64;
            files.insert(rel, bytes);
        }
    }
    Ok(files)
}

fn unsafe_entry(path: String, reason: &'static str) -> Error {
    massorca_ext_package::Error::UnsafeEntry { path, reason }.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_parse_by_prefix() {
        assert_eq!(
            Source::parse("catalogue:massorca.claude").expect("catalogue"),
            Source::Catalogue("massorca.claude".to_owned())
        );
        assert_eq!(
            Source::parse("dir:C:/Users/me/kit").expect("dir"),
            Source::Dir(PathBuf::from("C:/Users/me/kit"))
        );
        assert_eq!(
            Source::parse("file:kit.mxt").expect("file"),
            Source::File(PathBuf::from("kit.mxt"))
        );
        for bad in ["kit", "dir:", "http:x"] {
            let error = Source::parse(bad).expect_err(bad);
            assert_eq!(error.code(), crate::error::codes::INVALID_SOURCE);
        }
    }

    #[test]
    fn the_canonical_hash_covers_paths_lengths_and_bytes_in_sorted_order() {
        let files: Files = [
            ("b".to_owned(), b"2".to_vec()),
            ("a".to_owned(), b"1".to_vec()),
        ]
        .into();
        assert_eq!(
            canonical_sha256(&files),
            sha256_hex(b"a\x001\x001b\x001\x002")
        );
    }
}
