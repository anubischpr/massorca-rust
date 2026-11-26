//! Where a profile's extensions live, under `<profile>/extensions/`.

use std::path::{Path, PathBuf};

use massorca_profile::Profile;

/// The extensions folder of one profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(profile: &Profile) -> Self {
        Self::at(profile.extensions_dir())
    }

    /// A layout rooted at `root`, the `extensions` folder itself.
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `pkgs/`, every unpacked package.
    pub fn pkgs_dir(&self) -> PathBuf {
        self.root.join("pkgs")
    }

    /// `pkgs/<id>/`, every unpacked version of one package.
    pub fn package_root(&self, id: &str) -> PathBuf {
        self.pkgs_dir().join(id)
    }

    /// The lockfile-relative path of one unpacked package: `pkgs/<id>/<version>-<hash8>`.
    pub fn package_rel_path(id: &str, version: &str, sha256: &str) -> String {
        let hash8: String = sha256.chars().take(8).collect();
        format!("pkgs/{id}/{version}-{hash8}")
    }

    /// Resolves a lockfile-relative path (always `/`-separated) under the root.
    pub fn resolve(&self, rel: &str) -> PathBuf {
        rel.split('/')
            .fold(self.root.clone(), |path, segment| path.join(segment))
    }

    /// `data/<id>/`, the package's own data and its `install.log`.
    pub fn data_dir(&self, id: &str) -> PathBuf {
        self.root.join("data").join(id)
    }

    /// `data/<id>/install.log`, written by the vendor installer run.
    pub fn install_log(&self, id: &str) -> PathBuf {
        self.data_dir(id).join("install.log")
    }

    /// `logs/<id>-uninstall.log`. Outside `data/<id>/`, which removal deletes before the uninstall runs.
    pub fn uninstall_log(&self, id: &str) -> PathBuf {
        self.root.join("logs").join(format!("{id}-uninstall.log"))
    }

    /// `logs/<id>-runs.json`, the package's run log. It stays after removal, so the removal is in it.
    pub fn run_log(&self, id: &str) -> PathBuf {
        self.root.join("logs").join(format!("{id}-runs.json"))
    }

    /// `lock.json`, the source of truth for what is installed.
    pub fn lock_path(&self) -> PathBuf {
        self.root.join("lock.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_follow_the_documented_layout() {
        let layout = Layout::at(PathBuf::from("ext"));
        let rel = Layout::package_rel_path("massorca.claude", "1.0.0", "1a2b3c4d5e6f");
        assert_eq!(rel, "pkgs/massorca.claude/1.0.0-1a2b3c4d");
        assert_eq!(
            layout.resolve(&rel),
            PathBuf::from("ext")
                .join("pkgs")
                .join("massorca.claude")
                .join("1.0.0-1a2b3c4d")
        );
        assert_eq!(
            layout.install_log("a.b"),
            PathBuf::from("ext")
                .join("data")
                .join("a.b")
                .join("install.log")
        );
        assert_eq!(layout.lock_path(), PathBuf::from("ext").join("lock.json"));
    }
}
