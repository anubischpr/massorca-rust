use crate::version::GitVersion;

/// What the host's git is, as `Git::availability` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitAvailability {
    /// A git at or above `GitVersion::BASELINE`.
    Present(GitVersion),
    /// No git executable at the configured path.
    Missing,
    /// A git below the baseline; every command is refused with `GitError::TooOld`.
    TooOld(GitVersion),
    /// The probe could not run git or read a version from its output.
    Unknown(String),
}

impl GitAvailability {
    pub(crate) fn of_version(found: GitVersion) -> Self {
        if found < GitVersion::BASELINE {
            Self::TooOld(found)
        } else {
            Self::Present(found)
        }
    }
}
