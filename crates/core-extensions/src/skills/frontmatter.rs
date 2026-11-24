//! A `SKILL.md`'s name, description and slash-menu flag, read the way pv96's `skill-metadata.ts` reads them.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Only this many leading bytes of a skill file are read.
const MAX_READ_BYTES: u64 = 256 * 1024;
/// A description keeps at most this many UTF-16 units: a frontmatter description is cut here, and the fallback
/// paragraph stops growing once it passes it.
const DESCRIPTION_UNITS: usize = 240;

/// What a skill file says about itself, as discovery shows it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SkillSummary {
    pub name: Option<String>,
    pub description: Option<String>,
    pub disable_slash_command: bool,
}

/// The summary of the skill file at `path`. `None` when the file cannot be read.
pub fn read_skill_summary(path: &Path) -> Option<SkillSummary> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(MAX_READ_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(summarize(&String::from_utf8_lossy(&bytes)))
}

fn summarize(markdown: &str) -> SkillSummary {
    let text = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    let (fields, body) = match split_frontmatter(text) {
        Some((front, body)) => (parse_fields(front), body),
        None => (Vec::new(), text),
    };
    let field = |key: &str| {
        fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.trim())
            .filter(|value| !value.is_empty())
    };
    SkillSummary {
        name: field("name")
            .map(str::to_owned)
            .or_else(|| first_heading(body)),
        description: field("description")
            .map(|text| cut_units(text, DESCRIPTION_UNITS).to_owned())
            .or_else(|| first_paragraph(body)),
        disable_slash_command: field("disable-slash-command") == Some("true"),
    }
}

/// The lines between an opening and a closing `---`, and the text after the closing line. The block needs at least
/// one line inside it.
fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let (opening, rest) = text.split_once('\n')?;
    if opening.trim_end() != "---" {
        return None;
    }
    let mut position = 0;
    for line in rest.split_inclusive('\n') {
        if position > 0 && line.trim_end() == "---" {
            return Some((&rest[..position - 1], &rest[position + line.len()..]));
        }
        position += line.len();
    }
    None
}

/// The top-level `key: value` lines. A block scalar folds to one line of words; a list has no value of its own, so
/// its key reads as empty and its item lines match no key.
fn parse_fields(front: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = front.lines().collect();
    let mut fields = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        index += 1;
        let Some((key, value)) = line.split_once(':').filter(|(key, _)| is_key(key)) else {
            continue;
        };
        let text = if matches!(value.trim(), "|" | "|-" | ">" | ">-") {
            let mut words = Vec::new();
            while index < lines.len()
                && (lines[index].trim().is_empty() || lines[index].starts_with("  "))
            {
                words.extend(lines[index].split_whitespace());
                index += 1;
            }
            words.join(" ")
        } else {
            unquote(value).to_owned()
        };
        fields.push((key.to_owned(), text));
    }
    fields
}

fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

/// The longest prefix of `text` that holds at most `limit` UTF-16 units, cut between characters and trimmed at the end.
fn cut_units(text: &str, limit: usize) -> &str {
    let mut units = 0;
    for (index, character) in text.char_indices() {
        units += character.len_utf16();
        if units > limit {
            return text[..index].trim_end();
        }
    }
    text
}

/// The value without surrounding whitespace and one pair of matching quotes.
fn unquote(value: &str) -> &str {
    let value = value.trim();
    ['"', '\'']
        .into_iter()
        .find_map(|quote| value.strip_prefix(quote)?.strip_suffix(quote))
        .unwrap_or(value)
}

/// The text of the first `# heading` line. A heading with no text yields nothing, as pv96 does.
fn first_heading(body: &str) -> Option<String> {
    let heading = body.lines().find_map(|line| {
        let rest = line.strip_prefix('#')?;
        rest.starts_with(char::is_whitespace)
            .then(|| rest.trim().to_owned())
    })?;
    (!heading.is_empty()).then_some(heading)
}

/// The first run of prose lines. Blank lines, headings and code fences come before it and end it.
fn first_paragraph(body: &str) -> Option<String> {
    let mut paragraph: Vec<&str> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        paragraph.push(trimmed);
        if paragraph.join(" ").encode_utf16().count() > DESCRIPTION_UNITS {
            break;
        }
    }
    (!paragraph.is_empty()).then(|| paragraph.join(" "))
}
