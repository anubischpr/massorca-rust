//! Credential homes, one per profile and kind (decision 0056): import, show and remove.
//!
//! Import copies only the files the kind declares, only regular files, at most [`MAX_FILE_BYTES`] each. The new home
//! is built in a temporary sibling directory and renamed into place, so a failed import leaves an existing home as it
//! was and never leaves a half-made one. File contents are never logged or returned: names and sizes only.
//!
//! The files come from one folder the person names, or each from where the kind's CLI keeps it (decision 0097). A
//! declared file listed in `credentials.optional` is skipped when it is missing; every other declared file is required.
//! A missing required file, or any file that is a link, a directory or anything but a regular file, fails the import
//! with `agent.credentials_invalid`, naming each one and why; nothing is copied then. A kind's state file is built from
//! its source's kept keys instead of copied (decision 0099, [`crate::state_file`]).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::UNIX_EPOCH;

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    CredentialFile, CredentialsImported, CredentialsRemoved, CredentialsShown, ImportedFile,
};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::catalog::Catalog;
use crate::kind::{KindDecl, valid_id};
use crate::packages::KindPackages;
use crate::paths;
use crate::sessions::AgentError;
use crate::sources;
use crate::state_file;

/// Largest credential file import copies.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Serialises imports and removals in this process, so two never swap the same home at once.
static GATE: Mutex<()> = Mutex::new(());

/// Holds the credential homes gate: no import or removal runs until the guard drops. Launch can hold it while it
/// resolves a home, so a run never sees a home mid-swap.
pub fn lock_homes() -> MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Purposes of the temporary siblings of a home: a new home being built, and an old one being replaced. Their names
/// start with a dot and kind ids never do, so a sibling never names a home.
const STAGING: &str = "import";
const REPLACED: &str = "old";

/// Where an import reads the kind's declared files from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFrom<'a> {
    /// One absolute folder holding every declared file.
    Folder(&'a Path),
    /// Each file from where the kind's CLI keeps it, as this process resolves it (decision 0097).
    Detected,
}

/// One declared file to import: where it is read from (`None` when no location is known) and whether it is required.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceFile {
    name: String,
    path: Option<PathBuf>,
    required: bool,
}

/// Copies the kind's declared files from `from` into `<profile>/agents/homes/<kind>/`.
///
/// Every declared file found is read in full, at most [`MAX_FILE_BYTES`] each, before anything is written; then a new
/// home is written and renamed into place. An existing home is replaced only with `replace`.
pub fn import(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
    from: &Path,
    replace: bool,
) -> Result<CredentialsImported, AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let home = home_of(profile_dir, &decl.id)?;
    let files = source_files(&decl, ImportFrom::Folder(from))?;
    import_at(profile_dir, &decl, &["homes"], &home, &files, replace)
}

/// Copies a new account's declared files into `<profile>/agents/accounts/<kind>/<account>` (decision 0059). An account
/// is always new, so an existing one is refused and never replaced.
pub fn import_account(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
    account: &str,
    from: ImportFrom<'_>,
) -> Result<CredentialsImported, AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let home = account_home_of(profile_dir, &decl.id, account)?;
    let files = source_files(&decl, from)?;
    import_at(
        profile_dir,
        &decl,
        &["accounts", decl.id.as_str()],
        &home,
        &files,
        false,
    )
}

/// The declared files of `decl` and where each is read from. A kind that declares none refuses the import; a folder
/// must be an absolute path to a directory.
fn source_files(decl: &KindDecl, from: ImportFrom<'_>) -> Result<Vec<SourceFile>, AgentError> {
    let credentials = decl
        .credentials
        .as_ref()
        .filter(|credentials| !credentials.files.is_empty())
        .ok_or_else(|| {
            AgentError::new(
                codes::AGENT_CREDENTIALS_UNSUPPORTED,
                format!(
                    "agent kind {} declares no credential files to import",
                    decl.id
                ),
            )
        })?;
    match from {
        ImportFrom::Folder(from) => {
            if !from.is_absolute() {
                return Err(invalid(format!(
                    "the source {} is not an absolute path",
                    from.display()
                )));
            }
            if !from.is_dir() {
                return Err(invalid(format!(
                    "the source {} is not a directory",
                    from.display()
                )));
            }
            Ok(credentials
                .files
                .iter()
                .map(|name| SourceFile {
                    name: name.clone(),
                    path: Some(from.join(name)),
                    required: credentials.required(name),
                })
                .collect())
        }
        ImportFrom::Detected => Ok(sources::detect_here(credentials)
            .into_iter()
            .map(|file| SourceFile {
                name: file.name,
                path: file.path,
                required: file.required,
            })
            .collect()),
    }
}

/// The shared import: the declared `files`, written into a new `home` made in the directory `chain` names under
/// `<profile>/agents`. A missing optional file is skipped. Nothing but `home` is created or replaced.
fn import_at(
    profile_dir: &Path,
    decl: &KindDecl,
    chain: &[&str],
    home: &Path,
    files: &[SourceFile],
    replace: bool,
) -> Result<CredentialsImported, AgentError> {
    let name = home
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(decl.id.as_str())
        .to_owned();

    let _gate = lock_homes();
    if home_state(home)? == HomeState::Directory && !replace {
        return Err(exists(&name));
    }

    let state = decl
        .credentials
        .as_ref()
        .and_then(|credentials| credentials.state.as_ref());
    let mut contents = Vec::with_capacity(files.len());
    let mut notes = Vec::new();
    let mut problems = Vec::new();
    let mut found_any = false;
    for file in files {
        // The state file is built, never copied raw: only its kept keys and its set keys (decision 0099).
        if let Some(rule) = state.filter(|rule| rule.file == file.name) {
            match state_file::import_source(rule, file.path.as_deref(), file.required) {
                state_file::Imported::Built { bytes, found, note } => {
                    found_any |= found;
                    if let Some(note) = &note {
                        tracing::info!(kind = %decl.id, file = %file.name, "built the state file without its source");
                        notes.push((file.name.as_str(), note.clone()));
                    }
                    contents.push((file.name.as_str(), bytes));
                }
                state_file::Imported::Refused(reason) if reason == MISSING => {
                    problems.push(missing_entry(file));
                }
                state_file::Imported::Refused(reason) => {
                    problems.push(source_problem(file, &reason))
                }
            }
            continue;
        }
        let Some(path) = &file.path else {
            if file.required {
                problems.push(missing_entry(file));
            }
            continue;
        };
        match read_source(path) {
            Ok(bytes) => {
                found_any = true;
                contents.push((file.name.as_str(), bytes));
            }
            // An optional file the import cannot use (missing, a link, too large...) is skipped; it is never read
            // further. A required one refuses the import.
            Err(reason) if !file.required => {
                tracing::info!(kind = %decl.id, file = %file.name, %reason, "skipped an optional credential file");
            }
            Err(reason) => problems.push(source_problem(file, &reason)),
        }
    }
    if problems.is_empty() && !found_any {
        // Every declared file is optional and none is there: there is nothing to import.
        problems = files.iter().map(missing_entry).collect();
    }
    if !problems.is_empty() {
        return Err(refused_files(&decl.id, problems));
    }

    // `agents` and `homes` stay pinned (Windows) from before their access changes until the swap is done.
    let (root, _pinned) = root_dir(profile_dir, chain)?;
    #[cfg(test)]
    tests::run_after_access_change(&paths::agents_dir(profile_dir));
    refuse_replaced_leftover(&root, &name)?;
    clear_leftovers(&root, &name);
    let staging_path = root.join(sibling_name(&name, STAGING));
    let placed = Staging::create(staging_path.clone()).and_then(|staging| {
        write_files(&staging.path, &contents)?;
        check_staging(&staging.path)?;
        put_in_place(&staging, home, &name, replace)
    });
    // The staging pin was dropped with the closure, so a failed staging directory can be deleted here.
    if let Err(error) = placed {
        if fs::symlink_metadata(&staging_path).is_ok() {
            let _ = fs::remove_dir_all(&staging_path);
        }
        return Err(error);
    }
    let files: Vec<ImportedFile> = contents
        .iter()
        .map(|(name, bytes)| ImportedFile {
            name: (*name).to_owned(),
            bytes: bytes.len() as u64,
            reason: notes
                .iter()
                .find(|(noted, _)| noted == name)
                .map(|(_, note)| note.clone()),
        })
        .collect();
    tracing::info!(
        kind = %decl.id,
        files = ?files.iter().map(|file| (&file.name, file.bytes)).collect::<Vec<_>>(),
        "imported agent credentials"
    );
    Ok(CredentialsImported {
        kind: decl.id.clone(),
        home: home.display().to_string(),
        files,
    })
}

/// Whether the kind's home exists, and each declared file in it: present when it is a regular file, with its size
/// and modification time.
pub fn show(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
) -> Result<CredentialsShown, AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let home = home_of(profile_dir, &decl.id)?;
    show_at(profile_dir, &decl, &["homes"], &home)
}

/// Whether one account's home exists, and each declared file in it (decision 0059); names, sizes and times only.
pub fn show_account(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
    account: &str,
) -> Result<CredentialsShown, AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let home = account_home_of(profile_dir, &decl.id, account)?;
    show_at(profile_dir, &decl, &["accounts", decl.id.as_str()], &home)
}

fn show_at(
    profile_dir: &Path,
    decl: &KindDecl,
    chain: &[&str],
    home: &Path,
) -> Result<CredentialsShown, AgentError> {
    check_chain(profile_dir, chain)?;
    let exists = home_state(home)? == HomeState::Directory;
    let names = decl
        .credentials
        .as_ref()
        .map(|credentials| credentials.files.clone())
        .unwrap_or_default();
    let files = names
        .into_iter()
        .map(|name| {
            let meta = exists
                .then(|| fs::symlink_metadata(home.join(&name)).ok())
                .flatten()
                .filter(|meta| meta.is_file() && !is_link(meta));
            CredentialFile {
                present: meta.is_some(),
                bytes: meta.as_ref().map(fs::Metadata::len),
                modified_at_ms: meta.as_ref().and_then(modified_ms),
                name,
            }
        })
        .collect();
    Ok(CredentialsShown {
        kind: decl.id.clone(),
        home: home.display().to_string(),
        exists,
        files,
    })
}

/// Deletes the kind's home. A kind that is no longer declared still has its home removed; a home that is a link is
/// refused, never followed.
pub fn remove(profile_dir: &Path, kind: &str) -> Result<CredentialsRemoved, AgentError> {
    let home = home_of(profile_dir, kind)?;
    remove_at(profile_dir, &["homes"], &home, kind)
}

/// [`remove`] for a caller that already holds [`lock_homes`].
pub(crate) fn remove_locked(
    profile_dir: &Path,
    kind: &str,
) -> Result<CredentialsRemoved, AgentError> {
    let home = home_of(profile_dir, kind)?;
    remove_at_locked(profile_dir, &["homes"], &home, kind)
}

/// Deletes one account's home (decision 0059). Its kind's other accounts and the 0056 home are left alone.
pub fn remove_account(
    profile_dir: &Path,
    kind: &str,
    account: &str,
) -> Result<CredentialsRemoved, AgentError> {
    let home = account_home_of(profile_dir, kind, account)?;
    remove_at(profile_dir, &["accounts", kind], &home, account)
}

/// The shared removal: deletes `home` when it is a plain directory, after checking the directories of `chain`.
fn remove_at(
    profile_dir: &Path,
    chain: &[&str],
    home: &Path,
    name: &str,
) -> Result<CredentialsRemoved, AgentError> {
    let _gate = lock_homes();
    remove_at_locked(profile_dir, chain, home, name)
}

/// [`remove_at`] for a caller that already holds [`lock_homes`]: the gate is not reentrant.
fn remove_at_locked(
    profile_dir: &Path,
    chain: &[&str],
    home: &Path,
    name: &str,
) -> Result<CredentialsRemoved, AgentError> {
    check_chain(profile_dir, chain)?;
    let removed = match home_state(home)? {
        HomeState::Missing => false,
        HomeState::Directory => {
            fs::remove_dir_all(home)
                .map_err(|error| internal("delete the credential home", &error))?;
            true
        }
    };
    if removed {
        tracing::info!(kind = name, "removed agent credentials");
    }
    Ok(CredentialsRemoved {
        kind: name.to_owned(),
        removed,
    })
}

/// [`remove_account`] for a caller that already holds [`lock_homes`].
pub(crate) fn remove_account_locked(
    profile_dir: &Path,
    kind: &str,
    account: &str,
) -> Result<CredentialsRemoved, AgentError> {
    let home = account_home_of(profile_dir, kind, account)?;
    remove_at_locked(profile_dir, &["accounts", kind], &home, account)
}

/// Gives the account `into` the declared credential files the new account `from` holds, both of `kind` (decision 0098),
/// then deletes `from`'s home. Only the declared files `from` has are written, each inside `into`'s home: read in full
/// first (a link, anything but a regular file, or a file over [`MAX_FILE_BYTES`] refuses the update before anything is
/// written), then written to a new owner-only sibling and renamed over the old one. Every other file of `into`'s home
/// stays as it is, and a declared file `from` did not bring is kept: an agent home also holds transcripts, history and
/// settings. A link at a declared file in `into`'s home is refused, never followed. The caller holds [`lock_homes`].
pub(crate) fn update_account_files_locked(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
    from: &str,
    into: &str,
) -> Result<(), AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let source = account_home_of(profile_dir, &decl.id, from)?;
    let target = account_home_of(profile_dir, &decl.id, into)?;
    let (_root, _pins) = root_dir(profile_dir, &["accounts", decl.id.as_str()])?;
    if home_state(&source)? != HomeState::Directory {
        return Err(invalid(format!(
            "account {from} of agent kind {kind} has no credential home to update from"
        )));
    }
    if home_state(&target)? != HomeState::Directory {
        return Err(invalid(format!(
            "account {into} of agent kind {kind} has no credential home; remove it and add the account again"
        )));
    }
    #[cfg(windows)]
    let _target_pin = pin(&target)?;
    let names: Vec<String> = decl
        .credentials
        .as_ref()
        .map(|credentials| credentials.files.clone())
        .unwrap_or_default();
    // The state file is the CLI's own and keeps its data in the existing home; it is repaired before the next check and
    // launch (decision 0099), never replaced by the one an import built.
    let state_file = decl
        .credentials
        .as_ref()
        .and_then(|credentials| credentials.state.as_ref())
        .map(|state| state.file.as_str());
    let mut contents = Vec::new();
    for name in &names {
        if Some(name.as_str()) == state_file {
            continue;
        }
        let path = source.join(name);
        if matches!(fs::symlink_metadata(&path), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            continue;
        }
        let bytes = read_source(&path).map_err(|reason| {
            invalid(format!(
                "the new {name} of agent kind {kind} cannot be used: {reason}"
            ))
        })?;
        contents.push((name.as_str(), bytes));
    }
    if contents.is_empty() {
        return Err(invalid(format!(
            "account {from} of agent kind {kind} holds no declared file to update account {into} with"
        )));
    }
    for (name, _) in &contents {
        match fs::symlink_metadata(target.join(name)) {
            Ok(meta) if is_link(&meta) || !meta.is_file() => {
                return Err(invalid(format!(
                    "{} is not a regular file; credential files are never written through a link, remove it by hand",
                    target.join(name).display()
                )));
            }
            _ => {}
        }
    }
    for (name, bytes) in &contents {
        replace_file(&target.join(name), bytes)?;
    }
    remove_at_locked(profile_dir, &["accounts", decl.id.as_str()], &source, from)?;
    tracing::info!(kind, files = ?contents.iter().map(|(name, _)| *name).collect::<Vec<_>>(), "updated an account's credential files");
    Ok(())
}

/// Writes `bytes` to a new owner-only sibling of `path` and renames it over `path`, so a reader sees the old file or the
/// new one, never a part. The sibling is deleted when the write or the rename fails.
fn replace_file(path: &Path, bytes: &[u8]) -> Result<(), AgentError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("credential");
    let staged = path.with_file_name(sibling_name(name, STAGING));
    let written = write_private_file(&staged, bytes).and_then(|()| fs::rename(&staged, path));
    if let Err(error) = written {
        if fs::symlink_metadata(&staged).is_ok() {
            let _ = fs::remove_file(&staged);
        }
        return Err(internal("put the new credential file in place", &error));
    }
    Ok(())
}

/// Settles the old homes a replace set aside and a crash left behind (decision 0098), at startup under [`lock_homes`]: in
/// `<profile>/agents/homes` and each `<profile>/agents/accounts/<kind>`, an aside folder `.<name>.old-<id>` whose home
/// `<name>` is missing is renamed back to it, and one whose home exists, so the swap finished, is deleted. Links and
/// names that are not a kind or an account id are left alone.
pub(crate) fn restore_replaced_leftovers_locked(profile_dir: &Path) {
    let mut roots: Vec<(Vec<String>, bool)> = vec![(vec!["homes".to_owned()], false)];
    if let Ok(kinds) = fs::read_dir(paths::agents_dir(profile_dir).join("accounts")) {
        for kind in kinds
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
        {
            if valid_id(&kind) {
                roots.push((vec!["accounts".to_owned(), kind], true));
            }
        }
    }
    for (chain, accounts) in roots {
        let chain: Vec<&str> = chain.iter().map(String::as_str).collect();
        if check_chain(profile_dir, &chain).is_err() {
            continue;
        }
        let root = dir_chain(profile_dir, &chain).pop().unwrap_or_default();
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(file) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((home_name, _)) = file
                .strip_prefix('.')
                .and_then(|rest| rest.rsplit_once(&format!(".{REPLACED}-")))
            else {
                continue;
            };
            let named = if accounts {
                paths::valid_account_id(home_name)
            } else {
                valid_id(home_name)
            };
            let aside = entry.path();
            let plain_dir =
                fs::symlink_metadata(&aside).is_ok_and(|meta| meta.is_dir() && !is_link(&meta));
            if !named || !plain_dir {
                continue;
            }
            let home = root.join(home_name);
            match home_state(&home) {
                Ok(HomeState::Missing) => match fs::rename(&aside, &home) {
                    Ok(()) => tracing::warn!("a credential home a crash left aside was put back"),
                    Err(error) => {
                        tracing::error!(aside = %aside.display(), %error, "a credential home left aside could not be put back")
                    }
                },
                Ok(HomeState::Directory) => {
                    if fs::remove_dir_all(&aside).is_err() {
                        tracing::warn!(aside = %aside.display(), "a replaced credential home could not be deleted");
                    }
                }
                Err(_) => {}
            }
        }
    }
}

/// Deletes what is left of a kind's account folder once its accounts are gone (decision 0098): the hidden folders an
/// interrupted import or replace left in `<profile>/agents/accounts/<kind>/` (they may hold credential copies), the
/// folder itself when it is then empty, and the hidden siblings of the kind's 0056 home. A link is refused, never
/// followed, and named in the returned problems; so is anything that could not be deleted. The caller holds
/// [`lock_homes`].
pub(crate) fn remove_kind_leftovers_locked(profile_dir: &Path, kind: &str) -> Vec<AgentError> {
    let mut problems = Vec::new();
    if !valid_id(kind) {
        return problems;
    }
    let remove_hidden = |root: &Path, prefix: &str, problems: &mut Vec<AgentError>| {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_str().is_some_and(|name| name.starts_with(prefix)) {
                continue;
            }
            let path = entry.path();
            match fs::symlink_metadata(&path) {
                Ok(meta) if is_link(&meta) => problems.push(invalid(format!(
                    "{} is a link; it is never followed, remove it by hand",
                    path.display()
                ))),
                Ok(meta) if meta.is_dir() => {
                    if let Err(error) = fs::remove_dir_all(&path) {
                        problems.push(internal("delete a leftover credential folder", &error));
                    }
                }
                Ok(_) => {
                    if let Err(error) = fs::remove_file(&path) {
                        problems.push(internal("delete a leftover credential file", &error));
                    }
                }
                Err(_) => {}
            }
        }
    };
    match check_chain(profile_dir, &["accounts", kind]) {
        Ok(()) => {
            let accounts = paths::accounts_dir(profile_dir, kind);
            if fs::symlink_metadata(&accounts).is_ok() {
                remove_hidden(&accounts, ".", &mut problems);
                match fs::remove_dir(&accounts) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    // A busy account kept its home; that is reported already.
                    Err(_)
                        if fs::read_dir(&accounts)
                            .is_ok_and(|mut entries| entries.next().is_some()) => {}
                    Err(error) => {
                        problems.push(internal("delete the kind's account folder", &error))
                    }
                }
            }
        }
        Err(error) => problems.push(error),
    }
    match check_homes_dir(profile_dir) {
        Ok(()) => {
            let homes = paths::agents_dir(profile_dir).join("homes");
            remove_hidden(&homes, &format!(".{kind}."), &mut problems);
        }
        Err(error) => problems.push(error),
    }
    problems
}

/// The account's home when its kind's directory and it exist as plain directories, `None` when it does not exist yet
/// (decision 0059). Launch uses this under [`lock_homes`] instead of `is_dir`, so a link is refused, never followed.
pub fn usable_account_home(
    profile_dir: &Path,
    kind: &str,
    account: &str,
) -> Result<Option<PathBuf>, AgentError> {
    let home = account_home_of(profile_dir, kind, account)?;
    check_chain(profile_dir, &["accounts", kind])?;
    Ok((home_state(&home)? == HomeState::Directory).then_some(home))
}

/// The contents of the account's credential file (decision 0176): the first required file the kind declares. The
/// checks are those of [`show_account`] and launch: a valid kind and account id, `<profile>/agents` and
/// `accounts/<kind>` plain directories and the home a plain directory ([`usable_account_home`]), then the file a regular
/// file opened without following a link, at most [`MAX_FILE_BYTES`] ([`read_capped_into`]). The caller holds
/// [`lock_homes`]. The returned bytes are zeroed when dropped; an error names the file and why, never anything read.
pub(crate) fn read_account_credential(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
    account: &str,
) -> Result<Zeroizing<Vec<u8>>, AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let name = decl
        .credentials
        .as_ref()
        .and_then(|credentials| {
            credentials
                .files
                .iter()
                .find(|name| credentials.required(name))
        })
        .ok_or_else(|| {
            AgentError::new(
                codes::AGENT_CREDENTIALS_UNSUPPORTED,
                format!("agent kind {kind} declares no credential file"),
            )
        })?;
    let home = usable_account_home(profile_dir, &decl.id, account)?.ok_or_else(|| {
        invalid(format!(
            "account {account} of agent kind {kind} has no credential home"
        ))
    })?;
    let mut bytes = Zeroizing::new(Vec::new());
    read_capped_into(&home.join(name), MAX_FILE_BYTES, &mut bytes).map_err(|reason| {
        invalid(format!(
            "the credential file {name} of account {account} of agent kind {kind} cannot be used: {reason}"
        ))
    })?;
    Ok(bytes)
}

/// The refusal of a credential file that is not UTF-8 text; it names nothing of the contents.
pub(crate) fn not_text(kind: &str, account: &str) -> AgentError {
    invalid(format!(
        "the credential file of account {account} of agent kind {kind} cannot be used: not text"
    ))
}

/// The declaration of a known, valid kind: `agent.kind_not_found` or `agent.credentials_unsupported` otherwise.
pub(crate) fn declared_kind(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
) -> Result<KindDecl, AgentError> {
    declared(profile_dir, packages, kind)
}

/// Creates an empty, owner-only home for a new account, so a sign-in can write into it (decision 0059). The caller holds
/// [`lock_homes`].
pub(crate) fn create_account_home(
    profile_dir: &Path,
    kind: &str,
    account: &str,
) -> Result<PathBuf, AgentError> {
    let home = account_home_of(profile_dir, kind, account)?;
    let (_root, _pins) = root_dir(profile_dir, &["accounts", kind])?;
    if home_state(&home)? == HomeState::Directory {
        return Err(exists(account));
    }
    create_private_dir(&home)
        .map_err(|error| internal("create the account's credential home", &error))?;
    Ok(home)
}

/// Moves 0056's single home of `kind` into the account directory `account`: one same-volume rename, so the files are
/// never copied (decision 0059). The caller holds [`lock_homes`]; nothing is moved when the legacy home is absent.
pub(crate) fn adopt_legacy_home(
    profile_dir: &Path,
    kind: &str,
    account: &str,
) -> Result<(), AgentError> {
    let legacy = home_of(profile_dir, kind)?;
    let target = account_home_of(profile_dir, kind, account)?;
    // `agents` is pinned once, by the accounts chain; `homes` is pinned on top of it (Windows), so neither can be
    // swapped for a junction while the home moves.
    let (_root, _root_pin) = root_dir(profile_dir, &["accounts", kind])?;
    #[cfg(windows)]
    let _homes_pin = pin(&paths::agents_dir(profile_dir).join("homes"))?;
    if home_state(&legacy)? != HomeState::Directory {
        return Ok(());
    }
    if home_state(&target)? != HomeState::Missing {
        return Err(exists(account));
    }
    fs::rename(&legacy, &target)
        .map_err(|error| internal("move the credential home into an account", &error))?;
    tracing::info!(kind, "moved the credential home into an account");
    Ok(())
}

/// Whether every required file the kind declares is present in the account's home as a regular file, never a link. The caller
/// holds [`lock_homes`] or accepts that a concurrent change may follow.
pub(crate) fn account_files_present(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
    account: &str,
) -> Result<bool, AgentError> {
    let decl = declared(profile_dir, packages, kind)?;
    let home = account_home_of(profile_dir, &decl.id, account)?;
    let names: Vec<String> = decl
        .credentials
        .as_ref()
        .map(|credentials| {
            credentials
                .files
                .iter()
                .filter(|name| credentials.required(name))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if names.is_empty() || home_state(&home)? != HomeState::Directory {
        return Ok(false);
    }
    Ok(names.iter().all(|name| {
        fs::symlink_metadata(home.join(name)).is_ok_and(|meta| meta.is_file() && !is_link(&meta))
    }))
}

/// The home of an account of a valid kind and id; anything else is refused before the disk is touched.
fn account_home_of(profile_dir: &Path, kind: &str, account: &str) -> Result<PathBuf, AgentError> {
    paths::valid_account_home(profile_dir, kind, account)
        .ok_or_else(|| invalid(format!("{account:?} is not an account id")))
}

/// The kind's declaration, when it is known and valid.
fn declared(
    profile_dir: &Path,
    packages: &dyn KindPackages,
    kind: &str,
) -> Result<KindDecl, AgentError> {
    let catalog = Catalog::load(packages, profile_dir);
    let entry = catalog.find(kind).ok_or_else(|| not_found(kind))?;
    entry.valid().cloned().ok_or_else(|| {
        AgentError::new(
            codes::AGENT_CREDENTIALS_UNSUPPORTED,
            format!(
                "agent kind {kind} is invalid, so it has no usable credentials: {}",
                entry.problems.join("; ")
            ),
        )
    })
}

/// The home of a valid kind id; any other id is an unknown kind.
fn home_of(profile_dir: &Path, kind: &str) -> Result<PathBuf, AgentError> {
    paths::valid_home(profile_dir, kind).ok_or_else(|| not_found(kind))
}

/// What is at a credential home path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeState {
    Missing,
    /// A plain directory, not a link.
    Directory,
}

/// The kind's home when the profile has one: `<profile>/agents` and `homes` are plain directories (or absent) and the
/// home is a plain directory. A link at any of them is refused with `agent.credentials_invalid`, never followed.
/// Launch can use this instead of `is_dir`, under [`lock_homes`].
pub fn usable_home(profile_dir: &Path, kind: &str) -> Result<Option<PathBuf>, AgentError> {
    let home = home_of(profile_dir, kind)?;
    check_homes_dir(profile_dir)?;
    Ok((home_state(&home)? == HomeState::Directory).then_some(home))
}

/// What is at the home path, without following a link there. A link or anything but a directory is refused.
pub fn home_state(home: &Path) -> Result<HomeState, AgentError> {
    match fs::symlink_metadata(home) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HomeState::Missing),
        Err(error) => Err(internal("read the credential home", &error)),
        Ok(meta) if is_link(&meta) => Err(invalid(format!(
            "the credential home {} is a link; it is never followed, remove it by hand",
            home.display()
        ))),
        Ok(meta) if meta.is_dir() => Ok(HomeState::Directory),
        Ok(_) => Err(invalid(format!(
            "the credential home {} is not a directory; remove it by hand",
            home.display()
        ))),
    }
}

/// Directories held open so they cannot be renamed or replaced: on Windows, [`massorca_profile::PinnedDir`]s;
/// elsewhere nothing, and the link checks stand alone.
#[cfg(windows)]
type Pins = Vec<massorca_profile::PinnedDir>;
#[cfg(not(windows))]
type Pins = Vec<std::convert::Infallible>;

/// The directories from `<profile>/agents` down through `chain`, in order.
fn dir_chain(profile_dir: &Path, chain: &[&str]) -> Vec<PathBuf> {
    let mut current = paths::agents_dir(profile_dir);
    let mut dirs = vec![current.clone()];
    for part in chain {
        current = current.join(part);
        dirs.push(current.clone());
    }
    dirs
}

/// The directory a credential home is made in, the last of `<profile>/agents` and `chain` (`homes` for 0056, or
/// `accounts/<kind>` for an account), created when missing. None of those directories may be a link. On Windows each
/// is pinned before its access is checked or changed, and stays pinned while the returned [`Pins`] live, so nobody can
/// swap one for a junction in between.
fn root_dir(profile_dir: &Path, chain: &[&str]) -> Result<(PathBuf, Pins), AgentError> {
    let dirs = dir_chain(profile_dir, chain);
    #[cfg(windows)]
    let mut pins: Pins = Vec::with_capacity(dirs.len());
    #[cfg(not(windows))]
    let pins = Pins::new();
    for dir in &dirs {
        match create_private_dir(dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(internal("create the credential homes directory", &error)),
        }
        let meta = fs::symlink_metadata(dir)
            .map_err(|error| internal("read the credential homes directory", &error))?;
        if is_link(&meta) || !meta.is_dir() {
            return Err(invalid(format!(
                "{} is not a plain directory; credential homes are never written through a link",
                dir.display()
            )));
        }
        #[cfg(windows)]
        let pinned = pin(dir)?;
        // A directory made before this service restricted it keeps its inherited access, and one an elevated run
        // made is owned by Administrators; restrict it now, which also makes the user its owner, so no other user can
        // replace an entry in it and a non-elevated run accepts it later.
        #[cfg(windows)]
        if !massorca_profile::has_owner_only_dacl(dir).unwrap_or(false)
            || !pinned.owned_by_token_user().unwrap_or(false)
        {
            massorca_profile::restrict_dir_to_owner(dir).map_err(|error| {
                internal(
                    "restrict the credential homes directory to its owner",
                    &error,
                )
            })?;
        }
        #[cfg(windows)]
        pins.push(pinned);
    }
    let last = dirs.last().cloned().unwrap_or_default();
    Ok((last, pins))
}

/// Creates `<profile>/agents` when it is missing. On Windows it is created owner-only and owned by the token user
/// (see [`create_private_dir`]), so a run's files never leave it owned by Administrators after an elevated run;
/// elsewhere nothing is done here, and the caller's own directory creation applies.
pub fn create_agents_dir(profile_dir: &Path) -> io::Result<()> {
    #[cfg(windows)]
    match create_private_dir(&paths::agents_dir(profile_dir)) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    #[cfg(not(windows))]
    let _ = profile_dir;
    Ok(())
}

/// Pins `dir`, refusing a reparse point or anything but a directory with `agent.credentials_invalid`.
#[cfg(windows)]
fn pin(dir: &Path) -> Result<massorca_profile::PinnedDir, AgentError> {
    massorca_profile::PinnedDir::open(dir).map_err(|error| pin_refusal(dir, &error))
}

/// Why `dir` could not be pinned: a link or not a directory, or owned by another user (whose owner rights would let
/// them rewrite its access), both refused with `agent.credentials_invalid`; anything else is internal.
#[cfg(windows)]
fn pin_refusal(dir: &Path, error: &io::Error) -> AgentError {
    match error.kind() {
        io::ErrorKind::InvalidInput => invalid(format!(
            "{} is not a plain directory; credential homes are never written through a link",
            dir.display()
        )),
        io::ErrorKind::PermissionDenied => invalid(format!(
            "{} cannot hold credentials: {error}",
            dir.display()
        )),
        _ => internal("hold the credential homes directory", error),
    }
}

/// The new home being built: created owner-only, and on Windows pinned from creation until it is moved into place
/// through its own handle.
struct Staging {
    path: PathBuf,
    #[cfg(windows)]
    pin: Option<massorca_profile::PinnedDir>,
}

impl Staging {
    fn create(path: PathBuf) -> Result<Self, AgentError> {
        create_private_dir(&path)
            .map_err(|error| internal("create the new credential home", &error))?;
        Ok(Self {
            #[cfg(windows)]
            pin: Some(pin(&path)?),
            path,
        })
    }

    /// Moves the new home to `target`: through its pin on Windows, so it stays pinned until the move is done.
    fn move_to(&self, target: &Path) -> io::Result<()> {
        #[cfg(windows)]
        if let Some(pin) = &self.pin {
            return pin.rename_to(target);
        }
        fs::rename(&self.path, target)
    }
}

/// Checks the 0056 homes directory, see [`check_chain`].
fn check_homes_dir(profile_dir: &Path) -> Result<(), AgentError> {
    check_chain(profile_dir, &["homes"])
}

/// Checks, without creating anything, that `<profile>/agents` and the directories of `chain` are plain directories
/// when they exist.
fn check_chain(profile_dir: &Path, chain: &[&str]) -> Result<(), AgentError> {
    for dir in dir_chain(profile_dir, chain) {
        match fs::symlink_metadata(&dir) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(internal("read the credential homes directory", &error)),
            Ok(meta) if is_link(&meta) || !meta.is_dir() => {
                return Err(invalid(format!(
                    "{} is not a plain directory; credential homes are never reached through a link",
                    dir.display()
                )));
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

/// Refuses an import while an old home of `kind` set aside by an earlier replace is still there: it may be the only
/// copy of a login, so it is never deleted silently. The owner moves or deletes it by hand.
fn refuse_replaced_leftover(homes: &Path, kind: &str) -> Result<(), AgentError> {
    let prefix = format!(".{kind}.{REPLACED}-");
    let entries =
        fs::read_dir(homes).map_err(|error| internal("list the credential homes", &error))?;
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(&prefix))
        {
            let path = entry.path();
            let mut error = invalid(format!(
                "{} holds an old credential home of {kind} set aside by an earlier replace; rename it back to {kind} if the home is missing, or move or delete it by hand, then import again",
                path.display()
            ));
            error.data = Some(json!({ "leftover": path.display().to_string() }));
            return Err(error);
        }
    }
    Ok(())
}

fn sibling_name(kind: &str, purpose: &str) -> String {
    format!(".{kind}.{purpose}-{}", uuid::Uuid::new_v4().simple())
}

/// Deletes the half-built homes an interrupted import of `kind` left behind; they may hold credential copies. An old
/// home set aside by a replace is never deleted here (see [`refuse_replaced_leftover`]).
fn clear_leftovers(homes: &Path, kind: &str) {
    let Ok(entries) = fs::read_dir(homes) else {
        return;
    };
    let prefix = format!(".{kind}.{STAGING}-");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(&prefix) {
            continue;
        }
        let path = entry.path();
        let plain_dir =
            fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir() && !is_link(&meta));
        if plain_dir && fs::remove_dir_all(&path).is_err() {
            tracing::warn!(
                kind,
                leftover = name,
                "a leftover credential directory could not be deleted"
            );
        }
    }
}

/// Reads one declared source file: a regular file, opened without following a link, at most [`MAX_FILE_BYTES`] as
/// read from the open handle. The error is the reason given to the person, never anything of the content.
fn read_source(path: &Path) -> Result<Vec<u8>, String> {
    read_capped(path, MAX_FILE_BYTES)
}

/// [`read_source`] with its own cap: the state file's source may be larger (decision 0099).
pub(crate) fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    read_capped_into(path, cap, &mut bytes)?;
    Ok(bytes)
}

/// [`read_capped`] into `bytes`, reserved once at the file's size, so no reallocation leaves a copy of the contents behind
/// unless the file grows while it is read. A caller that zeroes `bytes` afterwards (decision 0176) zeroes every copy.
pub(crate) fn read_capped_into(path: &Path, cap: u64, bytes: &mut Vec<u8>) -> Result<(), String> {
    let before = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(MISSING.to_owned()),
        Err(_) => return Err(UNREADABLE.to_owned()),
        Ok(meta) => meta,
    };
    not_regular(&before)?;
    let file = open_no_follow(path).map_err(|_| UNREADABLE.to_owned())?;
    let after = file.metadata().map_err(|_| UNREADABLE.to_owned())?;
    not_regular(&after)?;
    if !same_file(&before, &after) {
        return Err("replaced while it was read".to_owned());
    }
    if after.len() > cap {
        return Err(too_large(cap));
    }
    bytes.reserve_exact(usize::try_from(after.len().saturating_add(1)).unwrap_or(0));
    file.take(cap + 1)
        .read_to_end(bytes)
        .map_err(|_| UNREADABLE.to_owned())?;
    if bytes.len() as u64 > cap {
        return Err(too_large(cap));
    }
    Ok(())
}

/// The reason of a file over `cap`: `larger than 1 MiB`, `larger than 16 MiB`, or a byte count.
pub(crate) fn too_large(cap: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if cap.is_multiple_of(MIB) {
        format!("larger than {} MiB", cap / MIB)
    } else {
        format!("larger than {cap} bytes")
    }
}

const UNREADABLE: &str = "unreadable";
/// The reason of a file that is not there.
pub(crate) const MISSING: &str = "missing";
/// Where a file with no declared location was looked for.
const NO_LOCATION: &str = "the kind declares no location for it; import from a folder";

/// The refusal entry of a file that is there but cannot be used: its name, why, and where it is.
fn source_problem(file: &SourceFile, reason: &str) -> Value {
    let path = file
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    json!({ "name": file.name, "reason": reason, "path": path })
}

/// The refusal entry of a file that is not there: its name and where it was looked for.
fn missing_entry(file: &SourceFile) -> Value {
    match &file.path {
        Some(path) => {
            json!({ "name": file.name, "reason": MISSING, "path": path.display().to_string() })
        }
        None => json!({ "name": file.name, "reason": MISSING }),
    }
}

/// Whether an import would accept the file at `path`, at most `cap` bytes, judged from its metadata alone, without
/// opening it; the reason when it would not (`missing`, `a link`, `a directory`, `larger than 1 MiB`...).
pub(crate) fn judge(path: &Path, cap: u64) -> Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(MISSING.to_owned()),
        Err(_) => return Err(UNREADABLE.to_owned()),
        Ok(meta) => meta,
    };
    not_regular(&meta)?;
    if meta.len() > cap {
        return Err(too_large(cap));
    }
    Ok(())
}

/// Why an entry is not a regular file, if it is not.
pub(crate) fn not_regular(meta: &fs::Metadata) -> Result<(), String> {
    let reason = if is_link(meta) {
        "a link"
    } else if meta.is_dir() {
        "a directory"
    } else if !meta.is_file() {
        "not a regular file"
    } else {
        return Ok(());
    };
    Err(reason.to_owned())
}

/// A symbolic link, or on Windows any reparse point (junctions included).
pub(crate) fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}

/// Opens for reading without following a final link: `O_NOFOLLOW` on Unix (with `O_NONBLOCK`, so a FIFO swapped in
/// never blocks the open), `FILE_FLAG_OPEN_REPARSE_POINT` on Windows, which opens a reparse point itself.
fn open_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

/// Whether two metadata reads are of one file: device and inode on Unix. Elsewhere the handle's own check (no
/// reparse point, a regular file) stands for it.
#[cfg(unix)]
fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn same_file(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

/// Creates `dir` and writes each file new in it. Creating fails when anything is at a path, so no link is followed.
fn write_files(dir: &Path, contents: &[(&str, Vec<u8>)]) -> Result<(), AgentError> {
    for (name, bytes) in contents {
        write_private_file(&dir.join(name), bytes)
            .map_err(|error| internal("write a credential file", &error))?;
    }
    Ok(())
}

/// Checks the finished new home before it is moved into place: still a plain directory, and on Windows still carrying
/// the protected owner-only DACL it was created with. Anything else is refused, and the caller deletes it.
fn check_staging(dir: &Path) -> Result<(), AgentError> {
    let meta = fs::symlink_metadata(dir)
        .map_err(|error| internal("read the new credential home", &error))?;
    if is_link(&meta) || !meta.is_dir() {
        return Err(invalid(format!(
            "the new credential home {} was replaced while it was written; nothing was imported",
            dir.display()
        )));
    }
    #[cfg(windows)]
    if !massorca_profile::has_owner_only_dacl(dir)
        .map_err(|error| internal("read the new credential home's access", &error))?
    {
        return Err(invalid(format!(
            "the new credential home {} does not carry the owner-only access it was created with; nothing was \
             imported",
            dir.display()
        )));
    }
    Ok(())
}

/// A new directory that only its owner can reach: `0700` on Unix; on Windows created with a protected owner-only DACL
/// (the owner and SYSTEM), since the profile directory keeps its inherited access (0004). Its files inherit it.
/// Creating fails when anything is at the path, so no link is followed.
fn create_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new().mode(0o700).create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
    }
    #[cfg(windows)]
    {
        massorca_profile::create_dir_owner_only(dir)
    }
    #[cfg(not(any(unix, windows)))]
    fs::DirBuilder::new().create(dir)
}

/// A new file (`create_new` never opens through a link), `0600` on Unix, written and flushed to disk.
pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()
}

/// Renames the finished `staging` directory to `home`. An existing home is refused without `replace`; with it, the
/// old home is moved aside, put back when the swap fails, and deleted after it succeeds.
fn put_in_place(
    staging: &Staging,
    home: &Path,
    kind: &str,
    replace: bool,
) -> Result<(), AgentError> {
    match home_state(home)? {
        HomeState::Missing => staging
            .move_to(home)
            .map_err(|error| internal("move the new credential home into place", &error)),
        HomeState::Directory if !replace => Err(exists(kind)),
        HomeState::Directory => {
            let aside = home.with_file_name(sibling_name(kind, REPLACED));
            fs::rename(home, &aside)
                .map_err(|error| internal("move the old credential home aside", &error))?;
            if let Err(error) = staging.move_to(home) {
                if fs::rename(&aside, home).is_err() {
                    return Err(not_put_back(kind, &aside, &error));
                }
                return Err(internal("move the new credential home into place", &error));
            }
            if fs::remove_dir_all(&aside).is_err() {
                tracing::warn!(
                    kind,
                    aside = %aside.display(),
                    "the replaced credential home could not be deleted; delete it by hand before the next import"
                );
            }
            Ok(())
        }
    }
}

/// The swap failed and the old home could not be put back either: it is still at `aside`, named in the text and in
/// `data`, so the owner can rename it to the home by hand.
fn not_put_back(kind: &str, aside: &Path, error: &io::Error) -> AgentError {
    tracing::error!(kind, aside = %aside.display(), "the old credential home could not be put back");
    let mut refused = AgentError::new(
        codes::INTERNAL,
        format!(
            "cannot move the new credential home of agent kind {kind} into place ({error}), and the old one could not be put back: it is at {}; rename it to the home by hand",
            aside.display()
        ),
    );
    refused.data = Some(json!({ "aside": aside.display().to_string() }));
    refused
}

fn modified_ms(meta: &fs::Metadata) -> Option<i64> {
    let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(since.as_millis()).ok()
}

fn not_found(kind: &str) -> AgentError {
    AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {kind}"))
}

fn exists(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_CREDENTIALS_EXIST,
        format!("agent kind {kind} already has a credential home; import with replace to swap it"),
    )
}

fn invalid(message: String) -> AgentError {
    AgentError::new(codes::AGENT_CREDENTIALS_INVALID, message)
}

/// The declared files that cannot be imported, each with its reason and where it was looked for. When every one is
/// missing, the message names each missing file and where it was looked for, so the person knows what to sign in or copy.
fn refused_files(kind: &str, problems: Vec<Value>) -> AgentError {
    let text = |problem: &Value, key: &str| problem[key].as_str().unwrap_or_default().to_owned();
    let all_missing = problems.iter().all(|problem| problem["reason"] == MISSING);
    let message = if all_missing {
        let listed = problems
            .iter()
            .map(|problem| match problem["path"].as_str() {
                Some(path) => format!("{} (looked for at {path})", text(problem, "name")),
                None => format!("{} ({NO_LOCATION})", text(problem, "name")),
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("nothing was imported for agent kind {kind}; a required file is missing: {listed}")
    } else {
        let listed = problems
            .iter()
            .map(|problem| format!("{} ({})", text(problem, "name"), text(problem, "reason")))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "nothing was imported for agent kind {kind}; every file imported must be a regular file: {listed}"
        )
    };
    let mut error = invalid(message);
    error.data = Some(json!({ "files": problems }));
    error
}

/// An IO failure: what was being done and the error. The error's text may name a path, never anything read.
fn internal(doing: &str, error: &io::Error) -> AgentError {
    AgentError::new(codes::INTERNAL, format!("cannot {doing}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use massorca_protocol::codes;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::packages::BuiltInPackages;
    use crate::paths;

    // The functions under test with the built-in packages: these tests are about the files, not the packages.
    fn import(
        profile_dir: &Path,
        kind: &str,
        from: &Path,
        replace: bool,
    ) -> Result<CredentialsImported, AgentError> {
        super::import(profile_dir, &BuiltInPackages, kind, from, replace)
    }

    fn import_account(
        profile_dir: &Path,
        kind: &str,
        account: &str,
        from: &Path,
    ) -> Result<CredentialsImported, AgentError> {
        super::import_account(
            profile_dir,
            &BuiltInPackages,
            kind,
            account,
            ImportFrom::Folder(from),
        )
    }

    fn show(profile_dir: &Path, kind: &str) -> Result<CredentialsShown, AgentError> {
        super::show(profile_dir, &BuiltInPackages, kind)
    }

    const FAKE_TOKEN: &str = r#"{"fake":true}"#;

    /// A hook the test seam runs with the `agents` path.
    type Hook = Box<dyn FnMut(&Path)>;

    thread_local! {
        /// Test-only seam: runs on the importing thread after `agents` and `homes` got their access, before the new
        /// home is written and swapped in. Never compiled outside tests.
        static AFTER_ACCESS_CHANGE: std::cell::RefCell<Option<Hook>> =
            std::cell::RefCell::new(None);
    }

    pub(super) fn run_after_access_change(agents: &Path) {
        AFTER_ACCESS_CHANGE.with(|hook| {
            if let Some(hook) = hook.borrow_mut().as_mut() {
                hook(agents);
            }
        });
    }

    /// A profile declaring kind `fake` with `files`, and an empty source directory.
    struct Fixture {
        profile: TempDir,
        source: TempDir,
    }

    impl Fixture {
        fn new(files: &[&str]) -> Self {
            let fixture = Self {
                profile: tempfile::tempdir().unwrap(),
                source: tempfile::tempdir().unwrap(),
            };
            fixture.declare(json!({ "home_env": "FAKE_AGENT_HOME", "files": files }));
            fixture
        }

        fn declare(&self, credentials: serde_json::Value) {
            let kinds = paths::kinds_dir(self.profile.path());
            fs::create_dir_all(&kinds).unwrap();
            let kind = json!({
                "id": "fake", "name": "Fake", "program": ["fake-agent"], "credentials": credentials
            });
            fs::write(kinds.join("fake.json"), kind.to_string()).unwrap();
        }

        fn source_file(&self, name: &str, content: &[u8]) {
            fs::write(self.source.path().join(name), content).unwrap();
        }

        fn import(&self, replace: bool) -> Result<CredentialsImported, AgentError> {
            import(self.profile.path(), "fake", self.source.path(), replace)
        }

        fn home(&self) -> PathBuf {
            paths::home_dir(self.profile.path(), "fake")
        }

        /// Every entry of the homes directory, by name.
        fn homes_entries(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.home().parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    #[test]
    fn import_copies_only_the_declared_files() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.source_file("history.jsonl", b"not declared");
        let imported = fixture.import(false).unwrap();
        assert_eq!(imported.kind, "fake");
        assert_eq!(imported.home, fixture.home().display().to_string());
        assert_eq!(imported.files.len(), 1);
        assert_eq!(imported.files[0].name, "token.json");
        assert_eq!(imported.files[0].bytes, FAKE_TOKEN.len() as u64);
        assert_eq!(
            fs::read_to_string(fixture.home().join("token.json")).unwrap(),
            FAKE_TOKEN
        );
        assert!(!fixture.home().join("history.jsonl").exists());
        assert_eq!(
            fixture.homes_entries(),
            ["fake"],
            "no staging directory is left"
        );
    }

    #[test]
    fn a_second_import_needs_replace_and_replace_swaps_the_whole_home() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.import(false).unwrap();
        fs::write(fixture.home().join("stray"), b"left by the agent").unwrap();

        let refused = fixture.import(false).unwrap_err();
        assert_eq!(refused.code, codes::AGENT_CREDENTIALS_EXIST);
        assert!(
            fixture.home().join("stray").exists(),
            "refusal changes nothing"
        );

        fixture.source_file("token.json", br#"{"fake":2}"#);
        fixture.import(true).unwrap();
        assert_eq!(
            fs::read_to_string(fixture.home().join("token.json")).unwrap(),
            r#"{"fake":2}"#
        );
        assert!(
            !fixture.home().join("stray").exists(),
            "the old home is gone"
        );
        assert_eq!(fixture.homes_entries(), ["fake"]);
    }

    #[test]
    fn a_failed_replace_leaves_the_existing_home_untouched() {
        let fixture = Fixture::new(&["token.json", "config.toml"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.source_file("config.toml", b"fake = true\n");
        fixture.import(false).unwrap();

        fs::remove_file(fixture.source.path().join("config.toml")).unwrap();
        fixture.source_file("token.json", br#"{"fake":2}"#);
        let error = fixture.import(true).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(error.message.contains("config.toml"), "{}", error.message);
        assert_eq!(
            fs::read_to_string(fixture.home().join("token.json")).unwrap(),
            FAKE_TOKEN
        );
        assert!(fixture.home().join("config.toml").exists());
        assert_eq!(fixture.homes_entries(), ["fake"]);
    }

    #[test]
    fn a_failed_swap_puts_the_old_home_back() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.import(false).unwrap();
        let missing_staging = fixture.home().with_file_name(".fake.import-missing");
        let missing_staging = Staging {
            path: missing_staging,
            #[cfg(windows)]
            pin: None,
        };
        let error = put_in_place(&missing_staging, &fixture.home(), "fake", true).unwrap_err();
        assert_eq!(error.code, codes::INTERNAL);
        assert_eq!(
            fs::read_to_string(fixture.home().join("token.json")).unwrap(),
            FAKE_TOKEN
        );
        assert_eq!(fixture.homes_entries(), ["fake"]);
    }

    #[test]
    fn a_missing_first_import_leaves_no_home() {
        let fixture = Fixture::new(&["token.json"]);
        let error = fixture.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert_eq!(error.data.unwrap()["files"][0]["reason"], "missing");
        assert!(!fixture.home().exists());
    }

    /// A fixture with a required `token.json` and a state file `state.json`, optional unless `required`.
    fn with_state(required: bool) -> Fixture {
        let fixture = Fixture::new(&[]);
        let optional: &[&str] = if required { &[] } else { &["state.json"] };
        fixture.declare(json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.json", "state.json"],
            "optional": optional,
            "state": { "file": "state.json", "keep": ["account", "theme"],
                       "set": { "onboarded": true }, "max_source_bytes": 4096 }
        }));
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture
    }

    fn home_state_file(fixture: &Fixture) -> Value {
        serde_json::from_slice(&fs::read(fixture.home().join("state.json")).unwrap()).unwrap()
    }

    #[test]
    fn the_state_file_is_built_from_its_kept_keys_and_set_keys_never_copied_raw() {
        let fixture = with_state(false);
        fixture.source_file(
            "state.json",
            json!({ "account": "fake", "projects": { "fake": [1] }, "onboarded": false })
                .to_string()
                .as_bytes(),
        );
        let imported = fixture.import(false).unwrap();
        assert_eq!(
            home_state_file(&fixture),
            json!({ "account": "fake", "onboarded": true })
        );
        assert!(imported.files.iter().all(|file| file.reason.is_none()));

        let missing = with_state(false);
        let imported = missing.import(false).unwrap();
        assert_eq!(home_state_file(&missing), json!({ "onboarded": true }));
        assert!(imported.files.iter().all(|file| file.reason.is_none()));
    }

    #[test]
    fn an_unusable_optional_state_source_is_imported_with_the_set_keys_and_a_reason() {
        for (content, reason) in [
            (b"[]".to_vec(), "not a JSON object"),
            (vec![b' '; 5000], "larger than 4096 bytes"),
        ] {
            let fixture = with_state(false);
            fixture.source_file("state.json", &content);
            let imported = fixture.import(false).unwrap();
            assert_eq!(home_state_file(&fixture), json!({ "onboarded": true }));
            let state = imported
                .files
                .iter()
                .find(|file| file.name == "state.json")
                .unwrap();
            let said = state.reason.as_deref().expect("the import says why");
            assert!(said.contains(reason), "{said}");

            let required = with_state(true);
            required.source_file("state.json", &content);
            let error = required.import(false).unwrap_err();
            assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
            assert_eq!(error.data.unwrap()["files"][0]["reason"], reason);
            assert!(!required.home().exists());
        }
    }

    #[test]
    fn a_state_file_alone_built_from_nothing_is_nothing_to_import() {
        let fixture = Fixture::new(&[]);
        fixture.declare(json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["state.json"],
            "optional": ["state.json"],
            "state": { "file": "state.json", "set": { "onboarded": true } }
        }));
        let error = fixture.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(!fixture.home().exists());
    }

    #[test]
    fn a_missing_optional_file_is_skipped_and_a_missing_required_one_is_named_with_its_place() {
        let fixture = Fixture::new(&[]);
        fixture.declare(json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.json", "extra.json"],
            "optional": ["extra.json"]
        }));
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let imported = fixture.import(false).unwrap();
        let names: Vec<&str> = imported
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["token.json"],
            "the missing optional file is skipped"
        );
        assert!(!fixture.home().join("extra.json").exists());

        let fixture = Fixture::new(&[]);
        fixture.declare(json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.json", "extra.json"],
            "optional": ["extra.json"]
        }));
        fixture.source_file("extra.json", b"{}");
        let error = fixture.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        let looked = fixture.source.path().join("token.json");
        assert_eq!(
            error.message,
            format!(
                "nothing was imported for agent kind fake; a required file is missing: token.json (looked for at {})",
                looked.display()
            )
        );
        let data = error.data.unwrap();
        assert_eq!(data["files"].as_array().unwrap().len(), 1);
        assert_eq!(data["files"][0]["name"], "token.json");
        assert_eq!(data["files"][0]["reason"], "missing");
        assert!(!fixture.home().exists());
    }

    #[test]
    fn an_unusable_optional_file_is_skipped_but_an_unusable_required_one_refuses() {
        /// A fixture with a required `token.json` and an optional `extra.json`.
        fn with_optional() -> Fixture {
            let fixture = Fixture::new(&[]);
            fixture.declare(json!({
                "home_env": "FAKE_AGENT_HOME",
                "files": ["token.json", "extra.json"],
                "optional": ["extra.json"]
            }));
            fixture
        }
        fn imports_the_token_only(fixture: &Fixture) {
            let imported = fixture.import(false).unwrap();
            let names: Vec<&str> = imported
                .files
                .iter()
                .map(|file| file.name.as_str())
                .collect();
            assert_eq!(names, ["token.json"]);
            assert!(!fixture.home().join("extra.json").exists());
        }

        let directory = with_optional();
        directory.source_file("token.json", FAKE_TOKEN.as_bytes());
        fs::create_dir(directory.source.path().join("extra.json")).unwrap();
        imports_the_token_only(&directory);

        let oversized = with_optional();
        oversized.source_file("token.json", FAKE_TOKEN.as_bytes());
        oversized.source_file("extra.json", &vec![b'x'; MAX_FILE_BYTES as usize + 1]);
        imports_the_token_only(&oversized);

        // The skip never applies to a required file.
        let required = with_optional();
        required.source_file("token.json", &vec![b'x'; MAX_FILE_BYTES as usize + 1]);
        required.source_file("extra.json", b"{}");
        let error = required.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert_eq!(
            error.data.unwrap()["files"][0]["reason"],
            "larger than 1 MiB"
        );
        assert!(!required.home().exists());
    }

    #[test]
    #[cfg_attr(
        windows,
        expect(
            clippy::print_stderr,
            reason = "a skipped test must name why it skipped"
        )
    )]
    fn a_linked_optional_file_is_skipped_when_a_link_can_be_made() {
        let fixture = Fixture::new(&[]);
        fixture.declare(json!({
            "home_env": "FAKE_AGENT_HOME",
            "files": ["token.json", "extra.json"],
            "optional": ["extra.json"]
        }));
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("real.json"), b"{}").unwrap();
        let link = fixture.source.path().join("extra.json");
        #[cfg(unix)]
        std::os::unix::fs::symlink(elsewhere.path().join("real.json"), &link).unwrap();
        #[cfg(windows)]
        if let Err(error) =
            std::os::windows::fs::symlink_file(elsewhere.path().join("real.json"), &link)
        {
            eprintln!("skipped: symbolic links cannot be made here ({error})");
            return;
        }
        let imported = fixture.import(false).unwrap();
        let names: Vec<&str> = imported
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect();
        assert_eq!(names, ["token.json"], "the linked optional file is skipped");
        assert!(!fixture.home().join("extra.json").exists());
    }

    #[test]
    fn a_file_over_the_limit_or_a_directory_is_refused() {
        let fixture = Fixture::new(&["token.json", "config.toml"]);
        fixture.source_file("token.json", &vec![b'x'; MAX_FILE_BYTES as usize + 1]);
        fs::create_dir(fixture.source.path().join("config.toml")).unwrap();
        let error = fixture.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        let files = error.data.unwrap()["files"].clone();
        assert_eq!(files[0]["name"], "token.json");
        assert_eq!(files[0]["reason"], "larger than 1 MiB");
        assert_eq!(files[1]["reason"], "a directory");
        assert!(!fixture.home().exists());

        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", &vec![b'x'; MAX_FILE_BYTES as usize]);
        assert_eq!(
            fixture.import(false).unwrap().files[0].bytes,
            MAX_FILE_BYTES
        );
    }

    #[test]
    fn the_source_must_be_an_absolute_existing_directory() {
        let fixture = Fixture::new(&["token.json"]);
        for from in [
            PathBuf::from("relative"),
            fixture.source.path().join("absent"),
        ] {
            let error = import(fixture.profile.path(), "fake", &from, false).unwrap_err();
            assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID, "{from:?}");
        }
    }

    #[test]
    fn kinds_without_files_or_unknown_kinds_are_refused() {
        let fixture = Fixture::new(&[]);
        assert_eq!(
            fixture.import(false).unwrap_err().code,
            codes::AGENT_CREDENTIALS_UNSUPPORTED
        );
        let error = import(
            fixture.profile.path(),
            "antigravity",
            fixture.source.path(),
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_UNSUPPORTED);
        let error =
            import(fixture.profile.path(), "nope", fixture.source.path(), false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_KIND_NOT_FOUND);
    }

    #[test]
    fn a_declared_name_that_could_leave_the_source_is_refused() {
        for name in [
            "../token.json",
            "a/b",
            "a\\b",
            "/etc/token",
            "C:token",
            "..",
        ] {
            let fixture = Fixture::new(&[name]);
            let error = fixture.import(false).unwrap_err();
            assert_eq!(error.code, codes::AGENT_CREDENTIALS_UNSUPPORTED, "{name}");
            assert!(
                error.message.contains("plain file name"),
                "{name}: {}",
                error.message
            );
        }
    }

    #[test]
    fn show_and_remove_follow_the_home() {
        let fixture = Fixture::new(&["token.json", "config.toml"]);
        let before = show(fixture.profile.path(), "fake").unwrap();
        assert!(!before.exists);
        assert_eq!(before.files.len(), 2);
        assert!(
            before
                .files
                .iter()
                .all(|file| !file.present && file.bytes.is_none())
        );

        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.source_file("config.toml", b"fake = true\n");
        fixture.import(false).unwrap();
        fs::remove_file(fixture.home().join("config.toml")).unwrap();
        let after = show(fixture.profile.path(), "fake").unwrap();
        assert!(after.exists);
        assert_eq!(after.home, fixture.home().display().to_string());
        assert!(after.files[0].present);
        assert_eq!(after.files[0].bytes, Some(FAKE_TOKEN.len() as u64));
        assert!(after.files[0].modified_at_ms.is_some_and(|ms| ms > 0));
        assert!(!after.files[1].present);

        assert!(remove(fixture.profile.path(), "fake").unwrap().removed);
        assert!(!fixture.home().exists());
        assert!(!remove(fixture.profile.path(), "fake").unwrap().removed);
        assert_eq!(
            remove(fixture.profile.path(), "../x").unwrap_err().code,
            codes::AGENT_KIND_NOT_FOUND
        );
    }

    #[cfg(unix)]
    #[test]
    fn links_are_refused_in_the_source_and_never_followed_at_the_home() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new(&["token.json"]);
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("real.json"), FAKE_TOKEN).unwrap();
        symlink(
            elsewhere.path().join("real.json"),
            fixture.source.path().join("token.json"),
        )
        .unwrap();
        let error = fixture.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert_eq!(error.data.unwrap()["files"][0]["reason"], "a link");

        fs::remove_file(fixture.source.path().join("token.json")).unwrap();
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fs::create_dir_all(fixture.home().parent().unwrap()).unwrap();
        symlink(elsewhere.path(), fixture.home()).unwrap();
        for error in [
            fixture.import(true).unwrap_err(),
            show(fixture.profile.path(), "fake").unwrap_err(),
            remove(fixture.profile.path(), "fake").unwrap_err(),
        ] {
            assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        }
        assert!(
            elsewhere.path().join("real.json").exists(),
            "the link target is untouched"
        );
    }

    /// A directory link: a symlink on Unix, a junction (no privilege needed) on Windows.
    fn link_dir(target: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
        #[cfg(windows)]
        {
            let made = massorca_process::command("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .unwrap();
            assert!(made.status.success(), "mklink /J failed");
        }
    }

    #[test]
    fn show_and_remove_refuse_a_linked_agents_or_homes_directory() {
        for linked in ["agents", "homes"] {
            let fixture = Fixture::new(&["token.json"]);
            let elsewhere = tempfile::tempdir().unwrap();
            fs::create_dir_all(elsewhere.path().join("homes").join("fake")).unwrap();
            fs::create_dir_all(elsewhere.path().join("fake")).unwrap();
            fs::write(
                elsewhere
                    .path()
                    .join("homes")
                    .join("fake")
                    .join("token.json"),
                FAKE_TOKEN,
            )
            .unwrap();
            fs::write(elsewhere.path().join("fake").join("token.json"), FAKE_TOKEN).unwrap();
            let agents = paths::agents_dir(fixture.profile.path());
            if linked == "agents" {
                // Keep the declared kind reachable: move it into the link target first.
                fs::create_dir_all(elsewhere.path().join("kinds")).unwrap();
                fs::rename(
                    agents.join("kinds").join("fake.json"),
                    elsewhere.path().join("kinds").join("fake.json"),
                )
                .unwrap();
                fs::remove_dir_all(&agents).unwrap();
                link_dir(elsewhere.path(), &agents);
            } else {
                link_dir(elsewhere.path(), &agents.join("homes"));
            }
            for error in [
                show(fixture.profile.path(), "fake").unwrap_err(),
                remove(fixture.profile.path(), "fake").unwrap_err(),
            ] {
                assert_eq!(
                    error.code,
                    codes::AGENT_CREDENTIALS_INVALID,
                    "{linked}: {}",
                    error.message
                );
            }
            assert!(
                elsewhere
                    .path()
                    .join("homes")
                    .join("fake")
                    .join("token.json")
                    .exists()
            );
            assert!(elsewhere.path().join("fake").join("token.json").exists());
        }
    }

    #[test]
    fn an_old_home_left_aside_refuses_the_import_and_is_kept() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let homes = fixture.home().parent().unwrap().to_path_buf();
        let aside = homes.join(".fake.old-0123");
        fs::create_dir_all(&aside).unwrap();
        fs::write(aside.join("token.json"), FAKE_TOKEN).unwrap();
        let error = fixture.import(true).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(
            error.message.contains(".fake.old-0123"),
            "{}",
            error.message
        );
        assert_eq!(error.data.unwrap()["leftover"], aside.display().to_string());
        assert!(aside.join("token.json").exists(), "the only copy is kept");
        assert!(!fixture.home().exists());
    }

    #[test]
    fn a_failed_put_back_names_where_the_old_home_is() {
        let aside = PathBuf::from("/profiles/default/agents/homes/.fake.old-0123");
        let error = not_put_back("fake", &aside, &io::Error::other("busy"));
        assert!(
            error.message.contains(&aside.display().to_string()),
            "{}",
            error.message
        );
        assert_eq!(error.data.unwrap()["aside"], aside.display().to_string());
    }

    #[test]
    fn a_staging_leftover_is_cleared_by_the_next_import() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let leftover = fixture.home().with_file_name(".fake.import-0123");
        fs::create_dir_all(&leftover).unwrap();
        fixture.import(false).unwrap();
        assert_eq!(fixture.homes_entries(), ["fake"]);
    }

    /// The home gets its own protected DACL: the owner and SYSTEM only, nothing inherited from the profile's parent.
    #[cfg(windows)]
    #[test]
    fn the_home_has_a_protected_owner_only_dacl() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.import(false).unwrap();
        let homes = fixture.home().parent().unwrap().to_path_buf();
        let agents = homes.parent().unwrap().to_path_buf();
        let user = massorca_profile::Sid::parse(&current_user()).unwrap();
        let user_and_system = [user.clone(), massorca_profile::Sid::parse("SY").unwrap()];
        for dir in [&agents, &homes, &fixture.home()] {
            assert!(
                massorca_profile::has_owner_only_dacl(dir).unwrap(),
                "{}",
                dir.display()
            );
            let owner = massorca_profile::PinnedDir::open(dir)
                .unwrap()
                .owner_sid()
                .unwrap();
            assert!(
                massorca_profile::owner_matches(&owner, std::slice::from_ref(&user)),
                "{} is owned by {owner}",
                dir.display()
            );
        }
        let file = massorca_profile::dacl_grantees(&fixture.home().join("token.json")).unwrap();
        assert!(!file.is_empty());
        for grantee in &file {
            assert!(
                massorca_profile::owner_matches(grantee, &user_and_system),
                "{grantee}"
            );
        }
    }

    /// The token user's SID: the owner `create_dir_owner_only` names.
    #[cfg(windows)]
    fn current_user() -> String {
        let dir = tempfile::tempdir().unwrap();
        let owned = dir.path().join("owned");
        massorca_profile::create_dir_owner_only(&owned).unwrap();
        massorca_profile::PinnedDir::open(&owned)
            .unwrap()
            .owner_sid()
            .unwrap()
            .to_string()
    }

    /// Run files create `agents` the same way: owner-only and owned by the token user.
    #[cfg(windows)]
    #[test]
    fn the_agents_directory_is_created_owner_only() {
        let profile = tempfile::tempdir().unwrap();
        create_agents_dir(profile.path()).unwrap();
        let agents = paths::agents_dir(profile.path());
        assert!(massorca_profile::has_owner_only_dacl(&agents).unwrap());
        let owner = massorca_profile::PinnedDir::open(&agents)
            .unwrap()
            .owner_sid()
            .unwrap();
        assert_eq!(owner.to_string(), current_user());
        create_agents_dir(profile.path()).unwrap();
    }

    /// `agents` and `homes` that already exist with inherited access are restricted by the next import.
    #[cfg(windows)]
    #[test]
    fn existing_agents_and_homes_directories_are_restricted() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let homes = fixture.home().parent().unwrap().to_path_buf();
        fs::create_dir_all(&homes).unwrap();
        assert!(!massorca_profile::has_owner_only_dacl(&homes).unwrap());
        fixture.import(false).unwrap();
        assert!(massorca_profile::has_owner_only_dacl(&homes).unwrap());
        assert!(massorca_profile::has_owner_only_dacl(homes.parent().unwrap()).unwrap());
    }

    /// The race the pinning closes: between the access change and the swap, another user renames `agents` away and
    /// plants a junction in its place. The rename must be refused, and nothing may be written behind the junction.
    #[cfg(windows)]
    #[test]
    fn agents_cannot_be_swapped_for_a_junction_during_an_import() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let attacker = tempfile::tempdir().unwrap();
        fs::create_dir_all(attacker.path().join("homes")).unwrap();
        let attacker_dir = attacker.path().to_path_buf();
        let renamed = std::rc::Rc::new(std::cell::RefCell::new(None));
        let seen = std::rc::Rc::clone(&renamed);
        AFTER_ACCESS_CHANGE.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |agents: &Path| {
                let moved = agents.with_file_name("agents-moved");
                let result = fs::rename(agents, &moved);
                if result.is_ok() {
                    link_dir(&attacker_dir, agents);
                }
                *seen.borrow_mut() = Some(result.map_err(|error| error.raw_os_error()));
            }));
        });
        let outcome = fixture.import(false);
        AFTER_ACCESS_CHANGE.with(|hook| *hook.borrow_mut() = None);

        let mut planted = Vec::new();
        let mut pending = vec![attacker.path().to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    planted.push(path);
                }
            }
        }
        assert!(
            planted.is_empty(),
            "credential files were written behind the junction: {planted:?}"
        );
        let rename = renamed.borrow_mut().take().expect("the hook ran");
        assert_eq!(
            rename,
            Err(Some(32)),
            "renaming the pinned `agents` must be a sharing violation"
        );
        outcome.unwrap();
        assert_eq!(
            fs::read_to_string(fixture.home().join("token.json")).unwrap(),
            FAKE_TOKEN
        );
    }

    /// A pinned directory owned by someone else (refused by the pin with `PermissionDenied`) refuses the import.
    /// Ownership by another principal cannot be set up here without admin rights, so the mapping is tested directly.
    #[cfg(windows)]
    #[test]
    fn a_directory_owned_by_someone_else_refuses_the_import() {
        let dir = Path::new("C:/profiles/default/agents/homes");
        let refused = io::Error::new(
            io::ErrorKind::PermissionDenied,
            "owned by S-1-5-21-9-9-9-1005",
        );
        let error = pin_refusal(dir, &refused);
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(
            error.message.contains("owned by S-1-5-21-9-9-9-1005"),
            "the pin's reason is kept: {}",
            error.message
        );
        let link = io::Error::new(io::ErrorKind::InvalidInput, "a reparse point");
        assert_eq!(
            pin_refusal(dir, &link).code,
            codes::AGENT_CREDENTIALS_INVALID
        );
        let other = io::Error::other("busy");
        assert_eq!(pin_refusal(dir, &other).code, codes::INTERNAL);
    }

    /// A junction needs no privilege on Windows, so this always runs there.
    #[cfg(windows)]
    #[test]
    fn a_junction_at_the_home_is_never_followed() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("real.json"), FAKE_TOKEN).unwrap();
        fs::create_dir_all(fixture.home().parent().unwrap()).unwrap();
        let made = massorca_process::command("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(fixture.home())
            .arg(elsewhere.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink /J failed");
        for error in [
            fixture.import(true).unwrap_err(),
            show(fixture.profile.path(), "fake").unwrap_err(),
            remove(fixture.profile.path(), "fake").unwrap_err(),
        ] {
            assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        }
        assert!(elsewhere.path().join("real.json").exists());
        assert!(!elsewhere.path().join("token.json").exists());
    }

    /// A file symlink needs Developer Mode or a privilege on Windows; without it the test says so and passes.
    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::print_stderr,
        reason = "a skipped test must name why it skipped"
    )]
    fn a_source_symlink_is_refused_when_one_can_be_made() {
        let fixture = Fixture::new(&["token.json"]);
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("real.json"), FAKE_TOKEN).unwrap();
        if let Err(error) = std::os::windows::fs::symlink_file(
            elsewhere.path().join("real.json"),
            fixture.source.path().join("token.json"),
        ) {
            eprintln!("skipped: symbolic links cannot be made here ({error})");
            return;
        }
        let error = fixture.import(false).unwrap_err();
        assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID);
        assert_eq!(error.data.unwrap()["files"][0]["reason"], "a link");
        assert!(!fixture.home().exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_home_is_0700_and_its_files_0600() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        fixture.import(false).unwrap();
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&fixture.home()), 0o700);
        assert_eq!(mode(&fixture.home().join("token.json")), 0o600);
    }

    const ACCOUNT: &str = "0195c0de-0000-7000-8000-000000000001";
    const OTHER_ACCOUNT: &str = "0195c0de-0000-7000-8000-000000000002";

    fn account_home(fixture: &Fixture, account: &str) -> PathBuf {
        paths::account_home(fixture.profile.path(), "fake", account)
    }

    #[test]
    fn an_account_is_imported_into_its_own_directory_and_never_replaced() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let imported = import_account(
            fixture.profile.path(),
            "fake",
            ACCOUNT,
            fixture.source.path(),
        )
        .unwrap();
        assert_eq!(
            imported.home,
            account_home(&fixture, ACCOUNT).display().to_string()
        );
        assert!(account_home(&fixture, ACCOUNT).join("token.json").is_file());
        assert!(
            !fixture.home().exists(),
            "the kind's 0056 home is not touched"
        );
        let again = import_account(
            fixture.profile.path(),
            "fake",
            ACCOUNT,
            fixture.source.path(),
        )
        .unwrap_err();
        assert_eq!(again.code, codes::AGENT_CREDENTIALS_EXIST);
    }

    #[test]
    fn an_account_home_is_usable_only_once_it_exists() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let profile = fixture.profile.path();
        assert_eq!(usable_account_home(profile, "fake", ACCOUNT).unwrap(), None);
        import_account(profile, "fake", ACCOUNT, fixture.source.path()).unwrap();
        assert_eq!(
            usable_account_home(profile, "fake", ACCOUNT).unwrap(),
            Some(account_home(&fixture, ACCOUNT))
        );
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn an_update_writes_only_the_files_the_new_account_brought_and_keeps_everything_else() {
        let fixture = Fixture::new(&["token.json", "extra.json"]);
        let profile = fixture.profile.path();
        fixture.source_file("token.json", b"old");
        fixture.source_file("extra.json", b"old extra");
        import_account(profile, "fake", ACCOUNT, fixture.source.path()).unwrap();
        let home = account_home(&fixture, ACCOUNT);
        fs::create_dir_all(home.join("projects")).unwrap();
        fs::write(home.join("projects").join("x.jsonl"), b"transcript").unwrap();
        // The new account brought only the token.
        let new_home = create_account_home(profile, "fake", OTHER_ACCOUNT).unwrap();
        fs::write(new_home.join("token.json"), b"new").unwrap();

        update_account_files_locked(profile, &BuiltInPackages, "fake", OTHER_ACCOUNT, ACCOUNT)
            .unwrap();

        assert_eq!(fs::read(home.join("token.json")).unwrap(), b"new");
        assert_eq!(fs::read(home.join("extra.json")).unwrap(), b"old extra");
        assert_eq!(
            fs::read(home.join("projects").join("x.jsonl")).unwrap(),
            b"transcript"
        );
        assert_eq!(entries(&home), ["extra.json", "projects", "token.json"]);
        assert!(!new_home.exists(), "the new account's home is deleted");
        assert_eq!(entries(home.parent().unwrap()), [ACCOUNT]);
    }

    #[test]
    fn an_update_keeps_the_existing_state_file_and_never_replaces_it() {
        let fixture = with_state(false);
        let profile = fixture.profile.path();
        import_account(profile, "fake", ACCOUNT, fixture.source.path()).unwrap();
        let home = account_home(&fixture, ACCOUNT);
        // The CLI wrote its own data into the existing account's state file.
        fs::write(
            home.join("state.json"),
            br#"{"onboarded":true,"projects":{"x":1}}"#,
        )
        .unwrap();
        let new_home = create_account_home(profile, "fake", OTHER_ACCOUNT).unwrap();
        fs::write(new_home.join("token.json"), b"new").unwrap();
        fs::write(new_home.join("state.json"), br#"{"onboarded":true}"#).unwrap();

        update_account_files_locked(profile, &BuiltInPackages, "fake", OTHER_ACCOUNT, ACCOUNT)
            .unwrap();

        assert_eq!(fs::read(home.join("token.json")).unwrap(), b"new");
        assert_eq!(
            fs::read(home.join("state.json")).unwrap(),
            br#"{"onboarded":true,"projects":{"x":1}}"#
        );
    }

    #[test]
    fn an_update_refuses_a_linked_file_and_a_missing_home_and_changes_nothing() {
        let fixture = Fixture::new(&["token.json"]);
        let profile = fixture.profile.path();
        let missing =
            update_account_files_locked(profile, &BuiltInPackages, "fake", OTHER_ACCOUNT, ACCOUNT)
                .unwrap_err();
        assert_eq!(missing.code, codes::AGENT_CREDENTIALS_INVALID);
        fixture.source_file("token.json", b"old");
        import_account(profile, "fake", ACCOUNT, fixture.source.path()).unwrap();
        let new_home = create_account_home(profile, "fake", OTHER_ACCOUNT).unwrap();
        let empty =
            update_account_files_locked(profile, &BuiltInPackages, "fake", OTHER_ACCOUNT, ACCOUNT)
                .unwrap_err();
        assert_eq!(empty.code, codes::AGENT_CREDENTIALS_INVALID);
        assert!(new_home.is_dir(), "a refused update deletes nothing");
        let home = account_home(&fixture, ACCOUNT);
        assert_eq!(fs::read(home.join("token.json")).unwrap(), b"old");
    }

    #[test]
    fn the_startup_restores_an_aside_home_whose_home_is_missing_and_deletes_one_whose_home_exists()
    {
        let fixture = Fixture::new(&["token.json"]);
        let profile = fixture.profile.path();
        fixture.source_file("token.json", b"kept");
        import_account(profile, "fake", ACCOUNT, fixture.source.path()).unwrap();
        let home = account_home(&fixture, ACCOUNT);
        let finished = home.with_file_name(format!(".{ACCOUNT}.{REPLACED}-1"));
        fs::create_dir(&finished).unwrap();
        let lost = account_home(&fixture, OTHER_ACCOUNT);
        let crashed = lost.with_file_name(format!(".{OTHER_ACCOUNT}.{REPLACED}-2"));
        fs::create_dir(&crashed).unwrap();
        fs::write(crashed.join("token.json"), b"only copy").unwrap();
        let legacy_aside = fixture.home().with_file_name(format!(".fake.{REPLACED}-3"));
        fs::create_dir_all(&legacy_aside).unwrap();

        restore_replaced_leftovers_locked(profile);

        assert!(!finished.exists(), "a finished swap's old home is deleted");
        assert_eq!(fs::read(lost.join("token.json")).unwrap(), b"only copy");
        assert!(!crashed.exists());
        assert!(fixture.home().is_dir(), "the 0056 home is put back too");
        assert_eq!(fs::read(home.join("token.json")).unwrap(), b"kept");
    }

    #[test]
    fn the_kind_leftovers_and_its_empty_account_folder_are_deleted() {
        let fixture = Fixture::new(&["token.json"]);
        let profile = fixture.profile.path();
        let accounts = paths::accounts_dir(profile, "fake");
        fs::create_dir_all(accounts.join(format!(".{ACCOUNT}.{REPLACED}-1"))).unwrap();
        fs::create_dir_all(accounts.join(format!(".{ACCOUNT}.{STAGING}-2"))).unwrap();
        fs::write(
            accounts
                .join(format!(".{ACCOUNT}.{STAGING}-2"))
                .join("token.json"),
            b"copy",
        )
        .unwrap();
        let homes = fixture.home().parent().unwrap().to_path_buf();
        fs::create_dir_all(homes.join(format!(".fake.{REPLACED}-3"))).unwrap();
        fs::create_dir_all(homes.join(".other.old-4")).unwrap();

        let problems = remove_kind_leftovers_locked(profile, "fake");

        assert!(problems.is_empty(), "{problems:?}");
        assert!(!accounts.exists(), "the kind's account folder is deleted");
        assert_eq!(
            entries(&homes),
            [".other.old-4"],
            "another kind's folder stays"
        );
    }

    #[test]
    fn remove_account_deletes_only_that_account() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let profile = fixture.profile.path();
        import_account(profile, "fake", ACCOUNT, fixture.source.path()).unwrap();
        import_account(profile, "fake", OTHER_ACCOUNT, fixture.source.path()).unwrap();
        assert!(remove_account(profile, "fake", ACCOUNT).unwrap().removed);
        assert!(!account_home(&fixture, ACCOUNT).exists());
        assert!(
            account_home(&fixture, OTHER_ACCOUNT)
                .join("token.json")
                .is_file()
        );
        assert!(!remove_account(profile, "fake", ACCOUNT).unwrap().removed);
    }

    #[test]
    fn an_account_id_that_is_not_a_canonical_uuid_is_refused_before_anything_is_written() {
        let fixture = Fixture::new(&["token.json"]);
        fixture.source_file("token.json", FAKE_TOKEN.as_bytes());
        let profile = fixture.profile.path();
        for bad in [
            "../escape",
            ".hidden",
            "not-a-uuid",
            "0195C0DE-0000-7000-8000-000000000001",
        ] {
            let error = import_account(profile, "fake", bad, fixture.source.path()).unwrap_err();
            assert_eq!(error.code, codes::AGENT_CREDENTIALS_INVALID, "{bad}");
        }
        assert!(!paths::agents_dir(profile).join("accounts").exists());
    }
}
