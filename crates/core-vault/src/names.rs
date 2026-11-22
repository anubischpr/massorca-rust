//! Secret names, owning scopes and request reasons: untrusted text checked before it is stored or shown.

use std::fmt;
use std::str::FromStr;

const MAX_NAME_CHARS: usize = 64;
const MAX_SCOPE_ID_CHARS: usize = 128;
const MAX_REASON_CHARS: usize = 200;
const MAX_SESSION_CHARS: usize = 128;

/// Unicode format characters (general category Cf): invisible, and the bidi ones reorder what a reader sees.
const FORMAT_CHARS: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{180E}', '\u{180E}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFF9}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0001}', '\u{E0001}'),
    ('\u{E0020}', '\u{E007F}'),
];

/// Control, format (Cf) and line or paragraph separator characters: none belongs in text a person reads.
fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{2028}' | '\u{2029}')
        || FORMAT_CHARS
            .iter()
            .any(|&(first, last)| (first..=last).contains(&c))
}

fn is_plain(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars && !text.chars().any(is_hidden)
}

/// Whether `name` is a valid variable name: 1 to 64 ASCII letters, digits or `_`, not starting with a digit.
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    name.len() <= MAX_NAME_CHARS
        && (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Whether `reason` is acceptable request text: at most 200 characters, none of them control or invisible.
#[must_use]
pub fn is_valid_reason(reason: &str) -> bool {
    is_plain(reason, MAX_REASON_CHARS)
}

/// Whether `session` is an acceptable requesting session: at most 128 characters, none control or invisible.
#[must_use]
pub fn is_valid_session(session: &str) -> bool {
    is_plain(session, MAX_SESSION_CHARS)
}

/// The scope that owns or links a secret.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    Global,
    Project(String),
    Team(String),
}

/// Why a scope's text was refused. It never repeats the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("a scope is `global`, `project:<id>` or `team:<id>`")]
    UnknownKind,
    #[error("a scope id is 1 to 128 characters with no `/` and no control or invisible characters")]
    InvalidId,
}

fn is_valid_scope_id(id: &str) -> bool {
    !id.is_empty() && is_plain(id, MAX_SCOPE_ID_CHARS) && !id.contains('/')
}

impl Scope {
    /// Whether the id meets the rules [`FromStr`] enforces; a scope built directly may not.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Global => true,
            Self::Project(id) | Self::Team(id) => is_valid_scope_id(id),
        }
    }
}

impl FromStr for Scope {
    type Err = ScopeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text == "global" {
            return Ok(Self::Global);
        }
        let (kind, id) = text.split_once(':').ok_or(ScopeError::UnknownKind)?;
        let build: fn(String) -> Self = match kind {
            "project" => Self::Project,
            "team" => Self::Team,
            _ => return Err(ScopeError::UnknownKind),
        };
        if !is_valid_scope_id(id) {
            return Err(ScopeError::InvalidId);
        }
        Ok(build(id.to_owned()))
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => f.write_str("global"),
            Self::Project(id) => write!(f, "project:{id}"),
            Self::Team(id) => write!(f, "team:{id}"),
        }
    }
}
