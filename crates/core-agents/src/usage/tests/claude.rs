//! The Claude reader against fake session logs.

use massorca_protocol::methods::usage::WindowKind;
use serde_json::json;

use super::{NOW, iso, write_lines};
use crate::usage::{HOUR_MS, MINUTE_MS, read};
use std::fs;

fn reply(at_ms: i64, id: &str, input: u64, output: u64) -> serde_json::Value {
    json!({
        "type": "assistant", "timestamp": iso(at_ms), "requestId": format!("req_{id}"),
        "message": {
            "id": format!("msg_{id}"), "role": "assistant",
            "content": [{ "type": "text", "text": "fake reply" }],
            "usage": { "input_tokens": input, "output_tokens": output,
                       "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0 }
        }
    })
}

fn notice(at_ms: i64, text: &str) -> serde_json::Value {
    json!({
        "type": "assistant", "timestamp": iso(at_ms), "isApiErrorMessage": true,
        "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] }
    })
}

#[test]
fn claude_counts_each_reply_once_in_the_open_session_and_the_week() {
    let home = tempfile::tempdir().unwrap();
    let log = home
        .path()
        .join("projects")
        .join("-fake-project")
        .join("s1.jsonl");
    write_lines(
        &log,
        &[
            // Three days ago: in the week, not in the session.
            reply(NOW - 72 * HOUR_MS, "old", 1000, 0),
            // The session opens at 11:00 (the hour of 11:10) and resets at 16:00.
            reply(NOW - 80 * MINUTE_MS, "a", 100, 20),
            // The same reply again, one line per content block: counted once.
            reply(NOW - 80 * MINUTE_MS, "a", 100, 20),
            reply(NOW - 10 * MINUTE_MS, "b", 5, 5),
            // Eight days ago: outside every window.
            reply(NOW - 8 * 24 * HOUR_MS, "gone", 9999, 0),
        ],
    );
    fs::write(
        log.with_file_name("broken.jsonl"),
        "{\"type\":\"assistant\",\"time",
    )
    .unwrap();

    let reading = read("claude", home.path(), NOW).unwrap().unwrap();
    let session = &reading.windows[0];
    assert_eq!(session.window, WindowKind::Session);
    assert_eq!(session.tokens, Some(130));
    assert_eq!(session.started_at_ms, Some(NOW - 90 * MINUTE_MS));
    assert_eq!(
        session.resets_at_ms,
        Some(NOW - 90 * MINUTE_MS + 5 * HOUR_MS)
    );
    assert_eq!(session.used_percent, None, "Claude records no limit");
    assert!(!session.limited);
    let weekly = &reading.windows[1];
    assert_eq!(weekly.window, WindowKind::Weekly);
    assert_eq!(weekly.tokens, Some(1130));
    assert_eq!(reading.observed_at_ms, Some(NOW - 10 * MINUTE_MS));
}

#[test]
fn claude_without_an_open_session_shows_no_reset() {
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("projects").join("p").join("s.jsonl");
    write_lines(&log, &[reply(NOW - 6 * HOUR_MS, "a", 10, 10)]);
    let reading = read("claude", home.path(), NOW).unwrap().unwrap();
    assert_eq!(reading.windows[0].tokens, Some(0));
    assert_eq!(reading.windows[0].resets_at_ms, None);
    assert_eq!(reading.windows[1].tokens, Some(20));
}

#[test]
fn claude_limit_notices_mark_their_window_until_the_reset() {
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("projects").join("p").join("s.jsonl");
    let reset_s = (NOW + 2 * HOUR_MS) / 1000;
    write_lines(
        &log,
        &[
            reply(NOW - 30 * MINUTE_MS, "a", 10, 10),
            notice(
                NOW - 5 * MINUTE_MS,
                &format!("Claude AI usage limit reached|{reset_s}"),
            ),
            notice(NOW - 4 * MINUTE_MS, "Weekly limit reached · resets Mon"),
        ],
    );
    let reading = read("claude", home.path(), NOW).unwrap().unwrap();
    let session = &reading.windows[0];
    assert!(session.limited);
    assert_eq!(session.used_percent, Some(100.0));
    assert_eq!(session.resets_at_ms, Some(reset_s * 1000));
    assert!(reading.windows[1].limited);
    assert_eq!(reading.windows[1].resets_at_ms, None);

    // Once the recorded reset has passed, the notice no longer counts.
    let later = read("claude", home.path(), reset_s * 1000 + 1)
        .unwrap()
        .unwrap();
    assert!(!later.windows[0].limited);
}
