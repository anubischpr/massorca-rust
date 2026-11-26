//! Codex's usage: the rate limits its session logs (`sessions/YYYY/MM/DD/rollout-*.jsonl`) record with each token count,
//! as the vendor reported them on the last reply. The newest record wins (decision 0154).

use std::io;
use std::path::Path;

use massorca_protocol::methods::usage::{UsageSource as SourceName, UsageWindow, WindowKind};

use super::{MINUTE_MS, UsageReading, UsageSource, recent_logs, scan, timestamp_ms};

pub(super) const KIND: &str = "codex";

/// Windows up to a day long are the session limit; longer ones are the weekly limit.
const SESSION_MAX_MINUTES: u64 = 24 * 60;

/// Codex's session logs in the account's folder.
#[derive(Debug)]
pub(super) struct CodexLogs;

impl UsageSource for CodexLogs {
    fn name(&self) -> SourceName {
        SourceName::LocalLogs
    }

    fn read(&self, home: &Path, now_ms: i64) -> io::Result<Option<UsageReading>> {
        read(home, now_ms)
    }
}

fn read(home: &Path, now_ms: i64) -> io::Result<Option<UsageReading>> {
    let Some(files) = recent_logs(&home.join("sessions"), 3, now_ms)? else {
        return Ok(None);
    };
    let mut newest: Option<(i64, serde_json::Value)> = None;
    for file in &files {
        scan(file, "\"rate_limits\"", |line| {
            let Some(at_ms) = line.get("timestamp").and_then(timestamp_ms) else {
                return;
            };
            if at_ms > now_ms || newest.as_ref().is_some_and(|(seen, _)| *seen >= at_ms) {
                return;
            }
            let Some(limits) = line
                .get("payload")
                .filter(|payload| {
                    payload.get("type").and_then(|kind| kind.as_str()) == Some("token_count")
                })
                .and_then(|payload| payload.get("rate_limits"))
                .filter(|limits| limits.is_object())
            else {
                return;
            };
            newest = Some((at_ms, limits.clone()));
        });
    }
    let Some((at_ms, limits)) = newest else {
        return Ok(None);
    };
    let windows = ["primary", "secondary"]
        .iter()
        .enumerate()
        .filter_map(|(index, name)| window(limits.get(*name)?, index == 0, at_ms, now_ms))
        .collect();
    Ok(Some(UsageReading {
        source: SourceName::LocalLogs,
        windows,
        observed_at_ms: Some(at_ms),
    }))
}

/// One limit as Codex recorded it at `at_ms`. A newer Codex writes the reset as Unix seconds (`resets_at`), an older one
/// as seconds from the record (`resets_in_seconds`). Once the reset has passed the recorded percent is stale: it is left
/// unknown rather than shown as the old figure or as zero.
fn window(
    limit: &serde_json::Value,
    primary: bool,
    at_ms: i64,
    now_ms: i64,
) -> Option<UsageWindow> {
    let used = limit.get("used_percent")?.as_f64()?;
    let minutes = limit
        .get("window_minutes")
        .and_then(serde_json::Value::as_u64);
    let kind = match minutes {
        Some(minutes) if minutes <= SESSION_MAX_MINUTES => WindowKind::Session,
        Some(_) => WindowKind::Weekly,
        None if primary => WindowKind::Session,
        None => WindowKind::Weekly,
    };
    let resets_at_ms = limit
        .get("resets_at")
        .and_then(serde_json::Value::as_i64)
        .map(|seconds| seconds.saturating_mul(1000))
        .or_else(|| {
            let seconds = limit.get("resets_in_seconds")?.as_i64()?;
            Some(at_ms.saturating_add(seconds.saturating_mul(1000)))
        });
    let window_minutes = minutes.and_then(|minutes| u32::try_from(minutes).ok());
    let stale = resets_at_ms.is_some_and(|reset| reset <= now_ms);
    let used_percent = (!stale).then_some(used.clamp(0.0, 100.0));
    Some(UsageWindow {
        window_minutes,
        used_percent,
        started_at_ms: resets_at_ms
            .zip(window_minutes)
            .map(|(reset, minutes)| reset - i64::from(minutes) * MINUTE_MS),
        resets_at_ms,
        tokens: None,
        limited: used_percent.is_some_and(|used| used >= 100.0),
        ..UsageWindow::new(kind)
    })
}
