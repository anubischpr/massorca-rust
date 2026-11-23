//! The closed permission vocabulary (`docs/architecture/extensions.md` §6), with typed scopes and their validation.
//!
//! A permission without a scope means "any". Unknown kinds and unknown scope fields are errors, never ignored.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::display_chars;
use crate::error::{Problem, Problems, at};
use crate::ids;
use crate::package_path;

/// Every permission kind a manifest may declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionKind {
    Storage,
    Notifications,
    WorkspacesRead,
    FilesRead,
    FilesWrite,
    Net,
    Secrets,
    TerminalsRead,
    TerminalsWrite,
    AgentsRead,
    AgentsControl,
    SessionsRead,
    Process,
    Credentials,
}

const WIRE: [(&str, PermissionKind); 14] = [
    ("storage", PermissionKind::Storage),
    ("notifications", PermissionKind::Notifications),
    ("workspaces:read", PermissionKind::WorkspacesRead),
    ("files:read", PermissionKind::FilesRead),
    ("files:write", PermissionKind::FilesWrite),
    ("net", PermissionKind::Net),
    ("secrets", PermissionKind::Secrets),
    ("terminals:read", PermissionKind::TerminalsRead),
    ("terminals:write", PermissionKind::TerminalsWrite),
    ("agents:read", PermissionKind::AgentsRead),
    ("agents:control", PermissionKind::AgentsControl),
    ("sessions:read", PermissionKind::SessionsRead),
    ("process", PermissionKind::Process),
    ("credentials", PermissionKind::Credentials),
];

impl PermissionKind {
    /// The manifest spelling, such as `files:read`.
    pub fn wire(self) -> &'static str {
        WIRE.iter()
            .find(|(_, kind)| *kind == self)
            .map_or("", |(text, _)| text)
    }

    /// Reads the manifest spelling; `None` for anything outside the vocabulary.
    pub fn parse(text: &str) -> Option<Self> {
        WIRE.iter()
            .find(|(wire, _)| *wire == text)
            .map(|(_, kind)| *kind)
    }

    /// Whether the kind accepts a scope at all.
    pub fn takes_scope(self) -> bool {
        matches!(
            self,
            Self::FilesRead
                | Self::FilesWrite
                | Self::Net
                | Self::Secrets
                | Self::Process
                | Self::Credentials
        )
    }
}

/// A scope, typed by the kind it narrows. Each list holds at least one entry once validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionScope {
    /// Globs relative to a workspace root.
    Files(Vec<String>),
    /// Exact hosts, or `*.domain`, or `*` for any host.
    Net(Vec<String>),
    /// Secret names the extension may use by name.
    Secrets(Vec<String>),
    /// Program plus argument patterns, such as `git status *`.
    Process(Vec<String>),
    /// Agent kind ids whose credentials may be used.
    Credentials(Vec<String>),
}

/// One declared permission with its reason. Built only from a manifest entry that passed the kind and scope checks.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawPermission")]
pub struct Permission {
    pub kind: PermissionKind,
    pub reason: String,
    pub scope: Option<PermissionScope>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPermission {
    kind: String,
    reason: String,
    #[serde(default)]
    scope: Option<serde_json::Value>,
}

impl TryFrom<RawPermission> for Permission {
    type Error = String;

    fn try_from(raw: RawPermission) -> Result<Self, Self::Error> {
        let kind = PermissionKind::parse(&raw.kind)
            .ok_or_else(|| format!("unknown permission kind `{}`", raw.kind))?;
        let scope = match raw.scope {
            None => None,
            Some(_) if !kind.takes_scope() => {
                return Err(format!("permission `{}` takes no scope", raw.kind));
            }
            Some(value) => Some(parse_scope(kind, value)?),
        };
        Ok(Self {
            kind,
            reason: raw.reason,
            scope,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paths {
    paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hosts {
    hosts: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Names {
    names: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Programs {
    programs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Agents {
    agents: Vec<String>,
}

fn parse_scope(kind: PermissionKind, value: serde_json::Value) -> Result<PermissionScope, String> {
    let decoded =
        |error: serde_json::Error| format!("invalid scope for `{}`: {error}", kind.wire());
    Ok(match kind {
        PermissionKind::FilesRead | PermissionKind::FilesWrite => PermissionScope::Files(
            serde_json::from_value::<Paths>(value)
                .map_err(decoded)?
                .paths,
        ),
        PermissionKind::Net => PermissionScope::Net(
            serde_json::from_value::<Hosts>(value)
                .map_err(decoded)?
                .hosts,
        ),
        PermissionKind::Secrets => PermissionScope::Secrets(
            serde_json::from_value::<Names>(value)
                .map_err(decoded)?
                .names,
        ),
        PermissionKind::Process => PermissionScope::Process(
            serde_json::from_value::<Programs>(value)
                .map_err(decoded)?
                .programs,
        ),
        _ => PermissionScope::Credentials(
            serde_json::from_value::<Agents>(value)
                .map_err(decoded)?
                .agents,
        ),
    })
}

/// Checks declared permissions: one entry per kind, a reason on each, and well-formed scope entries. `base` is the
/// JSON-pointer prefix of the list (`/permissions` in a manifest).
pub fn validate(permissions: &[Permission], base: &str) -> Vec<Problem> {
    let mut problems = Problems::default();
    let mut seen = BTreeSet::new();
    for (index, permission) in permissions.iter().enumerate() {
        let here = at(base, index);
        if !seen.insert(permission.kind) {
            problems.add(
                at(&here, "kind"),
                format!("`{}` is declared more than once", permission.kind.wire()),
            );
        }
        if permission.reason.trim().is_empty() {
            problems.add(at(&here, "reason"), "a reason is required");
        } else if display_chars::hidden_char(&permission.reason).is_some() {
            problems.add(at(&here, "reason"), crate::manifest::HIDDEN_TEXT);
        }
        if let Some(scope) = &permission.scope {
            validate_scope(scope, &at(&here, "scope"), &mut problems);
        }
    }
    problems.into_vec()
}

fn validate_scope(scope: &PermissionScope, base: &str, problems: &mut Problems) {
    let (field, items, check): (&str, &[String], fn(&str) -> bool) = match scope {
        PermissionScope::Files(paths) => ("paths", paths, |path| {
            package_path::check(path, true).is_ok()
        }),
        PermissionScope::Net(hosts) => ("hosts", hosts, is_host_pattern),
        PermissionScope::Secrets(names) => ("names", names, is_secret_name),
        PermissionScope::Process(programs) => ("programs", programs, is_program_pattern),
        PermissionScope::Credentials(agents) => ("agents", agents, ids::is_kind_id),
    };
    let list = at(base, field);
    if items.is_empty() {
        problems.add(list.clone(), "lists at least one entry");
    }
    for (index, item) in items.iter().enumerate() {
        if !check(item) {
            problems.add(
                at(&list, index),
                format!("`{item}` is not a valid {field} entry"),
            );
        }
    }
}

/// `*`, an exact lowercase host, or `*.domain`.
fn is_host_pattern(text: &str) -> bool {
    if text == "*" {
        return true;
    }
    let domain = text.strip_prefix("*.").unwrap_or(text);
    !domain.is_empty() && domain.split('.').all(is_host_label)
}

fn is_host_label(label: &str) -> bool {
    label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !label.is_empty()
}

fn is_secret_name(text: &str) -> bool {
    let mut bytes = text.bytes();
    text.len() <= 64
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

/// A pattern whose program part is non-empty, with no leading, trailing or control whitespace.
fn is_program_pattern(text: &str) -> bool {
    text.trim() == text
        && !text.chars().any(char::is_control)
        && text
            .split_whitespace()
            .next()
            .is_some_and(|program| !program.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_their_spelling() {
        for (text, kind) in WIRE {
            assert_eq!(PermissionKind::parse(text), Some(kind));
            assert_eq!(kind.wire(), text);
        }
        assert_eq!(PermissionKind::parse("files:*"), None);
        assert_eq!(PermissionKind::parse("terminals"), None);
    }

    #[test]
    fn only_the_scoped_kinds_take_a_scope() {
        assert!(PermissionKind::FilesRead.takes_scope());
        assert!(PermissionKind::Process.takes_scope());
        assert!(!PermissionKind::Storage.takes_scope());
        assert!(!PermissionKind::AgentsControl.takes_scope());
    }

    #[test]
    fn host_patterns() {
        assert!(is_host_pattern("api.example.com"));
        assert!(is_host_pattern("*.example.com"));
        assert!(is_host_pattern("*"));
        assert!(!is_host_pattern("*."));
        assert!(!is_host_pattern("Api.example.com"));
        assert!(!is_host_pattern("-bad.example.com"));
        assert!(!is_host_pattern("a..b"));
        assert!(!is_host_pattern("example.com/path"));
    }

    #[test]
    fn secret_names_and_program_patterns() {
        assert!(is_secret_name("example-token"));
        assert!(!is_secret_name("-token"));
        assert!(!is_secret_name("a b"));
        assert!(is_program_pattern("git status *"));
        assert!(!is_program_pattern(" git"));
        assert!(!is_program_pattern("   "));
    }
}
