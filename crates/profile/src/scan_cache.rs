//! Windows only: Unix reads an entry's owner and mode with one `lstat`, which a full scan affords on every start.
//! The folders a clean content scan found, with their modification times (ADR 0073). A folder whose time is unchanged
//! gained, lost and renamed no entry since, so a later start reads the descriptors of changed folders only. Used only
//! while the root stays private: then only trusted principals can change anything below it.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The cache file in the profile root.
pub(crate) const CACHE_FILE: &str = ".access-scan";

#[derive(Debug, Default)]
pub(crate) struct ScanCache {
    clean: HashMap<PathBuf, u128>,
}

impl ScanCache {
    /// The folders the last clean scan of `root` recorded; empty when there is no readable cache.
    pub(crate) fn load(root: &Path) -> Self {
        let Ok(text) = fs::read_to_string(root.join(CACHE_FILE)) else {
            return Self::default();
        };
        let clean = text
            .lines()
            .filter_map(|line| {
                let (stamp, relative) = line.split_once('\t')?;
                Some((PathBuf::from(relative), stamp.parse().ok()?))
            })
            .collect();
        Self { clean }
    }

    /// Whether `dir` is recorded clean with this modification time.
    pub(crate) fn is_unchanged(&self, root: &Path, dir: &Path, stamp: Option<u128>) -> bool {
        let (Some(stamp), Ok(relative)) = (stamp, dir.strip_prefix(root)) else {
            return false;
        };
        self.clean.get(relative) == Some(&stamp)
    }

    pub(crate) fn record(&mut self, root: &Path, dir: &Path, stamp: Option<u128>) {
        if let (Some(stamp), Ok(relative)) = (stamp, dir.strip_prefix(root)) {
            self.clean.insert(relative.to_path_buf(), stamp);
        }
    }

    /// Writes the cache next to the folders it describes, through a temporary file renamed into place.
    pub(crate) fn save(&self, root: &Path) -> io::Result<()> {
        let mut text = String::new();
        for (relative, stamp) in &self.clean {
            if let Some(relative) = relative.to_str() {
                text.push_str(&format!("{stamp}\t{relative}\n"));
            }
        }
        let temp = root.join(format!("{CACHE_FILE}.tmp"));
        fs::write(&temp, text)?;
        fs::rename(&temp, root.join(CACHE_FILE))
    }

    /// Forgets a previous scan, so the next start reads every descriptor.
    pub(crate) fn clear(root: &Path) {
        let _ = fs::remove_file(root.join(CACHE_FILE));
    }
}

/// A folder's modification time in nanoseconds, or `None` when it cannot be read.
pub(crate) fn stamp(modified: io::Result<SystemTime>) -> Option<u128> {
    modified
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_nanos())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::ScanCache;

    #[test]
    fn a_saved_cache_reads_back_and_a_changed_time_misses() {
        let temp = tempfile::tempdir().expect("a folder");
        let root = temp.path();
        let mut cache = ScanCache::default();
        cache.record(root, &root.join("agents").join("homes"), Some(42));
        cache.record(root, root, Some(7));
        cache.save(root).expect("saved");

        let loaded = ScanCache::load(root);
        assert!(loaded.is_unchanged(root, &root.join("agents").join("homes"), Some(42)));
        assert!(loaded.is_unchanged(root, root, Some(7)));
        assert!(!loaded.is_unchanged(root, &root.join("agents").join("homes"), Some(43)));
        assert!(!loaded.is_unchanged(root, &root.join("agents"), Some(42)));
        assert!(!loaded.is_unchanged(root, root, None));
        assert!(!ScanCache::load(Path::new("no-such-folder")).is_unchanged(root, root, Some(7)));
    }
}
