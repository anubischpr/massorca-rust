//! Setting declarations: each setting's owner, type and default. Values are stored by `core-store`, validated against these.

use serde_json::Value;

use crate::names;
use crate::owner::Owner;
use crate::registry::{RegistrationError, RegistrationHandle, Registry};
use crate::setting_values;

/// The type a setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    String,
    Bool,
    Integer,
    /// An absolute path, written as a string.
    Path,
    /// One of a fixed list of strings.
    Choice(&'static [&'static str]),
    /// A whole number from `min` to `max`, both inclusive.
    Range {
        min: i64,
        max: i64,
    },
    /// A list of distinct strings drawn from a fixed list.
    Set(&'static [&'static str]),
    /// A preset name from the list, or `#rrggbb`.
    Colour(&'static [&'static str]),
    /// An object from action id to a shortcut string, or to `null` for unbound.
    Keybindings,
}

impl SettingKind {
    /// The kind's name on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Bool => "bool",
            Self::Integer | Self::Range { .. } => "integer",
            Self::Path => "path",
            Self::Choice(_) => "choice",
            Self::Set(_) => "set",
            Self::Colour(_) => "colour",
            Self::Keybindings => "keybindings",
        }
    }

    /// The fixed list a choice, set or colour draws from.
    pub fn choices(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Choice(list) | Self::Set(list) | Self::Colour(list) => Some(list),
            _ => None,
        }
    }

    /// The inclusive bounds of a ranged integer.
    pub fn range(self) -> Option<(i64, i64)> {
        match self {
            Self::Range { min, max } => Some((min, max)),
            _ => None,
        }
    }
}

/// One declared setting. Its key is unique per owner, so two owners may declare the same key.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingDecl {
    pub owner: Owner,
    pub key: String,
    pub kind: SettingKind,
    pub default: Value,
    pub description: String,
}

impl SettingDecl {
    /// Why `value` is not acceptable for this setting, or `None` when it is.
    pub fn check(&self, value: &Value) -> Option<String> {
        setting_values::check(self.kind, value).err()
    }
}

/// The settings the service declares, keyed by owner and key. Clones share one table.
#[derive(Clone, Debug, Default)]
pub struct SettingsRegistry {
    entries: Registry<SettingDecl>,
}

/// The registry key of a declaration. Owner ids and keys follow the name rule, which excludes the unit separator, so the
/// pair is unambiguous.
fn slot(owner: &Owner, key: &str) -> String {
    format!("{owner}\u{1f}{key}")
}

impl SettingsRegistry {
    pub fn register(&self, decl: SettingDecl) -> Result<RegistrationHandle, RegistrationError> {
        if !names::is_valid(&decl.key) {
            return Err(RegistrationError::InvalidName { name: decl.key });
        }
        let slot = slot(&decl.owner, &decl.key);
        let owner = decl.owner.clone();
        self.entries.register(owner, slot, decl)
    }

    /// The declaration `owner` made for `key`.
    pub fn find(&self, owner: &Owner, key: &str) -> Option<SettingDecl> {
        self.entries
            .get(&slot(owner, key))
            .map(|entry| entry.value.clone())
    }

    /// Every declaration, ordered by owner and then key.
    pub fn declarations(&self) -> Vec<SettingDecl> {
        let mut declarations: Vec<SettingDecl> = self
            .entries
            .list()
            .iter()
            .map(|entry| entry.value.clone())
            .collect();
        declarations.sort_by_key(|decl| (decl.owner.to_string(), decl.key.clone()));
        declarations
    }
}
