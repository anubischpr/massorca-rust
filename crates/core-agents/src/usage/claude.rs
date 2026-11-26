//! Claude Code's usage: the token counts its session logs (`projects/<folder>/<session>.jsonl`) record per reply, grouped
//! into the 5-hour session and the last 7 days. Claude Code records no limit, so the percent is known only when a reply
//! says the limit was reached (decision 0154).

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use massorca_protocol::methods::usage::{UsageSource as SourceName, UsageWindow, WindowKind};
use regex::Regex;

use super::{HOUR_MS, UsageReading, UsageSource, WEEK_MS, recent_logs, scan, timestamp_ms};

pub(super) const KIND: &str = "claude";

/// A session window: starts at the hour of its first reply and lasts five hours.
const SESSION_MS: i64 = 5 * HOUR_MS;

/// The limit message an older Claude Code writes, with the reset as Unix seconds.
static LIMIT_RESET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"usage limit reached\|(\d{9,11})").expect("a valid pattern"));
/// The limit messages a newer one writes, without a machine-readable reset.
static LIMIT_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(5-hour|session|weekly|opus weekly) limit reached").expect("a valid pattern")
});

/// One reply with its tokens, or a limit notice.
struct Record {
    at_ms: i64,
    tokens: u64,
    limit: Option<Limit>,
}

#[derive(Clone, Copy)]
struct Limit {
    weekly: bool,
    resets_at_ms: Option<i64>,
}

/// Claude Code's session logs in the account's folder.
#[derive(Debug)]
pub(super) struct ClaudeLogs;

impl UsageSource for ClaudeLogs {
    fn name(&self) -> SourceName {
        SourceName::LocalLogs
    }

    fn read(&self, home: &Path, now_ms: i64) -> io::Result<Option<UsageReading>> {
        read(home, now_ms)
    }
}

fn read(home: &Path, now_ms: i64) -> io::Result<Option<UsageReading>> {
    let Some(files) = recent_logs(&home.join("projects"), 2, now_ms)? else {
        return Ok(None);
    };
    let since = now_ms - WEEK_MS;
    let mut seen = HashSet::new();
    let mut records = Vec::new();
    for file in &files {
        scan(file, "\"assistant\"", |line| {
            if let Some(record) = record(&line, &mut seen)
                && record.at_ms >= since
                && record.at_ms <= now_ms
            {
                records.push(record);
            }
        });
    }
    records.sort_by_key(|record| record.at_ms);
    Ok(Some(reading(&records, now_ms)))
}

/// A reply's tokens, counted once: Claude Code writes one line per content block of a reply, each with the reply's usage.
fn record(line: &serde_json::Value, seen: &mut HashSet<String>) -> Option<Record> {
    if line.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let at_ms = timestamp_ms(line.get("timestamp")?)?;
    let message = line.get("message")?;
    let limit = limit(message);
    let tokens = match message.get("usage") {
        Some(usage) => {
            let key = format!(
                "{}:{}",
                message.get("id").and_then(|id| id.as_str()).unwrap_or(""),
                line.get("requestId")
                    .and_then(|id| id.as_str())
                    .unwrap_or("")
            );
            if key != ":" && !seen.insert(key) {
                0
            } else {
                [
                    "input_tokens",
                    "output_tokens",
                    "cache_creation_input_tokens",
                    "cache_read_input_tokens",
                ]
                .iter()
                .filter_map(|name| usage.get(*name).and_then(serde_json::Value::as_u64))
                .fold(0_u64, u64::saturating_add)
            }
        }
        None => 0,
    };
    (tokens > 0 || limit.is_some()).then_some(Record {
        at_ms,
        tokens,
        limit,
    })
}

fn limit(message: &serde_json::Value) -> Option<Limit> {
    let content = message.get("content")?.as_array()?;
    content
        .iter()
        .filter_map(|block| block.get("text").and_then(serde_json::Value::as_str))
        .find_map(|text| {
            if let Some(found) = LIMIT_RESET.captures(text) {
                let seconds: i64 = found.get(1)?.as_str().parse().ok()?;
                return Some(Limit {
                    weekly: false,
                    resets_at_ms: Some(seconds.saturating_mul(1000)),
                });
            }
            let found = LIMIT_TEXT.captures(text)?;
            let which = found.get(1)?.as_str().to_ascii_lowercase();
            Some(Limit {
                weekly: which.contains("weekly"),
                resets_at_ms: None,
            })
        })
}

fn reading(records: &[Record], now_ms: i64) -> UsageReading {
    let mut session = UsageWindow {
        window_minutes: Some(300),
        tokens: Some(0),
        ..UsageWindow::new(WindowKind::Session)
    };
    let mut weekly = UsageWindow {
        window_minutes: Some(7 * 24 * 60),
        started_at_ms: Some(now_ms - WEEK_MS),
        tokens: Some(records.iter().map(|record| record.tokens).sum()),
        ..UsageWindow::new(WindowKind::Weekly)
    };
    if let Some(start) = current_block(records, now_ms) {
        let end = start + SESSION_MS;
        session.started_at_ms = Some(start);
        session.resets_at_ms = Some(end);
        let in_block = records.iter().filter(|record| record.at_ms >= start);
        session.tokens = Some(in_block.clone().map(|record| record.tokens).sum());
        if let Some(limit) = in_block
            .filter_map(|record| record.limit)
            .filter(|limit| !limit.weekly)
            .last()
        {
            mark_limited(&mut session, limit.resets_at_ms.or(Some(end)), now_ms);
        }
    }
    if let Some(limit) = records
        .iter()
        .filter_map(|record| record.limit)
        .filter(|limit| limit.weekly)
        .last()
    {
        mark_limited(&mut weekly, limit.resets_at_ms, now_ms);
    }
    UsageReading {
        source: SourceName::LocalLogs,
        windows: vec![session, weekly],
        observed_at_ms: records.last().map(|record| record.at_ms),
    }
}

/// A limit notice counts until its reset; one with no known reset counts until a later window replaces it.
fn mark_limited(window: &mut UsageWindow, resets_at_ms: Option<i64>, now_ms: i64) {
    if resets_at_ms.is_some_and(|reset| reset <= now_ms) {
        return;
    }
    window.limited = true;
    window.used_percent = Some(100.0);
    if resets_at_ms.is_some() {
        window.resets_at_ms = resets_at_ms;
    }
}

/// The start of the session window that is open at `now_ms`, if any. A window opens at the hour of the first reply after
/// the previous window ended, and stays open five hours.
fn current_block(records: &[Record], now_ms: i64) -> Option<i64> {
    let mut start: Option<i64> = None;
    for record in records {
        if start.is_none_or(|open| record.at_ms >= open + SESSION_MS) {
            start = Some(record.at_ms - record.at_ms.rem_euclid(HOUR_MS));
        }
    }
    start.filter(|open| now_ms < open + SESSION_MS)
}
