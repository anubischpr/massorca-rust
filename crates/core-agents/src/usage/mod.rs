//! Usage and limits of an account, read from the vendor CLI's own logs in the account's folder (decision 0154).
//!
//! Only log and state files that hold no secrets are read, and only their numbers are kept. Nothing is fetched over the
//! network. The walks never follow a link out of the folder and stop at fixed bounds, so a large history stays cheap.

mod claude;
mod codex;

use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use massorca_protocol::methods::usage::{UsageSource as SourceName, UsageWindow};

/// How far back the readers look: the longest window, plus a day for files touched just before it opened.
const LOOKBACK: Duration = Duration::from_secs(8 * 24 * 60 * 60);
/// Files read per account at most, newest first.
const MAX_FILES: usize = 400;
/// Lines longer than this are skipped unread: usage records are short, transcripts of large outputs are not.
const MAX_LINE_BYTES: usize = 1024 * 1024;

const MINUTE_MS: i64 = 60 * 1000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const WEEK_MS: i64 = 7 * 24 * HOUR_MS;

/// What one read of an account found.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageReading {
    /// The source that produced the reading.
    pub source: SourceName,
    pub windows: Vec<UsageWindow>,
    /// The time of the newest record used.
    pub observed_at_ms: Option<i64>,
}

/// Whether `kind` has a usage reader.
pub fn supports(kind: &str) -> bool {
    !sources(kind).is_empty()
}

/// One way to read a vendor's usage for an account (decision 0154). A kind lists its sources in order; the local logs come
/// last, as the fallback of any source added before them.
pub trait UsageSource: Send + Sync {
    /// The name answers carry for readings of this source.
    fn name(&self) -> SourceName;
    /// Reads the account whose vendor config folder is `home`, as of `now_ms`. `Ok(None)` when this source has nothing
    /// for it, so the next source is tried.
    fn read(&self, home: &Path, now_ms: i64) -> io::Result<Option<UsageReading>>;
}

/// The sources of `kind`, in the order they are tried.
pub fn sources(kind: &str) -> &'static [&'static dyn UsageSource] {
    match kind {
        claude::KIND => &[&claude::ClaudeLogs],
        codex::KIND => &[&codex::CodexLogs],
        _ => &[],
    }
}

/// Reads the usage of the account whose vendor config folder is `home`, as of `now_ms`, from the first of its kind's
/// sources that has a reading. `Ok(None)` when none has; an error when a source failed and no later source had a reading.
pub fn read(kind: &str, home: &Path, now_ms: i64) -> io::Result<Option<UsageReading>> {
    let mut failure = None;
    for source in sources(kind) {
        match source.read(home, now_ms) {
            Ok(Some(reading)) => return Ok(Some(reading)),
            Ok(None) => {}
            Err(error) => failure = Some(error),
        }
    }
    failure.map_or(Ok(None), Err)
}

/// The `.jsonl` files under `root`, at most `depth` folders down, modified since `LOOKBACK` before `now_ms`, newest
/// first and at most `MAX_FILES`. Links are not followed, `root` included. `None` when `root` does not exist or is not a
/// plain folder.
fn recent_logs(root: &Path, depth: usize, now_ms: i64) -> io::Result<Option<Vec<PathBuf>>> {
    // A link or junction at the root could point anywhere: it counts as no data.
    match fs::symlink_metadata(root) {
        Ok(meta) if meta.is_dir() && !meta.is_symlink() => {}
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let since = system_time(now_ms).checked_sub(LOOKBACK);
    let mut found = Vec::new();
    collect(entries, depth, since, &mut found);
    found.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    found.truncate(MAX_FILES);
    Ok(Some(found.into_iter().map(|(_, path)| path).collect()))
}

fn collect(
    entries: fs::ReadDir,
    depth: usize,
    since: Option<SystemTime>,
    found: &mut Vec<(SystemTime, PathBuf)>,
) {
    for entry in entries.flatten() {
        // `file_type` does not follow links, so a link is neither a folder nor a file here and is skipped.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            if depth > 0
                && let Ok(inner) = fs::read_dir(&path)
            {
                collect(inner, depth - 1, since, found);
            }
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
                continue;
            };
            if since.is_none_or(|since| modified >= since) {
                found.push((modified, path));
            }
        }
    }
}

/// Calls `line` with each line of `path` that contains `needle`, parsed as JSON. Unreadable files, oversized lines and
/// lines that are not JSON are skipped: a log the CLI is still writing ends in a partial line.
fn scan(path: &Path, needle: &str, mut line: impl FnMut(serde_json::Value)) {
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    // Checked on the open handle, so a file swapped for a pipe or device after the walk is not read.
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return;
    }
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    let cap = u64::try_from(MAX_LINE_BYTES + 1).unwrap_or(u64::MAX);
    loop {
        buffer.clear();
        match reader.by_ref().take(cap).read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if buffer.len() > MAX_LINE_BYTES {
            if buffer.last() != Some(&b'\n') && !skip_line(&mut reader) {
                return;
            }
            continue;
        }
        let Ok(text) = std::str::from_utf8(&buffer) else {
            continue;
        };
        if !text.contains(needle) {
            continue;
        }
        if let Ok(value) = serde_json::from_str(text) {
            line(value);
        }
    }
}

/// Skips the rest of the current line without storing it. Returns whether a newline was found before the end.
fn skip_line(reader: &mut impl BufRead) -> bool {
    loop {
        let chunk = match reader.fill_buf() {
            Ok(chunk) if !chunk.is_empty() => chunk,
            _ => return false,
        };
        if let Some(at) = chunk.iter().position(|byte| *byte == b'\n') {
            reader.consume(at + 1);
            return true;
        }
        let read = chunk.len();
        reader.consume(read);
    }
}

/// An RFC 3339 timestamp as Unix milliseconds.
fn timestamp_ms(value: &serde_json::Value) -> Option<i64> {
    let text = value.as_str()?;
    let parsed: jiff::Timestamp = text.parse().ok()?;
    Some(parsed.as_millisecond())
}

fn system_time(ms: i64) -> SystemTime {
    let millis = u64::try_from(ms).unwrap_or(0);
    SystemTime::UNIX_EPOCH + Duration::from_millis(millis)
}

#[cfg(test)]
mod tests;
