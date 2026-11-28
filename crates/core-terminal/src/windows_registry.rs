//! The `Path` value of the machine and user environment, read from the registry (decision 0032, TERM-17).
//!
//! Read-only. Each call opens its key, reads the value and closes the key again.
#![allow(unsafe_code)]

use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey,
    RegOpenKeyExW, RegQueryValueExW,
};

/// `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`.
const MACHINE_ENVIRONMENT: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
/// `HKCU\Environment`.
const USER_ENVIRONMENT: &str = "Environment";
const PATH_VALUE: &str = "Path";

/// Which environment's `Path` is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    Machine,
    User,
}

/// A string value. `expandable` is set for `REG_EXPAND_SZ`, whose `%NAME%` references are expanded by the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryPath {
    pub text: String,
    pub expandable: bool,
}

/// Why a value could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// The registry call failed with this Windows error code.
    Windows(u32),
    /// The value holds something other than a string, so it is not a PATH.
    UnexpectedType,
}

/// The `Path` value of `hive`: `Ok(None)` when the key or the value is not there.
pub fn read_path(hive: Hive) -> Result<Option<RegistryPath>, ReadError> {
    let (root, subkey) = match hive {
        Hive::Machine => (HKEY_LOCAL_MACHINE, MACHINE_ENVIRONMENT),
        Hive::User => (HKEY_CURRENT_USER, USER_ENVIRONMENT),
    };
    let Some(key) = Key::open(root, subkey)? else {
        return Ok(None);
    };
    key.read_string(PATH_VALUE)
}

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Key {
    /// Opens `subkey` under `root` for reading; `Ok(None)` when it does not exist.
    fn open(root: HKEY, subkey: &str) -> Result<Option<Self>, ReadError> {
        let name = wide(subkey);
        let mut key: HKEY = ptr::null_mut();
        // SAFETY: `name` is NUL-terminated and lives across the call; `key` receives the handle on success.
        let status = unsafe { RegOpenKeyExW(root, name.as_ptr(), 0, KEY_READ, &mut key) };
        match status {
            ERROR_SUCCESS => Ok(Some(Self(key))),
            ERROR_FILE_NOT_FOUND => Ok(None),
            failed => Err(ReadError::Windows(failed)),
        }
    }

    /// Reads the string value `name`; `Ok(None)` when the value is not there.
    fn read_string(&self, name: &str) -> Result<Option<RegistryPath>, ReadError> {
        let value = wide(name);
        let mut kind: u32 = 0;
        let mut size: u32 = 0;
        // SAFETY: `value` is NUL-terminated; a null buffer makes Windows report only the size and the type.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                value.as_ptr(),
                ptr::null(),
                &mut kind,
                ptr::null_mut(),
                &mut size,
            )
        };
        match status {
            ERROR_SUCCESS => {}
            ERROR_FILE_NOT_FOUND => return Ok(None),
            failed => return Err(ReadError::Windows(failed)),
        }
        let mut units = vec![0u16; (size as usize).div_ceil(2)];
        let mut bytes = size;
        // SAFETY: `units` holds at least `bytes` bytes, so Windows writes within the buffer.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                value.as_ptr(),
                ptr::null(),
                &mut kind,
                units.as_mut_ptr().cast::<u8>(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(ReadError::Windows(status));
        }
        let expandable = match kind {
            REG_EXPAND_SZ => true,
            REG_SZ => false,
            _ => return Err(ReadError::UnexpectedType),
        };
        let end = units
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(units.len());
        Ok(Some(RegistryPath {
            text: String::from_utf16_lossy(&units[..end]),
            expandable,
        }))
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful RegOpenKeyExW and is closed only here.
        unsafe { RegCloseKey(self.0) };
    }
}

/// A UTF-16 copy of `text` with its NUL terminator, as the registry API expects.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
