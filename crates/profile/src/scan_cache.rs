//! Windows only: Unix reads an entry's owner and mode with one `lstat`, which a full scan affords on every start.
//! The folders a clean content scan found, with their modification times (ADR 0073). A folder whose time is unchanged
//! gained, lost and renamed no entry since, so a later start reads the descriptors of changed folders only. A change
//! to an existing entry's owner or access list does not change its folder's time, so a full scan still runs once a
//! day, after a version change, and whenever the cache is missing or unreadable.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The cache file in the profile root.
pub(crate) const CACHE_FILE: &str = ".access-scan";

/// How long a full scan stays valid.
pub(crate) const FULL_SCAN_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// A cache larger than this is not read; a full scan runs instead.
const MAX_CACHE_BYTES: u64 = 16 * 1024 * 1024;

/// The version a cache is written by; another version's cache is ignored.
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Default)]
pub(crate) struct ScanCache {
    /// When the last full scan ran, in seconds since the Unix epoch.
    full_scan_at: u64,
    clean: HashMap<PathBuf, u128>,
}

impl ScanCache {
    /// The folders the last clean scan of `root` recorded, if that scan is recent enough and this version wrote it.
    /// `None` means a full scan is due.
    pub(crate) fn load(root: &Path, now: SystemTime) -> Option<Self> {
        let text = read_bounded(&root.join(CACHE_FILE))?;
        let mut lines = text.lines();
        let (version, full_scan_at) = lines.next()?.split_once('\t')?;
        let full_scan_at: u64 = full_scan_at.parse().ok()?;
        let age = seconds(now)?.checked_sub(full_scan_at)?;
        if version != VERSION || age >= FULL_SCAN_EVERY.as_secs() {
            return None;
        }
        let clean = lines
            .filter_map(|line| {
                let (stamp, relative) = line.split_once('\t')?;
                Some((PathBuf::from(relative), stamp.parse().ok()?))
            })
            .collect();
        Some(Self {
            full_scan_at,
            clean,
        })
    }

    /// An empty cache for a full scan that runs at `now`.
    pub(crate) fn full_scan(now: SystemTime) -> Self {
        Self {
            full_scan_at: seconds(now).unwrap_or_default(),
            clean: HashMap::new(),
        }
    }

    /// An empty cache that keeps this one's full-scan time, for a scan that skipped unchanged folders.
    pub(crate) fn next(&self) -> Self {
        Self {
            full_scan_at: self.full_scan_at,
            clean: HashMap::new(),
        }
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
        let mut text = format!("{VERSION}\t{}\n", self.full_scan_at);
        for (relative, stamp) in &self.clean {
            if let Some(relative) = relative.to_str() {
                text.push_str(&format!("{stamp}\t{relative}\n"));
            }
        }
        // Per process, so two services starting at once never write or rename each other's file.
        let temp = root.join(format!("{CACHE_FILE}.{}.tmp", std::process::id()));
        fs::write(&temp, text)?;
        fs::rename(&temp, root.join(CACHE_FILE)).inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })
    }

    /// Forgets a previous scan, so the next start reads every descriptor.
    pub(crate) fn clear(root: &Path) {
        let _ = fs::remove_file(root.join(CACHE_FILE));
    }
}

/// The file's text, or `None` when it is missing, unreadable, not UTF-8, or larger than `MAX_CACHE_BYTES`.
fn read_bounded(path: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(MAX_CACHE_BYTES + 1)
        .read_to_string(&mut text)
        .ok()?;
    (text.len() as u64 <= MAX_CACHE_BYTES).then_some(text)
}

fn seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
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
    use std::time::{Duration, SystemTime};

    use super::{CACHE_FILE, FULL_SCAN_EVERY, ScanCache};

    #[test]
    fn a_saved_cache_reads_back_and_a_changed_time_misses() {
        let temp = tempfile::tempdir().expect("a folder");
        let root = temp.path();
        let now = SystemTime::now();
        let mut cache = ScanCache::full_scan(now);
        cache.record(root, &root.join("agents").join("homes"), Some(42));
        cache.record(root, root, Some(7));
        cache.save(root).expect("saved");

        let loaded = ScanCache::load(root, now).expect("a fresh cache");
        assert!(loaded.is_unchanged(root, &root.join("agents").join("homes"), Some(42)));
        assert!(loaded.is_unchanged(root, root, Some(7)));
        assert!(!loaded.is_unchanged(root, &root.join("agents").join("homes"), Some(43)));
        assert!(!loaded.is_unchanged(root, &root.join("agents"), Some(42)));
        assert!(!loaded.is_unchanged(root, root, None));
        assert!(ScanCache::load(Path::new("no-such-folder"), now).is_none());
    }

    #[test]
    fn a_cache_from_another_version_a_day_old_or_oversized_is_not_used() {
        let temp = tempfile::tempdir().expect("a folder");
        let root = temp.path();
        let now = SystemTime::now();
        ScanCache::full_scan(now).save(root).expect("saved");
        assert!(
            ScanCache::load(root, now + FULL_SCAN_EVERY).is_none(),
            "a day old"
        );
        assert!(ScanCache::load(root, now + Duration::from_secs(60)).is_some());

        std::fs::write(root.join(CACHE_FILE), "0.0.0-other\t0\n").expect("another version");
        assert!(ScanCache::load(root, now).is_none(), "another version");

        let huge = "x".repeat(16 * 1024 * 1024 + 1);
        std::fs::write(root.join(CACHE_FILE), huge).expect("an oversized cache");
        assert!(ScanCache::load(root, now).is_none(), "oversized");
    }

    #[test]
    fn a_partial_scan_keeps_the_time_of_the_last_full_scan() {
        let temp = tempfile::tempdir().expect("a folder");
        let root = temp.path();
        let then = SystemTime::now() - Duration::from_secs(3600);
        ScanCache::full_scan(then).save(root).expect("saved");
        let loaded = ScanCache::load(root, SystemTime::now()).expect("fresh");
        loaded.next().save(root).expect("saved again");

        assert!(ScanCache::load(root, then + FULL_SCAN_EVERY).is_none());
    }
}
