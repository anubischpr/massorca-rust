//! Secret names, owning scopes and request reasons: untrusted text checked before it is stored or shown.

use std::fmt;
use std::str::FromStr;

const MAX_NAME_CHARS: usize = 64;
const MAX_SCOPE_ID_CHARS: usize = 128;
const MAX_REASON_CHARS: usize = 200;

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

/// Whether `reason` is acceptable request text: at most 200 characters and no control characters.
#[must_use]
pub fn is_valid_reason(reason: &str) -> bool {
    reason.chars().count() <= MAX_REASON_CHARS && !reason.chars().any(char::is_control)
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
    #[error("a scope id is 1 to 128 characters with no `/` and no control characters")]
    InvalidId,
}

fn is_valid_scope_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MAX_SCOPE_ID_CHARS
        && !id.chars().any(|c| c == '/' || c.is_control())
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
