//! The Codex reader against fake session logs.

use massorca_protocol::methods::usage::WindowKind;
use serde_json::json;

use super::{NOW, iso, write_lines};
use crate::usage::{HOUR_MS, MINUTE_MS, read};

fn token_count(at_ms: i64, limits: serde_json::Value) -> serde_json::Value {
    json!({
        "timestamp": iso(at_ms), "type": "event_msg",
        "payload": { "type": "token_count", "info": { "total_token_usage": { "total_tokens": 5 } },
                     "rate_limits": limits }
    })
}

#[test]
fn codex_takes_the_newest_recorded_limits() {
    let home = tempfile::tempdir().unwrap();
    let day = home
        .path()
        .join("sessions")
        .join("2026")
        .join("01")
        .join("05");
    write_lines(
        &day.join("rollout-old.jsonl"),
        &[token_count(
            NOW - 2 * HOUR_MS,
            json!({ "primary": { "used_percent": 90.0, "window_minutes": 300, "resets_in_seconds": 60 } }),
        )],
    );
    let reset_s = (NOW + HOUR_MS) / 1000;
    write_lines(
        &day.join("rollout-new.jsonl"),
        &[
            json!({ "timestamp": iso(NOW - 3 * MINUTE_MS), "type": "event_msg",
                    "payload": { "type": "token_count", "info": null, "rate_limits": null } }),
            token_count(
                NOW - 5 * MINUTE_MS,
                json!({
                    "primary": { "used_percent": 42.5, "window_minutes": 300, "resets_at": reset_s },
                    "secondary": { "used_percent": 100.0, "window_minutes": 10080, "resets_in_seconds": 3600 }
                }),
            ),
        ],
    );

    let reading = read("codex", home.path(), NOW).unwrap().unwrap();
    assert_eq!(reading.observed_at_ms, Some(NOW - 5 * MINUTE_MS));
    let session = &reading.windows[0];
    assert_eq!(session.window, WindowKind::Session);
    assert_eq!(session.used_percent, Some(42.5));
    assert_eq!(session.resets_at_ms, Some(reset_s * 1000));
    assert_eq!(
        session.started_at_ms,
        Some(reset_s * 1000 - 300 * MINUTE_MS)
    );
    assert!(!session.limited);
    let weekly = &reading.windows[1];
    assert_eq!(weekly.window, WindowKind::Weekly);
    assert_eq!(weekly.resets_at_ms, Some(NOW - 5 * MINUTE_MS + HOUR_MS));
    assert!(weekly.limited);
}

#[test]
fn codex_forgets_a_percent_whose_window_has_reset() {
    let home = tempfile::tempdir().unwrap();
    let log = home
        .path()
        .join("sessions")
        .join("2026")
        .join("01")
        .join("05")
        .join("rollout-a.jsonl");
    write_lines(
        &log,
        &[token_count(
            NOW - 2 * HOUR_MS,
            json!({ "primary": { "used_percent": 80.0, "window_minutes": 60, "resets_in_seconds": 60 } }),
        )],
    );
    let reading = read("codex", home.path(), NOW).unwrap().unwrap();
    assert_eq!(reading.windows[0].used_percent, None);
    assert!(!reading.windows[0].limited);
}

#[test]
fn codex_with_logs_but_no_limits_has_no_data() {
    let home = tempfile::tempdir().unwrap();
    let log = home
        .path()
        .join("sessions")
        .join("2026")
        .join("01")
        .join("05")
        .join("rollout-a.jsonl");
    write_lines(
        &log,
        &[json!({ "timestamp": iso(NOW), "type": "session_meta", "payload": {} })],
    );
    assert_eq!(read("codex", home.path(), NOW).unwrap(), None);
}
