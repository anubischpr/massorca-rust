//! Setting declarations: each setting's owner, type and default. Values are stored by `core-store`, validated against these.

use std::path::Path;

use serde_json::Value;

use crate::names;
use crate::owner::Owner;
use crate::registry::{RegistrationError, RegistrationHandle, Registry};

/// The type a setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    String,
    Bool,
    Integer,
    /// An absolute path, written as a string.
    Path,
}

impl SettingKind {
    fn expectation(self) -> &'static str {
        match self {
            Self::String => "expects a string",
            Self::Bool => "expects true or false",
            Self::Integer => "expects a whole number",
            Self::Path => "expects an absolute path",
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
    pub fn check(&self, value: &Value) -> Option<&'static str> {
        let acceptable = match self.kind {
            SettingKind::String => value.is_string(),
            SettingKind::Bool => value.is_boolean(),
            SettingKind::Integer => value.is_i64(),
            SettingKind::Path => value
                .as_str()
                .is_some_and(|path| Path::new(path).is_absolute()),
        };
        (!acceptable).then_some(self.kind.expectation())
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
