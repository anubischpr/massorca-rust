//! Where agents keep their files in a profile (`docs/architecture/agents.md`, "Profile files").

use std::path::{Path, PathBuf};

use crate::kind::valid_id;

/// `<profile>/agents`.
pub fn agents_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("agents")
}

/// `<profile>/agents/kinds`: the profile's own kind declarations.
pub fn kinds_dir(profile_dir: &Path) -> PathBuf {
    agents_dir(profile_dir).join("kinds")
}

/// `<profile>/agents/homes/<kind>`: the kind's credential home (0056).
pub fn home_dir(profile_dir: &Path, kind: &str) -> PathBuf {
    agents_dir(profile_dir).join("homes").join(kind)
}

/// The credential home of a valid kind id; `None` for an id that could leave the homes directory.
pub fn valid_home(profile_dir: &Path, kind: &str) -> Option<PathBuf> {
    valid_id(kind).then(|| home_dir(profile_dir, kind))
}

/// `<profile>/agents/runs/<session>`: files generated for a session's runs (0053).
pub fn run_dir(profile_dir: &Path, session: &str) -> PathBuf {
    agents_dir(profile_dir).join("runs").join(session)
}
