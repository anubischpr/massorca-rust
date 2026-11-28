//! The PATH a terminal starts with (decision 0032, TERM-17).
//!
//! On Windows the machine and user `Path` values are read from the registry at every start, so a tool installed after
//! the service started is found. They are merged the way Explorer builds a new process's environment (machine first,
//! then user). Entries only the service's own PATH lists are kept after them.

use std::borrow::Cow;
use std::collections::HashSet;

use crate::shell::EnvSnapshot;
#[cfg(windows)]
use crate::windows_registry::{Hive, ReadError, RegistryPath, read_path};

/// Where a terminal's PATH came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSource {
    /// The machine and user PATH from the registry, merged with the service's own.
    Registry,
    /// The service's own PATH: no registry value was found.
    Inherited,
}

impl PathSource {
    /// The name the CLI and the protocol show.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Inherited => "inherited",
        }
    }
}

/// The environment a terminal's program is resolved and started with, and where its PATH came from. On Windows the
/// PATH is the registry's, merged with the service's; elsewhere the service's environment is used as it is.
pub fn start_environment(base: &EnvSnapshot) -> (Cow<'_, EnvSnapshot>, PathSource) {
    #[cfg(windows)]
    {
        if let Some(path) = registry_path(base) {
            return (
                Cow::Owned(base.clone().with_var("PATH", &path)),
                PathSource::Registry,
            );
        }
    }
    (Cow::Borrowed(base), PathSource::Inherited)
}

/// The merged PATH, or `None` when neither registry value is set. A value that cannot be read counts as not set.
#[cfg(windows)]
fn registry_path(base: &EnvSnapshot) -> Option<String> {
    let machine = read_path(Hive::Machine);
    let user = read_path(Hive::User);
    if !matches!(machine, Ok(Some(_))) && !matches!(user, Ok(Some(_))) {
        return None;
    }
    Some(merge_path(
        &registry_text(machine, base),
        &registry_text(user, base),
        base.get("PATH").unwrap_or(""),
    ))
}

#[cfg(windows)]
fn registry_text(value: Result<Option<RegistryPath>, ReadError>, env: &EnvSnapshot) -> String {
    match value {
        Ok(Some(path)) if path.expandable => expand_references(&path.text, env),
        Ok(Some(path)) => path.text,
        Ok(None) | Err(_) => String::new(),
    }
}

/// The machine PATH, then the user PATH, then the entries of `inherited` that neither lists. Empty entries are dropped.
pub fn merge_path(machine: &str, user: &str, inherited: &str) -> String {
    let mut entries: Vec<&str> = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    for entry in machine
        .split(';')
        .chain(user.split(';'))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        listed.insert(comparable(entry));
        entries.push(entry);
    }
    for entry in inherited
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if listed.insert(comparable(entry)) {
            entries.push(entry);
        }
    }
    entries.join(";")
}

/// Windows compares PATH entries without regard to case or a trailing separator.
fn comparable(entry: &str) -> String {
    entry.trim_end_matches(['\\', '/']).to_ascii_lowercase()
}

/// Expands `%NAME%` references from `env`; an unknown or unclosed reference is kept as written.
pub fn expand_references(value: &str, env: &EnvSnapshot) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            break;
        };
        let name = &after[..end];
        out.push_str(&rest[..start]);
        match env.get(name) {
            Some(found) if !name.is_empty() => out.push_str(found),
            _ => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
