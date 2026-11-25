//! The last lines of a failed vendor command's log, as a failure carries them (decision 0092).

use std::fs;
use std::path::Path;

/// Lines of a vendor command's log that a failure carries, the last ones.
const DIAGNOSIS_LINES: usize = 20;
/// Characters kept of one line, the marker included.
const LINE_CHARS: usize = 500;
/// Bytes kept of the whole tail, so one failure never carries megabytes of output.
const TAIL_BYTES: usize = 8 * 1024;
const REDACTED: &str = "[redacted]";
/// Lowercase markers of a secret. Each keeps its key and redacts the value after it, except `npm_`, which is the token.
const SECRET_MARKERS: &[&str] = &["token=", "password=", "authorization:", "bearer ", "npm_"];
/// Bytes that end a `key=value` secret: whitespace and the usual URL and quote separators.
const VALUE_END: &[char] = &['&', ';', ',', '\'', '"'];

/// The last lines of a command's log, for the person who asks why it failed. Each line is redacted and cut, and the
/// newest lines stay when the total would pass `TAIL_BYTES`.
pub(crate) fn last_log_lines(log: &Path) -> Vec<String> {
    let text = fs::read_to_string(log).unwrap_or_default();
    let mut kept = Vec::new();
    let mut bytes = 0;
    for line in text.lines().rev().take(DIAGNOSIS_LINES) {
        let line = cut_line(&redact_secrets(line));
        if bytes + line.len() > TAIL_BYTES {
            break;
        }
        bytes += line.len();
        kept.push(line);
    }
    kept.reverse();
    kept
}

/// The line cut to `LINE_CHARS` characters on a character boundary, the last one a `…` marker.
fn cut_line(line: &str) -> String {
    if line.chars().nth(LINE_CHARS).is_none() {
        return line.to_owned();
    }
    let mut cut: String = line.chars().take(LINE_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// The line with each secret value replaced by `[redacted]`. Matching ignores ASCII case, which keeps byte positions.
fn redact_secrets(line: &str) -> String {
    let mut spans = secret_spans(line);
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    let mut out = String::with_capacity(line.len());
    let mut copied = 0;
    for (start, end) in merged {
        out.push_str(&line[copied..start]);
        out.push_str(REDACTED);
        copied = end;
    }
    out.push_str(&line[copied..]);
    out
}

/// The byte ranges of the secret values in `line`, one per marker occurrence that has a value.
fn secret_spans(line: &str) -> Vec<(usize, usize)> {
    let lower = line.to_ascii_lowercase();
    let mut spans = Vec::new();
    for marker in SECRET_MARKERS {
        for (at, _) in lower.match_indices(marker) {
            let after = at + marker.len();
            let span = match *marker {
                "npm_" => (at, word_end(line, after)),
                "authorization:" => (skip_spaces(line, after), line.len()),
                "bearer " => {
                    let start = skip_spaces(line, after);
                    (start, value_end(line, start))
                }
                _ => (after, value_end(line, after)),
            };
            if span.0 < span.1 {
                spans.push(span);
            }
        }
    }
    spans
}

/// The end of a `key=value` value starting at `from`: the next whitespace or separator.
fn value_end(line: &str, from: usize) -> usize {
    line[from..]
        .find(|c: char| c.is_whitespace() || VALUE_END.contains(&c))
        .map_or(line.len(), |offset| from + offset)
}

/// The first byte at or after `from` that is not a space or tab.
fn skip_spaces(line: &str, from: usize) -> usize {
    line[from..]
        .find(|c: char| c != ' ' && c != '\t')
        .map_or(line.len(), |offset| from + offset)
}

/// The end of a token of letters, digits, `_` and `-` starting at `from`.
fn word_end(line: &str, from: usize) -> usize {
    line[from..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .map_or(line.len(), |offset| from + offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tail of `text`, read from a log file the way a failure reads it.
    fn tail_of(text: &str) -> Vec<String> {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        fs::write(&log, text).expect("a log");
        last_log_lines(&log)
    }

    #[test]
    fn only_the_last_twenty_lines_are_kept() {
        let text: String = (0..30).map(|i| format!("line {i}\n")).collect();

        let tail = tail_of(&text);

        assert_eq!(tail.len(), 20, "{tail:?}");
        assert_eq!(tail.last().map(String::as_str), Some("line 29"));
        assert_eq!(tail.first().map(String::as_str), Some("line 10"));
    }

    #[test]
    fn a_line_of_more_than_a_megabyte_is_cut_to_500_characters() {
        let text = format!("{}\nend\n", "x".repeat(1024 * 1024 + 10));

        let tail = tail_of(&text);

        assert_eq!(tail.len(), 2, "{tail:?}");
        assert!(tail[0].chars().count() <= 500, "{}", tail[0].len());
        assert!(tail[0].ends_with('…'), "{}", tail[0]);
        assert_eq!(tail[1], "end");
    }

    #[test]
    fn a_long_multibyte_line_is_cut_on_a_character_boundary() {
        let text = format!("{}\n", "é".repeat(600));

        let tail = tail_of(&text);

        assert_eq!(tail[0].chars().count(), 500, "{}", tail[0]);
        assert!(tail[0].starts_with("ééé"), "{}", tail[0]);
        assert!(tail[0].ends_with('…'), "{}", tail[0]);
    }

    #[test]
    fn the_tail_is_capped_at_8_kib_in_total_and_keeps_the_newest_lines() {
        let text: String = (0..20)
            .map(|i| format!("{i:03}{}\n", "a".repeat(477)))
            .collect();

        let tail = tail_of(&text);

        let total: usize = tail.iter().map(String::len).sum();
        assert!(total <= 8 * 1024, "{total} bytes kept");
        assert!(tail.len() < 20, "{}", tail.len());
        assert!(tail.last().is_some_and(|line| line.starts_with("019")));
        assert!(tail.iter().all(|line| line.starts_with('0')));
    }

    #[test]
    fn a_line_without_a_secret_is_kept_as_it_is() {
        assert_eq!(tail_of("fetching the index\n"), vec!["fetching the index"]);
    }

    #[test]
    fn a_token_value_is_redacted() {
        let tail = tail_of("GET https://host/x?token=abc123&page=2\n");

        assert_eq!(tail, vec!["GET https://host/x?token=[redacted]&page=2"]);
    }

    #[test]
    fn a_password_value_is_redacted() {
        let tail = tail_of("password=hunter2 rejected\n");

        assert_eq!(tail, vec!["password=[redacted] rejected"]);
    }

    #[test]
    fn an_authorization_header_is_redacted() {
        let tail = tail_of("Authorization: Basic dXNlcjpwYXNz\n");

        assert_eq!(tail, vec!["Authorization: [redacted]"]);
    }

    #[test]
    fn a_bearer_token_is_redacted() {
        let tail = tail_of("sent Bearer abc.def.ghi to host\n");

        assert_eq!(tail, vec!["sent Bearer [redacted] to host"]);
    }

    #[test]
    fn an_npm_token_is_redacted() {
        let tail = tail_of("auth npm_AbC123xyz failed\n");

        assert_eq!(tail, vec!["auth [redacted] failed"]);
    }
}
