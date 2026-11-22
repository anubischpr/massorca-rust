//! Credential homes, one per profile and kind (decision 0056): import, show and remove.
//!
//! Import copies only the files the kind declares, only regular files, at most [`MAX_FILE_BYTES`] each. The new home
//! is built in a temporary sibling directory and renamed into place, so a failed import leaves an existing home as it
//! was and never leaves a half-made one. File contents are never logged or returned: names and sizes only.
//!
//! Every declared file is required. The declaration has no optional flag, so a declared name that is missing, a link,
//! a directory or anything but a regular file fails the import with `agent.credentials_invalid`, naming each one and
//! why; nothing is copied then.

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

use crate::catalog::Catalog;
use crate::kind::KindDecl;
use crate::paths;
use crate::sessions::AgentError;

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

/// Copies the kind's declared files from `from` into `<profile>/agents/homes/<kind>/`.
///
/// Every declared file is read in full, at most [`MAX_FILE_BYTES`] each, before anything is written; then a new home
/// is written and renamed into place. An existing home is replaced only with `replace`.
pub fn import(
    profile_dir: &Path,
    kind: &str,
    from: &Path,
    replace: bool,
) -> Result<CredentialsImported, AgentError> {
    let decl = declared(profile_dir, kind)?;
    let names = decl
        .credentials
        .as_ref()
        .map(|credentials| credentials.files.clone())
        .filter(|files| !files.is_empty())
        .ok_or_else(|| {
            AgentError::new(
                codes::AGENT_CREDENTIALS_UNSUPPORTED,
                format!("agent kind {kind} declares no credential files to import"),
            )
        })?;
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

    let _gate = lock_homes();
    let home = home_of(profile_dir, &decl.id)?;
    if home_state(&home)? == HomeState::Directory && !replace {
        return Err(exists(&decl.id));
    }

    let mut contents = Vec::with_capacity(names.len());
    let mut problems = Vec::new();
    for name in &names {
        match read_source(&from.join(name)) {
            Ok(bytes) => contents.push((name.as_str(), bytes)),
            Err(reason) => problems.push(json!({ "name": name, "reason": reason })),
        }
    }
    if !problems.is_empty() {
        return Err(refused_files(&decl.id, problems));
    }

    // `agents` and `homes` stay pinned (Windows) from before their access changes until the swap is done.
    let (homes, _pinned) = homes_dir(profile_dir)?;
    #[cfg(test)]
    tests::run_after_access_change(&paths::agents_dir(profile_dir));
    refuse_replaced_leftover(&homes, &decl.id)?;
    clear_leftovers(&homes, &decl.id);
    let staging_path = homes.join(sibling_name(&decl.id, STAGING));
    let placed = Staging::create(staging_path.clone()).and_then(|staging| {
        write_files(&staging.path, &contents)?;
        check_staging(&staging.path)?;
        put_in_place(&staging, &home, &decl.id, replace)
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
pub fn show(profile_dir: &Path, kind: &str) -> Result<CredentialsShown, AgentError> {
    let decl = declared(profile_dir, kind)?;
    let home = home_of(profile_dir, &decl.id)?;
    check_homes_dir(profile_dir)?;
    let exists = home_state(&home)? == HomeState::Directory;
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
    let _gate = lock_homes();
    check_homes_dir(profile_dir)?;
    let removed = match home_state(&home)? {
        HomeState::Missing => false,
        HomeState::Directory => {
            fs::remove_dir_all(&home)
                .map_err(|error| internal("delete the credential home", &error))?;
            true
        }
    };
    if removed {
        tracing::info!(kind, "removed agent credentials");
    }
    Ok(CredentialsRemoved {
        kind: kind.to_owned(),
        removed,
    })
}

/// The kind's declaration, when it is known and valid.
fn declared(profile_dir: &Path, kind: &str) -> Result<KindDecl, AgentError> {
    let catalog = Catalog::load(profile_dir);
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

/// `<profile>/agents/homes`, created when missing; neither it nor `agents` may be a link. On Windows both are pinned
/// before their access is checked or changed, and stay pinned while the returned [`Pins`] live, so nobody can swap
/// either for a junction in between.
fn homes_dir(profile_dir: &Path) -> Result<(PathBuf, Pins), AgentError> {
    let agents = paths::agents_dir(profile_dir);
    let homes = agents.join("homes");
    #[cfg(windows)]
    let mut pins: Pins = Vec::with_capacity(2);
    #[cfg(not(windows))]
    let pins = Pins::new();
    for dir in [&agents, &homes] {
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
    Ok((homes, pins))
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

/// Checks, without creating anything, that `<profile>/agents` and `homes` are plain directories when they exist.
fn check_homes_dir(profile_dir: &Path) -> Result<(), AgentError> {
    let agents = paths::agents_dir(profile_dir);
    for dir in [agents.clone(), agents.join("homes")] {
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
                "{} holds an old credential home of agent kind {kind} set aside by an earlier replace; move or delete it by hand, then import again",
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
fn read_source(path: &Path) -> Result<Vec<u8>, &'static str> {
    let before = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err("missing"),
        Err(_) => return Err("unreadable"),
        Ok(meta) => meta,
    };
    not_regular(&before)?;
    let file = open_no_follow(path).map_err(|_| "unreadable")?;
    let after = file.metadata().map_err(|_| "unreadable")?;
    not_regular(&after)?;
    if !same_file(&before, &after) {
        return Err("replaced while it was read");
    }
    if after.len() > MAX_FILE_BYTES {
        return Err(TOO_LARGE);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreadable")?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(TOO_LARGE);
    }
    Ok(bytes)
}

const TOO_LARGE: &str = "larger than 1 MiB";

/// Why an entry is not a regular file, if it is not.
fn not_regular(meta: &fs::Metadata) -> Result<(), &'static str> {
    if is_link(meta) {
        Err("a link")
    } else if meta.is_dir() {
        Err("a directory")
    } else if !meta.is_file() {
        Err("not a regular file")
    } else {
        Ok(())
    }
}

/// A symbolic link, or on Windows any reparse point (junctions included).
fn is_link(meta: &fs::Metadata) -> bool {
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
fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
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

/// The declared files that cannot be imported, each with its reason; names only.
fn refused_files(kind: &str, problems: Vec<Value>) -> AgentError {
    let listed = problems
        .iter()
        .map(|problem| {
            format!(
                "{} ({})",
                problem["name"].as_str().unwrap_or_default(),
                problem["reason"].as_str().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut error = invalid(format!(
        "nothing was imported for agent kind {kind}; every declared file must be a regular file: {listed}"
    ));
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
    use crate::paths;

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
            let made = std::process::Command::new("cmd")
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
        let made = std::process::Command::new("cmd")
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
}
