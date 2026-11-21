//! Who owns a contribution: the core, or one extension. The wire form is `core` or `ext:<id>`.

use std::fmt;

use crate::names;

/// The owner of a method, capability, command or setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    Core,
    Extension(String),
}

impl Owner {
    /// Reads the wire form; `None` for anything other than `core` or `ext:<id>` with an id that follows the name rule.
    pub fn parse(text: &str) -> Option<Self> {
        if text == "core" {
            return Some(Self::Core);
        }
        text.strip_prefix("ext:")
            .filter(|id| names::is_valid(id))
            .map(|id| Self::Extension(id.to_owned()))
    }
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => f.write_str("core"),
            Self::Extension(id) => write!(f, "ext:{id}"),
        }
    }
}
