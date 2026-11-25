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
/// Lowercase markers of a secret. Each keeps its key and redacts the value after it. `authorization:` redacts the rest of
/// the line, and `--token `/`--password `/`basic ` redact the value after the space.
const SECRET_MARKERS: &[&str] = &[
    "token=",
    "password=",
    "secret=",
    "api_key=",
    "apikey=",
    "--token ",
    "--password ",
    "authorization:",
    "bearer ",
    "basic ",
];
/// Lowercase prefixes of a token, which is redacted from its prefix to the end of its word. A prefix inside a word
/// (`task-sk-`) is not a token.
const TOKEN_PREFIXES: &[&str] = &["npm_", "ghp_", "gho_", "github_pat_", "sk-"];
/// Bytes that end a `key=value` secret: whitespace and the usual URL and quote separators.
const VALUE_END: &[char] = &['&', ';', ',', '\'', '"'];

/// The last lines of a command's log, for the person who asks why it failed. Each line is redacted and cut, and the
/// newest lines stay when the total would pass `TAIL_BYTES`.
pub(crate) fn last_log_lines(log: &Path) -> Vec<String> {
    lines_tail(&fs::read_to_string(log).unwrap_or_default())
}

/// The last lines a command wrote to `log` after its first `offset` bytes, as `last_log_lines` carries them.
pub(crate) fn last_log_lines_after(log: &Path, offset: u64) -> Vec<String> {
    let bytes = fs::read(log).unwrap_or_default();
    let from = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(bytes.len());
    lines_tail(&String::from_utf8_lossy(&bytes[from..]))
}

fn lines_tail(text: &str) -> Vec<String> {
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
                "authorization:" => (skip_spaces(line, after), line.len()),
                "bearer " | "basic " | "--token " | "--password " => {
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
    for prefix in TOKEN_PREFIXES {
        for (at, _) in lower.match_indices(prefix) {
            if !preceded_by_word_char(line, at) {
                spans.push((at, word_end(line, at + prefix.len())));
            }
        }
    }
    spans.extend(url_userinfo_spans(line));
    spans
}

/// Whether the byte before `at` is a letter, digit, `_` or `-`, so a prefix at `at` sits inside a word.
fn preceded_by_word_char(line: &str, at: usize) -> bool {
    line[..at]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The `user:secret` userinfo of each `scheme://user:secret@host` URL in `line`.
fn url_userinfo_spans(line: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for (at, _) in line.match_indices("://") {
        let start = at + "://".len();
        let end = line[start..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '/' | '?' | '#' | '\'' | '"' | '<' | '>')
            })
            .map_or(line.len(), |offset| start + offset);
        if let Some(at_sign) = line[start..end].rfind('@')
            && at_sign > 0
        {
            spans.push((start, start + at_sign));
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

    #[test]
    fn a_secret_key_and_api_key_values_are_redacted() {
        assert_eq!(tail_of("secret=s3cr3t ok\n"), vec!["secret=[redacted] ok"]);
        assert_eq!(tail_of("api_key=KEY1 ok\n"), vec!["api_key=[redacted] ok"]);
        assert_eq!(tail_of("apikey=KEY2 ok\n"), vec!["apikey=[redacted] ok"]);
    }

    #[test]
    fn a_flag_followed_by_a_space_and_its_value_is_redacted() {
        assert_eq!(
            tail_of("run --token abc123 now\n"),
            vec!["run --token [redacted] now"]
        );
        assert_eq!(
            tail_of("run --password hunter2 now\n"),
            vec!["run --password [redacted] now"]
        );
    }

    #[test]
    fn a_basic_credential_is_redacted() {
        assert_eq!(
            tail_of("sent Basic dXNlcjpwYXNz to host\n"),
            vec!["sent Basic [redacted] to host"]
        );
    }

    #[test]
    fn a_github_or_openai_style_token_is_redacted() {
        assert_eq!(
            tail_of("push ghp_abc123XYZ failed\n"),
            vec!["push [redacted] failed"]
        );
        assert_eq!(
            tail_of("push gho_abc123XYZ failed\n"),
            vec!["push [redacted] failed"]
        );
        assert_eq!(
            tail_of("pat github_pat_11AB_x-y failed\n"),
            vec!["pat [redacted] failed"]
        );
        assert_eq!(
            tail_of("key sk-proj-abc123 failed\n"),
            vec!["key [redacted] failed"]
        );
    }

    #[test]
    fn a_token_prefix_inside_a_word_is_not_taken_for_a_token() {
        assert_eq!(
            tail_of("task-list and risk-sk-x\n"),
            vec!["task-list and risk-sk-x"]
        );
    }

    #[test]
    fn the_userinfo_of_a_url_is_redacted_and_the_host_kept() {
        assert_eq!(
            tail_of("clone https://user:pass@github.com/x ok\n"),
            vec!["clone https://[redacted]@github.com/x ok"]
        );
    }
}
