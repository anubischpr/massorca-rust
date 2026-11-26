//! The naming rules of a chat title (decision 0167): what the title request says, what a first prompt and a reply may
//! contribute, and how a reply becomes a slug. Pure functions, so each rule has a unit test and none needs an agent run.

use massorca_protocol::codes;

use crate::launch::check_prompt;
use crate::sessions::AgentError;

/// Most words of a title.
const TITLE_WORDS: usize = 4;
/// Most characters of a title.
const TITLE_MAX_CHARS: usize = 48;
/// The most of a reply the title request carries, in bytes; a title of four words needs little more.
const TITLE_REPLY_BYTES: usize = 4 * 1024;
/// The request for the title: English ASCII of two to four words, one line, whatever language the chat is in.
const TITLE_INSTRUCTION: &str = "Name this coding chat with a short task title of 2 to 4 English words, using only ASCII letters, digits and spaces, whatever language the request is in. Output ONLY the title on one line, with no quotes, labels or punctuation, and nothing else.";
/// The labels a reply may put before its title, compared without case: `Title: Fix login` names the title `Fix login`.
const TITLE_LABELS: [&str; 6] = [
    "title",
    "chat title",
    "task title",
    "name",
    "título",
    "titulo",
];

/// The slug of a reply: the first line that holds a usable word, without a leading title label, its words folded to `a-z0-9`
/// (accents removed, so `canción` is `cancion`), at most four words joined by `-`, at most 48 characters. `None` when no
/// line holds a usable word.
pub fn title_slug(text: &str) -> Option<String> {
    text.lines().find_map(slug_of_line)
}

/// The slug of one line, or `None` when the line holds no usable word.
fn slug_of_line(line: &str) -> Option<String> {
    let folded = fold_to_ascii(without_label(line));
    let mut slug = String::new();
    for word in folded
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(TITLE_WORDS)
    {
        let length = if slug.is_empty() {
            word.len()
        } else {
            slug.len() + 1 + word.len()
        };
        if length > TITLE_MAX_CHARS {
            if slug.is_empty() {
                slug.push_str(&word[..TITLE_MAX_CHARS]);
            }
            break;
        }
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(word);
    }
    (!slug.is_empty()).then_some(slug)
}

/// The line without a leading title label such as `Title:`, `**Title:**` or `Título:`; any other line is returned as it is,
/// so `Fix: login` keeps its words.
fn without_label(line: &str) -> &str {
    let body = line
        .trim_start_matches(|ch: char| matches!(ch, '*' | '#' | '"' | '\'') || ch.is_whitespace());
    let Some((label, rest)) = body.split_once(':') else {
        return line;
    };
    let label = label
        .trim_matches(|ch: char| matches!(ch, '*' | '"' | '\'') || ch.is_whitespace())
        .to_lowercase();
    if TITLE_LABELS.contains(&label.as_str()) {
        rest
    } else {
        line
    }
}

/// The text in lowercase ASCII: accents removed (`canción` is `cancion`), ligatures spelled out, and every other
/// character a separator.
fn fold_to_ascii(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for ch in text.chars().filter(|ch| !is_combining_mark(*ch)) {
        for lower in ch.to_lowercase().filter(|lower| !is_combining_mark(*lower)) {
            if lower.is_ascii_alphanumeric() {
                folded.push(lower);
            } else {
                folded.push_str(fold_letter(lower).unwrap_or(" "));
            }
        }
    }
    folded
}

/// Combining marks (U+0300 to U+036F): part of the letter before them, so they are dropped, never read as separators.
fn is_combining_mark(ch: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&ch)
}

/// The ASCII spelling of a lowercase letter that is not `a-z`, or `None` for a character with no spelling here.
fn fold_letter(ch: char) -> Option<&'static str> {
    Some(match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' | 'ĝ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ň' | 'ņ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ř' => "r",
        'ś' | 'š' | 'ş' | 'ŝ' => "s",
        'ß' => "ss",
        'ť' | 'ţ' | 'ŧ' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

/// The one line a title run receives as its prompt: the request for an English ASCII title, the chat's first prompt and,
/// when there is one, the agent's first reply (cut to [`TITLE_REPLY_BYTES`]). Every line break and run of whitespace becomes
/// one space: a Windows `.cmd` launcher refuses an argument that holds a line break (decision 0167).
pub fn title_prompt(prompt: &str, reply: Option<&str>) -> String {
    let mut text = format!("{TITLE_INSTRUCTION} First request: {}", prompt.trim());
    if let Some(reply) = reply.map(str::trim).filter(|reply| !reply.is_empty()) {
        text.push_str(" The agent's first reply: ");
        text.push_str(cut_at_char_boundary(reply, TITLE_REPLY_BYTES));
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The longest prefix of `text` that is at most `max` bytes long and ends on a character boundary.
fn cut_at_char_boundary(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// The first prompt, trimmed, when it may be passed to a run: not empty, and within the prompt rules (size, no NUL, no
/// control character other than tab, line feed and carriage return).
pub(crate) fn validated_prompt(prompt: &str) -> Result<String, AgentError> {
    if prompt.trim().is_empty() {
        return Err(AgentError::new(
            codes::AGENT_PROMPT_INVALID,
            "the chat's first prompt is empty",
        ));
    }
    check_prompt(prompt)
        .map_err(|error| AgentError::new(codes::AGENT_PROMPT_INVALID, error.to_string()))?;
    Ok(prompt.trim().to_owned())
}

/// The agent's reply without the control characters a run must not receive (tab, line feed and carriage return stay).
pub(crate) fn plain_reply(reply: &str) -> String {
    reply
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\t' | '\n' | '\r'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_folds_accents_and_keeps_every_letter() {
        assert_eq!(title_slug("canción").as_deref(), Some("cancion"));
        assert_eq!(
            title_slug("Explora el proyecto").as_deref(),
            Some("explora-el-proyecto")
        );
        assert_eq!(
            title_slug("Ñandú, reñido ß").as_deref(),
            Some("nandu-renido-ss")
        );
    }

    #[test]
    fn punctuation_and_spacing_become_single_dashes() {
        assert_eq!(
            title_slug("  Fix: login -- bug!  ").as_deref(),
            Some("fix-login-bug")
        );
        assert_eq!(
            title_slug("v2.0 release_notes").as_deref(),
            Some("v2-0-release-notes")
        );
    }

    #[test]
    fn a_slug_keeps_at_most_four_words() {
        assert_eq!(
            title_slug("one two three four five six").as_deref(),
            Some("one-two-three-four")
        );
    }

    #[test]
    fn a_slug_is_at_most_48_characters_and_cuts_on_a_word_boundary() {
        let words = format!("{} {} {}", "a".repeat(20), "b".repeat(20), "c".repeat(20));
        assert_eq!(
            title_slug(&words),
            Some(format!("{}-{}", "a".repeat(20), "b".repeat(20)))
        );
        assert_eq!(title_slug(&"d".repeat(60)), Some("d".repeat(48)));
    }

    #[test]
    fn only_the_first_line_with_a_usable_word_counts() {
        assert_eq!(
            title_slug("\n\n  First title\nsecond line").as_deref(),
            Some("first-title")
        );
        assert_eq!(title_slug("—\nUsable one").as_deref(), Some("usable-one"));
    }

    #[test]
    fn a_non_latin_first_line_falls_through_to_the_next_usable_line() {
        assert_eq!(
            title_slug("日本語のタイトル\nFix login").as_deref(),
            Some("fix-login")
        );
    }

    #[test]
    fn a_reply_without_a_usable_word_is_no_title() {
        assert_eq!(title_slug("!!! ??? ..."), None);
        assert_eq!(title_slug("   \n\t"), None);
        assert_eq!(title_slug(""), None);
    }

    #[test]
    fn a_leading_title_label_is_not_part_of_the_slug_but_other_colons_are_words() {
        assert_eq!(
            title_slug("Title: Fix login bug").as_deref(),
            Some("fix-login-bug")
        );
        assert_eq!(
            title_slug("**Title:** Fix login").as_deref(),
            Some("fix-login")
        );
        assert_eq!(
            title_slug("Chat title: \"Fix login\"").as_deref(),
            Some("fix-login")
        );
        assert_eq!(
            title_slug("Título: Arreglar login").as_deref(),
            Some("arreglar-login")
        );
        assert_eq!(
            title_slug("Title:\nFix login").as_deref(),
            Some("fix-login")
        );
        assert_eq!(
            title_slug("Fix: login bug").as_deref(),
            Some("fix-login-bug")
        );
    }

    #[test]
    fn the_title_prompt_names_the_request_and_the_reply_once() {
        let text = title_prompt("Fix the login bug", Some("I will read the auth module"));
        assert!(text.contains("First request: Fix the login bug"), "{text}");
        assert!(
            text.contains("The agent's first reply: I will read the auth module"),
            "{text}"
        );
        let without = title_prompt("Fix the login bug", None);
        assert!(!without.contains("first reply"), "{without}");
    }

    #[test]
    fn the_title_prompt_is_one_line_with_no_carriage_return_or_line_feed() {
        let text = title_prompt(
            "Fix\r\nthe login\n\tbug",
            Some("I will\r\nread  the\nauth   module"),
        );
        assert!(!text.contains('\r') && !text.contains('\n'), "{text:?}");
        assert!(text.contains("First request: Fix the login bug"), "{text}");
        assert!(
            text.ends_with("The agent's first reply: I will read the auth module"),
            "{text}"
        );
    }

    #[test]
    fn the_title_prompt_asks_for_an_english_ascii_title_whatever_the_language() {
        let text = title_prompt("Explora el proyecto", None);
        assert!(text.contains("English"), "{text}");
        assert!(text.contains("ASCII"), "{text}");
        assert!(text.contains("Explora el proyecto"), "{text}");
    }

    #[test]
    fn the_reply_is_cut_to_its_limit_on_a_character_boundary() {
        let reply = "é".repeat(TITLE_REPLY_BYTES);
        let text = title_prompt("p", Some(&reply));
        assert!(
            text.contains(&"é".repeat(TITLE_REPLY_BYTES / 2)),
            "the limit is kept"
        );
        assert!(
            !text.contains(&"é".repeat(TITLE_REPLY_BYTES / 2 + 1)),
            "nothing past the limit"
        );
    }

    #[test]
    fn an_empty_or_unsafe_prompt_is_prompt_invalid() {
        assert_eq!(
            validated_prompt("   ").expect_err("refused").code,
            codes::AGENT_PROMPT_INVALID
        );
        assert_eq!(
            validated_prompt("fix\0login").expect_err("refused").code,
            codes::AGENT_PROMPT_INVALID
        );
        assert_eq!(
            validated_prompt("  Fix the login bug \n").expect("accepted"),
            "Fix the login bug"
        );
    }

    #[test]
    fn a_reply_keeps_its_line_breaks_and_loses_other_control_characters() {
        assert_eq!(plain_reply("a\u{7}b\nc\td\r"), "ab\nc\td\r");
    }
}
