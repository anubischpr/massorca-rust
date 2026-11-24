//! A project's `orca.yaml` scripts: `scripts.setup` of the new worktree's copy (decision 0086) and `scripts.archive`
//! of the project's copy (decision 0092). The runner that a service terminal starts is in `setup_runner`.

use std::fmt;
use std::path::Path;

/// The committed file that names the setup script, read from the new worktree (its base's copy).
pub const SETUP_FILE: &str = "orca.yaml";
/// A larger file is not read: setup scripts are short, and the read happens on every create.
pub const MAX_SETUP_FILE_BYTES: u64 = 256 * 1024;

/// The setup script of the worktree at `workspace_dir`, or `None` when it has none. An unreadable or oversized
/// `orca.yaml` is an error, so the create can report it instead of skipping setup unseen.
pub fn read_setup_script(workspace_dir: &Path) -> Result<Option<String>, SetupFileError> {
    read_script(workspace_dir, SETUP_KEY)
}

/// The key of the setup script under `scripts:`.
pub const SETUP_KEY: &str = "setup";
/// The key of the archive script under `scripts:` (decision 0092).
pub const ARCHIVE_KEY: &str = "archive";

/// `scripts.<key>` of the `orca.yaml` in `dir`, or `None` when it has none; errors as [`read_setup_script`].
pub fn read_script(dir: &Path, key: &str) -> Result<Option<String>, SetupFileError> {
    let path = dir.join(SETUP_FILE);
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SetupFileError::Read(error.to_string())),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_SETUP_FILE_BYTES {
        return Err(SetupFileError::TooLarge);
    }
    let text =
        std::fs::read_to_string(&path).map_err(|error| SetupFileError::Read(error.to_string()))?;
    Ok(parse_script(&text, key))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupFileError {
    TooLarge,
    Read(String),
}

impl fmt::Display for SetupFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(
                f,
                "{SETUP_FILE} is larger than {MAX_SETUP_FILE_BYTES} bytes"
            ),
            Self::Read(error) => write!(f, "{SETUP_FILE} could not be read: {error}"),
        }
    }
}

/// `scripts.setup` of an `orca.yaml` text, trimmed; `None` when absent or empty. Reads the block-mapping subset the
/// file uses: a plain, single- or double-quoted one-line value, or a `|` / `>` block. Any other form reads as absent.
pub fn parse_setup_script(text: &str) -> Option<String> {
    parse_script(text, SETUP_KEY)
}

/// `scripts.<key>` of an `orca.yaml` text, read as [`parse_setup_script`] reads `scripts.setup`.
pub fn parse_script(text: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let start = lines
        .iter()
        .position(|line| indent_of(line) == 0 && key_value(line, "scripts") == Some(""))?;
    let mut child_indent = None;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if is_blank_or_comment(line) {
            continue;
        }
        let indent = indent_of(line);
        if indent == 0 {
            return None;
        }
        let child = *child_indent.get_or_insert(indent);
        if indent != child {
            continue;
        }
        if let Some(value) = key_value(line, key) {
            return scalar(value, &lines[index + 1..], child).filter(|script| !script.is_empty());
        }
    }
    None
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// The text after `key:` on `line`, comment-free when the value is empty.
fn key_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix(key)?.strip_prefix(':')?;
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let value = rest.trim();
    Some(if value.starts_with('#') { "" } else { value })
}

fn scalar(value: &str, following: &[&str], key_indent: usize) -> Option<String> {
    if let Some(header) = value.strip_prefix('|') {
        return block_header_ok(header).then(|| block(following, key_indent, false));
    }
    if let Some(header) = value.strip_prefix('>') {
        return block_header_ok(header).then(|| block(following, key_indent, true));
    }
    if let Some(body) = value.strip_prefix('"') {
        return double_quoted(body);
    }
    if let Some(body) = value.strip_prefix('\'') {
        return single_quoted(body);
    }
    let plain = match value.find(" #") {
        Some(comment) => &value[..comment],
        None => value,
    };
    Some(plain.trim().to_owned())
}

/// A block header: chomping and indentation indicators, then at most a comment.
fn block_header_ok(header: &str) -> bool {
    let indicators = header.split('#').next().unwrap_or_default().trim();
    indicators
        .chars()
        .all(|c| matches!(c, '-' | '+' | '1'..='9'))
}

fn block(following: &[&str], key_indent: usize, folded: bool) -> String {
    let mut body: Vec<&str> = Vec::new();
    let mut indent = None;
    for line in following {
        if line.trim().is_empty() {
            body.push("");
            continue;
        }
        let own = indent_of(line);
        if own <= key_indent {
            break;
        }
        let block_indent = *indent.get_or_insert(own);
        if own < block_indent {
            break;
        }
        body.push(&line[block_indent..]);
    }
    let text = if folded {
        body.split(|line| line.is_empty())
            .map(|paragraph| paragraph.join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        body.join("\n")
    };
    text.trim().to_owned()
}

fn double_quoted(body: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out.trim().to_owned()),
            '\\' => out.push(match chars.next()? {
                'n' => '\n',
                't' => '\t',
                other => other,
            }),
            other => out.push(other),
        }
    }
    None
}

fn single_quoted(body: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                out.push('\'');
                continue;
            }
            return Some(out.trim().to_owned());
        }
        out.push(c);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_is_read_from_plain_quoted_and_block_values() {
        assert_eq!(
            parse_setup_script("scripts:\n  setup: pnpm install # deps\n").as_deref(),
            Some("pnpm install")
        );
        assert_eq!(
            parse_setup_script("scripts:\n  archive: x\n  setup: \"echo \\\"hi\\\"\"\n").as_deref(),
            Some("echo \"hi\"")
        );
        assert_eq!(
            parse_setup_script("scripts:\n  setup: 'it''s'\n").as_deref(),
            Some("it's")
        );
        assert_eq!(
            parse_setup_script(
                "issueCommand: x\nscripts:\r\n  setup: |\r\n    pnpm install\r\n\r\n    echo done\r\n  archive: y\r\n"
            )
            .as_deref(),
            Some("pnpm install\n\necho done")
        );
        assert_eq!(
            parse_setup_script("scripts:\n    setup: >-\n      echo a\n      b\n").as_deref(),
            Some("echo a b")
        );
    }

    #[test]
    fn the_archive_script_is_read_beside_setup() {
        let text = "scripts:
  setup: pnpm install
  archive: |
    tar czf ../backup.tgz .
";
        assert_eq!(
            parse_script(text, ARCHIVE_KEY).as_deref(),
            Some("tar czf ../backup.tgz .")
        );
        assert_eq!(
            parse_script(
                "scripts:
  setup: x
",
                ARCHIVE_KEY
            ),
            None
        );
    }

    #[test]
    fn an_absent_empty_or_foreign_setup_reads_as_none() {
        assert_eq!(parse_setup_script(""), None);
        assert_eq!(parse_setup_script("scripts:\n  archive: x\n"), None);
        assert_eq!(parse_setup_script("scripts:\n  setup:\n"), None);
        assert_eq!(
            parse_setup_script("other:\n  setup: x\nscripts:\n  x: y\n"),
            None
        );
        assert_eq!(parse_setup_script("scripts: {setup: x}\n"), None);
        assert_eq!(
            parse_setup_script("scripts:\n  inner:\n    setup: x\n"),
            None
        );
    }
}
