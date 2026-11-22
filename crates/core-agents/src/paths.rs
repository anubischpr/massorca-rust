//! Where agents keep their files in a profile (`docs/architecture/agents.md`, "Profile files").

use std::path::{Path, PathBuf};

use crate::kind::valid_id;

/// `<profile>/agents`.
pub fn agents_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("agents")
}

/// `<profile>/agents/kinds`: the profile's own kind declarations.
pub fn kinds_dir(profile_dir: &Path) -> PathBuf {
    agents_dir(profile_dir).join("kinds")
}

/// `<profile>/agents/homes/<kind>`: the kind's credential home (0056).
pub fn home_dir(profile_dir: &Path, kind: &str) -> PathBuf {
    agents_dir(profile_dir).join("homes").join(kind)
}

/// The credential home of a valid kind id; `None` for an id that could leave the homes directory.
pub fn valid_home(profile_dir: &Path, kind: &str) -> Option<PathBuf> {
    valid_id(kind).then(|| home_dir(profile_dir, kind))
}

/// `<profile>/agents/accounts/<kind>`: the kind's account homes (decision 0059), one directory per account.
pub fn accounts_dir(profile_dir: &Path, kind: &str) -> PathBuf {
    agents_dir(profile_dir).join("accounts").join(kind)
}

/// `<profile>/agents/accounts/<kind>/<account>`: one account's credential home (decision 0059).
pub fn account_home(profile_dir: &Path, kind: &str, account: &str) -> PathBuf {
    accounts_dir(profile_dir, kind).join(account)
}

/// The home of an account of a valid kind id and a canonical UUID; `None` for anything that could leave the accounts
/// directory, including the lower-case rule, so one account has one spelling on disk.
pub fn valid_account_home(profile_dir: &Path, kind: &str, account: &str) -> Option<PathBuf> {
    (valid_id(kind) && valid_account_id(account)).then(|| account_home(profile_dir, kind, account))
}

/// Whether `account` is an account id: a canonical lower-case UUID, as the store generates them.
pub fn valid_account_id(account: &str) -> bool {
    uuid::Uuid::parse_str(account).is_ok_and(|parsed| parsed.hyphenated().to_string() == account)
}

/// `<profile>/agents/runs/<session>`: files generated for a session's runs (0053).
pub fn run_dir(profile_dir: &Path, session: &str) -> PathBuf {
    agents_dir(profile_dir).join("runs").join(session)
}
