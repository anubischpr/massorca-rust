//! `git.log`: pages of history, each pinned to the commit id the first page resolved (decision 0011).

use std::path::Path;

use massorca_protocol::methods::git::log::{Commit, GitLog, MAX_MESSAGE_BYTES, Signature};

use crate::error::GitError;
use crate::git::{Captured, Git};

/// Fields per commit in the `--format` below; the message is last, so it may hold any byte but NUL.
const FIELDS: usize = 9;
const FORMAT: &str = "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B";

/// Where a page starts: a commit id and how many commits of its history to skip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogCursor {
    pub start: String,
    pub skip: u64,
}

impl LogCursor {
    /// `<commit id>:<skip>`, opaque to clients.
    pub fn encode(&self) -> String {
        format!("{}:{}", self.start, self.skip)
    }

    pub fn decode(text: &str) -> Option<Self> {
        let (start, skip) = text.split_once(':')?;
        let skip = skip.parse().ok()?;
        let hex =
            !start.is_empty() && start.len() <= 64 && start.bytes().all(|b| b.is_ascii_hexdigit());
        hex.then(|| Self {
            start: start.to_owned(),
            skip,
        })
    }
}

impl Git {
    /// The commit id `revision` resolves to, or `None` when it names no commit (an unborn HEAD included).
    pub fn resolve_commit(&self, dir: &Path, revision: &str) -> Result<Option<String>, GitError> {
        if revision.starts_with('-') {
            return Ok(None);
        }
        let commit = format!("{revision}^{{commit}}");
        let captured = self.run(
            Some(dir),
            ["rev-parse", "--verify", "--quiet", commit.as_str()],
        )?;
        if !captured.succeeded() {
            return Ok(None);
        }
        Ok(Some(captured.text()?.trim().to_owned()))
    }

    /// One page of at most `limit` commits from `cursor`.
    pub fn log(&self, dir: &Path, cursor: &LogCursor, limit: usize) -> Result<GitLog, GitError> {
        let skip = format!("--skip={}", cursor.skip);
        // One extra commit tells whether another page exists.
        let count = format!("--max-count={}", limit + 1);
        let captured = self.run(
            Some(dir),
            [
                "log",
                "-z",
                "--no-color",
                "--no-show-signature",
                FORMAT,
                skip.as_str(),
                count.as_str(),
                cursor.start.as_str(),
                "--",
            ],
        )?;
        captured.require_success()?;
        let mut commits = parse(&captured)?;
        let next_cursor = (commits.len() > limit).then(|| {
            commits.truncate(limit);
            LogCursor {
                start: cursor.start.clone(),
                skip: cursor.skip + limit as u64,
            }
            .encode()
        });
        Ok(GitLog {
            commits,
            next_cursor,
        })
    }
}

fn parse(captured: &Captured) -> Result<Vec<Commit>, GitError> {
    let bytes = captured.bytes();
    let fields: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
    let mut commits = Vec::new();
    // `-z` ends each commit with a NUL; the last split is the empty tail.
    let (chunks, rest) = fields.as_chunks::<FIELDS>();
    for chunk in chunks {
        let text = |index: usize| String::from_utf8_lossy(chunk[index]).into_owned();
        let time = |index: usize| -> Result<i64, GitError> {
            text(index)
                .trim()
                .parse::<i64>()
                .map(|seconds| seconds.saturating_mul(1000))
                .map_err(|_| captured.invalid_output("a commit time is not a number"))
        };
        let id = text(0);
        // Records after the first start with the newline git puts between commits.
        let id = id.trim_start_matches('\n').to_owned();
        let (message, message_truncated) = cut_message(chunk[8]);
        commits.push(Commit {
            parents: text(1).split_whitespace().map(str::to_owned).collect(),
            author: Signature {
                name: text(2),
                email: text(3),
                time_ms: time(4)?,
            },
            committer: Signature {
                name: text(5),
                email: text(6),
                time_ms: time(7)?,
            },
            subject: message.lines().next().unwrap_or_default().to_owned(),
            message,
            message_truncated,
            id,
        });
    }
    if rest
        .iter()
        .any(|field| !field.iter().all(u8::is_ascii_whitespace))
    {
        return Err(captured.invalid_output("a log record has too few fields"));
    }
    Ok(commits)
}

/// The message without its trailing newline, cut at a character boundary within the byte budget.
fn cut_message(bytes: &[u8]) -> (String, bool) {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_end_matches('\n');
    if text.len() <= MAX_MESSAGE_BYTES {
        return (text.to_owned(), false);
    }
    let mut end = MAX_MESSAGE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

#[cfg(test)]
mod tests {
    use super::{LogCursor, cut_message};

    #[test]
    fn a_cursor_round_trips_and_rejects_anything_else() {
        let cursor = LogCursor {
            start: "abc123".to_owned(),
            skip: 50,
        };
        assert_eq!(LogCursor::decode(&cursor.encode()), Some(cursor));
        for bad in ["", "abc", "xyz:1", "abc:-1", ":3", "abc:1:2", "--all:0"] {
            assert_eq!(LogCursor::decode(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_long_message_is_cut_on_a_character_boundary() {
        let long = "é".repeat(5000);
        let (message, truncated) = cut_message(long.as_bytes());
        assert!(truncated);
        assert!(message.len() <= super::MAX_MESSAGE_BYTES);
        assert!(message.chars().all(|c| c == 'é'));
    }
}
