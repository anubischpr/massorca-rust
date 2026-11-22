//! The profile's master key and where it is protected. The protection is recorded at creation and later opens
//! use exactly that one: a key that will not open is reported locked, never replaced.

mod file;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod owner_only;
#[cfg(any(target_os = "linux", test))]
mod vault_id;
#[cfg(windows)]
mod windows;

use std::fmt;
use std::io;
use std::path::Path;
use std::str::FromStr;

use zeroize::Zeroizing;

const KEY_LEN: usize = 32;
const KEY_FILE: &str = "key";
const PROTECTION_FILE: &str = "protection";
const ID_FILE: &str = "id";

/// The 32-byte key that seals every secret of one profile. It never prints and is zeroed on drop.
pub struct MasterKey(Zeroizing<[u8; KEY_LEN]>);

impl MasterKey {
    /// Draws a new key from the operating system's random source.
    ///
    /// # Errors
    /// [`KeyError::Random`] when the random source is unavailable.
    pub fn generate() -> Result<Self, KeyError> {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        getrandom::fill(&mut *bytes).map_err(|_| KeyError::Random)?;
        Ok(Self(bytes))
    }

    fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != KEY_LEN {
            return None;
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        key.copy_from_slice(bytes);
        Some(Self(key))
    }

    pub(crate) fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey([redacted])")
    }
}

/// Where the master key is protected, as `<vault>/protection` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protection {
    /// Windows DPAPI, current user, wrapped key in `<vault>/key`.
    Dpapi,
    /// An item in the Linux Secret Service default collection.
    SecretService,
    /// The raw key in an owner-only `<vault>/key`; it protects only against copies of the database.
    File,
}

impl Protection {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dpapi => "dpapi",
            Self::SecretService => "secret-service",
            Self::File => "file",
        }
    }
}

impl fmt::Display for Protection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The text is not a known protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("unknown key protection")]
pub struct UnknownProtection;

impl FromStr for Protection {
    type Err = UnknownProtection;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        [Self::Dpapi, Self::SecretService, Self::File]
            .into_iter()
            .find(|protection| protection.as_str() == text)
            .ok_or(UnknownProtection)
    }
}

/// Why an existing vault is locked. Nothing was changed on disk or in the keyring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockReason {
    /// The protection is recorded but its key is gone.
    MissingKey,
    /// The key is there but does not unwrap, or is not 32 bytes.
    Unwrap,
    /// The protection file names no known protection.
    UnknownProtection,
    /// The recorded protection does not exist on this host, or its store is unreachable.
    ProtectionUnavailable,
    /// A key exists without a protection file, so which protection wraps it is unknown.
    KeyWithoutProtection,
    /// The key opens but is not the one that sealed this vault: its key check does not open.
    KeyMismatch,
    /// The vault path is a link, a junction or not a directory; nothing is read or written through it.
    UnsafeDirectory,
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("the vault is locked: its key will not open ({0:?})")]
    Locked(LockReason),
    #[error("the operating system's random source is unavailable")]
    Random,
    #[error("the vault key could not be stored: {0}")]
    Io(#[from] io::Error),
}

/// Opens or creates the master key of one vault directory.
#[derive(Debug)]
pub struct KeyStore;

impl KeyStore {
    /// Opens the key with the recorded protection, or creates one when the vault has none yet. `force_file`
    /// chooses File at creation only; an existing vault keeps its protection.
    ///
    /// # Errors
    /// [`KeyError::Locked`] for an existing key that will not open, left untouched; `Random` or `Io` when a
    /// new key cannot be created.
    pub fn open_or_create(
        vault_dir: &Path,
        force_file: bool,
    ) -> Result<(Protection, MasterKey), KeyError> {
        file::secure_existing(vault_dir, &[KEY_FILE, PROTECTION_FILE, ID_FILE])?;
        match file::read_small(&vault_dir.join(PROTECTION_FILE)) {
            Ok(word) => {
                let protection = std::str::from_utf8(&word)
                    .ok()
                    .and_then(|word| word.trim().parse().ok())
                    .ok_or(KeyError::Locked(LockReason::UnknownProtection))?;
                let key = open_existing(protection, vault_dir).map_err(KeyError::Locked)?;
                Ok((protection, key))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => create(vault_dir, force_file),
            Err(_) => Err(KeyError::Locked(LockReason::UnknownProtection)),
        }
    }

    /// Removes the key, its protection and id, and its keyring item. The caller makes sure nothing is sealed with it.
    ///
    /// # Errors
    /// `Io` when a file exists and cannot be removed.
    pub fn reset(vault_dir: &Path) -> Result<(), KeyError> {
        if !file::check_dir(vault_dir)? {
            return Ok(());
        }
        reset(vault_dir)
    }
}

fn open_existing(protection: Protection, vault_dir: &Path) -> Result<MasterKey, LockReason> {
    match protection {
        Protection::File => {
            let bytes = read_key_file(vault_dir)?;
            MasterKey::from_slice(&bytes).ok_or(LockReason::Unwrap)
        }
        #[cfg(windows)]
        Protection::Dpapi => {
            let blob = read_key_file(vault_dir)?;
            let bytes = windows::unprotect(&blob).map_err(|_| LockReason::Unwrap)?;
            MasterKey::from_slice(&bytes).ok_or(LockReason::Unwrap)
        }
        #[cfg(target_os = "linux")]
        Protection::SecretService => linux::open(vault_dir),
        // The protections of other hosts.
        _ => Err(LockReason::ProtectionUnavailable),
    }
}

/// Forgets the key of `vault_dir`: its keyring item, the key, protection and id files. A missing file is fine.
fn reset(vault_dir: &Path) -> Result<(), KeyError> {
    #[cfg(target_os = "linux")]
    linux::delete_all(vault_dir);
    for name in [KEY_FILE, PROTECTION_FILE, ID_FILE] {
        match std::fs::remove_file(vault_dir.join(name)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    Ok(())
}

fn read_key_file(vault_dir: &Path) -> Result<Zeroizing<Vec<u8>>, LockReason> {
    file::read_small(&vault_dir.join(KEY_FILE)).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => LockReason::MissingKey,
        _ => LockReason::Unwrap,
    })
}

fn create(vault_dir: &Path, force_file: bool) -> Result<(Protection, MasterKey), KeyError> {
    file::ensure_private_dir(vault_dir)?;
    let key_path = vault_dir.join(KEY_FILE);
    if file::exists_no_follow(&key_path)? {
        return Err(KeyError::Locked(LockReason::KeyWithoutProtection));
    }
    let key = MasterKey::generate()?;
    let (protection, created_item) = store_new(vault_dir, &key, force_file)?;
    let mut word = protection.as_str().to_owned();
    word.push('\n');
    if let Err(error) = file::write_new(&vault_dir.join(PROTECTION_FILE), word.as_bytes()) {
        // Nothing was sealed with this key yet, so undoing its store loses nothing.
        match protection {
            Protection::File | Protection::Dpapi => {
                let _ = std::fs::remove_file(&key_path);
            }
            // Only the item this call created: a racing creator's item is left alone.
            #[cfg(target_os = "linux")]
            Protection::SecretService => {
                if let Some(item) = &created_item {
                    linux::delete_created(vault_dir, item);
                }
            }
            #[cfg(not(target_os = "linux"))]
            Protection::SecretService => drop(created_item),
        }
        return Err(error.into());
    }
    Ok((protection, key))
}

/// Stores a new key with the host's preferred protection and returns which one was used, with the unique tag of the
/// keyring item it created, if any.
fn store_new(
    vault_dir: &Path,
    key: &MasterKey,
    force_file: bool,
) -> Result<(Protection, Option<String>), KeyError> {
    let key_path = vault_dir.join(KEY_FILE);
    if !force_file {
        #[cfg(windows)]
        {
            let blob = windows::protect(key.bytes())?;
            file::write_new(&key_path, &blob)?;
            return Ok((Protection::Dpapi, None));
        }
        #[cfg(target_os = "linux")]
        match linux::create(vault_dir, key) {
            linux::Created::Stored(item) => return Ok((Protection::SecretService, Some(item))),
            linux::Created::AlreadyThere => {
                return Err(KeyError::Locked(LockReason::KeyWithoutProtection));
            }
            linux::Created::Unavailable => {}
        }
    }
    file::write_new(&key_path, key.bytes())?;
    Ok((Protection::File, None))
}
