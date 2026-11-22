//! Errors, and the reserved `extension.*` codes that cross the protocol later.

use std::fmt;

/// Stable codes for the protocol. They use the reserved `extension.` prefix.
pub mod codes {
    pub const INVALID_MANIFEST: &str = "extension.invalid_manifest";
    pub const INVALID_HASH: &str = "extension.invalid_hash";
    pub const HASH_MISMATCH: &str = "extension.hash_mismatch";
    pub const INVALID_ARCHIVE: &str = "extension.invalid_archive";
    pub const UNSAFE_PATH: &str = "extension.unsafe_path";
    pub const UNSAFE_ENTRY: &str = "extension.unsafe_entry";
    pub const LIMIT_EXCEEDED: &str = "extension.limit_exceeded";
    pub const MISSING_FILE: &str = "extension.missing_file";
}

/// One rule the manifest breaks. `path` is a JSON-pointer-like location such as `/contributes/commands/0/id`; the
/// empty string names the whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = if self.path.is_empty() {
            "<document>"
        } else {
            self.path.as_str()
        };
        write!(f, "{location}: {}", self.message)
    }
}

/// Everything this crate refuses. Each variant maps to one code through [`Error::code`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid manifest: {}", join(.0))]
    InvalidManifest(Vec<Problem>),
    #[error("invalid SHA-256 pin: {0}")]
    InvalidHash(String),
    #[error("archive hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("invalid archive: {0}")]
    InvalidArchive(String),
    #[error("unsafe path {path:?}: {reason}")]
    UnsafePath { path: String, reason: &'static str },
    #[error("unsafe archive entry {path:?}: {reason}")]
    UnsafeEntry { path: String, reason: &'static str },
    #[error("archive limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("referenced files are missing: {}", join(.0))]
    MissingFiles(Vec<Problem>),
}

impl Error {
    /// The protocol code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidManifest(_) => codes::INVALID_MANIFEST,
            Self::InvalidHash(_) => codes::INVALID_HASH,
            Self::HashMismatch { .. } => codes::HASH_MISMATCH,
            Self::InvalidArchive(_) => codes::INVALID_ARCHIVE,
            Self::UnsafePath { .. } => codes::UNSAFE_PATH,
            Self::UnsafeEntry { .. } => codes::UNSAFE_ENTRY,
            Self::LimitExceeded(_) => codes::LIMIT_EXCEEDED,
            Self::MissingFiles(_) => codes::MISSING_FILE,
        }
    }
}

fn join(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_maps_to_a_reserved_extension_code() {
        let errors = [
            Error::InvalidManifest(Vec::new()),
            Error::InvalidHash(String::new()),
            Error::HashMismatch {
                expected: String::new(),
                actual: String::new(),
            },
            Error::InvalidArchive(String::new()),
            Error::UnsafePath {
                path: String::new(),
                reason: "absolute path",
            },
            Error::UnsafeEntry {
                path: String::new(),
                reason: "symbolic link",
            },
            Error::LimitExceeded(String::new()),
            Error::MissingFiles(Vec::new()),
        ];
        let mut codes_seen: Vec<&str> = errors.iter().map(Error::code).collect();
        codes_seen.sort_unstable();
        codes_seen.dedup();
        assert_eq!(codes_seen.len(), errors.len());
        assert!(codes_seen.iter().all(|code| code.starts_with("extension.")));
    }

    #[test]
    fn a_problem_displays_its_path_or_the_document_root() {
        let at_root = Problem {
            path: String::new(),
            message: "is required".to_owned(),
        };
        let nested = Problem {
            path: "/name".to_owned(),
            message: "is required".to_owned(),
        };
        assert_eq!(at_root.to_string(), "<document>: is required");
        assert_eq!(nested.to_string(), "/name: is required");
    }
}
