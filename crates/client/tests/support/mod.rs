//! A private home for client tests.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
pub struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}
